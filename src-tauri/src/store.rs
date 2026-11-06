use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub const BUILTIN_PROJECT_ID: &str = "builtin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub system_prompt: String,
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            system_prompt: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
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
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
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

pub fn load(app: &AppHandle) -> Database {
    let path = data_file(app);
    let mut db: Database = match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Database::default(),
    };
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

pub fn save(app: &AppHandle, db: &Database) -> Result<(), String> {
    let path = data_file(app);
    let text = serde_json::to_string_pretty(db).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
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
