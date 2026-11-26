use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const BUILTIN_PROJECT_ID: &str = "builtin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_prompt")]
    pub system_prompt: String,
    #[serde(default = "default_subagent_prompt")]
    pub subagent_prompt: String,
    #[serde(default = "default_strip")]
    pub strip_thinking: bool,
    #[serde(default = "default_true")]
    pub strip_duplicate_tool_results: bool,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_strip() -> bool {
    true
}

fn default_true() -> bool {
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

pub const DEFAULT_SUBAGENT_PROMPT: &str = "你是子代理，你必须总是遵循规范：
只读环境，用接主输入语言输出正文，深入反复思考，反复确认，多用工具验证事实，自主完成任务，不得猜测，正文篇幅要极简，少废话，理性，中立，严格遵循主输入要求，正文不得先说结论，正文必须先列举和分析，最后才能说结论，只允许执行只读行动，行动时想好再调用工具，不得随意，禁止执行增删改写操作
回复规范：
正文只输出必要内容和重要内容列举以及完整结果，不得主动放弃，不得输出无关内容，不得做与主输入意愿不符的事情，需顺应主输入意愿
每次思考反复确认以上要求是否做到";

fn default_subagent_prompt() -> String {
    DEFAULT_SUBAGENT_PROMPT.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            system_prompt: default_prompt(),
            subagent_prompt: default_subagent_prompt(),
            strip_thinking: default_strip(),
            strip_duplicate_tool_results: default_true(),
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
    // 工具配置跟随会话保存，旧数据缺失或损坏时回默认值
    #[serde(default)]
    pub tools: SessionTools,
}

// 提示词注入工具的配置：位置只有 before / after 两种，非法值按 before 处理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInject {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub content: String,
    #[serde(default = "default_position")]
    pub position: String,
}

fn default_position() -> String {
    "before".to_string()
}

impl Default for PromptInject {
    fn default() -> Self {
        Self {
            enabled: false,
            content: String::new(),
            position: default_position(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTools {
    #[serde(default)]
    pub prompt_inject: PromptInject,
}

impl Default for SessionTools {
    fn default() -> Self {
        Self {
            prompt_inject: PromptInject::default(),
        }
    }
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
    #[serde(default)]
    pub tool_call_id: String,
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub tool_args: String,
    #[serde(default)]
    pub children: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub existed: bool,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default = "default_true")]
    pub known: bool,
    #[serde(default)]
    pub after: String,
    #[serde(default)]
    pub created_at: i64,
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
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub elapsed_ms: u64,
    #[serde(default)]
    pub output_ms: u64,
    #[serde(default)]
    pub output_tokens: u64,
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
    #[serde(default)]
    pub changes: Vec<FileChange>,
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
            changes: Vec::new(),
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

// 会话附件目录：与 data.json 同级，独立于项目文件夹
pub fn session_dir(app: &AppHandle, session_id: &str) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    dir.join("uploads").join(session_id)
}

fn unique_name(dir: &PathBuf, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for i in 2.. {
        let candidate = if ext.is_empty() {
            format!("{stem}-{i}")
        } else {
            format!("{stem}-{i}.{ext}")
        };
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{stem}-x")
}

pub fn save_attachment(
    app: &AppHandle,
    session_id: &str,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<Attachment, String> {
    let dir = session_dir(app, session_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = unique_name(&dir, name);
    fs::write(dir.join(&file), bytes).map_err(|e| e.to_string())?;
    Ok(Attachment {
        id: new_id(),
        name: name.to_string(),
        file,
        mime: mime.to_string(),
        size: bytes.len() as u64,
        inline: false,
    })
}

pub fn fmt_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes}B")
    }
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

    if let Some(list) = value.get("changes").and_then(|v| v.as_array()) {
        let parsed: Vec<FileChange> = list
            .iter()
            .filter_map(|item| serde_json::from_value(item.clone()).ok())
            .collect();
        if !parsed.is_empty() {
            db.changes = parsed;
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
    if db.settings.subagent_prompt.is_empty() {
        db.settings.subagent_prompt = DEFAULT_SUBAGENT_PROMPT.to_string();
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
    let tmp = path.with_file_name("data.json.tmp");

    if write_json(&tmp, db) && fs::rename(&tmp, &path).is_ok() {
        return;
    }
    let _ = fs::remove_file(&tmp);

    if write_json(&path, db) {
        return;
    }

    let stamp = now_ms();
    let fallback = path.with_file_name(format!("data.force-{stamp}.json"));
    let _ = write_json(&fallback, db);
}

fn write_json(path: &PathBuf, db: &Database) -> bool {
    let Ok(file) = fs::File::create(path) else {
        return false;
    };
    let mut writer = std::io::BufWriter::new(file);
    if serde_json::to_writer_pretty(&mut writer, db).is_err() {
        return false;
    }
    writer.flush().is_ok()
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
