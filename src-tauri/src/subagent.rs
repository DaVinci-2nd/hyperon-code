use crate::approval::Pending;
use crate::chat::{self, ChatMessage, Ctx, EventSink, StreamEvent};
use crate::store::{Part, Provider};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct Outcome {
    pub content: String,
    pub parts: Vec<Part>,
    pub ok: bool,
}

pub async fn run(
    provider: &Provider,
    prompt: &str,
    system_prompt: &str,
    id: &str,
    root: Option<String>,
    pending: Arc<Pending>,
    cancel: Arc<AtomicBool>,
    sink: &EventSink,
) -> Outcome {
    let mut messages = vec![ChatMessage::text("user", prompt)];
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let inner: EventSink = {
        let outer = sink.clone();
        let id = id.to_string();
        let error = error.clone();
        Arc::new(move |event| match event {
            StreamEvent::Delta { text } => outer(StreamEvent::SubagentDelta {
                id: id.clone(),
                text,
            }),
            StreamEvent::Reasoning { text } => outer(StreamEvent::SubagentReasoning {
                id: id.clone(),
                text,
            }),
            StreamEvent::ToolEnd {
                summary, detail, ok, ..
            } => outer(StreamEvent::SubagentTool {
                id: id.clone(),
                summary,
                detail,
                ok,
            }),
            StreamEvent::Error { message } => {
                if let Ok(mut slot) = error.lock() {
                    *slot = Some(message);
                }
            }
            _ => {}
        })
    };

    let ctx = Ctx {
        root,
        pending,
        cancel,
        readonly: true,
        subagent_prompt: String::new(),
    };

    match chat::stream(provider, &mut messages, system_prompt, inner, &ctx).await {
        Ok(acc) => {
            let failed = error.lock().map(|slot| slot.is_some()).unwrap_or(false);
            Outcome {
                content: acc.content,
                parts: acc.parts,
                ok: !acc.truncated && !failed,
            }
        }
        Err(err) => Outcome {
            content: format!("error: {err}"),
            parts: Vec::new(),
            ok: false,
        },
    }
}
