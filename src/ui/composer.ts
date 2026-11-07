import * as api from "../api";
import { state, rememberComposer } from "../state";
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

let usageTagEl: HTMLElement | null = null;
let lastUsage: {
  sessionId: string;
  used: number;
  window: number;
  cached: number;
} | null = null;

function fmtCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function paintUsageTag(
  tag: HTMLElement,
  used: number,
  window: number,
  cached: number | null | undefined,
  hitBase: number
): void {
  const pct = window > 0 ? (used / window) * 100 : 0;
  const hasCached = cached !== null && cached !== undefined;
  const hit = hasCached && hitBase > 0 ? ((cached as number) / hitBase) * 100 : null;
  const usageText = `${fmtCount(used)}/${fmtCount(window)} ${pct.toFixed(1)}%`;
  const hitText =
    hit === null
      ? ""
      : `命中 ${hit > 99 ? Math.min(hit, 99.9).toFixed(1) : hit.toFixed(0)}% · `;
  tag.textContent = `${hitText}${usageText}`;
}

async function refreshUsageInto(tag: HTMLElement): Promise<void> {
  if (!state.currentSessionId) {
    tag.textContent = "";
    return;
  }
  try {
    const [used, window, stored] = await api.estimateContext(
      state.currentSessionId,
      state.currentProviderId,
      state.settings.system_prompt,
      state.settings.strip_thinking
    );
    const current =
      lastUsage &&
      lastUsage.sessionId === state.currentSessionId &&
      lastUsage.window === window
        ? lastUsage
        : null;
    const cached = current ? current.cached : stored ? stored[1] : null;
    const hitBase = current ? current.used : stored ? stored[0] : 0;
    paintUsageTag(tag, used, window, cached, hitBase);
  } catch {
    tag.textContent = "";
  }
}

export function refreshUsageDisplay(): void {
  if (usageTagEl) void refreshUsageInto(usageTagEl);
}

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
    rememberComposer();
  });
  bar.appendChild(model);

  const think = el("select", "composer-select");
  Object.entries(THINK_LABEL).forEach(([key, label]) => {
    const opt = option(label, key);
    if (state.thinkingLevel === key) opt.selected = true;
    think.appendChild(opt);
  });
  think.addEventListener("change", () => {
    state.thinkingLevel = think.value;
    rememberComposer();
  });
  bar.appendChild(think);

  bar.appendChild(el("div", "composer-spacer"));

  const usageTag = el("div", "usage-tag");
  bar.appendChild(usageTag);
  usageTagEl = usageTag;

  const send = el("button", "composer-send");
  bar.appendChild(send);

  const SEND_ICON =
    '<svg width="14" height="14" viewBox="0 0 14 14" fill="none"><path d="M7 11.5V2.5M3.2 6.3 7 2.5l3.8 3.8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  const STOP_ICON =
    '<svg width="11" height="11" viewBox="0 0 11 11"><rect x="1.5" y="1.5" width="8" height="8" fill="currentColor"/></svg>';

  const setSendIdle = (): void => {
    send.innerHTML = SEND_ICON;
    send.disabled = input.value.trim().length === 0;
    send.title = "发送";
  };

  const setSendBusy = (): void => {
    send.innerHTML = STOP_ICON;
    send.disabled = false;
    send.title = "停止";
  };

  setSendIdle();

  const paint = (
    sessionId: string,
    used: number,
    window: number,
    cached: number | null
  ): void => {
    if (cached !== null && used > 0) {
      lastUsage = { sessionId, used, window, cached };
    }
    paintUsageTag(usageTag, used, window, cached, used);
  };

  const refresh = async (): Promise<void> => {
    await refreshUsageInto(usageTag);
  };

  void refresh();

  const doSend = async (): Promise<void> => {
    const text = input.value.trim();
    if (!text || state.streaming) return;
    if (!state.currentSessionId) return;
    if (!state.currentProviderId) return;

    const sessionId = state.currentSessionId;
    const providerId = state.currentProviderId;

    input.value = "";
    state.streaming = true;
    setSendBusy();

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
        } else if (event.type === "usage") {
          paint(
            sessionId,
            event.prompt_tokens,
            event.context_window,
            event.cached_tokens
          );
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
      setSendIdle();
      onDone();
      void refresh();
    }
  };

  send.addEventListener("click", () => {
    if (state.streaming) {
      void api.cancelMessage();
      return;
    }
    void doSend();
  });
  input.addEventListener("input", () => {
    if (!state.streaming) {
      send.disabled = input.value.trim().length === 0;
    }
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      void doSend();
    }
  });

  box.appendChild(bar);

  return box;
}
