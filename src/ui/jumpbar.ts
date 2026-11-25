import { state } from "../state";
import { ensureLoaded, scrollToMessage } from "./chat";
import { el } from "./dom";

let bar: HTMLElement | null = null;
let tip: HTMLElement | null = null;

function hideTip(): void {
  tip?.remove();
  tip = null;
}

function showTip(tick: HTMLElement, text: string): void {
  hideTip();
  const raw = text.trim();
  const label = raw.length > 40 ? `${raw.slice(0, 40)}…` : raw;
  tip = el("div", "jump-tip", label || "空消息");
  document.body.appendChild(tip);
  const rect = tick.getBoundingClientRect();
  tip.style.left = `${rect.left - 10}px`;
  tip.style.top = `${rect.top + rect.height / 2}px`;
}

export function refreshJumpbar(): void {
  if (!bar) return;
  hideTip();
  bar.replaceChildren();
  const users = state.messages.filter((m) => m.role === "user");
  bar.classList.toggle("is-empty", users.length === 0);
  users.forEach((m) => {
    const tick = el("button", "jump-tick");
    tick.type = "button";
    tick.addEventListener("mouseenter", () => showTip(tick, m.content));
    tick.addEventListener("mouseleave", hideTip);
    tick.addEventListener("click", () => {
      hideTip();
      if (ensureLoaded(m.id)) scrollToMessage(m.id);
    });
    bar!.appendChild(tick);
  });
}

export function buildJumpbar(): HTMLElement {
  bar = el("div", "jumpbar is-empty");
  window.addEventListener("hyperon:messages-changed", refreshJumpbar);
  refreshJumpbar();
  return bar;
}
