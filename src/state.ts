import type {
  MainTab,
  Message,
  Preset,
  Project,
  Provider,
  Session,
  Settings,
  Theme,
} from "./types";

export const state = {
  theme: "dark" as Theme,
  tab: "chat" as MainTab,
  settingsTab: "providers" as string,
  rightTab: "tools" as string,

  projects: [] as Project[],
  sessions: [] as Session[],
  providers: [] as Provider[],
  presets: [] as Preset[],
  messages: [] as Message[],

  settings: {
    theme: "dark",
    system_prompt: "",
    strip_thinking: true,
  } as Settings,

  currentProjectId: null as string | null,
  currentSessionId: null as string | null,
  currentProviderId: "",
  thinkingLevel: "medium" as string,

  streaming: false,
};

export function currentProject(): Project | null {
  return state.projects.find((p) => p.id === state.currentProjectId) ?? null;
}

export function currentSession(): Session | null {
  return state.sessions.find((s) => s.id === state.currentSessionId) ?? null;
}

export function currentProvider(): Provider | null {
  return state.providers.find((p) => p.id === state.currentProviderId) ?? null;
}

export function sessionsOf(projectId: string): Session[] {
  return state.sessions
    .filter((s) => s.project_id === projectId)
    .sort((a, b) => b.created_at - a.created_at);
}

export function newestSessionOf(projectId: string): Session | null {
  const list = sessionsOf(projectId);
  return list.length > 0 ? list[0] : null;
}

const LAST_KEY = "hyperon.lastSelection";

export function rememberSelection(): void {
  try {
    localStorage.setItem(
      LAST_KEY,
      JSON.stringify({
        project: state.currentProjectId,
        session: state.currentSessionId,
      })
    );
  } catch {
    /* 忽略 */
  }
}

export function recallSelection(): { project: string | null; session: string | null } {
  try {
    const raw = localStorage.getItem(LAST_KEY);
    if (!raw) return { project: null, session: null };
    const parsed = JSON.parse(raw) as { project?: string | null; session?: string | null };
    return {
      project: parsed.project ?? null,
      session: parsed.session ?? null,
    };
  } catch {
    return { project: null, session: null };
  }
}

const COMPOSER_KEY = "hyperon.composer";

export function rememberComposer(): void {
  try {
    localStorage.setItem(
      COMPOSER_KEY,
      JSON.stringify({
        model: state.currentProviderId,
        thinking: state.thinkingLevel,
      })
    );
  } catch {
    /* 忽略 */
  }
}

export function recallComposer(): { model: string | null; thinking: string | null } {
  try {
    const raw = localStorage.getItem(COMPOSER_KEY);
    if (!raw) return { model: null, thinking: null };
    const parsed = JSON.parse(raw) as { model?: string | null; thinking?: string | null };
    return {
      model: parsed.model ?? null,
      thinking: parsed.thinking ?? null,
    };
  } catch {
    return { model: null, thinking: null };
  }
}
