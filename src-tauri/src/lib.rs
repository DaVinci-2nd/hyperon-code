mod approval;
mod chat;
mod presets;
mod rollback;
mod store;
mod tools;

use approval::Pending;
use chat::ChatMessage;
use presets::Preset;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use store::{Database, Message, Part, Project, Provider, Session, Settings, BUILTIN_PROJECT_ID};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

pub struct AppState {
    pending: Arc<Pending>,
    cancel: Arc<AtomicBool>,
}

impl AppState {
    fn new() -> Self {
        Self {
            pending: Arc::new(Pending::new()),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[tauri::command]
fn approve(state: State<'_, AppState>, id: String, allowed: bool) -> bool {
    state.pending.resolve(&id, allowed)
}

#[tauri::command]
fn cancel_message(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
    state.pending.deny_all();
}

#[derive(Serialize)]
struct Snapshot {
    settings: Settings,
    providers: Vec<Provider>,
    projects: Vec<Project>,
    sessions: Vec<Session>,
    messages: Vec<Message>,
}

#[tauri::command]
fn load_all(app: AppHandle) -> Result<Snapshot, String> {
    let db = store::load(&app);
    Ok(Snapshot {
        settings: db.settings,
        providers: db.providers,
        projects: db.projects,
        sessions: db.sessions,
        messages: db.messages,
    })
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) {
    let mut db = store::load(&app);
    db.settings = settings;
    store::save(&app, &db);
}

#[tauri::command]
fn list_presets() -> Vec<Preset> {
    presets::PRESETS.to_vec()
}

#[tauri::command]
fn add_provider(app: AppHandle, provider: Provider) -> Result<Provider, String> {
    let mut db = store::load(&app);
    let mut p = provider;
    if p.id.is_empty() {
        p.id = store::new_id();
    }
    db.providers.push(p.clone());
    store::save(&app, &db);
    Ok(p)
}

#[tauri::command]
fn update_provider(app: AppHandle, provider: Provider) {
    let mut db = store::load(&app);
    if let Some(slot) = db.providers.iter_mut().find(|p| p.id == provider.id) {
        *slot = provider;
    }
    store::save(&app, &db);
}

#[tauri::command]
fn delete_provider(app: AppHandle, id: String) {
    let mut db = store::load(&app);
    db.providers.retain(|p| p.id != id);
    store::save(&app, &db);
}

#[tauri::command]
fn create_project(app: AppHandle, name: String, dirs: Vec<String>) -> Result<Project, String> {
    let mut db = store::load(&app);
    let project = Project {
        id: store::new_id(),
        name: if name.trim().is_empty() {
            "未命名工程".to_string()
        } else {
            name.trim().to_string()
        },
        dirs,
        builtin: false,
    };
    db.projects.push(project.clone());
    let last = db.projects.len() - 1;
    let moved = db.projects.remove(last);
    let insert_at = if db.projects.first().map(|p| p.builtin).unwrap_or(false) {
        1
    } else {
        0
    };
    db.projects.insert(insert_at.min(db.projects.len()), moved);
    store::save(&app, &db);
    Ok(project)
}

#[tauri::command]
fn reorder_projects(app: AppHandle, ids: Vec<String>) {
    let mut db = store::load(&app);
    let mut ordered: Vec<Project> = Vec::new();
    for id in &ids {
        if let Some(pos) = db.projects.iter().position(|p| &p.id == id) {
            ordered.push(db.projects.remove(pos));
        }
    }
    for p in db.projects.drain(..) {
        ordered.push(p);
    }
    let builtin = ordered.iter().position(|p| p.builtin);
    if let Some(pos) = builtin {
        if pos != 0 {
            let p = ordered.remove(pos);
            ordered.insert(0, p);
        }
    }
    db.projects = ordered;
    store::save(&app, &db);
}

#[tauri::command]
fn update_project(app: AppHandle, id: String, name: String, dirs: Vec<String>) -> Result<(), String> {
    if id == BUILTIN_PROJECT_ID {
        return Err("默认工程不可修改".to_string());
    }
    let mut db = store::load(&app);
    if let Some(p) = db.projects.iter_mut().find(|p| p.id == id) {
        p.name = if name.trim().is_empty() {
            "未命名工程".to_string()
        } else {
            name.trim().to_string()
        };
        p.dirs = dirs;
    }
    store::save(&app, &db);
    Ok(())
}

#[tauri::command]
fn delete_project(app: AppHandle, id: String) -> Result<(), String> {
    if id == BUILTIN_PROJECT_ID {
        return Err("默认工程不可删除".to_string());
    }
    let mut db = store::load(&app);
    db.projects.retain(|p| p.id != id);
    for s in db.sessions.iter_mut() {
        if s.project_id == id {
            s.project_id = BUILTIN_PROJECT_ID.to_string();
        }
    }
    store::save(&app, &db);
    Ok(())
}

#[tauri::command]
fn create_session(app: AppHandle, project_id: String) -> Result<Session, String> {
    let mut db = store::load(&app);
    if !db.projects.iter().any(|p| p.id == project_id) {
        return Err("工程不存在".to_string());
    }
    let count = db
        .sessions
        .iter()
        .filter(|s| s.project_id == project_id)
        .count();
    let session = Session {
        id: store::new_id(),
        project_id,
        title: format!("新会话 {}", count + 1),
        created_at: store::now_ms(),
        last_prompt_tokens: 0,
        last_cached_tokens: 0,
    };
    db.sessions.push(session.clone());
    store::save(&app, &db);
    Ok(session)
}

#[tauri::command]
fn update_session(app: AppHandle, id: String, title: String) {
    let mut db = store::load(&app);
    if let Some(s) = db.sessions.iter_mut().find(|s| s.id == id) {
        s.title = title.trim().to_string();
    }
    store::save(&app, &db);
}

#[tauri::command]
fn delete_session(app: AppHandle, id: String) {
    let mut db = store::load(&app);
    db.sessions.retain(|s| s.id != id);
    db.messages.retain(|m| m.session_id != id);
    store::save(&app, &db);
}

#[tauri::command]
fn get_messages(app: AppHandle, session_id: String) -> Result<Vec<Message>, String> {
    let db = store::load(&app);
    Ok(db
        .messages
        .into_iter()
        .filter(|m| m.session_id == session_id)
        .collect())
}

#[tauri::command]
fn get_messages_sync(app: AppHandle, session_id: String) -> Result<Vec<Message>, String> {
    get_messages(app, session_id)
}

#[tauri::command]
fn rollback_preview(
    app: AppHandle,
    session_id: String,
    message_id: String,
) -> Result<rollback::RollbackPlan, String> {
    let db = store::load(&app);
    rollback::preview(&db, &session_id, &message_id).ok_or_else(|| "消息不存在".to_string())
}

#[tauri::command]
fn rollback_apply(
    app: AppHandle,
    session_id: String,
    message_id: String,
    force: bool,
) -> Result<rollback::RollbackReport, String> {
    let mut db = store::load(&app);
    if !force {
        if let Some(plan) = rollback::preview(&db, &session_id, &message_id) {
            if !plan.restorable {
                return Err("部分文件无法还原，需确认后才能回溯".to_string());
            }
        }
    }
    let report = rollback::apply(&mut db, &session_id, &message_id)?;
    store::save(&app, &db);
    Ok(report)
}

fn estimate_tokens(text: &str) -> u64 {
    let mut cjk: f64 = 0.0;
    let mut other: f64 = 0.0;
    for ch in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3040}'..='\u{30ff}').contains(&ch)
            || ('\u{ac00}'..='\u{d7af}').contains(&ch)
        {
            cjk += 1.0;
        } else {
            other += 1.0;
        }
    }
    (cjk * 0.6 + other * 0.3).round() as u64
}

fn build_history(
    db: &Database,
    session_id: &str,
    protocol: &str,
    strip_thinking: bool,
    strip_duplicates: bool,
) -> Vec<ChatMessage> {
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    let mut out: Vec<ChatMessage> = Vec::new();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    let mut call_seq: usize = 0;
    let assistant_total = db
        .messages
        .iter()
        .filter(|m| m.session_id == session_id && m.role == "assistant")
        .count();
    let mut assistant_index: usize = 0;

    for m in db.messages.iter().filter(|m| m.session_id == session_id) {
        if m.role != "assistant" {
            out.push(ChatMessage::text(&m.role, &m.content));
            continue;
        }

        assistant_index += 1;
        // 只保留第一次和最近一次回复的思考，其余回复的思考在发送时置空
        let keep_thinking =
            !strip_thinking || assistant_index == 1 || assistant_index == assistant_total;
        let reasoning = if keep_thinking && !m.reasoning.is_empty() {
            Some(m.reasoning.clone())
        } else {
            None
        };

        let tool_parts: Vec<&Part> = m
            .parts
            .iter()
            .filter(|p| p.kind == "tool" && !p.tool_name.is_empty())
            .collect();

        if tool_parts.is_empty() {
            if m.content.is_empty() && reasoning.is_none() {
                continue;
            }
            out.push(ChatMessage::with_reasoning("assistant", &m.content, reasoning));
            continue;
        }

        let mut calls: Vec<serde_json::Value> = Vec::new();
        let mut results: Vec<(String, String, String)> = Vec::new();
        for p in &tool_parts {
            call_seq += 1;
            let id = if p.tool_call_id.is_empty() {
                format!("call_{call_seq}")
            } else {
                p.tool_call_id.clone()
            };
            let args = if p.tool_args.is_empty() {
                "{}".to_string()
            } else {
                p.tool_args.clone()
            };
            calls.push(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": p.tool_name.clone(),
                    "arguments": args
                }
            }));

            let mut detail = p.detail.clone();
            if strip_duplicates && !detail.is_empty() {
                let mut hasher = DefaultHasher::new();
                detail.hash(&mut hasher);
                let hash = hasher.finish();
                if let Some(first) = seen.get(&hash) {
                    detail = format!(
                        "[重复工具结果，与第 {first} 次相同，已省略 {} 字节]",
                        p.detail.len()
                    );
                } else {
                    seen.insert(hash, call_seq);
                }
            }
            results.push((id, p.tool_name.clone(), detail));
        }

        out.push(ChatMessage {
            role: "assistant".to_string(),
            content: m.content.clone(),
            reasoning,
            tool_calls: Some(serde_json::Value::Array(calls)),
            tool_call_id: None,
        });

        let first_id = results.first().map(|r| r.0.clone()).unwrap_or_default();
        if protocol == "anthropic" {
            let blocks: Vec<serde_json::Value> = results
                .iter()
                .map(|(id, _, detail)| {
                    serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": detail
                    })
                })
                .collect();
            out.push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(blocks).to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: Some(first_id),
            });
        } else if protocol == "gemini" {
            let parts: Vec<serde_json::Value> = results
                .iter()
                .map(|(_, name, detail)| {
                    serde_json::json!({
                        "functionResponse": {
                            "name": name,
                            "response": { "result": detail }
                        }
                    })
                })
                .collect();
            out.push(ChatMessage {
                role: "user".to_string(),
                content: serde_json::Value::Array(parts).to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: Some(first_id),
            });
        } else {
            for (id, _, detail) in results {
                out.push(ChatMessage {
                    role: "tool".to_string(),
                    content: detail,
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: Some(id),
                });
            }
        }
    }

    out
}

#[tauri::command]
fn estimate_context(
    session_id: String,
    provider_id: String,
    app: AppHandle,
    system_prompt: String,
    strip_thinking: bool,
) -> Result<(u64, u64, Option<(u64, u64)>), String> {
    let db = store::load(&app);
    let mut total = estimate_tokens(&system_prompt);

    let protocol = db
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .or_else(|| db.providers.first())
        .map(|p| p.protocol.clone())
        .unwrap_or_else(|| "openai".to_string());
    let tools_json = match protocol.as_str() {
        "anthropic" => tools::anthropic_tools(),
        "gemini" => tools::gemini_tools(),
        _ => tools::openai_tools(),
    };
    total += estimate_tokens(&tools_json.to_string());

    let history = build_history(
        &db,
        &session_id,
        &protocol,
        strip_thinking,
        db.settings.strip_duplicate_tool_results,
    );
    for m in &history {
        total += estimate_tokens(&serde_json::to_string(m).unwrap_or_default());
    }
    let window = db
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .or_else(|| db.providers.first())
        .map(|p| presets::context_window(&p.model))
        .unwrap_or(200_000);

    let last_usage = db
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| {
            if s.last_prompt_tokens > 0 {
                Some((s.last_prompt_tokens, s.last_cached_tokens))
            } else {
                None
            }
        });

    Ok((total, window, last_usage))
}

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    provider_id: String,
    content: String,
    channel: Channel<chat::StreamEvent>,
) -> Result<String, String> {
    state.cancel.store(false, Ordering::Relaxed);

    let assistant_id = store::new_id();
    let (provider, mut history, root, system_prompt) = {
        let mut db = store::load(&app);
        if !db.sessions.iter().any(|s| s.id == session_id) {
            return Err("会话不存在".to_string());
        }
        let provider = db
            .providers
            .iter()
            .find(|p| p.id == provider_id)
            .cloned()
            .ok_or_else(|| "模型未配置".to_string())?;
        db.messages.push(Message {
            id: store::new_id(),
            session_id: session_id.clone(),
            role: "user".to_string(),
            content: content.clone(),
            reasoning: String::new(),
            parts: Vec::new(),
            created_at: store::now_ms(),
        });
        if let Some(s) = db.sessions.iter_mut().find(|s| s.id == session_id) {
            if s.title.starts_with("新会话") {
                s.title = content.chars().take(20).collect();
            }
        }
        let history = build_history(
            &db,
            &session_id,
            &provider.protocol,
            db.settings.strip_thinking,
            db.settings.strip_duplicate_tool_results,
        );
        let root = db
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .and_then(|s| db.projects.iter().find(|p| p.id == s.project_id))
            .and_then(|p| p.dirs.first().cloned());
        db.messages.push(Message {
            id: assistant_id.clone(),
            session_id: session_id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            reasoning: String::new(),
            parts: Vec::new(),
            created_at: store::now_ms(),
        });
        store::save(&app, &db);
        (
            provider,
            history,
            root,
            db.settings.system_prompt.clone(),
        )
    };

    let usage_slot: Arc<std::sync::Mutex<Option<(u64, u64)>>> =
        Arc::new(std::sync::Mutex::new(None));
    let usage_tap = usage_slot.clone();

    let sink = Arc::new(move |event: chat::StreamEvent| {
        if let chat::StreamEvent::Usage {
            prompt_tokens,
            cached_tokens,
            ..
        } = &event
        {
            if let Ok(mut slot) = usage_tap.lock() {
                *slot = Some((*prompt_tokens, *cached_tokens));
            }
        }
        let _ = channel.send(event);
    });

    let ctx = chat::Ctx {
        root,
        pending: state.pending.clone(),
        cancel: state.cancel.clone(),
    };

    let result = chat::stream(
        &provider,
        &mut history,
        &system_prompt,
        sink,
        &ctx,
    )
    .await;

    match result {
        Ok(acc) => {
            let mut db3 = store::load(&app);
            if let Some(m) = db3.messages.iter_mut().find(|m| m.id == assistant_id) {
                m.content = acc.content.clone();
                m.reasoning = acc.reasoning.clone();
                m.parts = acc.parts.clone();
            }
            let created = store::now_ms();
            for change in &acc.changes {
                db3.changes.push(store::FileChange {
                    id: store::new_id(),
                    session_id: session_id.clone(),
                    message_id: assistant_id.clone(),
                    path: change.path.clone(),
                    existed: change.existed,
                    before: change.before.clone(),
                    known: change.known(),
                    after: change.after.clone(),
                    created_at: created,
                });
            }
            if let Ok(slot) = usage_slot.lock() {
                if let Some((prompt, cached)) = *slot {
                    if let Some(s) = db3.sessions.iter_mut().find(|s| s.id == session_id) {
                        s.last_prompt_tokens = prompt;
                        s.last_cached_tokens = cached;
                    }
                }
            }
            store::save(&app, &db3);
            Ok(assistant_id)
        }
        Err(err) => {
            let mut db3 = store::load(&app);
            if let Some(m) = db3.messages.iter_mut().find(|m| m.id == assistant_id) {
                m.content = format!("请求失败：{err}");
            }
            store::save(&app, &db3);
            Err(err)
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            load_all,
            save_settings,
            list_presets,
            add_provider,
            update_provider,
            delete_provider,
            create_project,
            update_project,
            reorder_projects,
            delete_project,
            create_session,
            update_session,
            delete_session,
            get_messages,
            get_messages_sync,
            rollback_preview,
            rollback_apply,
            estimate_context,
            approve,
            cancel_message,
            send_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
