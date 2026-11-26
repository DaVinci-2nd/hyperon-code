use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct ToolSpec {
    pub name: &'static str,
    pub desc: &'static str,
    pub params: &'static str,
}

pub const AGENT_TOOL: &str = "agent";

pub const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "read",
        desc: "读文件",
        params: r#"{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}"#,
    },
    ToolSpec {
        name: "write",
        desc: "写文件",
        params: r#"{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}"#,
    },
    ToolSpec {
        name: "edit",
        desc: "替换文本",
        params: r#"{"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"]}"#,
    },
    ToolSpec {
        name: "list",
        desc: "列目录",
        params: r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
    },
    ToolSpec {
        name: "find",
        desc: "按名找文件",
        params: r#"{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}"#,
    },
    ToolSpec {
        name: "grep",
        desc: "搜内容",
        params: r#"{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}"#,
    },
    ToolSpec {
        name: "bash",
        desc: "执行命令",
        params: r#"{"type":"object","properties":{"command":{"type":"string"},"cwd":{"type":"string"}},"required":["command"]}"#,
    },
    ToolSpec {
        name: "fetch",
        desc: "取网页",
        params: r#"{"type":"object","properties":{"url":{"type":"string"}},"required":["url"]}"#,
    },
    ToolSpec {
        name: "agent",
        desc: "子代理（只读）",
        params: r#"{"type":"object","properties":{"prompt":{"type":"string"}},"required":["prompt"]}"#,
    },
];

pub fn is_readonly(tool: &str) -> bool {
    matches!(tool, "read" | "list" | "find" | "grep" | "fetch")
}

pub fn needs_approval(tool: &str) -> bool {
    tool != "agent"
}

pub fn title(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .take(20)
        .collect()
}

fn tool_specs(readonly: bool) -> impl Iterator<Item = &'static ToolSpec> {
    TOOLS.iter().filter(move |t| !readonly || is_readonly(t.name))
}

pub fn openai_tools(readonly: bool) -> Value {
    let list: Vec<Value> = tool_specs(readonly)
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.desc,
                    "parameters": serde_json::from_str::<Value>(t.params).unwrap_or(json!({"type":"object","properties":{}}))
                }
            })
        })
        .collect();
    Value::Array(list)
}

pub fn anthropic_tools(readonly: bool) -> Value {
    let list: Vec<Value> = tool_specs(readonly)
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.desc,
                "input_schema": serde_json::from_str::<Value>(t.params).unwrap_or(json!({"type":"object","properties":{}}))
            })
        })
        .collect();
    Value::Array(list)
}

pub fn gemini_tools(readonly: bool) -> Value {
    let list: Vec<Value> = tool_specs(readonly)
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.desc,
                "parameters": serde_json::from_str::<Value>(t.params).unwrap_or(json!({"type":"object","properties":{}}))
            })
        })
        .collect();
    json!([{ "functionDeclarations": list }])
}

pub struct ApprovalPreview {
    pub summary: String,
    pub detail: String,
    pub kind: String,
}

pub struct FileChangeInfo {
    pub path: String,
    pub existed: bool,
    pub before: Option<String>,
    pub after: String,
    pub recorded: bool,
}

impl FileChangeInfo {
    pub fn known(&self) -> bool {
        self.recorded && (!self.existed || self.before.is_some())
    }
}

const MAX_SNAPSHOT_BYTES: u64 = 4 * 1024 * 1024;

fn snapshot(abs: &Path) -> Result<Option<String>, ()> {
    let meta = match fs::metadata(abs) {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    if !meta.is_file() || meta.len() > MAX_SNAPSHOT_BYTES {
        return Err(());
    }
    fs::read_to_string(abs).map(Some).map_err(|_| ())
}

fn resolve(root: Option<&str>, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    match root {
        Some(r) if !r.is_empty() => Path::new(r).join(p),
        _ => p.to_path_buf(),
    }
}

pub fn preview(name: &str, args: &Value, root: Option<&str>) -> ApprovalPreview {
    match name {
        "write" => {
            let path = args["path"].as_str().unwrap_or("");
            let abs = resolve(root, path);
            let old = fs::read_to_string(&abs).unwrap_or_default();
            let new = args["content"].as_str().unwrap_or("");
            ApprovalPreview {
                summary: format!("写入 {path}"),
                detail: diff_text(&old, new),
                kind: "write".to_string(),
            }
        }
        "edit" => {
            let path = args["path"].as_str().unwrap_or("");
            let abs = resolve(root, path);
            let old_file = fs::read_to_string(&abs).unwrap_or_default();
            let old = args["old"].as_str().unwrap_or("");
            let new = args["new"].as_str().unwrap_or("");
            let updated = old_file.replacen(old, new, 1);
            ApprovalPreview {
                summary: format!("编辑 {path}"),
                detail: diff_text(&old_file, &updated),
                kind: "edit".to_string(),
            }
        }
        "bash" => {
            let cmd = args["command"].as_str().unwrap_or("");
            let cwd = args["cwd"].as_str().unwrap_or(root.unwrap_or(""));
            let mut detail = cmd.to_string();
            if !cwd.is_empty() {
                detail.push_str(&format!("\ncwd: {cwd}"));
            }
            ApprovalPreview {
                summary: "执行命令".to_string(),
                detail,
                kind: "bash".to_string(),
            }
        }
        "fetch" => ApprovalPreview {
            summary: format!("取 {}", args["url"].as_str().unwrap_or("")),
            detail: String::new(),
            kind: "fetch".to_string(),
        },
        "agent" => {
            let prompt = args["prompt"].as_str().unwrap_or("");
            let brief = title(prompt);
            ApprovalPreview {
                summary: if brief.is_empty() {
                    "子代理".to_string()
                } else {
                    format!("子代理 · {brief}")
                },
                detail: prompt.to_string(),
                kind: "agent".to_string(),
            }
        }
        _ => ApprovalPreview {
            summary: format!("{name} {}", serde_json::to_string(args).unwrap_or_default()),
            detail: String::new(),
            kind: "read".to_string(),
        },
    }
}

// LCS 单元格数上限，超过时退化为整体替换式输出，避免大文件对比卡顿
const DIFF_CELL_LIMIT: usize = 1_500_000;

pub fn diff_text(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let n = a.len();
    let m = b.len();

    if n.saturating_mul(m) > DIFF_CELL_LIMIT {
        let mut out: Vec<String> = Vec::with_capacity(n + m + 1);
        out.push(format!("（对比规模超限，退化为整体替换：-{n} 行 / +{m} 行）"));
        for line in &a {
            out.push(format!("-{line}"));
        }
        for line in &b {
            out.push(format!("+{line}"));
        }
        return out.join("\n");
    }

    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut ops: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push((' ', a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            ops.push(('-', a[i]));
            i += 1;
        } else {
            ops.push(('+', b[j]));
            j += 1;
        }
    }
    while i < n {
        ops.push(('-', a[i]));
        i += 1;
    }
    while j < m {
        ops.push(('+', b[j]));
        j += 1;
    }

    let mut out: Vec<String> = Vec::new();
    for (mark, line) in ops {
        out.push(format!("{mark}{line}"));
    }
    out.join("\n")
}

fn change_detail(old: &str, new: &str) -> String {
    let raw = diff_text(old, new);
    if raw.is_empty() {
        return String::new();
    }
    raw
}

pub fn execute(name: &str, args: &Value, root: Option<&str>) -> String {
    match run(name, args, root) {
        Ok(s) => s,
        Err(e) => format!("error: {e}"),
    }
}

pub fn execute_tracked(
    name: &str,
    args: &Value,
    root: Option<&str>,
) -> (String, Option<FileChangeInfo>) {
    if name != "write" && name != "edit" {
        return (execute(name, args, root), None);
    }

    let path = args["path"].as_str().unwrap_or("");
    let abs = resolve(root, path);
    let existed = abs.exists();
    let before = snapshot(&abs);
    let recorded = before.is_ok();
    let before = before.ok().flatten();
    let output = match run(name, args, root) {
        Ok(s) => s,
        Err(e) => format!("error: {e}"),
    };
    if output.starts_with("error:") {
        return (output, None);
    }
    let after = fs::read_to_string(&abs).unwrap_or_default();
    let info = FileChangeInfo {
        path: abs.display().to_string(),
        existed,
        before,
        after,
        recorded,
    };
    (output, Some(info))
}

fn run(name: &str, args: &Value, root: Option<&str>) -> Result<String, String> {
    match name {
        "read" => {
            let path = args["path"].as_str().ok_or("缺 path")?;
            let abs = resolve(root, path);
            let text = match fs::read_to_string(&abs) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    // 二进制文件给一行提示，避免模型对着乱码反复重试
                    let size = fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
                    let ext = abs.extension().and_then(|s| s.to_str()).unwrap_or("");
                    return Err(format!(
                        "二进制文件 {} · {} · 无法直接读取",
                        ext,
                        crate::store::fmt_size(size)
                    ));
                }
                Err(e) => return Err(e.to_string()),
            };
            let has_offset = args["offset"].is_number();
            let has_limit = args["limit"].is_number();
            if !has_offset && !has_limit {
                return Ok(text);
            }
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let lines: Vec<&str> = if has_limit {
                let limit = args["limit"].as_u64().unwrap_or(0) as usize;
                text.lines().skip(offset).take(limit).collect()
            } else {
                text.lines().skip(offset).collect()
            };
            Ok(lines.join("\n"))
        }
        "write" => {
            let path = args["path"].as_str().ok_or("缺 path")?;
            let content = args["content"].as_str().unwrap_or("");
            let abs = resolve(root, path);
            let before = fs::read_to_string(&abs).unwrap_or_default();
            if let Some(parent) = abs.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&abs, content).map_err(|e| e.to_string())?;
            let diff = change_detail(&before, content);
            if diff.is_empty() {
                Ok(format!("ok {}", abs.display()))
            } else {
                Ok(format!("ok {}\n---diff---\n{}", abs.display(), diff))
            }
        }
        "edit" => {
            let path = args["path"].as_str().ok_or("缺 path")?;
            let old = args["old"].as_str().ok_or("缺 old")?;
            let new = args["new"].as_str().unwrap_or("");
            let abs = resolve(root, path);
            let text = fs::read_to_string(&abs).map_err(|e| e.to_string())?;
            if !text.contains(old) {
                return Err("未找到待替换文本".to_string());
            }
            let updated = text.replacen(old, new, 1);
            fs::write(&abs, &updated).map_err(|e| e.to_string())?;
            let diff = change_detail(&text, &updated);
            if diff.is_empty() {
                Ok(format!("ok {}", abs.display()))
            } else {
                Ok(format!("ok {}\n---diff---\n{}", abs.display(), diff))
            }
        }
        "list" => {
            let path = args["path"].as_str().unwrap_or(".");
            let abs = resolve(root, path);
            let mut out: Vec<String> = Vec::new();
            for entry in fs::read_dir(&abs).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let is_dir = entry.path().is_dir();
                let name = entry.file_name().to_string_lossy().to_string();
                out.push(if is_dir { format!("{name}/") } else { name });
            }
            out.sort();
            Ok(out.join("\n"))
        }
        "find" => {
            let pattern = args["pattern"].as_str().ok_or("缺 pattern")?;
            let base = resolve(root, args["path"].as_str().unwrap_or("."));
            let mut out: Vec<String> = Vec::new();
            walk(&base, pattern, &mut out);
            Ok(out.join("\n"))
        }
        "grep" => {
            let pattern = args["pattern"].as_str().ok_or("缺 pattern")?;
            let base = resolve(root, args["path"].as_str().unwrap_or("."));
            let mut out: Vec<String> = Vec::new();
            grep_dir(&base, pattern, &mut out);
            Ok(out.join("\n"))
        }
        "bash" => {
            let command = args["command"].as_str().ok_or("缺 command")?;
            let cwd = args["cwd"]
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| root.map(|s| s.to_string()))
                .unwrap_or_default();
            let mut cmd = Command::new("powershell");
            cmd.arg("-NoProfile").arg("-Command").arg(command);
            if !cwd.is_empty() {
                cmd.current_dir(&cwd);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            let output = cmd.output().map_err(|e| e.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut text = format!("> {command}\n");
            if !cwd.is_empty() {
                text.push_str(&format!("cwd: {cwd}\n"));
            }
            text.push_str("---output---\n");
            text.push_str(&stdout);
            if !stderr.is_empty() {
                text.push_str(&stderr);
            }
            Ok(text)
        }
        "fetch" => {
            let url = args["url"].as_str().ok_or("缺 url")?;
            ureq_get(url)
        }
        _ => Err(format!("未知工具 {name}")),
    }
}

pub fn execute_bash_streaming(
    args: &Value,
    root: Option<&str>,
    on_output: Arc<dyn Fn(String) + Send + Sync>,
) -> String {
    let command = match args["command"].as_str() {
        Some(value) => value.to_string(),
        None => return "error: 缺 command".to_string(),
    };
    let cwd = args["cwd"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| root.map(|s| s.to_string()))
        .unwrap_or_default();

    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile").arg("-Command").arg(&command);
    if !cwd.is_empty() {
        cmd.current_dir(&cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(value) => value,
        Err(error) => return format!("error: {error}"),
    };

    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || read_stream(stdout, false, tx)));
    }
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(thread::spawn(move || read_stream(stderr, true, tx)));
    }
    drop(tx);

    let mut stdout_all = String::new();
    let mut stderr_all = String::new();
    let mut pending = String::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(30)) {
            Ok((is_stderr, chunk)) => {
                if is_stderr {
                    stderr_all.push_str(&chunk);
                } else {
                    stdout_all.push_str(&chunk);
                }
                pending.push_str(&chunk);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !pending.is_empty() {
                    on_output(std::mem::take(&mut pending));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if !pending.is_empty() {
        on_output(pending);
    }
    for reader in readers {
        let _ = reader.join();
    }
    let _ = child.wait();

    let mut text = format!("> {command}\n");
    if !cwd.is_empty() {
        text.push_str(&format!("cwd: {cwd}\n"));
    }
    text.push_str("---output---\n");
    text.push_str(&stdout_all);
    if !stderr_all.is_empty() {
        text.push_str(&stderr_all);
    }
    text
}

fn read_stream<R: std::io::Read>(
    stream: R,
    is_stderr: bool,
    tx: mpsc::Sender<(bool, String)>,
) {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf).to_string();
                if tx.send((is_stderr, text)).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn ureq_get(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    resp.text().map_err(|e| e.to_string())
}

fn walk(dir: &Path, pattern: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if glob_match(pattern, &name) {
            out.push(path.display().to_string());
        }
        if path.is_dir() {
            walk(&path, pattern, out);
        }
    }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{suffix}"));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    name == pattern
}

fn grep_dir(dir: &Path, pattern: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            grep_dir(&path, pattern, out);
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            if line.contains(pattern) {
                out.push(format!("{}:{}:{}", path.display(), idx + 1, line.trim()));
            }
        }
    }
}
