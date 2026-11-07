import { renderMarkdown, renderMarkdownTail } from "../markdown";
import { state } from "../state";
import type { Part } from "../types";
import { el } from "./dom";

const INITIAL_LIMIT = 30;

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

function thinkPart(text: string): HTMLElement {
  const body = el("div", "think-body");
  body.innerHTML = renderMarkdown(text);
  return foldBlock("思考过程", body, "think-block");
}

function buildDiffElement(diffText: string): HTMLElement {
  const diff = el("div", "diff");
  let firstChange: HTMLElement | null = null;
  diffText.split("\n").forEach((line) => {
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
    diff.appendChild(row);
  });
  if (firstChange) {
    requestAnimationFrame(() => {
      const target = firstChange as HTMLElement;
      diff.scrollTop = Math.max(0, target.offsetTop - diff.clientHeight / 3);
    });
  }
  return diff;
}

function toolPart(part: Part): HTMLElement {
  const body = el("div", "tool-body");
  const MARKER = "---diff---";

  if (part.detail.includes(MARKER)) {
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
      const pre = el("pre", "tool-output");
      pre.textContent = part.detail;
      body.appendChild(pre);
    }
  } else {
    body.appendChild(el("div", "empty-hint", "无输出"));
  }

  const mark = part.ok ? "✓" : "✕";
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
  content: string;
  reasoning?: string;
  parts?: Part[];
}

function messageRow(role: string, message?: MessageLike): HTMLElement {
  const row = el("div", `msg msg-${role}`);
  const holder = el("div", role === "user" ? "msg-bubble" : "msg-plain");

  if (role === "user") {
    const body = el("div", "msg-body");
    body.innerHTML = renderMarkdown(message?.content ?? "");
    holder.appendChild(body);
  } else {
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
      fallback.forEach((p) => holder.appendChild(partNode(p)));
    } else {
      parts.forEach((p) => holder.appendChild(partNode(p)));
    }
  }

  row.appendChild(holder);
  return row;
}

export function buildMessages(): HTMLElement {
  const box = el("div", "messages");
  box.id = "messages";

  if (!state.currentSessionId) {
    box.appendChild(el("div", "empty-hint center", "选择或新建一个会话"));
    return box;
  }

  if (state.messages.length === 0) {
    box.appendChild(el("div", "empty-hint center", "发一条消息开始"));
    return box;
  }

  const total = state.messages.length;
  const start = Math.max(0, total - INITIAL_LIMIT);

  if (start > 0) {
    const more = el("button", "load-more", `加载更早的 ${start} 条`);
    more.addEventListener("click", () => {
      more.remove();
      const frag = document.createDocumentFragment();
      for (let i = 0; i < start; i++) {
        const m = state.messages[i];
        frag.appendChild(messageRow(m.role, m));
      }
      box.insertBefore(frag, box.firstChild);
      const prevTop = box.scrollHeight;
      requestAnimationFrame(() => {
        box.scrollTop = box.scrollTop + (box.scrollHeight - prevTop);
      });
    });
    box.appendChild(more);
  }

  for (let i = start; i < total; i++) {
    const m = state.messages[i];
    box.appendChild(messageRow(m.role, m));
  }
  return box;
}

let scrollPending = false;
let followTail = true;
let jumpButton: HTMLElement | null = null;

export function resetFollow(): void {
  followTail = true;
  syncJumpButton(true);
}

function syncJumpButton(atBottom: boolean): void {
  if (!jumpButton) return;
  jumpButton.classList.toggle("is-hidden", atBottom);
}

export function requestScroll(): void {
  if (scrollPending) return;
  scrollPending = true;
  requestAnimationFrame(() => {
    scrollPending = false;
    if (!followTail) return;
    const box = document.getElementById("messages");
    if (box) box.scrollTop = box.scrollHeight;
    syncJumpButton(true);
  });
}

export function bindFollow(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  const update = (): void => {
    const gap = box.scrollHeight - box.scrollTop - box.clientHeight;
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
    box.scrollTop = box.scrollHeight;
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

export function appendStreamingRow(): HTMLElement | null {
  const box = document.getElementById("messages");
  if (!box) return null;
  const row = messageRow("assistant", { content: "", parts: [] });
  row.id = "streaming-row";
  const holder = row.querySelector(".msg-plain");
  if (holder) holder.replaceChildren();
  box.appendChild(row);
  return row;
}

export interface StreamPatch {
  index: number;
  part: Part;
}

export function patchStreamingRow(row: HTMLElement, patch: StreamPatch): void {
  const holder = row.querySelector(".msg-plain") as HTMLElement | null;
  if (!holder) return;

  let node = holder.children[patch.index] as HTMLElement | undefined;
  if (!node) {
    node = partNode(patch.part);
    node.dataset.part = String(patch.index);
    holder.appendChild(node);
    return;
  }

  if (patch.part.kind === "think") {
    const body = node.querySelector(".think-body") as HTMLElement | null;
    if (body) {
      body.innerHTML = renderMarkdownTail(patch.part.text || "…");
      body.scrollTop = body.scrollHeight;
    }
  } else if (patch.part.kind === "tool") {
    const fresh = partNode(patch.part);
    fresh.dataset.part = String(patch.index);
    node.replaceWith(fresh);
  } else {
    node.innerHTML = renderMarkdownTail(patch.part.text || "…");
  }
}
