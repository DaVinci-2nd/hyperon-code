import * as api from "../api";
import { renderMarkdown } from "../markdown";
import { state } from "../state";
import type { Attachment, Message, Part } from "../types";
import { fmtSize } from "./attach";
import { copyText } from "./copy";
import { el } from "./dom";
import { openRollback } from "./rollback";
import { renderStreamingText } from "./stream";

const INITIAL_LIMIT = 30;

let loadedStart = -1;
let pageSession: string | null = null;

// 历史消息里的附件从会话文件夹读回字节渲染
function attachmentNode(a: Attachment): HTMLElement {
  if (a.mime.startsWith("image/")) {
    const img = el("img", "attach-thumb");
    img.alt = a.name;
    const sid = state.currentSessionId;
    if (sid) {
      void api.readAttachment(sid, a.file).then((buf) => {
        const url = URL.createObjectURL(new Blob([buf], { type: a.mime }));
        img.src = url;
        img.onload = () => URL.revokeObjectURL(url);
      });
    }
    return img;
  }
  const chip = el("div", "attach-file", `${a.name} · ${fmtSize(a.size)}`);
  if (a.inline) {
    chip.classList.add("is-inline");
    chip.title = "已内联发送给模型";
  }
  return chip;
}

function foldBlock(
  label: string,
  body: HTMLElement,
  extraClass = ""
): HTMLDetailsElement {
  const details = document.createElement("details");
  details.className = `fold-block ${extraClass}`.trim();
  details.open = true;
  const summary = document.createElement("summary");
  summary.className = "fold-head";
  summary.textContent = label;
  details.appendChild(summary);
  const inner = el("div", "fold-body");
  inner.appendChild(body);
  details.appendChild(inner);
  return details;
}

function chunkize(body: HTMLElement, maxChars = 8000): void {
  const nodes = Array.from(body.childNodes);
  if (nodes.length === 0) return;
  const groups: HTMLElement[] = [];
  let current = el("div", "fold-chunk");
  let size = 0;
  nodes.forEach((node) => {
    const text = node.textContent?.length ?? 0;
    if (size > 0 && size + text > maxChars) {
      groups.push(current);
      current = el("div", "fold-chunk");
      size = 0;
    }
    current.appendChild(node);
    size += text;
  });
  if (current.childNodes.length > 0) groups.push(current);
  body.replaceChildren(...groups);
}

function textChunks(text: string, className: string): HTMLElement[] {
  const lines = text.split("\n");
  const out: HTMLElement[] = [];
  for (let i = 0; i < lines.length; i += 200) {
    const pre = el("pre", `${className} fold-chunk`);
    pre.textContent = lines.slice(i, i + 200).join("\n");
    out.push(pre);
  }
  return out;
}

function thinkPart(text: string): HTMLElement {
  const body = el("div", "think-body");
  body.innerHTML = renderMarkdown(text);
  chunkize(body);
  return foldBlock("思考过程", body, "think-block");
}

export function buildDiffElement(diffText: string): HTMLElement {
  const diff = el("div", "diff");
  let firstChange: HTMLElement | null = null;
  let chunk = el("div", "fold-chunk");
  diff.appendChild(chunk);
  let count = 0;
  diffText.split("\n").forEach((line) => {
    if (count > 0 && count % 200 === 0) {
      chunk = el("div", "fold-chunk");
      diff.appendChild(chunk);
    }
    count++;
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
    chunk.appendChild(row);
  });
  if (firstChange) {
    requestAnimationFrame(() => {
      const target = firstChange as HTMLElement;
      diff.scrollTop = Math.max(0, target.offsetTop - diff.clientHeight / 3);
    });
  }
  return diff;
}

function subagentHead(part: Part): string {
  const mark = part.running ? "…" : part.ok ? "✓" : "✕";
  return `${mark} ${part.summary || "子代理"}`;
}

function fillSubagentFlow(flow: HTMLElement, part: Part): void {
  flow.replaceChildren();
  const children = part.children ?? [];
  children.forEach((child) => {
    if (child.kind === "think") {
      const body = el("div", "subagent-think");
      body.innerHTML = renderMarkdown(child.text || "…");
      flow.appendChild(body);
      return;
    }
    if (child.kind === "tool") {
      const wrap = el("div", "subagent-tool");
      wrap.appendChild(
        el(
          "div",
          "subagent-tool-line",
          `${child.ok ? "✓" : "✕"} ${child.summary}`
        )
      );
      if (child.detail) {
        const pre = el("pre", "subagent-tool-output");
        pre.textContent = child.detail;
        wrap.appendChild(pre);
      }
      flow.appendChild(wrap);
      return;
    }
    const body = el("div", "subagent-text");
    body.innerHTML = renderMarkdown(child.text || "…");
    flow.appendChild(body);
  });

  if (children.length === 0) {
    flow.appendChild(
      el("div", "empty-hint", part.running ? "运行中…" : "无输出")
    );
  }

  const box = flow.closest(".fold-body") as HTMLElement | null;
  if (box) {
    requestAnimationFrame(() => {
      box.scrollTop = box.scrollHeight;
    });
  }
}

function subagentPart(part: Part): HTMLElement {
  const flow = el("div", "subagent-flow");
  fillSubagentFlow(flow, part);
  return foldBlock(subagentHead(part), flow, "subagent-block");
}

function toolPart(part: Part): HTMLElement {
  if (part.tool_name === "agent") return subagentPart(part);
  const body = el("div", "tool-body");
  const MARKER = "---diff---";

  if (part.running) {
    const pre = el("pre", "tool-output");
    pre.textContent = part.detail;
    body.appendChild(pre);
  } else if (part.detail.includes(MARKER)) {
    const at = part.detail.indexOf(MARKER);
    const head = part.detail.slice(0, at).replace(/\n$/, "");
    const diffText = part.detail.slice(at + MARKER.length).replace(/^\n/, "");
    if (head) {
      const pre = el("pre", "tool-output");
      pre.textContent = head;
      body.appendChild(pre);
    }
    body.appendChild(buildDiffElement(diffText));
  } else if (part.detail) {
    const lines = part.detail.split("\n");
    const isEdit = part.summary.startsWith("编辑 ") || part.summary.startsWith("写入 ");
    const rest = lines.slice(1);
    const looksLikeDiff =
      isEdit &&
      rest.length > 0 &&
      rest.every((l) => l.startsWith(" ") || l.startsWith("-") || l.startsWith("+"));
    if (looksLikeDiff) {
      const head = lines[0].replace(/^ok /, "");
      const pre = el("pre", "tool-output");
      pre.textContent = head;
      body.appendChild(pre);
      body.appendChild(buildDiffElement(rest.join("\n")));
    } else {
      textChunks(part.detail, "tool-output").forEach((pre) => {
        body.appendChild(pre);
      });
    }
  } else {
    body.appendChild(el("div", "empty-hint", "无输出"));
  }

  const mark = part.running ? "…" : part.ok ? "✓" : "✕";
  return foldBlock(`${mark} ${part.summary}`, body, "tool-block");
}

function textPart(text: string): HTMLElement {
  const body = el("div", "msg-body");
  body.innerHTML = renderMarkdown(text);
  return body;
}

function partNode(p: Part): HTMLElement {
  if (p.kind === "think") return thinkPart(p.text);
  if (p.kind === "tool") return toolPart(p);
  return textPart(p.text);
}

interface MessageLike {
  id?: string;
  content: string;
  reasoning?: string;
  parts?: Part[];
  attachments?: Attachment[];
  model?: string;
  created_at?: number;
  elapsed_ms?: number;
  output_ms?: number;
  output_tokens?: number;
}

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function fmtTime(ms?: number): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(
    d.getHours()
  )}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

function fmtDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60_000);
  const s = Math.round((ms % 60_000) / 1000);
  return `${m}m ${pad2(s)}s`;
}

function fmtStats(message: MessageLike): string {
  const out: string[] = [];
  if (message.elapsed_ms && message.elapsed_ms > 0) {
    out.push(`用时 ${fmtDuration(message.elapsed_ms)}`);
  }
  if (
    message.output_ms &&
    message.output_ms > 0 &&
    message.output_tokens &&
    message.output_tokens > 0
  ) {
    const speed = message.output_tokens / (message.output_ms / 1000);
    out.push(`${speed.toFixed(1)} token/s`);
  }
  return out.join(" · ");
}

function buildMeta(
  role: string,
  message: MessageLike | undefined,
  streaming: boolean
): HTMLElement {
  const meta = el("div", "msg-meta");
  meta.appendChild(el("span", "msg-time", fmtTime(message?.created_at)));
  if (role === "assistant" && !streaming) {
    const stats = fmtStats(message ?? { content: "" });
    if (stats) meta.appendChild(el("span", "msg-speed", stats));
  }
  if (!streaming) {
    const copy = el("button", "msg-copy", "复制");
    copy.title = "复制正文";
    meta.appendChild(copy);
  }
  return meta;
}

function rerenderMessages(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  const next = buildMessages();
  box.replaceWith(next);
  window.dispatchEvent(new CustomEvent("hyperon:messages-changed"));
  requestAnimationFrame(() => {
    next.scrollTop = next.scrollHeight;
    bindFollow();
  });
}

function rollbackButton(messageId: string): HTMLElement {
  const btn = el("button", "rollback-btn", "回溯");
  btn.title = "回溯到这条消息之前，并还原文件改动";
  if (state.streaming) {
    btn.classList.add("is-disabled");
    btn.disabled = true;
    btn.title = "正在生成回复，暂不能回溯";
  }
  btn.addEventListener("click", () => {
    if (state.streaming) return;
    void openRollback(messageId, rerenderMessages);
  });
  return btn;
}

export function refreshRollbackButtons(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  box.querySelectorAll(".rollback-btn").forEach((node) => {
    const btn = node as HTMLButtonElement;
    btn.classList.toggle("is-disabled", state.streaming);
    btn.disabled = state.streaming;
    btn.title = state.streaming
      ? "正在生成回复，暂不能回溯"
      : "回溯到这条消息之前，并还原文件改动";
  });
}

function messageRow(
  role: string,
  message?: MessageLike,
  streaming = false
): HTMLElement {
  const row = el("div", `msg msg-${role}`);
  const id = message?.id ?? "";
  if (id) row.dataset.messageId = id;

  if (role === "user") {
    const wrap = el("div", "msg-col");
    const holder = el("div", "msg-bubble");
    const body = el("div", "msg-body");
    body.innerHTML = renderMarkdown(message?.content ?? "");
    holder.appendChild(body);
    (message?.attachments ?? []).forEach((a) => holder.appendChild(attachmentNode(a)));
    wrap.appendChild(holder);

    const meta = buildMeta(role, message, streaming);
    if (id && !id.startsWith("pending")) {
      meta.appendChild(rollbackButton(id));
    }
    wrap.appendChild(meta);
    row.appendChild(wrap);
    return row;
  }

  const holder = el("div", "msg-plain");
  const head = el("div", "msg-head");
  head.appendChild(el("span", "msg-model", message?.model || "AI"));
  holder.appendChild(head);

  const partsEl = el("div", "msg-parts");
  holder.appendChild(partsEl);
  if (!streaming) {
    const parts = message?.parts ?? [];
    if (parts.length === 0) {
      const fallback: Part[] = [];
      if (message?.reasoning) {
        fallback.push({
          kind: "think",
          text: message.reasoning,
          summary: "",
          detail: "",
          ok: true,
        });
      }
      fallback.push({
        kind: "text",
        text: message?.content ?? "",
        summary: "",
        detail: "",
        ok: true,
      });
      fallback.forEach((p) => partsEl.appendChild(partNode(p)));
    } else {
      parts.forEach((p) => partsEl.appendChild(partNode(p)));
    }
  }

  holder.appendChild(buildMeta(role, message, streaming));
  row.appendChild(holder);
  return row;
}

function messageCopyText(id: string): string {
  const message = state.messages.find((m) => m.id === id);
  if (!message) return "";
  if (message.role === "user") return message.content;
  const text = (message.parts ?? [])
    .filter((p) => p.kind === "text")
    .map((p) => p.text)
    .join("\n\n");
  return text || message.content;
}

function withFeedback(button: HTMLElement, text: string): void {
  const prev = button.textContent ?? "";
  button.textContent = text;
  window.setTimeout(() => {
    if (button.isConnected) button.textContent = prev;
  }, 1200);
}

function bindMessageActions(box: HTMLElement): void {
  box.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const codeBtn = target.closest(".code-copy") as HTMLElement | null;
    if (codeBtn) {
      const block = codeBtn.closest(".code-block");
      const code = block?.querySelector("code");
      if (!code) return;
      void copyText(code.textContent ?? "").then((ok) => {
        if (ok) withFeedback(codeBtn, "已复制");
      });
      return;
    }
    const msgBtn = target.closest(".msg-copy") as HTMLElement | null;
    if (!msgBtn) return;
    const row = msgBtn.closest(".msg") as HTMLElement | null;
    const id = row?.dataset.messageId ?? "";
    let text = messageCopyText(id);
    if (!text && row) {
      const body = row.querySelector(".msg-body, .msg-parts");
      text = body?.textContent ?? "";
    }
    if (!text) return;
    void copyText(text).then((ok) => {
      if (ok) withFeedback(msgBtn, "已复制");
    });
  });
}

function updateLoadMore(more: HTMLButtonElement): void {
  if (loadedStart <= 0) {
    more.remove();
    return;
  }
  more.textContent = `加载更早的 ${Math.min(INITIAL_LIMIT, loadedStart)} 条`;
}

function prependPage(box: HTMLElement, more: HTMLButtonElement | null): void {
  const from = Math.max(0, loadedStart - INITIAL_LIMIT);
  if (from >= loadedStart) return;
  const frag = document.createDocumentFragment();
  for (let i = from; i < loadedStart; i++) {
    const m = state.messages[i];
    frag.appendChild(messageRow(m.role, m));
  }
  if (more && more.parentElement === box) {
    box.insertBefore(frag, more.nextSibling);
  } else {
    box.insertBefore(frag, box.firstChild);
  }
  loadedStart = from;
  if (more) updateLoadMore(more);
}

export function buildMessages(): HTMLElement {
  const box = el("div", "messages");
  box.id = "messages";
  bindMessageActions(box);

  if (!state.currentSessionId) {
    box.appendChild(el("div", "empty-hint center", "选择或新建一个会话"));
    return box;
  }

  if (state.messages.length === 0) {
    box.appendChild(el("div", "empty-hint center", "发一条消息开始"));
    return box;
  }

  const total = state.messages.length;
  if (pageSession !== state.currentSessionId || loadedStart < 0 || loadedStart > total) {
    pageSession = state.currentSessionId;
    loadedStart = Math.max(0, total - INITIAL_LIMIT);
  }

  if (loadedStart > 0) {
    const more = el("button", "load-more");
    updateLoadMore(more);
    more.addEventListener("click", () => {
      const prevHeight = box.scrollHeight;
      prependPage(box, more);
      box.scrollTop += box.scrollHeight - prevHeight;
    });
    box.appendChild(more);
  }

  for (let i = loadedStart; i < total; i++) {
    const m = state.messages[i];
    box.appendChild(messageRow(m.role, m));
  }
  return box;
}

export function ensureLoaded(messageId: string): boolean {
  const index = state.messages.findIndex((m) => m.id === messageId);
  if (index < 0) return false;
  const box = document.getElementById("messages");
  if (!box) return false;
  if (
    pageSession !== state.currentSessionId ||
    loadedStart < 0 ||
    loadedStart > state.messages.length
  ) {
    pageSession = state.currentSessionId;
    loadedStart = Math.max(0, state.messages.length - INITIAL_LIMIT);
  }
  while (loadedStart > 0 && index < loadedStart) {
    const more = box.querySelector(".load-more") as HTMLButtonElement | null;
    prependPage(box, more);
  }
  return true;
}

export function scrollToMessage(messageId: string): void {
  const box = document.getElementById("messages");
  if (!box) return;
  const row = box.querySelector(
    `[data-message-id="${CSS.escape(messageId)}"]`
  ) as HTMLElement | null;
  if (!row) return;
  row.style.contentVisibility = "visible";
  const place = (): void => {
    const top =
      row.getBoundingClientRect().top -
      box.getBoundingClientRect().top +
      box.scrollTop -
      12;
    box.scrollTop = Math.max(0, top);
  };
  place();
  requestAnimationFrame(() => {
    place();
    row.style.contentVisibility = "";
  });
  followTail = false;
  syncJumpButton(false);
}

function modelLabel(): string {
  const provider = state.providers.find((p) => p.id === state.currentProviderId);
  if (!provider) return "AI";
  if (provider.name && provider.model) return `${provider.name} · ${provider.model}`;
  return provider.name || provider.model || "AI";
}

export function appendUserRow(message: Message): HTMLElement | null {
  const box = document.getElementById("messages");
  if (!box) return null;
  box.querySelector(".empty-hint")?.remove();
  const row = messageRow("user", message);
  box.appendChild(row);
  return row;
}

export function replaceUserRow(row: HTMLElement, message: Message): void {
  const next = messageRow("user", message);
  row.replaceWith(next);
  reveal(next);
}

export function appendStreamingRow(): HTMLElement | null {
  const box = document.getElementById("messages");
  if (!box) return null;
  box.querySelector(".empty-hint")?.remove();
  const row = messageRow(
    "assistant",
    {
      content: "",
      parts: [],
      model: modelLabel(),
      created_at: Date.now(),
    },
    true
  );
  row.id = "streaming-row";
  box.appendChild(row);
  reveal(row);
  return row;
}

export function finishStreamingRow(row: HTMLElement, message: Message): void {
  const next = messageRow("assistant", message);
  row.replaceWith(next);
  reveal(next);
}

function reveal(node: HTMLElement): void {
  node.style.contentVisibility = "visible";
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      node.style.contentVisibility = "";
    });
  });
}

let scrollPending = false;
let followTail = true;
let expectedTop = -1;
let jumpButton: HTMLElement | null = null;

export function resetFollow(): void {
  followTail = true;
  syncJumpButton(true);
}

function syncJumpButton(atBottom: boolean): void {
  if (!jumpButton) return;
  jumpButton.classList.toggle("is-hidden", atBottom);
}

function approvalInset(): number {
  const main = document.querySelector<HTMLElement>(".main");
  if (!main) return 0;
  const value = parseFloat(main.style.getPropertyValue("--approval-h"));
  return Number.isFinite(value) ? value : 0;
}

function scrollToBottom(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  box.scrollTop = box.scrollHeight;
  expectedTop = box.scrollTop;
}

export function requestScroll(): void {
  if (scrollPending) return;
  scrollPending = true;
  requestAnimationFrame(() => {
    scrollPending = false;
    if (!followTail) return;
    scrollToBottom();
    syncJumpButton(true);
  });
}

export function bindFollow(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  const update = (): void => {
    const inset = approvalInset();
    const gap = box.scrollHeight - box.scrollTop - box.clientHeight - inset;
    if (expectedTop >= 0 && Math.abs(box.scrollTop - expectedTop) <= 2) {
      syncJumpButton(true);
      return;
    }
    expectedTop = -1;
    followTail = gap < 80;
    syncJumpButton(gap < 80);
  };
  box.addEventListener("scroll", update, { passive: true });
  update();
}

export function buildJumpButton(): HTMLElement {
  const btn = el("button", "jump-bottom", "↓");
  btn.title = "滚动到底部";
  btn.classList.add("is-hidden");
  btn.addEventListener("click", () => {
    const box = document.getElementById("messages");
    if (!box) return;
    scrollToBottom();
    followTail = true;
    syncJumpButton(true);
  });
  jumpButton = btn;
  return btn;
}

export function mountJumpButton(host: HTMLElement): void {
  const btn = buildJumpButton();
  host.appendChild(btn);
}

export interface StreamPatch {
  index: number;
  part: Part;
  active: boolean;
}

export function patchStreamingRow(row: HTMLElement, patch: StreamPatch): void {
  const holder = row.querySelector(".msg-parts") as HTMLElement | null;
  if (!holder) return;

  let node = holder.children[patch.index] as HTMLElement | undefined;
  if (node && node.dataset.part !== String(patch.index)) {
    node = undefined;
  }
  if (!node) {
    node =
      (holder.querySelector(
        `[data-part="${patch.index}"]`
      ) as HTMLElement | null) ?? undefined;
  }
  if (!node) {
    if (patch.part.kind === "text") {
      node = el("div", "msg-body");
      node.dataset.textLen = "0";
    } else if (patch.part.kind === "think") {
      node = thinkPart("");
      const body = node.querySelector(".think-body") as HTMLElement | null;
      if (body) body.dataset.textLen = "0";
    } else {
      node = partNode(patch.part);
    }
    node.dataset.part = String(patch.index);
    holder.appendChild(node);
    reveal(node);
  }

  if (patch.part.kind === "think") {
    const body = node.querySelector(".think-body") as HTMLElement | null;
    if (!body) return;
    const prev = Number(body.dataset.textLen ?? "0");
    const next = patch.part.text || "…";
    if (followTail && next.slice(prev).includes("\n")) scrollToBottom();
    renderStreamingText(body, next, false);
    body.dataset.textLen = String(next.length);
    body.scrollTop = body.scrollHeight;
    if (followTail) scrollToBottom();
    return;
  }

  if (patch.part.kind === "tool") {
    if (patch.part.tool_name === "agent") {
      const head = node.querySelector(".fold-head") as HTMLElement | null;
      if (head) head.textContent = subagentHead(patch.part);
      const flow = node.querySelector(".subagent-flow") as HTMLElement | null;
      if (flow) fillSubagentFlow(flow, patch.part);
      return;
    }
    if (patch.part.running) {
      const head = node.querySelector(".fold-head") as HTMLElement | null;
      if (head) head.textContent = `… ${patch.part.summary}`;
      const pre = node.querySelector(".tool-output") as HTMLElement | null;
      if (pre) pre.textContent = patch.part.detail;
      const fold = node.querySelector(".fold-body") as HTMLElement | null;
      if (fold) fold.scrollTop = fold.scrollHeight;
      if (followTail) scrollToBottom();
      return;
    }
    const fresh = partNode(patch.part);
    fresh.dataset.part = String(patch.index);
    node.replaceWith(fresh);
    reveal(fresh);
    if (followTail) scrollToBottom();
    return;
  }

  const prev = Number(node.dataset.textLen ?? "0");
  const next = patch.part.text || "…";
  if (followTail && next.slice(prev).includes("\n")) scrollToBottom();
  renderStreamingText(node, next, patch.active);
  node.dataset.textLen = String(next.length);
  if (followTail) scrollToBottom();
}
