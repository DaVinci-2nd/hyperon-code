export type Role = "user" | "assistant";

export interface Part {
  kind: string;
  text: string;
  summary: string;
  detail: string;
  ok: boolean;
  tool_name?: string;
  tool_args?: string;
  children?: Part[];
  running?: boolean;
}

export interface Attachment {
  id: string;
  name: string;
  file: string;
  mime: string;
  size: number;
  inline: boolean;
}

export interface Message {
  id: string;
  session_id: string;
  role: Role;
  content: string;
  reasoning: string;
  parts: Part[];
  attachments?: Attachment[];
  created_at: number;
  model: string;
  elapsed_ms: number;
  output_ms: number;
  output_tokens: number;
}

export interface SendResult {
  user: Message;
  assistant: Message;
}

export interface Session {
  id: string;
  project_id: string;
  title: string;
  created_at: number;
  tools?: SessionTools;
}

export interface PromptInject {
  enabled: boolean;
  content: string;
  position: string;
}

export interface SessionTools {
  prompt_inject: PromptInject;
}

export interface ChangeSummary {
  id: string;
  message_id: string;
  path: string;
  existed: boolean;
  known: boolean;
  created_at: number;
}

export interface ChangeDiff {
  path: string;
  existed: boolean;
  known: boolean;
  diff: string;
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

export interface ModelSpec {
  id: string;
  thinking: string[];
  kinds: string[];
}

export interface ModelInfo {
  thinking: string[];
  kinds: string[];
}

export interface Preset {
  id: string;
  name: string;
  protocol: string;
  base_url: string;
  models: ModelSpec[];
  default_model: string;
}

export interface Settings {
  theme: string;
  system_prompt: string;
  subagent_prompt: string;
  strip_thinking: boolean;
  strip_duplicate_tool_results: boolean;
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
  | { type: "approvalstart"; id: string; tool: string; kind: string }
  | { type: "approvaldelta"; id: string; summary: string; append: string }
  | {
      type: "approval";
      id: string;
      tool: string;
      summary: string;
      detail: string;
      kind: string;
    }
  | {
      type: "toolstart";
      id: string;
      tool: string;
      summary: string;
    }
  | { type: "tooldelta"; id: string; text: string }
  | {
      type: "toolend";
      id: string;
      summary: string;
      detail: string;
      tool: string;
      ok: boolean;
    }
  | {
      type: "usage";
      prompt_tokens: number;
      cached_tokens: number;
      context_window: number;
    }
  | { type: "subagentstart"; id: string; title: string }
  | { type: "subagentreasoning"; id: string; text: string }
  | { type: "subagentdelta"; id: string; text: string }
  | {
      type: "subagenttool";
      id: string;
      summary: string;
      detail: string;
      ok: boolean;
    }
  | { type: "subagentend"; id: string; ok: boolean }
  | { type: "done" }
  | { type: "error"; message: string };

export interface UsageStat {
  used: number;
  window: number;
  cached: number;
  hitRate: number | null;
}

export interface RollbackFile {
  kind: string;
  path: string;
  action: string;
  restorable: boolean;
  reason: string;
}

export interface RollbackPlan {
  messages: number;
  files: RollbackFile[];
  blocked: string[];
  restorable: boolean;
}

export interface RollbackReport {
  removed: number;
  restored: string[];
  deleted: string[];
  skipped: string[];
  failed: string[];
}
