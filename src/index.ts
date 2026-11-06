import "./styles.css";
import * as api from "./api";
import { state } from "./state";
import { bindFollow, buildMessages } from "./ui/chat";
import { buildComposer } from "./ui/composer";
import { el } from "./ui/dom";
import { buildSettings } from "./ui/settings";
import { buildSidebar } from "./ui/sidebar";
import { buildTopbar } from "./ui/topbar";

const root = document.getElementById("app")!;

function buildMain(): HTMLElement {
  const main = el("div", "main");

  const session = state.sessions.find((s) => s.id === state.currentSessionId);
  main.appendChild(el("div", "main-header", session ? session.title : "未选择会话"));
  main.appendChild(buildMessages());
  main.appendChild(buildComposer(render));
  requestAnimationFrame(bindFollow);
  return main;
}

function buildRightPanel(): HTMLElement {
  const panel = el("div", "right-panel");

  const tabs = el("div", "right-tabs");
  [
    { key: "tools", label: "工具" },
    { key: "changes", label: "变更" },
  ].forEach((t) => {
    const btn = el("button", "right-tab", t.label);
    if (state.rightTab === t.key) btn.classList.add("is-active");
    btn.addEventListener("click", () => {
      state.rightTab = t.key;
      render();
    });
    tabs.appendChild(btn);
  });
  panel.appendChild(tabs);
  panel.appendChild(el("div", "right-body", "暂无内容"));

  return panel;
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

function renderMessages(): void {
  const old = root.querySelector(".messages");
  if (!old) {
    render();
    return;
  }
  const next = buildMessages();
  old.replaceWith(next);
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
  const snap = await api.loadAll();
  state.projects = snap.projects;
  state.sessions = snap.sessions;
  state.providers = snap.providers;
  state.messages = snap.messages;
  state.theme = snap.settings.theme === "light" ? "light" : "dark";
  document.documentElement.dataset.theme = state.theme;

  state.presets = await api.listPresets();

  const builtin = state.projects.find((p) => p.builtin);
  state.currentProjectId = builtin ? builtin.id : state.projects[0]?.id ?? null;

  const list = state.currentProjectId
    ? state.sessions.filter((s) => s.project_id === state.currentProjectId)
    : [];
  state.currentSessionId = list.length > 0 ? list[0].id : null;
  state.messages = state.currentSessionId
    ? await api.getMessages(state.currentSessionId)
    : [];

  if (state.providers.length > 0) {
    state.currentProviderId = state.providers[0].id;
  }

  render();
}

void boot();
