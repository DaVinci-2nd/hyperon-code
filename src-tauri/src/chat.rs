use crate::approval::Pending;
use crate::presets;
use crate::store::{Part, Provider};
use crate::tools;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    Delta { text: String },
    Reasoning { text: String },
    ReasoningEnd,
    TextStart,
    Approval {
        id: String,
        tool: String,
        summary: String,
        detail: String,
        kind: String,
    },
    ToolEnd {
        id: String,
        summary: String,
        detail: String,
        tool: String,
        ok: bool,
    },
    Done,
    Error { message: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn text(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn with_reasoning(role: &str, content: &str, reasoning: Option<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            reasoning,
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

pub type EventSink = Arc<dyn Fn(StreamEvent) + Send + Sync>;

pub struct Acc {
    pub content: String,
    pub reasoning: String,
    pub parts: Vec<Part>,
    pub tool_calls: Vec<ToolCall>,
    pub truncated: bool,
    reasoning_open: bool,
    text_open: bool,
}

impl Acc {
    fn new() -> Self {
        Self {
            content: String::new(),
            reasoning: String::new(),
            parts: Vec::new(),
            tool_calls: Vec::new(),
            truncated: false,
            reasoning_open: false,
            text_open: false,
        }
    }

    fn close_reasoning(&mut self, sink: &EventSink) {
        if self.reasoning_open {
            self.reasoning_open = false;
            sink(StreamEvent::ReasoningEnd);
        }
    }

    fn close_text(&mut self) {
        self.text_open = false;
    }
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

pub struct Ctx {
    pub root: Option<String>,
    pub pending: Arc<Pending>,
}

pub async fn stream(
    provider: &Provider,
    messages: &mut Vec<ChatMessage>,
    system_prompt: &str,
    sink: EventSink,
    ctx: &Ctx,
) -> Result<Acc, String> {
    let mut total = Acc::new();

    loop {
        let mut round = Acc::new();
        let outcome = run_round(provider, messages, system_prompt, &sink, &mut round).await;

        round.close_reasoning(&sink);
        total.content.push_str(&round.content);
        total.reasoning.push_str(&round.reasoning);
        total.parts.extend(round.parts);

        let calls = match outcome {
            Ok(calls) => calls,
            Err(err) => {
                total.truncated = true;
                sink(StreamEvent::Error {
                    message: err.clone(),
                });
                return Ok(total);
            }
        };

        if calls.is_empty() {
            sink(StreamEvent::Done);
            return Ok(total);
        }

        messages.push(build_assistant_call(&round.content, &calls));

        for call in &calls {
            total.tool_calls.push(call.clone());

            let preview = tools::preview(&call.name, &call.args, ctx.root.as_deref());
            let approved = if tools::needs_approval(&call.name) {
                let id = call.id.clone();
                sink(StreamEvent::Approval {
                    id: id.clone(),
                    tool: call.name.clone(),
                    summary: preview.summary.clone(),
                    detail: preview.detail.clone(),
                    kind: preview.kind.clone(),
                });
                let rx = ctx.pending.register(&id);
                rx.await.unwrap_or(false)
            } else {
                true
            };

            let result = if approved {
                tools::execute(&call.name, &call.args, ctx.root.as_deref())
            } else {
                "user denied".to_string()
            };

            total.parts.push(Part {
                kind: "tool".to_string(),
                text: String::new(),
                summary: preview.summary.clone(),
                detail: result.clone(),
                ok: approved,
            });

            sink(StreamEvent::ToolEnd {
                id: call.id.clone(),
                summary: preview.summary.clone(),
                detail: result.clone(),
                tool: call.name.clone(),
                ok: approved,
            });

            messages.push(build_tool_result(provider, call, &result));
        }
    }
}

async fn run_round(
    provider: &Provider,
    messages: &mut Vec<ChatMessage>,
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<Vec<ToolCall>, String> {
    match provider.protocol.as_str() {
        "anthropic" => stream_anthropic(provider, messages, system_prompt, sink, acc).await,
        "gemini" => stream_gemini(provider, messages, system_prompt, sink, acc).await,
        _ => stream_openai(provider, messages, system_prompt, sink, acc).await,
    }
}

fn build_assistant_call(content: &str, calls: &[ToolCall]) -> ChatMessage {
    let list: Vec<Value> = calls
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": "function",
                "function": {
                    "name": c.name,
                    "arguments": serde_json::to_string(&c.args).unwrap_or_else(|_| "{}".to_string())
                }
            })
        })
        .collect();
    ChatMessage {
        role: "assistant".to_string(),
        content: content.to_string(),
        reasoning: None,
        tool_calls: Some(Value::Array(list)),
        tool_call_id: None,
    }
}

fn build_tool_result(provider: &Provider, call: &ToolCall, result: &str) -> ChatMessage {
    if provider.protocol == "anthropic" {
        let payload = json!([{
            "type": "tool_result",
            "tool_use_id": call.id,
            "content": result
        }]);
        ChatMessage {
            role: "user".to_string(),
            content: payload.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some(call.id.clone()),
        }
    } else if provider.protocol == "gemini" {
        let payload = json!([{
            "functionResponse": {
                "name": call.name,
                "response": { "result": result }
            }
        }]);
        ChatMessage {
            role: "user".to_string(),
            content: payload.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some(call.id.clone()),
        }
    } else {
        ChatMessage {
            role: "tool".to_string(),
            content: result.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some(call.id.clone()),
        }
    }
}

fn emit_delta(sink: &EventSink, acc: &mut Acc, text: &str) {
    if text.is_empty() {
        return;
    }
    if !acc.text_open {
        acc.text_open = true;
        acc.parts.push(Part {
            kind: "text".to_string(),
            text: String::new(),
            summary: String::new(),
            detail: String::new(),
            ok: true,
        });
    }
    acc.content.push_str(text);
    if let Some(last) = acc.parts.last_mut() {
        if last.kind == "text" {
            last.text.push_str(text);
        }
    }
    sink(StreamEvent::Delta {
        text: text.to_string(),
    });
}

fn emit_reasoning(sink: &EventSink, acc: &mut Acc, text: &str) {
    if text.is_empty() {
        return;
    }
    if !acc.reasoning_open {
        acc.reasoning_open = true;
        acc.text_open = false;
        acc.parts.push(Part {
            kind: "think".to_string(),
            text: String::new(),
            summary: String::new(),
            detail: String::new(),
            ok: true,
        });
    }
    acc.reasoning.push_str(text);
    if let Some(last) = acc.parts.last_mut() {
        if last.kind == "think" {
            last.text.push_str(text);
        }
    }
    sink(StreamEvent::Reasoning {
        text: text.to_string(),
    });
}

fn build_openai_body(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
) -> Value {
    let mut list: Vec<Value> = Vec::new();
    if !system_prompt.is_empty() {
        list.push(json!({ "role": "system", "content": system_prompt }));
    }
    for m in messages {
        let mut item = json!({ "role": m.role, "content": m.content });
        if let Some(reasoning) = &m.reasoning {
            item["reasoning_content"] = json!(reasoning);
        }
        if let Some(calls) = &m.tool_calls {
            item["tool_calls"] = calls.clone();
        }
        if let Some(id) = &m.tool_call_id {
            item["tool_call_id"] = json!(id);
        }
        list.push(item);
    }

    let mut body = json!({
        "model": provider.model,
        "messages": list,
        "stream": true,
        "tools": tools::openai_tools(),
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
) -> Result<Vec<ToolCall>, String> {
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
    let mut collected: Vec<(String, String, String)> = Vec::new();

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
                return Ok(finish_openai_calls(collected));
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
            if let Some(calls) = delta["tool_calls"].as_array() {
                for c in calls {
                    let idx = c["index"].as_u64().unwrap_or(0) as usize;
                    while collected.len() <= idx {
                        collected.push((String::new(), String::new(), String::new()));
                    }
                    if let Some(id) = c["id"].as_str() {
                        collected[idx].0.push_str(id);
                    }
                    if let Some(name) = c["function"]["name"].as_str() {
                        collected[idx].1.push_str(name);
                    }
                    if let Some(args) = c["function"]["arguments"].as_str() {
                        collected[idx].2.push_str(args);
                    }
                }
            }
        }
    }

    Ok(finish_openai_calls(collected))
}

fn finish_openai_calls(collected: Vec<(String, String, String)>) -> Vec<ToolCall> {
    collected
        .into_iter()
        .filter(|(_, name, _)| !name.is_empty())
        .map(|(id, name, args)| ToolCall {
            id: if id.is_empty() {
                crate::store::new_id()
            } else {
                id
            },
            name,
            args: serde_json::from_str(&args).unwrap_or(json!({})),
        })
        .collect()
}

fn build_anthropic_body(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
) -> Value {
    let list: Vec<Value> = messages.iter().map(anthropic_message).collect();

    let mut budget: u32 = 0;
    let mut body = json!({
        "model": provider.model,
        "max_tokens": 8192,
        "messages": list,
        "stream": true,
        "tools": tools::anthropic_tools(),
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

fn anthropic_message(m: &ChatMessage) -> Value {
    let mut out = json!({ "role": m.role, "content": m.content });
    if let Some(calls) = &m.tool_calls {
        if let Some(list) = calls.as_array() {
            let mut blocks: Vec<Value> = Vec::new();
            if let Some(reasoning) = &m.reasoning {
                if !reasoning.is_empty() {
                    blocks.push(json!({
                        "type": "thinking",
                        "thinking": reasoning,
                        "signature": ""
                    }));
                }
            }
            for c in list {
                let args: Value = c["function"]["arguments"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(json!({}));
                blocks.push(json!({
                    "type": "tool_use",
                    "id": c["id"],
                    "name": c["function"]["name"],
                    "input": args
                }));
            }
            out["content"] = Value::Array(blocks);
        }
    } else if let Some(reasoning) = &m.reasoning {
        if !reasoning.is_empty() {
            out["content"] = json!([
                { "type": "thinking", "thinking": reasoning, "signature": "" },
                { "type": "text", "text": m.content }
            ]);
        }
    }
    if let Some(id) = &m.tool_call_id {
        if m.role == "user" {
            let blocks: Value =
                serde_json::from_str(&m.content).unwrap_or_else(|_| json!([]));
            let mut arr = blocks.as_array().cloned().unwrap_or_default();
            if arr.is_empty() {
                arr.push(json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": m.content
                }));
            }
            out["content"] = Value::Array(arr);
        }
    }
    out
}

async fn stream_anthropic(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<Vec<ToolCall>, String> {
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
    let mut calls: Vec<ToolCall> = Vec::new();
    let mut current: Option<(String, String, String)> = None;

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
                Some("content_block_start") => {
                    if value["content_block"]["type"].as_str() == Some("tool_use") {
                        current = Some((
                            value["content_block"]["id"].as_str().unwrap_or("").to_string(),
                            value["content_block"]["name"].as_str().unwrap_or("").to_string(),
                            String::new(),
                        ));
                    }
                }
                Some("content_block_delta") => {
                    let delta = &value["delta"];
                    if let Some(text) = delta["text"].as_str() {
                        emit_delta(sink, acc, text);
                    }
                    if let Some(thinking) = delta["thinking"].as_str() {
                        emit_reasoning(sink, acc, thinking);
                    }
                    if let Some(partial) = delta["partial_json"].as_str() {
                        if let Some((_, _, args)) = current.as_mut() {
                            args.push_str(partial);
                        }
                    }
                }
                Some("content_block_stop") => {
                    if let Some((id, name, args)) = current.take() {
                        if !name.is_empty() {
                            calls.push(ToolCall {
                                id,
                                name,
                                args: serde_json::from_str(&args).unwrap_or(json!({})),
                            });
                        }
                    }
                }
                Some("message_stop") => return Ok(calls),
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

    Ok(calls)
}

fn build_gemini_body(
    messages: &[ChatMessage],
    system_prompt: &str,
    provider: &Provider,
) -> Value {
    let contents: Vec<Value> = messages.iter().map(gemini_content).collect();

    let mut body = json!({ "contents": contents, "tools": tools::gemini_tools() });

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

fn gemini_content(m: &ChatMessage) -> Value {
    let role = if m.role == "assistant" { "model" } else { "user" };
    if let Some(calls) = &m.tool_calls {
        if let Some(list) = calls.as_array() {
            let mut parts: Vec<Value> = Vec::new();
            if let Some(reasoning) = &m.reasoning {
                if !reasoning.is_empty() {
                    parts.push(json!({ "text": reasoning, "thought": true }));
                }
            }
            for c in list {
                let args: Value = c["function"]["arguments"]
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or(json!({}));
                parts.push(json!({
                    "functionCall": {
                        "name": c["function"]["name"],
                        "args": args
                    }
                }));
            }
            return json!({ "role": role, "parts": parts });
        }
    }
    if m.tool_call_id.is_some() && m.role == "user" {
        let parts: Value = serde_json::from_str(&m.content).unwrap_or_else(|_| json!([]));
        return json!({ "role": "user", "parts": parts });
    }
    if let Some(reasoning) = &m.reasoning {
        if !reasoning.is_empty() {
            return json!({
                "role": role,
                "parts": [
                    { "text": reasoning, "thought": true },
                    { "text": m.content }
                ]
            });
        }
    }
    json!({ "role": role, "parts": [{ "text": m.content }] })
}

async fn stream_gemini(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
) -> Result<Vec<ToolCall>, String> {
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
    let mut calls: Vec<ToolCall> = Vec::new();

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
                if let Some(call) = part.get("functionCall") {
                    calls.push(ToolCall {
                        id: crate::store::new_id(),
                        name: call["name"].as_str().unwrap_or("").to_string(),
                        args: call["args"].clone(),
                    });
                    continue;
                }
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

    Ok(calls)
}
