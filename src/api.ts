import { invoke, Channel } from "@tauri-apps/api/core";
import type { Message, Preset, Project, Provider, Session, Settings, Snapshot, StreamEvent } from "./types";

export function loadAll(): Promise<Snapshot> {
  return invoke<Snapshot>("load_all");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

export function listPresets(): Promise<Preset[]> {
  return invoke<Preset[]>("list_presets");
}

export function addProvider(provider: Provider): Promise<Provider> {
  return invoke<Provider>("add_provider", { provider });
}

export function updateProvider(provider: Provider): Promise<void> {
  return invoke("update_provider", { provider });
}

export function deleteProvider(id: string): Promise<void> {
  return invoke("delete_provider", { id });
}

export function createProject(name: string, dirs: string[]): Promise<Project> {
  return invoke<Project>("create_project", { name, dirs });
}

export function updateProject(id: string, name: string, dirs: string[]): Promise<void> {
  return invoke("update_project", { id, name, dirs });
}

export function reorderProjects(ids: string[]): Promise<void> {
  return invoke("reorder_projects", { ids });
}

export function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

export function createSession(projectId: string): Promise<Session> {
  return invoke<Session>("create_session", { projectId });
}

export function updateSession(id: string, title: string): Promise<void> {
  return invoke("update_session", { id, title });
}

export function deleteSession(id: string): Promise<void> {
  return invoke("delete_session", { id });
}

export function getMessages(sessionId: string): Promise<Message[]> {
  return invoke<Message[]>("get_messages", { sessionId });
}

export function approve(id: string, allowed: boolean): Promise<boolean> {
  return invoke<boolean>("approve", { id, allowed });
}

export function estimateContext(
  sessionId: string,
  providerId: string,
  systemPrompt: string,
  stripThinking: boolean
): Promise<[number, number, [number, number] | null]> {
  return invoke<[number, number, [number, number] | null]>("estimate_context", {
    sessionId,
    providerId,
    systemPrompt,
    stripThinking,
  });
}

export function sendMessage(
  sessionId: string,
  providerId: string,
  content: string,
  onEvent: (event: StreamEvent) => void
): Promise<string> {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = onEvent;
  return invoke<string>("send_message", {
    sessionId,
    providerId,
    content,
    channel,
  });
}

export function cancelMessage(): Promise<void> {
  return invoke<void>("cancel_message");
}
