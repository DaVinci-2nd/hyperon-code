use crate::presets;
use crate::store::Provider;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    Delta { text: String },
    Reasoning { text: String },
    Done,
    Error { message: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub type EventSink = Arc<dyn Fn(StreamEvent) + Send + Sync>;

pub struct Acc {
    pub content: String,
    pub reasoning: String,
}

pub async fn stream(
    provider: &Provider,
    messages: Vec<ChatMessage>,
    system_prompt: &str,
    sink: EventSink,
) -> Result<Acc, String> {
    let mut acc = Acc {
        content: String::new(),
        reasoning: String::new(),
    };

    let result = match provider.protocol.as_str() {
        "anthropic" => stream_anthropic(provider, &messages, system_prompt, &sink, &mut acc).await,
        "gemini" => stream_gemini(provider, &messages, system_prompt, &sink, &mut acc).await,
        _ => stream_openai(provider, &messages, system_prompt, &sink, &mut acc).await,
    };

    match result {
        Ok(()) => {
            sink(StreamEvent::Done);
            Ok(acc)
        }
        Err(message) => {
            sink(StreamEvent::Error {
                message: message.clone(),
            });
            Err(message)
        }
    }
}

fn emit_delta(sink: &EventSink, acc: &mut Acc, text: &str) {
    if text.is_empty() {
        return;
    }
    acc.content.push_str(text);
    sink(StreamEvent::Delta {
        text: text.to_string(),
    });
}

fn emit_reasoning(sink: &EventSink, acc: &mut Acc, text: &str) {
    if text.is_empty() {
        return;
    }
    acc.reasoning.push_str(text);
    sink(StreamEvent::Reasoning {
        text: text.to_string(),
    });
}

fn build_openai_body(provider: &Provider, messages: &[ChatMessage], system_prompt: &str) -> Value {
    let mut list: Vec<Value> = Vec::new();
    if !system_prompt.is_empty() {
        list.push(json!({ "role": "system", "content": system_prompt }));
    }
    for m in messages {
        list.push(json!({ "role": m.role, "content": m.content }));
    }

    let mut body = json!({
        "model": provider.model,
        "messages": list,
        "stream": true,
    });

    let host = provider.base_url.to_lowercase();
    let is_deepseek = host.contains("deepseek");
    let is_glm = host.contains("bigmodel");

    if presets::is_thinking_on(&provider.thinking) {
        if let Some(effort) = presets::thinking_effort(&provider.thinking) {
            body["reasoning_effort"] = json!(effort);
        }
        if is_deepseek || is_glm {
            body["thinking"] = json!({ "type": "enabled" });
        }
    } else if is_deepseek {
        body["thinking"] = json!({ "type": "disabled" });
    } else if is_glm {
        body["thinking"] = json!({ "type": "enabled" });
        body["reasoning_effort"] = json!("low");
    }

    body
}

async fn stream_openai(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<(), String> {
    let body = build_openai_body(provider, messages, system_prompt);
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取失败: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf.drain(..=pos);
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                return Ok(());
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            if let Some(err) = value["error"]["message"].as_str() {
                return Err(err.to_string());
            }
            let delta = &value["choices"][0]["delta"];

            if let Some(reasoning) = delta["reasoning_content"].as_str() {
                emit_reasoning(sink, acc, reasoning);
            }
            if let Some(text) = delta["content"].as_str() {
                emit_delta(sink, acc, text);
            }
        }
    }

    Ok(())
}

fn build_anthropic_body(provider: &Provider, messages: &[ChatMessage], system_prompt: &str) -> Value {
    let list: Vec<Value> = messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();

    let mut budget: u32 = 0;
    let mut body = json!({
        "model": provider.model,
        "max_tokens": 8192,
        "messages": list,
        "stream": true,
    });

    if !system_prompt.is_empty() {
        body["system"] = json!(system_prompt);
    }

    if let Some(b) = presets::thinking_budget(&provider.thinking) {
        budget = b;
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": b });
    }

    let need = budget as u64 + 1024;
    if need > 8192 {
        body["max_tokens"] = json!(need);
    }

    body
}

async fn stream_anthropic(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<(), String> {
    let body = build_anthropic_body(provider, messages, system_prompt);
    let url = format!("{}/messages", provider.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取失败: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf.drain(..=pos);
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };

            match value["type"].as_str() {
                Some("content_block_delta") => {
                    let delta = &value["delta"];
                    if let Some(text) = delta["text"].as_str() {
                        emit_delta(sink, acc, text);
                    }
                    if let Some(thinking) = delta["thinking"].as_str() {
                        emit_reasoning(sink, acc, thinking);
                    }
                }
                Some("message_stop") => return Ok(()),
                Some("error") => {
                    return Err(value["error"]["message"]
                        .as_str()
                        .unwrap_or("未知错误")
                        .to_string());
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn build_gemini_body(messages: &[ChatMessage], system_prompt: &str, provider: &Provider) -> Value {
    let contents: Vec<Value> = messages
        .iter()
        .map(|m| {
            let role = if m.role == "assistant" { "model" } else { "user" };
            json!({ "role": role, "parts": [{ "text": m.content }] })
        })
        .collect();

    let mut body = json!({ "contents": contents });

    let mut gen = json!({});
    if let Some(budget) = presets::thinking_budget(&provider.thinking) {
        gen["thinkingConfig"] = json!({ "thinkingBudget": budget, "includeThoughts": true });
    } else if provider.thinking == "off" {
        gen["thinkingConfig"] = json!({ "thinkingBudget": 0 });
    }
    if gen.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        body["generationConfig"] = gen;
    }

    if !system_prompt.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system_prompt }] });
    }

    body
}

async fn stream_gemini(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<(), String> {
    let body = build_gemini_body(messages, system_prompt, provider);
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse&key={}",
        provider.base_url.trim_end_matches('/'),
        provider.model,
        provider.api_key
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取失败: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf.drain(..=pos);
            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            let Some(parts) = value["candidates"][0]["content"]["parts"].as_array() else {
                continue;
            };
            for part in parts {
                let Some(text) = part["text"].as_str() else {
                    continue;
                };
                if part["thought"].as_bool().unwrap_or(false) {
                    emit_reasoning(sink, acc, text);
                } else {
                    emit_delta(sink, acc, text);
                }
            }
        }
    }

    Ok(())
}
