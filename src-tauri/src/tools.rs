use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ToolSpec {
    pub name: &'static str,
    pub desc: &'static str,
    pub params: &'static str,
}

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
];

pub fn needs_approval(_tool: &str) -> bool {
    true
}

pub fn openai_tools() -> Value {
    let list: Vec<Value> = TOOLS
        .iter()
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

pub fn anthropic_tools() -> Value {
    let list: Vec<Value> = TOOLS
        .iter()
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

pub fn gemini_tools() -> Value {
    let list: Vec<Value> = TOOLS
        .iter()
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
            ApprovalPreview {
                summary: format!("执行 {cmd}"),
                detail: if cwd.is_empty() {
                    String::new()
                } else {
                    format!("cwd: {cwd}")
                },
                kind: "bash".to_string(),
            }
        }
        "fetch" => ApprovalPreview {
            summary: format!("取 {}", args["url"].as_str().unwrap_or("")),
            detail: String::new(),
            kind: "fetch".to_string(),
        },
        _ => ApprovalPreview {
            summary: format!("{name} {}", serde_json::to_string(args).unwrap_or_default()),
            detail: String::new(),
            kind: "read".to_string(),
        },
    }
}

fn diff_text(old: &str, new: &str) -> String {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    let mut shown = 0usize;

    while i < old_lines.len() || j < new_lines.len() {
        if shown >= 400 {
            out.push("… 差异过长，已截断".to_string());
            break;
        }
        if i < old_lines.len() && j < new_lines.len() && old_lines[i] == new_lines[j] {
            i += 1;
            j += 1;
            continue;
        }
        if i < old_lines.len() {
            out.push(format!("- {}", old_lines[i]));
            i += 1;
            shown += 1;
            continue;
        }
        out.push(format!("+ {}", new_lines[j]));
        j += 1;
        shown += 1;
    }
    out.join("\n")
}

pub fn execute(name: &str, args: &Value, root: Option<&str>) -> String {
    match run(name, args, root) {
        Ok(s) => s,
        Err(e) => format!("error: {e}"),
    }
}

fn run(name: &str, args: &Value, root: Option<&str>) -> Result<String, String> {
    match name {
        "read" => {
            let path = args["path"].as_str().ok_or("缺 path")?;
            let abs = resolve(root, path);
            let text = fs::read_to_string(&abs).map_err(|e| e.to_string())?;
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(2000) as usize;
            let lines: Vec<&str> = text.lines().skip(offset).take(limit).collect();
            Ok(lines.join("\n"))
        }
        "write" => {
            let path = args["path"].as_str().ok_or("缺 path")?;
            let content = args["content"].as_str().unwrap_or("");
            let abs = resolve(root, path);
            if let Some(parent) = abs.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&abs, content).map_err(|e| e.to_string())?;
            Ok(format!("ok {}", abs.display()))
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
            fs::write(&abs, text.replacen(old, new, 1)).map_err(|e| e.to_string())?;
            Ok(format!("ok {}", abs.display()))
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
            walk(&base, pattern, 0, &mut out);
            Ok(out.join("\n"))
        }
        "grep" => {
            let pattern = args["pattern"].as_str().ok_or("缺 pattern")?;
            let base = resolve(root, args["path"].as_str().unwrap_or("."));
            let mut out: Vec<String> = Vec::new();
            grep_dir(&base, pattern, &mut out);
            let total = out.len();
            out.truncate(200);
            if total > 200 {
                out.push(format!("… 共 {total} 条，已截断"));
            }
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
                cmd.current_dir(cwd);
            }
            let output = cmd.output().map_err(|e| e.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut text = stdout;
            if !stderr.is_empty() {
                text.push_str(&stderr);
            }
            if text.len() > 20000 {
                text.truncate(20000);
                text.push_str("\n… 输出过长，已截断");
            }
            Ok(text)
        }
        "fetch" => {
            let url = args["url"].as_str().ok_or("缺 url")?;
            let body = ureq_get(url)?;
            let trimmed: String = body.chars().take(20000).collect();
            Ok(trimmed)
        }
        _ => Err(format!("未知工具 {name}")),
    }
}

fn ureq_get(url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    resp.text().map_err(|e| e.to_string())
}

fn walk(dir: &Path, pattern: &str, depth: usize, out: &mut Vec<String>) {
    if depth > 6 || out.len() > 400 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        if glob_match(pattern, &name) {
            out.push(path.display().to_string());
        }
        if path.is_dir() {
            walk(&path, pattern, depth + 1, out);
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
    if out.len() > 400 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
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
                if out.len() > 400 {
                    return;
                }
            }
        }
    }
}
