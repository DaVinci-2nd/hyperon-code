import * as api from "../api";
import { state } from "../state";
import type { RollbackPlan, RollbackReport } from "../types";
import { el, openModal } from "./dom";

const ACTION_LABEL: Record<string, string> = {
  restore: "还原",
  delete: "删除",
  skip: "跳过",
  shell: "命令",
};

function notice(text: string): HTMLElement {
  const box = el("div", "rollback-notice");
  box.textContent = text;
  return box;
}

function fileList(plan: RollbackPlan): HTMLElement {
  const box = el("div", "rollback-files");
  plan.files.forEach((f) => {
    const row = el("div", "rollback-file");
    if (!f.restorable) row.classList.add("is-blocked");
    row.appendChild(el("div", "rollback-file-path", f.path));

    const meta = el("div", "rollback-file-meta");
    meta.appendChild(el("span", "rollback-tag", ACTION_LABEL[f.action] ?? f.action));
    meta.appendChild(el("span", "rollback-reason", f.reason));
    row.appendChild(meta);
    box.appendChild(row);
  });
  return box;
}

function failBox(message: string): void {
  const body = el("div", "form");
  body.appendChild(notice(message));
  openModal("回溯失败", body, [
    { label: "知道了", primary: true, onClick: (close) => close() },
  ]);
}

function showResult(report: RollbackReport): void {
  const parts: string[] = [`已回溯 ${report.removed} 条消息`];
  if (report.restored.length > 0) parts.push(`还原文件 ${report.restored.length} 个`);
  if (report.deleted.length > 0) parts.push(`删除新增文件 ${report.deleted.length} 个`);
  if (report.skipped.length > 0) parts.push(`未还原 ${report.skipped.length} 个`);
  if (report.failed.length > 0) parts.push(`失败 ${report.failed.length} 个`);

  const body = el("div", "form");
  body.appendChild(notice(parts.join("，")));

  const full = (title: string, items: string[]): void => {
    if (items.length === 0) return;
    body.appendChild(el("div", "rollback-sub", title));
    const list = el("div", "rollback-files");
    items.forEach((f) => {
      const row = el("div", "rollback-file is-blocked");
      row.appendChild(el("div", "rollback-file-path", f));
      list.appendChild(row);
    });
    body.appendChild(list);
  };

  full("保持现状的文件", report.skipped);
  full("处理失败的文件", report.failed);

  openModal("回溯完成", body, [
    { label: "知道了", primary: true, onClick: (close) => close() },
  ]);
}

async function execute(
  sessionId: string,
  messageId: string,
  onDone: () => void,
  force = false
): Promise<void> {
  try {
    const report = await api.rollbackApply(sessionId, messageId, force);
    state.messages = await api.getMessages(sessionId);
    onDone();
    window.dispatchEvent(new CustomEvent("hyperon:refresh-usage"));
    showResult(report);
  } catch (err) {
    let fresh: RollbackPlan | null = null;
    try {
      fresh = await api.rollbackPreview(sessionId, messageId);
    } catch {
      fresh = null;
    }
    if (!force && fresh && !fresh.restorable) {
      warnUnrestorable(sessionId, messageId, fresh, onDone);
      return;
    }
    failBox(`回溯失败：${String(err)}`);
  }
}

function warnUnrestorable(
  sessionId: string,
  messageId: string,
  plan: RollbackPlan,
  onDone: () => void
): void {
  const body = el("div", "form");
  body.appendChild(
    notice(
      `以下 ${plan.blocked.length} 个文件当前的改动无法还原，回溯后它们会保持现在的样子，不会恢复成模型修改前的状态。`
    )
  );
  body.appendChild(
    fileList({ ...plan, files: plan.files.filter((f) => !f.restorable) })
  );

  openModal("警告：部分文件无法还原", body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "接受并回溯",
      primary: true,
      onClick: (close) => {
        close();
        void execute(sessionId, messageId, onDone, true);
      },
    },
  ]);
}

export async function openRollback(
  messageId: string,
  onDone: () => void
): Promise<void> {
  const sessionId = state.currentSessionId;
  if (!sessionId || state.streaming) return;

  let plan: RollbackPlan | null = null;
  try {
    plan = await api.rollbackPreview(sessionId, messageId);
  } catch {
    plan = null;
  }

  const body = el("div", "form");
  body.appendChild(el("div", "rollback-title", "将回溯到这条消息之前"));

  if (!plan) {
    body.appendChild(notice("无法读取回溯信息，请稍后重试。"));
    openModal("无法回溯", body, [
      { label: "知道了", primary: true, onClick: (close) => close() },
    ]);
    return;
  }

  body.appendChild(
    notice(
      `这会删除 ${plan.messages} 条对话记录，且无法找回。文件改动会尽量还原，若无法还原会再次提示你确认。`
    )
  );

  if (plan.files.length > 0) {
    const fileChanges = plan.files.filter((f) => f.kind !== "shell");
    const shells = plan.files.filter((f) => f.kind === "shell");
    if (fileChanges.length > 0) {
      body.appendChild(
        el("div", "rollback-sub", `检测到 ${fileChanges.length} 个文件改动`)
      );
      body.appendChild(fileList({ ...plan, files: fileChanges }));
    }
    if (shells.length > 0) {
      body.appendChild(
        el(
          "div",
          "rollback-sub",
          `范围内执行过 ${shells.length} 条命令，其文件影响无法自动还原`
        )
      );
      body.appendChild(fileList({ ...plan, files: shells }));
    }
  } else {
    body.appendChild(el("div", "rollback-sub", "这段对话没有改动过文件"));
  }

  openModal("警告：回溯会删除对话", body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "继续",
      primary: true,
      onClick: async (close) => {
        close();
        let fresh: RollbackPlan;
        try {
          fresh = await api.rollbackPreview(sessionId, messageId);
        } catch (err) {
          failBox(`无法检测文件能否还原：${String(err)}`);
          return;
        }
        if (fresh.restorable) {
          void execute(sessionId, messageId, onDone);
        } else {
          warnUnrestorable(sessionId, messageId, fresh, onDone);
        }
      },
    },
  ]);
}
