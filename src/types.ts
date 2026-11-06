export type Role = "user" | "assistant";

export interface Part {
  kind: string;
  text: string;
  summary: string;
  detail: string;
  ok: boolean;
}

export interface Message {
  id: string;
  session_id: string;
  role: Role;
  content: string;
  reasoning: string;
  parts: Part[];
  created_at: number;
}

export interface Session {
  id: string;
  project_id: string;
  title: string;
  created_at: number;
}

export interface Project {
  id: string;
  name: string;
  dirs: string[];
  builtin: boolean;
}

export interface Provider {
  id: string;
  name: string;
  protocol: string;
  base_url: string;
  model: string;
  api_key: string;
  thinking: string;
  image: string;
  preset: string;
}

export interface Preset {
  id: string;
  name: string;
  protocol: string;
  base_url: string;
  models: string[];
  default_model: string;
}

export interface Settings {
  theme: string;
  system_prompt: string;
  strip_thinking: boolean;
}

export interface Snapshot {
  settings: Settings;
  providers: Provider[];
  projects: Project[];
  sessions: Session[];
  messages: Message[];
}

export type MainTab = "chat" | "plugins" | "settings";

export type Theme = "dark" | "light";

export type StreamEvent =
  | { type: "delta"; text: string }
  | { type: "reasoning"; text: string }
  | { type: "reasoningend" }
  | { type: "textstart" }
  | {
      type: "approval";
      id: string;
      tool: string;
      summary: string;
      detail: string;
      kind: string;
    }
  | {
      type: "toolend";
      id: string;
      summary: string;
      detail: string;
      tool: string;
      ok: boolean;
    }
  | { type: "done" }
  | { type: "error"; message: string };
