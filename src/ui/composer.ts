import * as api from "../api";
import { state, rememberComposer } from "../state";
import type { Attachment, Message, Part } from "../types";
import { buildChip, uploadFile, warnText } from "./attach";
import {
  beginApproval,
  closeApproval,
  openApproval,
  streamApproval,
} from "./approval";
import {
  appendStreamingRow,
  appendUserRow,
  finishStreamingRow,
  patchStreamingRow,
  refreshRollbackButtons,
  replaceUserRow,
  requestScroll,
  resetFollow,
} from "./chat";
import { el, option } from "./dom";
import { refreshJumpbar } from "./jumpbar";

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
  input.placeholder = "输入消息，Enter 发送，Shift+Enter 换行";
  input.rows = 2;
  box.appendChild(input);

  const chips = el("div", "attach-chips");
  box.appendChild(chips);

  const warn = el("div", "attach-warn");
  warn.hidden = true;
  box.appendChild(warn);

  const fileInput = el("input", "attach-input");
  fileInput.type = "file";
  fileInput.multiple = true;
  box.appendChild(fileInput);

  const bar = el("div", "composer-bar");

  const attach = el("button", "composer-btn", "＋");
  attach.title = "添加附件";
  bar.appendChild(attach);

  const pending: Attachment[] = [];

  const removeAttachment = (id: string): void => {
    const idx = pending.findIndex((a) => a.id === id);
    if (idx >= 0) pending.splice(idx, 1);
    paintChips();
  };

  const paintChips = (): void => {
    chips.replaceChildren();
    pending.forEach((a) => chips.appendChild(buildChip(a, removeAttachment)));
    const text = warnText(pending);
    warn.textContent = text;
    warn.hidden = text.length === 0;
    setSendIdle();
  };

  const addFiles = async (files: FileList | null): Promise<void> => {
    if (!files || files.length === 0 || !state.currentSessionId) return;
    const sessionId = state.currentSessionId;
    for (const file of Array.from(files)) {
      try {
        pending.push(await uploadFile(sessionId, file));
      } catch (err) {
        warn.textContent = `上传失败：${String(err)}`;
        warn.hidden = false;
        continue;
      }
      paintChips();
    }
  };

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
    send.disabled = input.value.trim().length === 0 && pending.length === 0;
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

  window.addEventListener("hyperon:refresh-usage", () => {
    void refresh();
  });

  void refresh();

  const doSend = async (): Promise<void> => {
    if (state.streaming) return;
    const text = input.value.trim();
    const attachments = pending.splice(0, pending.length);
    if (!text && attachments.length === 0) return;
    if (!state.currentSessionId) return;
    if (!state.currentProviderId) return;

    const sessionId = state.currentSessionId;
    const providerId = state.currentProviderId;

    input.value = "";
    state.streaming = true;
    setSendBusy();
    paintChips();
    refreshRollbackButtons();

    const pendingUser: Message = {
      id: "pending-user",
      session_id: sessionId,
      role: "user",
      content: text,
      reasoning: "",
      parts: [],
      attachments,
      created_at: Date.now(),
      model: "",
      elapsed_ms: 0,
      output_ms: 0,
      output_tokens: 0,
    };
    state.messages.push(pendingUser);
    const userRow = appendUserRow(pendingUser);
    refreshJumpbar();
    onDone();
    resetFollow();

    const row = appendStreamingRow();
    const parts: Part[] = [];
    let current: Part | null = null;
    const subagentIndex = new Map<string, number>();
    const toolIndex = new Map<string, number>();

    const activeSubagent = (
      id: string
    ): { index: number; children: Part[] } | null => {
      const index = subagentIndex.get(id);
      if (index === undefined) return null;
      const part = parts[index];
      if (!part || !part.children) return null;
      return { index, children: part.children };
    };

    let frame = 0;
    const dirty = new Set<number>();
    let activeIndex = -1;
    const flush = (): void => {
      frame = 0;
      if (row) {
        dirty.forEach((index) => {
          if (index >= 0 && index < parts.length) {
            patchStreamingRow(row, {
              index,
              part: parts[index],
              active: index === activeIndex,
            });
          }
        });
      }
      dirty.clear();
      requestScroll();
    };
    const schedule = (index: number): void => {
      dirty.add(index);
      if (!frame) frame = requestAnimationFrame(flush);
    };
    // 正文段结束时重渲染一次，让最新词的线框消失
    const settleText = (): void => {
      if (activeIndex >= 0) schedule(activeIndex);
      activeIndex = -1;
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
      const result = await api.sendMessage(
        sessionId,
        providerId,
        text,
        attachments,
        (event) => {
          if (event.type === "reasoning") {
            if (!current || current.kind !== "think") {
              settleText();
              pushPart("think");
            }
            current!.text += event.text;
            schedule(parts.length - 1);
          } else if (event.type === "delta") {
            if (!current || current.kind !== "text") pushPart("text");
            current!.text += event.text;
            activeIndex = parts.length - 1;
            schedule(activeIndex);
          } else if (event.type === "approvalstart") {
            beginApproval(event);
          } else if (event.type === "approvaldelta") {
            streamApproval(event);
          } else if (event.type === "approval") {
            openApproval(event);
          } else if (event.type === "subagentstart") {
            settleText();
            parts.push({
              kind: "tool",
              text: "",
              summary: event.title ? `子代理 · ${event.title}` : "子代理",
              detail: "",
              ok: true,
              tool_name: "agent",
              tool_args: "",
              children: [],
              running: true,
            });
            subagentIndex.set(event.id, parts.length - 1);
            current = null;
            schedule(parts.length - 1);
          } else if (event.type === "subagentreasoning") {
            const item = activeSubagent(event.id);
            if (item) {
              const last = item.children[item.children.length - 1];
              if (last && last.kind === "think") {
                last.text += event.text;
              } else {
                item.children.push({
                  kind: "think",
                  text: event.text,
                  summary: "",
                  detail: "",
                  ok: true,
                });
              }
              schedule(item.index);
            }
          } else if (event.type === "subagentdelta") {
            const item = activeSubagent(event.id);
            if (item) {
              const last = item.children[item.children.length - 1];
              if (last && last.kind === "text") {
                last.text += event.text;
              } else {
                item.children.push({
                  kind: "text",
                  text: event.text,
                  summary: "",
                  detail: "",
                  ok: true,
                });
              }
              schedule(item.index);
            }
          } else if (event.type === "subagenttool") {
            const item = activeSubagent(event.id);
            if (item) {
              item.children.push({
                kind: "tool",
                text: "",
                summary: event.summary,
                detail: event.detail,
                ok: event.ok,
              });
              schedule(item.index);
            }
          } else if (event.type === "subagentend") {
            const index = subagentIndex.get(event.id);
            const part = index === undefined ? null : parts[index];
            if (index !== undefined && part) {
              part.ok = event.ok;
              part.running = false;
              schedule(index);
            }
          } else if (event.type === "toolstart") {
            settleText();
            parts.push({
              kind: "tool",
              text: "",
              summary: event.summary,
              detail: "",
              ok: true,
              tool_name: event.tool,
              tool_args: "",
              running: true,
            });
            toolIndex.set(event.id, parts.length - 1);
            current = null;
            schedule(parts.length - 1);
          } else if (event.type === "tooldelta") {
            const index = toolIndex.get(event.id);
            if (index !== undefined && parts[index]) {
              parts[index].detail += event.text;
              schedule(index);
            }
          } else if (event.type === "toolend") {
            const index =
              event.tool === "agent"
                ? subagentIndex.get(event.id)
                : toolIndex.get(event.id);
            const part = index === undefined ? null : parts[index];
            if (index !== undefined && part) {
              part.summary = event.summary;
              part.detail = event.detail;
              part.ok = event.ok;
              part.running = false;
              schedule(index);
            } else {
              settleText();
              parts.push({
                kind: "tool",
                text: "",
                summary: event.summary,
                detail: event.detail,
                ok: event.ok,
              });
              current = null;
              schedule(parts.length - 1);
            }
          } else if (event.type === "usage") {
            paint(
              sessionId,
              event.prompt_tokens,
              event.context_window,
              event.cached_tokens
            );
          } else if (event.type === "error") {
            settleText();
            pushPart("text");
            current!.text += `\n\n请求失败：${event.message}`;
            schedule(parts.length - 1);
          } else if (event.type === "done") {
            settleText();
          }
        }
      );
      if (state.currentSessionId === sessionId) {
        state.streaming = false;
        const pendingIndex = state.messages.findIndex((m) => m.id === "pending-user");
        if (pendingIndex >= 0) {
          state.messages[pendingIndex] = result.user;
        } else {
          const userIndex = state.messages.findIndex((m) => m.id === result.user.id);
          if (userIndex >= 0) state.messages[userIndex] = result.user;
          else state.messages.push(result.user);
        }
        const assistantIndex = state.messages.findIndex(
          (m) => m.id === result.assistant.id
        );
        if (assistantIndex >= 0) state.messages[assistantIndex] = result.assistant;
        else state.messages.push(result.assistant);
        if (userRow) replaceUserRow(userRow, result.user);
        if (row) finishStreamingRow(row, result.assistant);
        refreshJumpbar();
      }
    } catch (err) {
      if (state.currentSessionId === sessionId) {
        state.messages = await api.getMessages(sessionId);
        userRow?.remove();
        settleText();
        pushPart("text");
        current!.text += `\n\n请求失败：${String(err)}`;
        schedule(parts.length - 1);
      }
    } finally {
      state.streaming = false;
      closeApproval();
      setSendIdle();
      refreshRollbackButtons();
      onDone();
      void refresh();
      // 本次回复可能产生了文件变更，通知右栏变更页刷新
      window.dispatchEvent(new CustomEvent("hyperon:changes-updated"));
    }
  };

  send.addEventListener("click", () => {
    if (state.streaming) {
      void api.cancelMessage();
      return;
    }
    void doSend();
  });
  attach.addEventListener("click", () => fileInput.click());
  fileInput.addEventListener("change", () => {
    void addFiles(fileInput.files);
    fileInput.value = "";
  });
  box.addEventListener("dragover", (e) => e.preventDefault());
  box.addEventListener("drop", (e) => {
    e.preventDefault();
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) void addFiles(files);
  });
  input.addEventListener("input", () => {
    if (!state.streaming) {
      send.disabled = input.value.trim().length === 0 && pending.length === 0;
    }
  });
  input.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    if (e.shiftKey || e.isComposing || e.keyCode === 229) return;
    e.preventDefault();
    void doSend();
  });
  input.addEventListener("paste", (e) => {
    const files = e.clipboardData?.files;
    if (files && files.length > 0) {
      e.preventDefault();
      void addFiles(files);
    }
  });

  box.appendChild(bar);

  return box;
}
