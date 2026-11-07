import * as api from "../api";
import { state, sessionsOf, rememberSelection } from "../state";
import type { Message, Project, Session } from "../types";
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

function projectForm(initialName: string, initialDirs: string[]): {
  body: HTMLElement;
  read: () => { name: string; dirs: string[] };
} {
  const body = el("div", "form");

  const nameInput = el("input", "input");
  nameInput.placeholder = "工程名称";
  nameInput.value = initialName;
  body.appendChild(field("名称", nameInput));

  const dirWrap = el("div", "dir-list");
  const addRow = (value: string): void => {
    const row = el("div", "dir-row");
    const input = el("input", "input");
    input.placeholder = "工作目录路径";
    input.value = value;
    const drop = el("button", "btn", "移除");
    drop.addEventListener("click", () => row.remove());
    row.appendChild(input);
    row.appendChild(drop);
    dirWrap.appendChild(row);
  };
  initialDirs.forEach(addRow);

  const addDir = el("button", "btn", "＋ 添加目录");
  addDir.addEventListener("click", () => addRow(""));

  body.appendChild(field("工作目录", dirWrap));
  body.appendChild(addDir);

  const read = (): { name: string; dirs: string[] } => {
    const dirs: string[] = [];
    dirWrap.querySelectorAll<HTMLInputElement>("input").forEach((i) => {
      const v = i.value.trim();
      if (v) dirs.push(v);
    });
    return { name: nameInput.value, dirs };
  };

  return { body, read };
}

function openNewProject(onDone: () => void): void {
  const form = projectForm("", []);
  openModal("新建工程", form.body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "创建",
      primary: true,
      onClick: async (close) => {
        const { name, dirs } = form.read();
        const created = await api.createProject(name, dirs);
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

function openEditProject(project: Project, onDone: () => void): void {
  const form = projectForm(project.name, project.dirs);
  openModal("修改工程", form.body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "保存",
      primary: true,
      onClick: async (close) => {
        const { name, dirs } = form.read();
        await api.updateProject(project.id, name, dirs);
        await reload();
        close();
        onDone();
      },
    },
  ]);
}

function openEditSession(session: Session, onDone: () => void): void {
  const body = el("div", "form");
  const input = el("input", "input");
  input.value = session.title;
  input.placeholder = "会话名称";
  body.appendChild(field("名称", input));

  openModal("修改会话", body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "保存",
      primary: true,
      onClick: async (close) => {
        await api.updateSession(session.id, input.value);
        await reload();
        const current = state.sessions.find((s) => s.id === state.currentSessionId);
        if (current) {
          close();
          onDone();
          return;
        }
        close();
        onDone();
      },
    },
  ]);
}

function openConfirm(
  title: string,
  message: string,
  onOk: () => Promise<void>
): void {
  const body = el("div", "form");
  body.appendChild(el("div", "confirm-text", message));

  openModal(title, body, [
    { label: "取消", onClick: (close) => close() },
    {
      label: "删除",
      primary: true,
      onClick: async (close) => {
        await onOk();
        close();
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

function attachItem(
  item: HTMLElement,
  actions: Array<{ label: string; run: () => void }>
): HTMLElement {
  const wrap = el("div", "list-row");
  wrap.appendChild(item);

  const more = el("button", "row-more", "…");
  more.addEventListener("click", (e) => {
    e.stopPropagation();
    document.querySelectorAll(".row-menu").forEach((n) => n.remove());
    const menu = el("div", "row-menu");
    actions.forEach((a) => {
      const btn = el("button", "row-menu-item", a.label);
      btn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        menu.remove();
        a.run();
      });
      menu.appendChild(btn);
    });
    document.body.appendChild(menu);
    const rect = more.getBoundingClientRect();
    menu.style.left = `${rect.right + 4}px`;
    menu.style.top = `${rect.top}px`;

    const close = (ev: MouseEvent): void => {
      if (!menu.contains(ev.target as Node)) {
        menu.remove();
        document.removeEventListener("click", close);
      }
    };
    setTimeout(() => document.addEventListener("click", close), 0);
  });
  wrap.appendChild(more);

  return wrap;
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

  const moveProject = async (id: string, delta: number): Promise<void> => {
    const ids = state.projects.map((p) => p.id);
    const from = ids.indexOf(id);
    if (from < 0) return;
    let to = from + delta;
    const builtinIndex = state.projects.findIndex((p) => p.builtin);
    if (builtinIndex >= 0 && to <= builtinIndex) to = builtinIndex + 1;
    if (to < 0 || to >= ids.length) return;
    ids.splice(from, 1);
    ids.splice(to, 0, id);
    await api.reorderProjects(ids);
    await reload();
    onChange();
  };

  state.projects.forEach((p) => {
    const item = el("button", "list-item", p.name);
    if (p.id === state.currentProjectId) item.classList.add("is-active");
    if (p.builtin) item.classList.add("is-builtin");
    item.addEventListener("click", async () => {
      state.currentProjectId = p.id;
      await refreshSessions(p.id);
      rememberSelection();
      onChange();
    });

    if (!p.builtin) {
      projectList.appendChild(
        attachItem(item, [
          { label: "修改", run: () => openEditProject(p, onChange) },
          { label: "上移", run: () => void moveProject(p.id, -1) },
          { label: "下移", run: () => void moveProject(p.id, 1) },
          {
            label: "删除",
            run: () =>
              openConfirm(
                "删除工程",
                `确定删除「${p.name}」？工程内的会话会移到默认工程，不会删除。`,
                async () => {
                  await api.deleteProject(p.id);
                  await reload();
                  if (state.currentProjectId === p.id) {
                    const builtin = state.projects.find((x) => x.builtin);
                    state.currentProjectId = builtin ? builtin.id : null;
                    if (state.currentProjectId) {
                      await refreshSessions(state.currentProjectId);
                    } else {
                      state.currentSessionId = null;
                      state.messages = [];
                    }
                  }
                  onChange();
                }
              ),
          },
        ])
      );
    } else {
      projectList.appendChild(item);
    }
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
        rememberSelection();
        onChange();
      });

      sessionList.appendChild(
        attachItem(item, [
          { label: "修改", run: () => openEditSession(s, onChange) },
          {
            label: "删除",
            run: () =>
              openConfirm("删除会话", `确定删除「${s.title}」？`, async () => {
                await api.deleteSession(s.id);
                await reload();
                if (state.currentSessionId === s.id) {
                  const rest = sessionsOf(s.project_id);
                  state.currentSessionId = rest.length > 0 ? rest[0].id : null;
                  state.messages = state.currentSessionId
                    ? await api.getMessages(state.currentSessionId)
                    : [];
                }
                onChange();
              }),
          },
        ])
      );
    });
  }
  sessionsSection.appendChild(sessionList);
  side.appendChild(sessionsSection);

  return side;
}

export { reload };
export type { Message };
