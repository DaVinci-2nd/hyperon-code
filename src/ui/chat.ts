import { renderMarkdown } from "../markdown";
import { state } from "../state";
import { el } from "./dom";

function thinkingBlock(text: string): HTMLElement {
  const details = document.createElement("details");
  details.className = "think-block";
  details.open = true;
  const summary = document.createElement("summary");
  summary.className = "think-head";
  summary.textContent = "思考过程";
  details.appendChild(summary);
  const body = el("div", "think-body");
  body.innerHTML = renderMarkdown(text);
  details.appendChild(body);
  return details;
}

function messageRow(
  role: string,
  content: string,
  reasoning = ""
): HTMLElement {
  const row = el("div", `msg msg-${role}`);
  const holder = el("div", role === "user" ? "msg-bubble" : "msg-plain");
  if (role === "assistant" && reasoning) {
    holder.appendChild(thinkingBlock(reasoning));
  }
  const body = el("div", "msg-body");
  body.innerHTML = renderMarkdown(content);
  holder.appendChild(body);
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
  }

  state.messages.forEach((m) => {
    box.appendChild(messageRow(m.role, m.content, m.reasoning));
  });
  return box;
}

export function scrollMessagesToEnd(): void {
  const box = document.getElementById("messages");
  if (box) box.scrollTop = box.scrollHeight;
}

export function appendStreamingRow(): HTMLElement | null {
  const box = document.getElementById("messages");
  if (!box) return null;
  const row = messageRow("assistant", "");
  row.id = "streaming-row";
  box.appendChild(row);
  return row;
}

export function updateStreamingRow(
  row: HTMLElement,
  text: string,
  reasoning = ""
): void {
  const holder = row.querySelector(".msg-plain");
  if (!holder) return;

  let block = holder.querySelector(".think-block") as HTMLDetailsElement | null;
  if (reasoning) {
    if (!block) {
      block = thinkingBlock(reasoning) as HTMLDetailsElement;
      holder.insertBefore(block, holder.firstChild);
    } else {
      const body = block.querySelector(".think-body");
      if (body) body.innerHTML = renderMarkdown(reasoning);
    }
  }

  const body = holder.querySelector(".msg-body");
  if (body) body.innerHTML = renderMarkdown(text || "…");
  scrollMessagesToEnd();
}
