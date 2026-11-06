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

function toolPart(part: Part): HTMLElement {
  const body = el("div", "tool-body");
  if (part.detail) {
    const pre = el("pre", "tool-output");
    pre.textContent = part.detail;
    body.appendChild(pre);
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

export function resetFollow(): void {
  followTail = true;
}

export function requestScroll(): void {
  if (scrollPending) return;
  scrollPending = true;
  requestAnimationFrame(() => {
    scrollPending = false;
    if (!followTail) return;
    const box = document.getElementById("messages");
    if (box) box.scrollTop = box.scrollHeight;
  });
}

export function bindFollow(): void {
  const box = document.getElementById("messages");
  if (!box) return;
  box.addEventListener(
    "scroll",
    () => {
      const gap = box.scrollHeight - box.scrollTop - box.clientHeight;
      followTail = gap < 80;
    },
    { passive: true }
  );
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
    const body = node.querySelector(".think-body");
    if (body) body.innerHTML = renderMarkdownTail(patch.part.text || "…");
  } else if (patch.part.kind === "tool") {
    const head = node.querySelector(".fold-head");
    if (head) {
      head.textContent = `${patch.part.ok ? "✓" : "✕"} ${patch.part.summary}`;
    }
    const out = node.querySelector(".tool-output");
    if (out) out.textContent = patch.part.detail;
  } else {
    node.innerHTML = renderMarkdownTail(patch.part.text || "…");
  }
}
