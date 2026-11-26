import * as api from "../api";
import { state, currentSession } from "../state";
import type { ChangeSummary, SessionTools } from "../types";
import { buildDiffElement } from "./chat";
import { el, option } from "./dom";

let paneHost: HTMLElement | null = null;

// 会话工具配置缺失或字段非法时回默认值，旧数据损坏不影响界面
function sessionTools(): SessionTools {
  const inject = currentSession()?.tools?.prompt_inject;
  return {
    prompt_inject: {
      enabled: inject?.enabled === true,
      content: typeof inject?.content === "string" ? inject.content : "",
      position: inject?.position === "after" ? "after" : "before",
    },
  };
}

async function saveTools(tools: SessionTools): Promise<void> {
  const session = currentSession();
  if (!session) return;
  session.tools = tools;
  try {
    await api.updateSessionTools(session.id, tools);
  } catch {
    /* 保存失败不打断界面，下次改动会再尝试 */
  }
}

function injectCard(): HTMLElement {
  const card = el("div", "tool-card");

  const head = el("div", "tool-card-head");
  const text = el("div", "tool-card-text");
  text.appendChild(el("div", "tool-card-name", "提示词注入"));
  text.appendChild(
    el("div", "tool-card-desc", "每次发送消息时，在消息前或后固定追加一段提示词")
  );
  head.appendChild(text);

  const toggle = el("button", "switch");
  const { enabled, content, position } = sessionTools().prompt_inject;
  if (enabled) toggle.classList.add("is-on");
  head.appendChild(toggle);
  card.appendChild(head);

  const body = el("div", "tool-card-body");
  body.hidden = !enabled;

  const area = el("textarea", "tool-area");
  area.placeholder = "要固定追加的提示词";
  area.rows = 5;
  area.value = content;

  const hint = el("div", "tool-field-hint", "内容为空时不会注入");
  hint.hidden = area.value.trim().length > 0;

  const select = el("select", "tool-select");
  select.appendChild(option("消息前", "before"));
  select.appendChild(option("消息后", "after"));
  select.value = position;

  body.appendChild(el("div", "tool-field-label", "注入内容"));
  body.appendChild(area);
  body.appendChild(hint);
  body.appendChild(el("div", "tool-field-label", "注入位置"));
  body.appendChild(select);
  card.appendChild(body);

  const read = (): SessionTools => ({
    prompt_inject: {
      enabled: toggle.classList.contains("is-on"),
      content: area.value,
      position: select.value === "after" ? "after" : "before",
    },
  });

  toggle.addEventListener("click", () => {
    toggle.classList.toggle("is-on");
    body.hidden = !toggle.classList.contains("is-on");
    void saveTools(read());
  });
  area.addEventListener("input", () => {
    hint.hidden = area.value.trim().length > 0;
  });
  // 逐键保存太频繁，失焦时落盘
  area.addEventListener("blur", () => void saveTools(read()));
  select.addEventListener("change", () => void saveTools(read()));

  return card;
}

function buildToolsPane(): HTMLElement {
  const pane = el("div", "right-pane");

  if (!currentSession()) {
    pane.appendChild(el("div", "empty-hint center", "先选择会话"));
    return pane;
  }

  // 工具列表，每项一张卡，便于后续扩展更多工具
  pane.appendChild(injectCard());
  return pane;
}

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function fmtChangeTime(ms: number): string {
  const d = new Date(ms);
  return `${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

function changeItem(c: ChangeSummary, sessionId: string): HTMLDetailsElement {
  const details = document.createElement("details");
  details.className = "fold-block change-item";

  const head = document.createElement("summary");
  head.className = "fold-head change-head";
  head.appendChild(el("span", c.existed ? "change-tag" : "change-tag is-new", c.existed ? "修改" : "新建"));
  const path = el("span", "change-path", c.path);
  path.title = c.path;
  head.appendChild(path);
  head.appendChild(el("span", "change-time", fmtChangeTime(c.created_at)));
  details.appendChild(head);

  const body = el("div", "fold-body");
  body.appendChild(el("div", "change-hint", "加载中…"));
  details.appendChild(body);

  // 展开时才拉取对比内容，列表保持轻量
  let loaded = false;
  details.addEventListener("toggle", () => {
    if (!details.open || loaded) return;
    loaded = true;
    void api
      .getChangeDiff(sessionId, c.id)
      .then((d) => {
        body.replaceChildren();
        if (!d.known) {
          body.appendChild(el("div", "change-hint", "修改前的内容未记录，无法对比"));
          return;
        }
        if (!d.diff) {
          body.appendChild(el("div", "change-hint", "内容没有变化"));
          return;
        }
        body.appendChild(buildDiffElement(d.diff));
      })
      .catch(() => {
        body.replaceChildren();
        body.appendChild(el("div", "change-hint", "加载失败，重新展开可重试"));
        loaded = false;
      });
  });
  return details;
}

async function buildChangesPane(): Promise<HTMLElement> {
  const pane = el("div", "right-pane");
  const sessionId = state.currentSessionId;
  if (!sessionId) {
    pane.appendChild(el("div", "empty-hint center", "先选择会话"));
    return pane;
  }

  let list: ChangeSummary[];
  try {
    list = await api.getChanges(sessionId);
  } catch {
    pane.appendChild(el("div", "empty-hint center", "变更记录加载失败"));
    return pane;
  }

  if (list.length === 0) {
    pane.appendChild(el("div", "empty-hint center", "本会话暂无文件变更"));
    return pane;
  }

  // 最新改动排最前
  list.sort((a, b) => b.created_at - a.created_at);
  list.forEach((c) => pane.appendChild(changeItem(c, sessionId)));
  return pane;
}

function showPane(key: string): void {
  if (!paneHost) return;
  if (key === "changes") {
    paneHost.replaceChildren(el("div", "empty-hint center", "加载中…"));
    void buildChangesPane().then((pane) => {
      // 异步返回时可能已切页或切会话，确认仍停留在此页才落地
      if (paneHost && state.rightTab === "changes") paneHost.replaceChildren(pane);
    });
  } else {
    paneHost.replaceChildren(buildToolsPane());
  }
}

export function buildRightPanel(): HTMLElement {
  const panel = el("div", "right-panel");

  const tabs = el("div", "right-tabs");
  [
    { key: "tools", label: "工具" },
    { key: "changes", label: "变更" },
  ].forEach((t) => {
    const btn = el("button", "right-tab", t.label);
    btn.dataset.key = t.key;
    if (state.rightTab === t.key) btn.classList.add("is-active");
    btn.addEventListener("click", () => {
      if (state.rightTab === t.key) return;
      state.rightTab = t.key;
      tabs.querySelectorAll(".right-tab").forEach((n) => {
        (n as HTMLElement).classList.toggle(
          "is-active",
          (n as HTMLElement).dataset.key === t.key
        );
      });
      showPane(t.key);
    });
    tabs.appendChild(btn);
  });
  panel.appendChild(tabs);

  paneHost = el("div", "right-body");
  panel.appendChild(paneHost);
  showPane(state.rightTab);

  return panel;
}

export function refreshRightPanel(): void {
  if (!paneHost || !paneHost.isConnected) return;
  showPane(state.rightTab);
}

// 发送完成后可能有新的变更记录，变更页可见时刷新
window.addEventListener("hyperon:changes-updated", () => {
  if (state.rightTab === "changes") refreshRightPanel();
});
