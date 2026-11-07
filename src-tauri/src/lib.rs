mod approval;
mod chat;
mod presets;
mod store;
mod tools;

use approval::Pending;
use chat::ChatMessage;
use presets::Preset;
use serde::Serialize;
use std::sync::Arc;
use store::{Message, Project, Provider, Session, Settings, BUILTIN_PROJECT_ID};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

pub struct AppState {
    pending: Arc<Pending>,
}

impl AppState {
    fn new() -> Self {
        Self {
            pending: Arc::new(Pending::new()),
        }
    }
}

#[tauri::command]
fn approve(state: State<'_, AppState>, id: String, allowed: bool) -> bool {
    state.pending.resolve(&id, allowed)
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
    store::save(&app, &db);
    Ok(project)
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
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    provider_id: String,
    content: String,
    channel: Channel<chat::StreamEvent>,
) -> Result<String, String> {
    {
        let mut db = store::load(&app);
        if !db.sessions.iter().any(|s| s.id == session_id) {
            return Err("会话不存在".to_string());
        }
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
        store::save(&app, &db);
    }

    let db = store::load(&app);
    let provider = db
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .cloned()
        .ok_or_else(|| "模型未配置".to_string())?;

    let strip = db.settings.strip_thinking;
    let mut history: Vec<ChatMessage> = db
        .messages
        .iter()
        .filter(|m| m.session_id == session_id)
        .map(|m| {
            if m.role == "assistant" {
                let reasoning = if strip {
                    String::new()
                } else {
                    m.reasoning.clone()
                };
                ChatMessage::with_reasoning(&m.role, &m.content, Some(reasoning))
            } else {
                ChatMessage::text(&m.role, &m.content)
            }
        })
        .collect();

    let root = db
        .sessions
        .iter()
        .find(|s| s.id == session_id)
        .and_then(|s| db.projects.iter().find(|p| p.id == s.project_id))
        .and_then(|p| p.dirs.first().cloned());

    let assistant_id = store::new_id();
    {
        let mut db2 = store::load(&app);
        db2.messages.push(Message {
            id: assistant_id.clone(),
            session_id: session_id.clone(),
            role: "assistant".to_string(),
            content: String::new(),
            reasoning: String::new(),
            parts: Vec::new(),
            created_at: store::now_ms(),
        });
        store::save(&app, &db2);
    }

    let sink = Arc::new(move |event: chat::StreamEvent| {
        let _ = channel.send(event);
    });

    let ctx = chat::Ctx {
        root,
        pending: state.pending.clone(),
    };

    let result = chat::stream(
        &provider,
        &mut history,
        &db.settings.system_prompt,
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
            delete_project,
            create_session,
            update_session,
            delete_session,
            get_messages,
            get_messages_sync,
            approve,
            send_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
