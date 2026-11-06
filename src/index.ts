import "./styles.css";
import * as api from "./api";
import { state } from "./state";
import { buildMessages } from "./ui/chat";
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

function render(): void {
  root.replaceChildren();

  const app = el("div", "app");
  app.appendChild(buildTopbar(render));

  if (state.tab === "chat") {
    app.appendChild(buildChatTab());
  } else if (state.tab === "settings") {
    app.appendChild(buildSettings(render));
  } else {
    app.appendChild(el("div", "blank-tab"));
  }

  root.appendChild(app);
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
