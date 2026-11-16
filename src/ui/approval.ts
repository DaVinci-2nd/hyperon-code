import * as api from "../api";
import { requestScroll } from "./chat";
import { el } from "./dom";

const KIND_LABEL: Record<string, string> = {
  write: "写入文件",
  edit: "编辑文件",
  bash: "执行命令",
  fetch: "获取网页",
  read: "读取",
};

const STREAM_LIMIT = 20_000;

interface ApprovalView {
  id: string;
  label: string;
  panel: HTMLElement;
  title: HTMLElement;
  state: HTMLElement;
  summary: HTMLElement;
  body: HTMLElement;
  stream: HTMLElement;
  streamText: string;
  toggle: HTMLButtonElement;
  deny: HTMLButtonElement;
  allow: HTMLButtonElement;
  stop: HTMLButtonElement;
  ready: boolean;
  stopped: boolean;
  collapsed: boolean;
}

let view: ApprovalView | null = null;
let locked: HTMLElement[] = [];
let observer: ResizeObserver | null = null;

function renderDiff(detail: string): HTMLElement {
  const box = el("div", "diff");
  if (!detail) {
    box.appendChild(el("div", "diff-empty", "无差异"));
    return box;
  }
  let firstChange: HTMLElement | null = null;
  detail.split("\n").forEach((line) => {
    const row = el("div", "diff-line");
    if (line.startsWith("+")) {
      row.classList.add("is-add");
      if (!firstChange) firstChange = row;
    } else if (line.startsWith("-")) {
      row.classList.add("is-del");
      if (!firstChange) firstChange = row;
    } else {
      row.classList.add("is-meta");
    }
    row.textContent = line.length > 0 ? line : " ";
    box.appendChild(row);
  });
  if (firstChange) {
    requestAnimationFrame(() => {
      const target = firstChange as HTMLElement;
      box.scrollTop = Math.max(0, target.offsetTop - box.clientHeight / 3);
    });
  }
  return box;
}

function setApprovalHeight(height: number): void {
  const main = document.querySelector<HTMLElement>(".main");
  if (main) main.style.setProperty("--approval-h", `${Math.round(height)}px`);
}

function blockClick(event: MouseEvent): void {
  if (view && event.target instanceof Node && view.panel.contains(event.target)) return;
  event.preventDefault();
  event.stopImmediatePropagation();
}

function lock(): void {
  if (locked.length > 0) return;
  [".topbar", ".sidebar", ".right-panel", ".composer-host"].forEach((selector) => {
    const node = document.querySelector<HTMLElement>(selector);
    if (!node) return;
    node.inert = true;
    locked.push(node);
  });
  document.addEventListener("click", blockClick, true);
}

function unlock(): void {
  locked.forEach((node) => {
    node.inert = false;
  });
  locked = [];
  document.removeEventListener("click", blockClick, true);
}

export function closeApproval(): void {
  if (observer) {
    observer.disconnect();
    observer = null;
  }
  if (view) {
    view.panel.remove();
    view = null;
  }
  unlock();
  setApprovalHeight(0);
}

function ensurePanel(id: string, kind: string, tool: string): ApprovalView {
  if (view && view.id === id) return view;
  if (view) closeApproval();

  const panel = el("div", "approval-panel");
  const head = el("div", "approval-head");
  const title = el("div", "approval-title", KIND_LABEL[kind] ?? tool);
  const state = el("div", "approval-state", "生成参数中…");
  head.appendChild(title);
  head.appendChild(state);

  const body = el("div", "approval-body");
  const summary = el("div", "approval-summary");
  const stream = el("pre", "approval-stream");
  body.appendChild(summary);
  body.appendChild(stream);

  const foot = el("div", "approval-foot");
  const toggle = el("button", "approval-toggle", "▾ 收起");
  const actions = el("div", "approval-actions");
  const stop = el("button", "btn", "停止生成");
  const deny = el("button", "btn", "拒绝");
  const allow = el("button", "btn btn-primary", "允许");
  deny.disabled = true;
  allow.disabled = true;
  actions.appendChild(stop);
  actions.appendChild(deny);
  actions.appendChild(allow);
  foot.appendChild(toggle);
  foot.appendChild(actions);

  panel.appendChild(head);
  panel.appendChild(body);
  panel.appendChild(foot);
  document.body.appendChild(panel);

  const current: ApprovalView = {
    id,
    label: KIND_LABEL[kind] ?? tool,
    panel,
    title,
    state,
    summary,
    body,
    stream,
    streamText: "",
    toggle,
    deny,
    allow,
    stop,
    ready: false,
    stopped: false,
    collapsed: false,
  };
  panel.classList.add("is-generating");

  toggle.addEventListener("click", () => {
    current.collapsed = !current.collapsed;
    current.panel.classList.toggle("is-collapsed", current.collapsed);
    current.toggle.textContent = current.collapsed ? "▸ 展开" : "▾ 收起";
    const brief = current.summary.textContent?.trim() ?? "";
    current.title.textContent = current.collapsed
      ? brief || current.label
      : current.label;
  });

  stop.addEventListener("click", () => {
    if (current.ready || current.stopped) return;
    current.stopped = true;
    current.state.textContent = "已停止";
    current.deny.disabled = true;
    current.allow.disabled = true;
    current.stop.hidden = true;
    void api.cancelMessage();
  });

  deny.addEventListener("click", () => {
    if (!current.ready) return;
    void api.approve(current.id, false);
    closeApproval();
  });

  allow.addEventListener("click", () => {
    if (!current.ready) return;
    void api.approve(current.id, true);
    closeApproval();
  });

  view = current;
  lock();

  if (observer) observer.disconnect();
  observer = new ResizeObserver(() => {
    setApprovalHeight(panel.getBoundingClientRect().height);
    requestScroll();
  });
  observer.observe(panel);

  return current;
}

export function beginApproval(event: {
  id: string;
  tool: string;
  kind: string;
}): void {
  const current = ensurePanel(event.id, event.kind, event.tool);
  if (current.ready) return;
  current.panel.classList.add("is-generating");
  current.label = KIND_LABEL[event.kind] ?? event.tool;
  if (!current.collapsed) current.title.textContent = current.label;
  current.state.textContent = "生成参数中…";
  current.deny.disabled = true;
  current.allow.disabled = true;
  current.stop.hidden = false;
}

export function streamApproval(event: {
  id: string;
  summary: string;
  append: string;
}): void {
  const current = ensurePanel(event.id, "read", "read");
  if (current.ready || current.stopped) return;
  if (event.summary) current.summary.textContent = event.summary;
  if (!event.append) return;
  current.streamText += event.append;
  if (current.streamText.length > STREAM_LIMIT) {
    current.streamText = current.streamText.slice(-STREAM_LIMIT);
    current.stream.textContent = `…（前文已折叠）\n${current.streamText}`;
  } else {
    current.stream.textContent = current.streamText;
  }
  requestAnimationFrame(() => {
    current.stream.scrollTop = current.stream.scrollHeight;
  });
}

export function openApproval(event: {
  id: string;
  tool: string;
  summary: string;
  detail: string;
  kind: string;
}): void {
  const current = ensurePanel(event.id, event.kind, event.tool);
  current.ready = true;
  current.panel.classList.remove("is-generating");
  current.label = KIND_LABEL[event.kind] ?? event.tool;
  current.summary.textContent = event.summary;
  if (current.collapsed) {
    current.title.textContent = event.summary || current.label;
  } else {
    current.title.textContent = current.label;
  }
  current.stop.hidden = true;

  if (current.stopped) {
    current.state.textContent = "已停止";
    current.deny.disabled = true;
    current.allow.disabled = true;
    return;
  }

  current.state.textContent = "等待确认";
  if (event.detail) {
    current.body.replaceChildren(current.summary, renderDiff(event.detail));
  } else {
    current.body.replaceChildren(current.summary);
  }
  current.deny.disabled = false;
  current.allow.disabled = false;
}
