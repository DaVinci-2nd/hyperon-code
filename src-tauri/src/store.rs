use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const BUILTIN_PROJECT_ID: &str = "builtin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_strip")]
    pub strip_thinking: bool,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_strip() -> bool {
    true
}

pub const DEFAULT_PROMPT: &str = "你必须总是回忆以下思维规范：
思考纯中文思考，用此用户语言输出正文，深入反复思考，反复确认，多用工具验证事实，多问用户，不得猜测，正文篇幅要极简，少废话，理性，中立，不得说教，严格遵循用户要求，正文不得先说结论，正文必须先列举和分析，最后才能说结论，行动前必须和用户确认，行动时想好再调用工具，不得随意，删改行动危险需谨慎并征得用户同意和确认
回复规范：
每次思考时必须以“我需要回忆系统提示和用户的要求：……”开头并回忆系统提示和用户的要求，正文不要写回忆过程，正文只输出必要内容和重要内容列举，态度友好，使用体贴风格语气，时刻注意你的言行，不得强迫用户，不得主动放弃，不得顶撞用户，不得做与用户意愿不符的事情，需顺应用户意愿
每次思考反复确认以上要求是否做到";

fn default_prompt() -> String {
    DEFAULT_PROMPT.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            system_prompt: default_prompt(),
            strip_thinking: default_strip(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub thinking: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_prompt_tokens: u64,
    #[serde(default)]
    pub last_cached_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
    #[serde(default)]
    pub parts: Vec<Part>,
    #[serde(default)]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            providers: Vec::new(),
            projects: vec![Project {
                id: BUILTIN_PROJECT_ID.to_string(),
                name: "默认工程".to_string(),
                dirs: Vec::new(),
                builtin: true,
            }],
            sessions: Vec::new(),
            messages: Vec::new(),
        }
    }
}

pub fn data_file(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = fs::create_dir_all(&dir);
    dir.join("data.json")
}

fn backup_corrupt(path: &PathBuf) {
    if let Ok(text) = fs::read_to_string(path) {
        let stamp = now_ms();
        let bak = path.with_file_name(format!("data.corrupt-{stamp}.json"));
        let _ = fs::write(bak, text);
    }
}

fn parse_lenient(text: &str) -> Option<Database> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut db: Database = serde_json::from_value(value.clone()).unwrap_or_default();

    if let Some(list) = value.get("messages").and_then(|v| v.as_array()) {
        let parsed: Vec<Message> = list
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect();
        if !parsed.is_empty() {
            db.messages = parsed;
        }
    }

    if let Some(list) = value.get("sessions").and_then(|v| v.as_array()) {
        let parsed: Vec<Session> = list
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect();
        if !parsed.is_empty() {
            db.sessions = parsed;
        }
    }

    if let Some(list) = value.get("projects").and_then(|v| v.as_array()) {
        let parsed: Vec<Project> = list
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect();
        if !parsed.is_empty() {
            db.projects = parsed;
        }
    }

    if let Some(list) = value.get("providers").and_then(|v| v.as_array()) {
        let parsed: Vec<Provider> = list
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect();
        if !parsed.is_empty() {
            db.providers = parsed;
        }
    }

    if let Some(obj) = value.get("settings") {
        if let Ok(s) = serde_json::from_value::<Settings>(obj.clone()) {
            db.settings = s;
        }
    }

    Some(db)
}

pub fn load(app: &AppHandle) -> Database {
    let path = data_file(app);
    let mut db: Database = match fs::read_to_string(&path) {
        Ok(text) => match parse_lenient(&text) {
            Some(parsed) => parsed,
            None => {
                backup_corrupt(&path);
                Database::default()
            }
        },
        Err(_) => Database::default(),
    };
    if db.settings.system_prompt.is_empty() {
        db.settings.system_prompt = DEFAULT_PROMPT.to_string();
    }
    if !db.projects.iter().any(|p| p.id == BUILTIN_PROJECT_ID) {
        db.projects.insert(
            0,
            Project {
                id: BUILTIN_PROJECT_ID.to_string(),
                name: "默认工程".to_string(),
                dirs: Vec::new(),
                builtin: true,
            },
        );
    }
    db
}

pub fn save(app: &AppHandle, db: &Database) {
    let path = data_file(app);
    let text = match serde_json::to_string_pretty(db) {
        Ok(t) => t,
        Err(_) => return,
    };

    let tmp = path.with_file_name("data.json.tmp");
    if fs::write(&tmp, &text).is_ok() && fs::rename(&tmp, &path).is_ok() {
        return;
    }
    let _ = fs::remove_file(&tmp);

    if fs::write(&path, &text).is_ok() {
        return;
    }

    let stamp = now_ms();
    let fallback = path.with_file_name(format!("data.force-{stamp}.json"));
    let _ = fs::write(&fallback, &text);
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
