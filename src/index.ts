import "./styles.css";
import * as api from "./api";
import { state, recallSelection, rememberSelection, recallComposer } from "./state";
import { bindFollow, buildMessages, mountJumpButton, resetFollow } from "./ui/chat";
import { buildComposer, refreshUsageDisplay } from "./ui/composer";
import { el } from "./ui/dom";
import { buildJumpbar, refreshJumpbar } from "./ui/jumpbar";
import { buildRightPanel, refreshRightPanel } from "./ui/rightpanel";
import { buildSettings } from "./ui/settings";
import { buildSidebar } from "./ui/sidebar";
import { buildTopbar } from "./ui/topbar";

const root = document.getElementById("app")!;

function buildMain(): HTMLElement {
  const main = el("div", "main");

  const session = state.sessions.find((s) => s.id === state.currentSessionId);
  main.appendChild(el("div", "main-header", session ? session.title : "未选择会话"));

  const wrap = el("div", "messages-wrap");
  wrap.appendChild(buildMessages());
  wrap.appendChild(buildJumpbar());
  main.appendChild(wrap);

  const composerHost = el("div", "composer-host");
  composerHost.appendChild(
    buildComposer(() => {
      renderHeader();
    })
  );
  main.appendChild(composerHost);

  requestAnimationFrame(() => {
    mountJumpButton(composerHost);
    bindFollow();
  });
  return main;
}

function buildChatTab(): HTMLElement {
  const body = el("div", "body");
  body.appendChild(buildSidebar(render));
  body.appendChild(buildMain());
  body.appendChild(buildRightPanel());
  return body;
}

function swapBody(next: HTMLElement): void {
  const old = root.querySelector(".body, .settings, .blank-tab");
  if (old) {
    old.replaceWith(next);
  } else {
    const app = root.querySelector(".app");
    if (app) app.appendChild(next);
    else render();
  }
}

function renderSidebar(): void {
  const old = root.querySelector(".sidebar");
  if (old) old.replaceWith(buildSidebar(render));
}

let lastRenderedSession: string | null = null;

function settleScrollToBottom(): void {
  const apply = (): void => {
    const el = document.getElementById("messages");
    if (el) el.scrollTop = el.scrollHeight;
  };
  apply();
  requestAnimationFrame(() => {
    apply();
    requestAnimationFrame(apply);
  });
}

function renderMessages(): void {
  const old = root.querySelector(".messages");
  if (!old) {
    render();
    return;
  }
  const switched = lastRenderedSession !== state.currentSessionId;
  const atBottom = old.scrollHeight - old.scrollTop - old.clientHeight < 80;
  const prevScroll = old.scrollTop;
  const wasStreaming = state.streaming;
  lastRenderedSession = state.currentSessionId;

  const next = buildMessages();
  old.replaceWith(next);
  refreshJumpbar();

  if (switched || atBottom || wasStreaming) {
    resetFollow();
    settleScrollToBottom();
  } else if (prevScroll > 0) {
    next.scrollTop = prevScroll;
  }
  if (switched) refreshUsageDisplay();
  requestAnimationFrame(bindFollow);
}

function renderHeader(): void {
  const head = root.querySelector(".main-header");
  if (!head) return;
  const session = state.sessions.find((s) => s.id === state.currentSessionId);
  head.textContent = session ? session.title : "未选择会话";
}

function renderTopbar(): void {
  const old = root.querySelector(".topbar");
  if (old) old.replaceWith(buildTopbar(render));
}

function render(): void {
  const app = root.querySelector(".app") as HTMLElement | null;

  if (!app) {
    root.replaceChildren();
    const fresh = el("div", "app");
    fresh.appendChild(buildTopbar(render));
    fresh.appendChild(buildChatTab());
    root.appendChild(fresh);
    return;
  }

  renderTopbar();
  renderHeader();

  if (state.tab === "chat") {
    const existing = app.querySelector(".body");
    if (existing) {
      renderSidebar();
      renderMessages();
      refreshRightPanel();
    } else {
      swapBody(buildChatTab());
    }
  } else if (state.tab === "settings") {
    if (!app.querySelector(".settings")) swapBody(buildSettings(render));
  } else {
    if (!app.querySelector(".blank-tab")) swapBody(el("div", "blank-tab"));
  }
}

async function boot(): Promise<void> {
  root.replaceChildren(el("div", "boot-hint", "加载中…"));
  const snap = await api.loadAll();
  state.projects = snap.projects;
  state.sessions = snap.sessions;
  state.providers = snap.providers;
  state.messages = snap.messages;
  state.settings = snap.settings;
  state.theme = snap.settings.theme === "light" ? "light" : "dark";
  document.documentElement.dataset.theme = state.theme;

  state.presets = await api.listPresets();

  const last = recallSelection();
  const rememberedProject =
    last.project && state.projects.some((p) => p.id === last.project)
      ? last.project
      : null;
  const builtin = state.projects.find((p) => p.builtin);
  state.currentProjectId =
    rememberedProject ?? (builtin ? builtin.id : state.projects[0]?.id ?? null);

  const list = state.currentProjectId
    ? state.sessions
        .filter((s) => s.project_id === state.currentProjectId)
        .sort((a, b) => b.created_at - a.created_at)
    : [];
  const rememberedSession =
    last.session && list.some((s) => s.id === last.session) ? last.session : null;
  state.currentSessionId =
    rememberedSession ?? (list.length > 0 ? list[0].id : null);
  state.messages = state.currentSessionId
    ? await api.getMessages(state.currentSessionId)
    : [];

  const savedComposer = recallComposer();
  const savedModel =
    savedComposer.model && state.providers.some((p) => p.id === savedComposer.model)
      ? savedComposer.model
      : null;
  state.currentProviderId = savedModel ?? state.providers[0]?.id ?? "";

  const levels = ["off", "low", "medium", "high", "max"];
  state.thinkingLevel =
    savedComposer.thinking && levels.includes(savedComposer.thinking)
      ? savedComposer.thinking
      : "medium";

  rememberSelection();
  render();
  lastRenderedSession = state.currentSessionId;
  requestAnimationFrame(() => {
    const box = document.getElementById("messages");
    if (!box) return;
    resetFollow();
    settleScrollToBottom();
    bindFollow();
  });
}

void boot().catch((err) => {
  root.replaceChildren(
    el("div", "boot-hint is-error", `启动失败：${String(err)}`)
  );
});
