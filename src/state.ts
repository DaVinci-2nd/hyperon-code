import type {
  MainTab,
  Message,
  Preset,
  Project,
  Provider,
  Session,
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
  return state.sessions.filter((s) => s.project_id === projectId);
}
