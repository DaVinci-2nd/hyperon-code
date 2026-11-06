import * as api from "../api";
import { state } from "../state";
import type { Part } from "../types";
import { openApproval } from "./approval";
import {
  appendStreamingRow,
  patchStreamingRow,
  requestScroll,
  resetFollow,
} from "./chat";
import { el, option } from "./dom";

const THINK_LABEL: Record<string, string> = {
  off: "思考：关闭",
  low: "思考：低",
  medium: "思考：中",
  high: "思考：高",
  max: "思考：最高",
};

export function buildComposer(onDone: () => void): HTMLElement {
  const box = el("div", "composer");

  const input = el("textarea", "composer-input");
  input.placeholder = "输入消息，Ctrl+Enter 发送";
  input.rows = 2;
  box.appendChild(input);

  const bar = el("div", "composer-bar");

  const attach = el("button", "composer-btn", "＋");
  attach.title = "添加附件";
  bar.appendChild(attach);

  const think = el("select", "composer-select");
  Object.entries(THINK_LABEL).forEach(([key, label]) => {
    const opt = option(label, key);
    if (state.thinkingLevel === key) opt.selected = true;
    think.appendChild(opt);
  });
  think.addEventListener("change", () => {
    state.thinkingLevel = think.value;
  });
  bar.appendChild(think);

  const model = el("select", "composer-select");
  if (state.providers.length === 0) {
    model.appendChild(option("未配置模型", ""));
  }
  state.providers.forEach((p) => {
    const opt = option(`${p.name} · ${p.model}`, p.id);
    if (state.currentProviderId === p.id) opt.selected = true;
    model.appendChild(opt);
  });
  if (!state.currentProviderId && state.providers.length > 0) {
    state.currentProviderId = state.providers[0].id;
  }
  model.addEventListener("change", () => {
    state.currentProviderId = model.value;
  });
  bar.appendChild(model);

  bar.appendChild(el("div", "composer-spacer"));

  const send = el("button", "composer-send", "发送");
  bar.appendChild(send);

  const doSend = async (): Promise<void> => {
    const text = input.value.trim();
    if (!text || state.streaming) return;
    if (!state.currentSessionId) return;
    if (!state.currentProviderId) return;

    const sessionId = state.currentSessionId;
    const providerId = state.currentProviderId;

    input.value = "";
    state.streaming = true;
    send.setAttribute("disabled", "true");
    send.textContent = "发送中";

    state.messages = await api.getMessages(sessionId);
    state.messages.push({
      id: "pending-user",
      session_id: sessionId,
      role: "user",
      content: text,
      reasoning: "",
      parts: [],
      created_at: Date.now(),
    });
    onDone();
    resetFollow();

    const row = appendStreamingRow();
    const parts: Part[] = [];
    let current: Part | null = null;

    let frame = 0;
    let dirtyIndex = -1;
    const flush = (): void => {
      frame = 0;
      if (row && dirtyIndex >= 0 && dirtyIndex < parts.length) {
        patchStreamingRow(row, { index: dirtyIndex, part: parts[dirtyIndex] });
      }
      dirtyIndex = -1;
      requestScroll();
    };
    const schedule = (index: number): void => {
      dirtyIndex = index;
      if (!frame) frame = requestAnimationFrame(flush);
    };

    const pushPart = (kind: string): Part => {
      const p: Part = {
        kind,
        text: "",
        summary: "",
        detail: "",
        ok: true,
      };
      parts.push(p);
      current = p;
      return p;
    };

    try {
      await api.sendMessage(sessionId, providerId, text, (event) => {
        if (event.type === "reasoning") {
          if (!current || current.kind !== "think") pushPart("think");
          current!.text += event.text;
          schedule(parts.length - 1);
        } else if (event.type === "delta") {
          if (!current || current.kind !== "text") pushPart("text");
          current!.text += event.text;
          schedule(parts.length - 1);
        } else if (event.type === "approval") {
          openApproval(event);
        } else if (event.type === "toolend") {
          parts.push({
            kind: "tool",
            text: "",
            summary: event.summary,
            detail: event.detail,
            ok: event.ok,
          });
          current = null;
          schedule(parts.length - 1);
        } else if (event.type === "error") {
          pushPart("text");
          current!.text += `\n\n请求失败：${event.message}`;
          schedule(parts.length - 1);
        }
      });
      state.messages = await api.getMessages(sessionId);
    } catch (err) {
      state.messages = await api.getMessages(sessionId);
      pushPart("text");
      current!.text += `\n\n请求失败：${String(err)}`;
      schedule(parts.length - 1);
    } finally {
      state.streaming = false;
      send.removeAttribute("disabled");
      send.textContent = "发送";
      onDone();
    }
  };

  send.addEventListener("click", () => void doSend());
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      void doSend();
    }
  });

  box.appendChild(bar);

  return box;
}
