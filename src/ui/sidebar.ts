import * as api from "../api";
import { state, sessionsOf } from "../state";
import type { Message } from "../types";
import { el, field, openModal } from "./dom";

async function refreshSessions(projectId: string): Promise<void> {
  state.sessions = (await api.loadAll()).sessions;
  const list = sessionsOf(projectId);
  state.currentSessionId = list.length > 0 ? list[0].id : null;
  if (state.currentSessionId) {
    state.messages = await api.getMessages(state.currentSessionId);
  } else {
    state.messages = [];
  }
}

async function reload(): Promise<void> {
  const snap = await api.loadAll();
  state.projects = snap.projects;
  state.sessions = snap.sessions;
  state.providers = snap.providers;
}

function openNewProject(onDone: () => void): void {
  const body = el("div", "form");

  const nameInput = el("input", "input");
  nameInput.placeholder = "工程名称";
  body.appendChild(field("名称", nameInput));

  const dirWrap = el("div", "dir-list");
  const addDir = el("button", "btn", "＋ 添加目录");
  const collect = (): string[] => {
    const out: string[] = [];
    dirWrap.querySelectorAll<HTMLInputElement>("input").forEach((i) => {
      const v = i.value.trim();
      if (v) out.push(v);
    });
    return out;
  };
  addDir.addEventListener("click", () => {
    const row = el("div", "dir-row");
    const input = el("input", "input");
    input.placeholder = "工作目录路径";
    const drop = el("button", "btn", "移除");
    drop.addEventListener("click", () => row.remove());
    row.appendChild(input);
    row.appendChild(drop);
    dirWrap.appendChild(row);
  });

  body.appendChild(field("工作目录", dirWrap));
  body.appendChild(addDir);

  openModal("新建工程", body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "创建",
      primary: true,
      onClick: async (close) => {
        const created = await api.createProject(nameInput.value, collect());
        await reload();
        state.currentProjectId = created.id;
        state.currentSessionId = null;
        state.messages = [];
        close();
        onDone();
      },
    },
  ]);
}

async function openNewSession(onDone: () => void): Promise<void> {
  const projectId = state.currentProjectId;
  if (!projectId) return;
  const created = await api.createSession(projectId);
  await reload();
  state.currentSessionId = created.id;
  state.messages = [];
  onDone();
}

export function buildSidebar(onChange: () => void): HTMLElement {
  const side = el("div", "sidebar");

  const projectsSection = el("div", "sidebar-section projects");
  const projectHead = el("div", "sidebar-title-row");
  projectHead.appendChild(el("div", "sidebar-title", "工程"));
  const addProject = el("button", "icon-btn", "＋");
  addProject.title = "新建工程";
  addProject.addEventListener("click", () => openNewProject(onChange));
  projectHead.appendChild(addProject);
  projectsSection.appendChild(projectHead);

  const projectList = el("div", "sidebar-list");
  state.projects.forEach((p) => {
    const item = el("button", "list-item", p.name);
    if (p.id === state.currentProjectId) item.classList.add("is-active");
    if (p.builtin) item.classList.add("is-builtin");
    item.addEventListener("click", async () => {
      state.currentProjectId = p.id;
      await refreshSessions(p.id);
      onChange();
    });
    projectList.appendChild(item);
  });
  projectsSection.appendChild(projectList);
  side.appendChild(projectsSection);

  const sessionsSection = el("div", "sidebar-section sessions");
  const sessionHead = el("div", "sidebar-title-row");
  sessionHead.appendChild(el("div", "sidebar-title", "会话"));
  const addSession = el("button", "icon-btn", "＋");
  addSession.title = "新建会话";
  const hasProject = state.currentProjectId !== null;
  if (!hasProject) {
    addSession.setAttribute("disabled", "true");
  }
  addSession.addEventListener("click", () => {
    if (!hasProject) return;
    void openNewSession(onChange);
  });
  sessionHead.appendChild(addSession);
  sessionsSection.appendChild(sessionHead);

  const sessionList = el("div", "sidebar-list");
  if (!hasProject) {
    sessionList.appendChild(el("div", "empty-hint", "先选择工程"));
  } else {
    const list = sessionsOf(state.currentProjectId!);
    if (list.length === 0) {
      sessionList.appendChild(el("div", "empty-hint", "暂无会话"));
    }
    list.forEach((s) => {
      const item = el("button", "list-item", s.title);
      if (s.id === state.currentSessionId) item.classList.add("is-active");
      item.addEventListener("click", async () => {
        state.currentSessionId = s.id;
        state.messages = await api.getMessages(s.id);
        onChange();
      });
      sessionList.appendChild(item);
    });
  }
  sessionsSection.appendChild(sessionList);
  side.appendChild(sessionsSection);

  return side;
}

export { reload };
export type { Message };
