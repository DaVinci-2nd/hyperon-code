use crate::store::Database;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct RollbackFile {
    pub kind: String,
    pub path: String,
    pub action: String,
    pub restorable: bool,
    pub reason: String,
}

#[derive(Serialize)]
pub struct RollbackPlan {
    pub messages: usize,
    pub files: Vec<RollbackFile>,
    pub blocked: Vec<String>,
    pub restorable: bool,
}

#[derive(Serialize)]
pub struct RollbackReport {
    pub removed: usize,
    pub restored: Vec<String>,
    pub deleted: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<String>,
}

struct Agg {
    path: String,
    existed: bool,
    before: Option<String>,
    after: String,
    known: bool,
}

fn order_of(db: &Database, session_id: &str, message_id: &str) -> Option<Vec<String>> {
    let ids: Vec<String> = db
        .messages
        .iter()
        .filter(|m| m.session_id == session_id)
        .map(|m| m.id.clone())
        .collect();
    let idx = ids.iter().position(|id| id == message_id)?;
    Some(ids[idx..].to_vec())
}

fn aggregate(db: &Database, session_id: &str, removed: &[String]) -> Vec<Agg> {
    let set: HashSet<&str> = removed.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<Agg> = Vec::new();
    for c in db
        .changes
        .iter()
        .filter(|c| c.session_id == session_id && set.contains(c.message_id.as_str()))
    {
        if let Some(slot) = out.iter_mut().find(|a| a.path == c.path) {
            slot.after = c.after.clone();
            slot.known = slot.known && c.known;
            if slot.before.is_none() {
                slot.before = c.before.clone();
            }
            continue;
        }
        out.push(Agg {
            path: c.path.clone(),
            existed: c.existed,
            before: c.before.clone(),
            after: c.after.clone(),
            known: c.known,
        });
    }
    out
}

fn restorable_content(agg: &Agg) -> bool {
    !agg.existed || (agg.known && agg.before.is_some())
}

fn judge(agg: &Agg) -> (String, bool, String) {
    if !restorable_content(agg) {
        return (
            "skip".to_string(),
            false,
            "修改前的文件内容未能记录".to_string(),
        );
    }
    let current = fs::read_to_string(&agg.path).ok();

    if !agg.existed {
        return match current {
            None => ("skip".to_string(), true, "文件已不存在".to_string()),
            Some(text) if text == agg.after => (
                "delete".to_string(),
                true,
                "删除模型新建的文件".to_string(),
            ),
            Some(_) => (
                "skip".to_string(),
                false,
                "文件在模型写入后又被修改".to_string(),
            ),
        };
    }

    match current {
        Some(text) if text == agg.after => (
            "restore".to_string(),
            true,
            "恢复为修改前的内容".to_string(),
        ),
        Some(_) => (
            "skip".to_string(),
            false,
            "文件在模型修改后又被改动".to_string(),
        ),
        None => (
            "restore".to_string(),
            true,
            "文件已丢失，将重新写入修改前的内容".to_string(),
        ),
    }
}

fn shell_calls(db: &Database, session_id: &str, removed: &[String]) -> Vec<String> {
    let set: HashSet<&str> = removed.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<String> = Vec::new();
    for m in db
        .messages
        .iter()
        .filter(|m| m.session_id == session_id && set.contains(m.id.as_str()))
    {
        for p in &m.parts {
            if p.kind != "tool" || p.tool_name != "bash" || !p.ok {
                continue;
            }
            let summary = if p.summary.is_empty() {
                "执行命令".to_string()
            } else {
                p.summary.clone()
            };
            if !out.contains(&summary) {
                out.push(summary);
            }
        }
    }
    out
}

pub fn preview(db: &Database, session_id: &str, message_id: &str) -> Option<RollbackPlan> {
    let removed = order_of(db, session_id, message_id)?;
    let aggs = aggregate(db, session_id, &removed);

    let mut files: Vec<RollbackFile> = Vec::new();
    for agg in &aggs {
        let (action, restorable, reason) = judge(agg);
        if action == "skip" && restorable {
            continue;
        }
        files.push(RollbackFile {
            kind: "file".to_string(),
            path: agg.path.clone(),
            action,
            restorable,
            reason,
        });
    }

    for call in shell_calls(db, session_id, &removed) {
        files.push(RollbackFile {
            kind: "shell".to_string(),
            path: call,
            action: "shell".to_string(),
            restorable: false,
            reason: "命令执行对文件的影响无法自动还原".to_string(),
        });
    }

    let blocked: Vec<String> = files
        .iter()
        .filter(|f| !f.restorable)
        .map(|f| f.path.clone())
        .collect();

    Some(RollbackPlan {
        messages: removed.len(),
        restorable: blocked.is_empty(),
        blocked,
        files,
    })
}

pub fn apply(
    db: &mut Database,
    session_id: &str,
    message_id: &str,
) -> Result<RollbackReport, String> {
    let removed = order_of(db, session_id, message_id).ok_or("消息不存在")?;
    let aggs = aggregate(db, session_id, &removed);

    let mut report = RollbackReport {
        removed: removed.len(),
        restored: Vec::new(),
        deleted: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };

    for agg in aggs.iter().rev() {
        let (action, restorable, reason) = judge(agg);
        if !restorable {
            report.skipped.push(format!("{}（{}）", agg.path, reason));
            continue;
        }
        if action == "delete" {
            match fs::remove_file(&agg.path) {
                Ok(_) => report.deleted.push(agg.path.clone()),
                Err(e) => report.failed.push(format!("{}（{e}）", agg.path)),
            }
            continue;
        }
        if action == "restore" {
            let Some(before) = &agg.before else {
                report.failed.push(format!("{}（缺少原始内容）", agg.path));
                continue;
            };
            let target = Path::new(&agg.path);
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::write(target, before) {
                Ok(_) => report.restored.push(agg.path.clone()),
                Err(e) => report.failed.push(format!("{}（{e}）", agg.path)),
            }
        }
    }

    let set: HashSet<&str> = removed.iter().map(|s| s.as_str()).collect();
    db.changes
        .retain(|c| !(c.session_id == session_id && set.contains(c.message_id.as_str())));
    db.messages
        .retain(|m| !(m.session_id == session_id && set.contains(m.id.as_str())));
    if let Some(s) = db.sessions.iter_mut().find(|s| s.id == session_id) {
        s.last_prompt_tokens = 0;
        s.last_cached_tokens = 0;
    }

    Ok(report)
}
