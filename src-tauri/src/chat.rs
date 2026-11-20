use crate::approval::Pending;
use crate::presets;
use crate::store::{Attachment, Part, Provider};
use crate::subagent;
use crate::tools::{self, FileChangeInfo};
use futures_util::{future::join_all, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::AppHandle;

const TAP_INTERVAL_MS: u128 = 80;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MediaPart {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    Delta { text: String },
    Reasoning { text: String },
    ReasoningEnd,
    TextStart,
    ApprovalStart {
        id: String,
        tool: String,
        kind: String,
    },
    ApprovalDelta {
        id: String,
        summary: String,
        append: String,
    },
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
    Usage {
        prompt_tokens: u64,
        cached_tokens: u64,
        context_window: u64,
    },
    SubagentStart {
        id: String,
        title: String,
    },
    SubagentReasoning {
        id: String,
        text: String,
    },
    SubagentDelta {
        id: String,
        text: String,
    },
    SubagentTool {
        id: String,
        summary: String,
        detail: String,
        ok: bool,
    },
    SubagentEnd {
        id: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<MediaPart>>,
}

impl ChatMessage {
    pub fn text(role: &str, content: &str) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            media: None,
        }
    }

    pub fn with_reasoning(role: &str, content: &str, reasoning: Option<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.to_string(),
            reasoning,
            tool_calls: None,
            tool_call_id: None,
            media: None,
        }
    }
}

pub type EventSink = Arc<dyn Fn(StreamEvent) + Send + Sync>;

pub struct Acc {
    pub content: String,
    pub reasoning: String,
    pub parts: Vec<Part>,
    pub tool_calls: Vec<ToolCall>,
    pub changes: Vec<FileChangeInfo>,
    pub truncated: bool,
    announced: Vec<String>,
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
            changes: Vec::new(),
            truncated: false,
            announced: Vec::new(),
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
    pub cancel: Arc<AtomicBool>,
    pub readonly: bool,
    pub subagent_prompt: String,
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
        if ctx.cancel.load(Ordering::Relaxed) {
            sink(StreamEvent::Done);
            return Ok(total);
        }

        let mut round = Acc::new();
        let outcome = run_round(provider, messages, system_prompt, &sink, &mut round, ctx).await;

        round.close_reasoning(&sink);
        total.content.push_str(&round.content);
        total.reasoning.push_str(&round.reasoning);
        total.parts.extend(round.parts);
        total.changes.extend(round.changes);

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

        let mut i = 0;
        while i < calls.len() {
            if ctx.cancel.load(Ordering::Relaxed) {
                sink(StreamEvent::Done);
                return Ok(total);
            }

            if calls[i].name == tools::AGENT_TOOL && !ctx.readonly {
                let mut j = i;
                while j < calls.len() && calls[j].name == tools::AGENT_TOOL {
                    j += 1;
                }
                run_agent_batch(provider, &calls[i..j], &sink, ctx, &mut total, messages).await;
                i = j;
                continue;
            }

            let call = &calls[i];
            total.tool_calls.push(call.clone());

            let preview = tools::preview(&call.name, &call.args, ctx.root.as_deref());
            let approved = if ctx.readonly {
                true
            } else if tools::needs_approval(&call.name) {
                let id = call.id.clone();
                if !round.announced.contains(&id) {
                    sink(StreamEvent::ApprovalStart {
                        id: id.clone(),
                        tool: call.name.clone(),
                        kind: tap_kind(&call.name).to_string(),
                    });
                }
                let rx = ctx.pending.register(&id);
                if ctx.cancel.load(Ordering::Relaxed) {
                    ctx.pending.resolve(&id, false);
                }
                sink(StreamEvent::Approval {
                    id: id.clone(),
                    tool: call.name.clone(),
                    summary: preview.summary.clone(),
                    detail: preview.detail.clone(),
                    kind: preview.kind.clone(),
                });
                rx.await.unwrap_or(false)
            } else {
                true
            };

            let blocked = ctx.readonly && !tools::is_readonly(&call.name);
            let mut produced: Option<FileChangeInfo> = None;
            let (result, ok) = if blocked {
                ("error: 只读环境".to_string(), false)
            } else if approved {
                let (text, change) =
                    tools::execute_tracked(&call.name, &call.args, ctx.root.as_deref());
                produced = change;
                (text, true)
            } else {
                ("user denied".to_string(), false)
            };
            if let Some(info) = produced {
                total.changes.push(info);
            }

            total.parts.push(Part {
                kind: "tool".to_string(),
                text: String::new(),
                summary: preview.summary.clone(),
                detail: result.clone(),
                ok,
                tool_call_id: call.id.clone(),
                tool_name: call.name.clone(),
                tool_args: serde_json::to_string(&call.args).unwrap_or_default(),
                children: Vec::new(),
            });

            sink(StreamEvent::ToolEnd {
                id: call.id.clone(),
                summary: preview.summary.clone(),
                detail: result.clone(),
                tool: call.name.clone(),
                ok,
            });

            messages.push(build_tool_result(provider, call, &result));
            i += 1;
        }
    }
}

async fn run_agent_batch(
    provider: &Provider,
    calls: &[ToolCall],
    sink: &EventSink,
    ctx: &Ctx,
    total: &mut Acc,
    messages: &mut Vec<ChatMessage>,
) {
    for call in calls {
        total.tool_calls.push(call.clone());
        let prompt = call.args["prompt"].as_str().unwrap_or("");
        sink(StreamEvent::SubagentStart {
            id: call.id.clone(),
            title: tools::title(prompt),
        });
    }

    let prompts: Vec<String> = calls
        .iter()
        .map(|call| call.args["prompt"].as_str().unwrap_or("").to_string())
        .collect();

    let futures = calls.iter().zip(prompts.iter()).map(|(call, prompt)| {
        subagent::run(
            provider,
            prompt,
            &ctx.subagent_prompt,
            &call.id,
            ctx.root.clone(),
            ctx.pending.clone(),
            ctx.cancel.clone(),
            sink,
        )
    });
    let outcomes = join_all(futures).await;

    for (call, outcome) in calls.iter().zip(outcomes) {
        let preview = tools::preview(&call.name, &call.args, ctx.root.as_deref());
        let result = if outcome.content.trim().is_empty() {
            if outcome.ok {
                "（子代理无正文）".to_string()
            } else {
                "error: 子代理失败".to_string()
            }
        } else {
            outcome.content.clone()
        };

        total.parts.push(Part {
            kind: "tool".to_string(),
            text: outcome.content.clone(),
            summary: preview.summary.clone(),
            detail: result.clone(),
            ok: outcome.ok,
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            tool_args: serde_json::to_string(&call.args).unwrap_or_default(),
            children: outcome.parts,
        });

        sink(StreamEvent::ToolEnd {
            id: call.id.clone(),
            summary: preview.summary,
            detail: result.clone(),
            tool: call.name.clone(),
            ok: outcome.ok,
        });
        sink(StreamEvent::SubagentEnd {
            id: call.id.clone(),
            ok: outcome.ok,
        });

        messages.push(build_tool_result(provider, call, &result));
    }
}

async fn run_round(
    provider: &Provider,
    messages: &mut Vec<ChatMessage>,
    system_prompt: &str,
    sink: &EventSink,
    acc: &mut Acc,
    ctx: &Ctx,
) -> Result<Vec<ToolCall>, String> {
    match provider.protocol.as_str() {
        "anthropic" => {
            stream_anthropic(
                provider,
                messages,
                system_prompt,
                sink,
                acc,
                &ctx.cancel,
                ctx.readonly,
            )
            .await
        }
        "gemini" => {
            stream_gemini(
                provider,
                messages,
                system_prompt,
                sink,
                acc,
                &ctx.cancel,
                ctx.readonly,
            )
            .await
        }
        _ => {
            stream_openai(
                provider,
                messages,
                system_prompt,
                sink,
                acc,
                &ctx.cancel,
                ctx.readonly,
            )
            .await
        }
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
        media: None,
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
            media: None,
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
            media: None,
        }
    } else {
        ChatMessage {
            role: "tool".to_string(),
            content: result.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: Some(call.id.clone()),
            media: None,
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
            tool_call_id: String::new(),
            tool_name: String::new(),
            tool_args: String::new(),
            children: Vec::new(),
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
            tool_call_id: String::new(),
            tool_name: String::new(),
            tool_args: String::new(),
            children: Vec::new(),
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

fn b64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(n >> 18) as usize & 63] as char);
        out.push(CHARS[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            CHARS[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn media_kind(mime: &str, kinds: &[&str], protocol: &str) -> Option<&'static str> {
    let kind = if mime.starts_with("image/") {
        if !matches!(
            mime,
            "image/png" | "image/jpeg" | "image/webp" | "image/gif" | "image/heic" | "image/heif"
        ) {
            return None;
        }
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if mime.starts_with("audio/") {
        // OpenAI 协议仅收 wav/mp3，其余模态交给原生支持方
        if protocol != "gemini" && !mime.contains("wav") && !mime.contains("mpeg") && !mime.contains("mp3") {
            return None;
        }
        "audio"
    } else if mime == "application/pdf" {
        "pdf"
    } else {
        return None;
    };
    if kinds.contains(&kind) {
        Some(kind)
    } else {
        None
    }
}

// 附件按能力表挑选可内联项，读盘转 base64；不可内联的附件由索引行兜底
pub fn inline_media(
    app: &AppHandle,
    provider: &Provider,
    session_id: &str,
    attachments: &[Attachment],
) -> Option<Vec<MediaPart>> {
    let kinds = presets::input_kinds(&provider.protocol, &provider.model);
    if kinds.is_empty() {
        return None;
    }
    let dir = crate::store::session_dir(app, session_id);
    let mut out: Vec<MediaPart> = Vec::new();
    for a in attachments {
        let Some(kind) = media_kind(&a.mime, kinds, &provider.protocol) else {
            continue;
        };
        if kind == "image" && provider.image == "off" {
            continue;
        }
        let Ok(bytes) = fs::read(dir.join(&a.file)) else {
            continue;
        };
        out.push(MediaPart {
            id: a.id.clone(),
            kind: kind.to_string(),
            mime: a.mime.clone(),
            data: b64_encode(&bytes),
            name: a.name.clone(),
        });
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn openai_media(m: &ChatMessage, host: &str, provider: &Provider) -> Value {
    let bare = host.contains("bigmodel");
    let with_detail = !bare
        && (host.contains("openai")
            || host.contains("openrouter")
            || host.contains("x.ai")
            || host.contains("deepseek"));
    let detail = presets::image_detail(&provider.image);
    let mut parts: Vec<Value> = vec![json!({ "type": "text", "text": m.content })];
    for p in m.media.as_deref().unwrap_or(&[]) {
        match p.kind.as_str() {
            "image" => {
                let url = if bare {
                    p.data.clone()
                } else {
                    format!("data:{};base64,{}", p.mime, p.data)
                };
                let mut image = json!({ "type": "image_url", "image_url": { "url": url } });
                if with_detail {
                    image["image_url"]["detail"] = json!(detail);
                }
                parts.push(image);
            }
            "audio" => {
                let fmt = if p.mime.contains("wav") { "wav" } else { "mp3" };
                parts.push(json!({
                    "type": "input_audio",
                    "input_audio": { "data": p.data, "format": fmt }
                }));
            }
            "pdf" => {
                parts.push(json!({
                    "type": "file",
                    "file": {
                        "filename": p.name,
                        "file_data": format!("data:application/pdf;base64,{}", p.data)
                    }
                }));
            }
            _ => {}
        }
    }
    Value::Array(parts)
}

fn build_openai_body(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    readonly: bool,
) -> Value {
    let host = provider.base_url.to_lowercase();
    let is_deepseek = host.contains("deepseek");
    let is_glm = host.contains("bigmodel");

    let mut list: Vec<Value> = Vec::new();
    if !system_prompt.is_empty() {
        list.push(json!({ "role": "system", "content": system_prompt }));
    }
    for m in messages {
        let mut item = json!({ "role": m.role, "content": m.content });
        if m.role == "user" && m.media.is_some() {
            item["content"] = openai_media(m, &host, provider);
        }
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
        "stream_options": { "include_usage": true },
        "tools": tools::openai_tools(readonly),
    });

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
    cancel: &Arc<AtomicBool>,
    readonly: bool,
) -> Result<Vec<ToolCall>, String> {
    let body = build_openai_body(provider, messages, system_prompt, readonly);
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
    let mut taps: Vec<Option<TapState>> = Vec::new();

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
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

            let usage = &value["usage"];
            if !usage.is_null() {
                let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
                let cached = usage["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .or_else(|| usage["prompt_cache_hit_tokens"].as_u64())
                    .unwrap_or(0);
                if prompt > 0 {
                    sink(StreamEvent::Usage {
                        prompt_tokens: prompt,
                        cached_tokens: cached,
                        context_window: presets::context_window(&provider.model),
                    });
                }
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
                        taps.push(None);
                    }
                    if let Some(id) = c["id"].as_str() {
                        if !id.is_empty() && collected[idx].0.is_empty() {
                            collected[idx].0 = id.to_string();
                        }
                    }
                    if let Some(name) = c["function"]["name"].as_str() {
                        if !name.is_empty() {
                            collected[idx].1.push_str(name);
                        }
                    }
                    if taps[idx].is_none() && !collected[idx].1.is_empty() {
                        let name = collected[idx].1.clone();
                        if !readonly && tools::needs_approval(&name) {
                            if collected[idx].0.is_empty() {
                                collected[idx].0 = crate::store::new_id();
                            }
                            let id = collected[idx].0.clone();
                            sink(StreamEvent::ApprovalStart {
                                id: id.clone(),
                                tool: name.clone(),
                                kind: tap_kind(&name).to_string(),
                            });
                            acc.announced.push(id);
                            taps[idx] = Some(TapState::new(&name));
                        }
                    }
                    if let Some(args) = c["function"]["arguments"].as_str() {
                        collected[idx].2.push_str(args);
                        if let Some(tap) = taps[idx].as_mut() {
                            tap_feed(tap, &collected[idx].2, &collected[idx].0, sink);
                        }
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

struct TapState {
    tool: String,
    kind: String,
    summary: String,
    body: String,
    published: usize,
    last: Instant,
}

impl TapState {
    fn new(tool: &str) -> Self {
        Self {
            tool: tool.to_string(),
            kind: tap_kind(tool).to_string(),
            summary: String::new(),
            body: String::new(),
            published: 0,
            last: Instant::now(),
        }
    }
}

fn tap_kind(tool: &str) -> &'static str {
    match tool {
        "write" => "write",
        "edit" => "edit",
        "bash" => "bash",
        "fetch" => "fetch",
        "agent" => "agent",
        _ => "read",
    }
}

fn tap_preview(state: &TapState, raw: &str) -> (String, String) {
    let fields = partial_fields(raw);
    let path = fields.get("path").map(String::as_str).unwrap_or("");
    match state.kind.as_str() {
        "write" => {
            let body = fields.get("content").cloned().unwrap_or_default();
            (format!("写入 {path}"), body)
        }
        "edit" => {
            let body = fields.get("new").cloned().unwrap_or_default();
            (format!("编辑 {path}"), body)
        }
        "bash" => {
            let body = fields.get("command").cloned().unwrap_or_default();
            ("执行命令".to_string(), body)
        }
        "fetch" => {
            let url = fields.get("url").map(String::as_str).unwrap_or("");
            (format!("取 {url}"), String::new())
        }
        "agent" => {
            let prompt = fields.get("prompt").map(String::as_str).unwrap_or("");
            let brief = tools::title(prompt);
            let summary = if brief.is_empty() {
                "子代理".to_string()
            } else {
                format!("子代理 · {brief}")
            };
            (summary, prompt.to_string())
        }
        _ => {
            let hint = if !path.is_empty() {
                path
            } else {
                fields.get("pattern").map(String::as_str).unwrap_or("")
            };
            let summary = format!("{} {}", state.tool, hint);
            (summary.trim().to_string(), String::new())
        }
    }
}

fn tap_feed(state: &mut TapState, raw: &str, id: &str, sink: &EventSink) {
    let now = Instant::now();
    if now.duration_since(state.last).as_millis() < TAP_INTERVAL_MS {
        return;
    }
    state.last = now;

    let (summary, body) = tap_preview(state, raw);
    if !body.starts_with(&state.body) {
        state.published = 0;
    }
    state.body = body;

    let append = if state.published <= state.body.len() {
        state.body[state.published..].to_string()
    } else {
        state.body.clone()
    };
    state.published = state.body.len();

    if append.is_empty() && summary == state.summary {
        return;
    }
    state.summary = summary.clone();
    sink(StreamEvent::ApprovalDelta {
        id: id.to_string(),
        summary,
        append,
    });
}

fn partial_fields(raw: &str) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0usize;
    let mut depth = 0i32;

    while i < chars.len() {
        let c = chars[i];
        if c == '{' || c == '[' {
            depth += 1;
            i += 1;
            continue;
        }
        if c == '}' || c == ']' {
            depth -= 1;
            i += 1;
            continue;
        }
        if c != '"' {
            i += 1;
            continue;
        }

        let (key, next, closed) = read_json_string(&chars, i);
        i = next;
        if depth != 1 || !closed {
            continue;
        }

        let mut j = i;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if j >= chars.len() || chars[j] != ':' {
            continue;
        }
        j += 1;
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if j >= chars.len() || chars[j] != '"' {
            continue;
        }
        let (value, next, _) = read_json_string(&chars, j);
        out.insert(key, value);
        i = next;
    }

    out
}

fn read_json_string(chars: &[char], start: usize) -> (String, usize, bool) {
    let mut out = String::new();
    let mut i = start + 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            return (out, i + 1, true);
        }
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        if i + 1 >= chars.len() {
            return (out, i + 1, false);
        }
        match chars[i + 1] {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{0008}'),
            'f' => out.push('\u{000c}'),
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'u' => {
                if i + 6 > chars.len() {
                    return (out, chars.len(), false);
                }
                let hex: String = chars[i + 2..i + 6].iter().collect();
                let mut code = u32::from_str_radix(&hex, 16).unwrap_or(0);
                let mut next = i + 6;
                if (0xD800..0xDC00).contains(&code)
                    && next + 6 <= chars.len()
                    && chars[next] == '\\'
                    && chars[next + 1] == 'u'
                {
                    let hex2: String = chars[next + 2..next + 6].iter().collect();
                    let low = u32::from_str_radix(&hex2, 16).unwrap_or(0);
                    if (0xDC00..0xE000).contains(&low) {
                        code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                        next += 6;
                    }
                }
                if let Some(ch) = char::from_u32(code) {
                    out.push(ch);
                }
                i = next;
                continue;
            }
            other => out.push(other),
        }
        i += 2;
    }
    (out, i, false)
}

fn build_anthropic_body(
    provider: &Provider,
    messages: &[ChatMessage],
    system_prompt: &str,
    readonly: bool,
) -> Value {
    let mut list: Vec<Value> = messages.iter().map(anthropic_message).collect();

    if list.len() >= 2 {
        let idx = list.len() - 2;
        let is_user = list[idx]["role"].as_str() == Some("user");
        if is_user {
            if let Some(text) = list[idx]["content"].as_str() {
                list[idx]["content"] = json!([{
                    "type": "text",
                    "text": text,
                    "cache_control": { "type": "ephemeral" }
                }]);
            }
        }
    }

    let mut budget: u32 = 0;
    let mut body = json!({
        "model": provider.model,
        "max_tokens": 8192,
        "messages": list,
        "stream": true,
    });

    let mut tool_list = tools::anthropic_tools(readonly);
    if let Some(arr) = tool_list.as_array_mut() {
        if let Some(last) = arr.last_mut() {
            last["cache_control"] = json!({ "type": "ephemeral" });
        }
    }
    body["tools"] = tool_list;

    if !system_prompt.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": system_prompt,
            "cache_control": { "type": "ephemeral" }
        }]);
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
    if m.role == "user" && m.tool_call_id.is_none() {
        if let Some(media) = &m.media {
            let mut blocks: Vec<Value> = vec![json!({ "type": "text", "text": m.content })];
            for p in media {
                match p.kind.as_str() {
                    "image" => blocks.push(json!({
                        "type": "image",
                        "source": { "type": "base64", "media_type": p.mime, "data": p.data }
                    })),
                    "pdf" => blocks.push(json!({
                        "type": "document",
                        "source": { "type": "base64", "media_type": "application/pdf", "data": p.data },
                        "title": p.name
                    })),
                    _ => {}
                }
            }
            out["content"] = Value::Array(blocks);
            return out;
        }
    }
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
            if !m.content.is_empty() {
                blocks.push(json!({ "type": "text", "text": m.content }));
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
    cancel: &Arc<AtomicBool>,
    readonly: bool,
) -> Result<Vec<ToolCall>, String> {
    let body = build_anthropic_body(provider, messages, system_prompt, readonly);
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
    let mut current: Option<(String, String, String, Option<TapState>)> = None;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
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
                Some("message_start") => {
                    let usage = &value["message"]["usage"];
                    let input = usage["input_tokens"].as_u64().unwrap_or(0);
                    let cached = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                    let created = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                    let total = input + cached + created;
                    if total > 0 {
                        sink(StreamEvent::Usage {
                            prompt_tokens: total,
                            cached_tokens: cached,
                            context_window: presets::context_window(&provider.model),
                        });
                    }
                }
                Some("message_delta") => {
                    let usage = &value["usage"];
                    let input = usage["input_tokens"].as_u64().unwrap_or(0);
                    let cached = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
                    let created = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                    let total = input + cached + created;
                    if total > 0 {
                        sink(StreamEvent::Usage {
                            prompt_tokens: total,
                            cached_tokens: cached,
                            context_window: presets::context_window(&provider.model),
                        });
                    }
                }
                Some("content_block_start") => {
                    if value["content_block"]["type"].as_str() == Some("tool_use") {
                        let id = value["content_block"]["id"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let name = value["content_block"]["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let tap = if !name.is_empty() && !readonly && tools::needs_approval(&name) {
                            sink(StreamEvent::ApprovalStart {
                                id: id.clone(),
                                tool: name.clone(),
                                kind: tap_kind(&name).to_string(),
                            });
                            acc.announced.push(id.clone());
                            Some(TapState::new(&name))
                        } else {
                            None
                        };
                        current = Some((id, name, String::new(), tap));
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
                        if let Some((id, _, args, tap)) = current.as_mut() {
                            args.push_str(partial);
                            if let Some(t) = tap.as_mut() {
                                tap_feed(t, args.as_str(), id.as_str(), sink);
                            }
                        }
                    }
                }
                Some("content_block_stop") => {
                    if let Some((id, name, args, _)) = current.take() {
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
    readonly: bool,
) -> Value {
    let contents: Vec<Value> = messages.iter().map(gemini_content).collect();

    let mut body = json!({ "contents": contents, "tools": tools::gemini_tools(readonly) });

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
    if m.role == "user" && m.tool_call_id.is_none() {
        if let Some(media) = &m.media {
            let mut parts: Vec<Value> = vec![json!({ "text": m.content })];
            for p in media {
                parts.push(json!({
                    "inline_data": { "mime_type": p.mime, "data": p.data }
                }));
            }
            return json!({ "role": role, "parts": parts });
        }
    }
    if let Some(calls) = &m.tool_calls {
        if let Some(list) = calls.as_array() {
            let mut parts: Vec<Value> = Vec::new();
            if let Some(reasoning) = &m.reasoning {
                if !reasoning.is_empty() {
                    parts.push(json!({ "text": reasoning, "thought": true }));
                }
            }
            if !m.content.is_empty() {
                parts.push(json!({ "text": m.content }));
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
    cancel: &Arc<AtomicBool>,
    readonly: bool,
) -> Result<Vec<ToolCall>, String> {
    let body = build_gemini_body(messages, system_prompt, provider, readonly);
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
        if cancel.load(Ordering::Relaxed) {
            break;
        }
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
            let usage = &value["usageMetadata"];
            if !usage.is_null() {
                let prompt = usage["promptTokenCount"].as_u64().unwrap_or(0);
                let cached = usage["cachedContentTokenCount"].as_u64().unwrap_or(0);
                if prompt > 0 {
                    sink(StreamEvent::Usage {
                        prompt_tokens: prompt,
                        cached_tokens: cached,
                        context_window: presets::context_window(&provider.model),
                    });
                }
            }
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
