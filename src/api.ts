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

export function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

export function createSession(projectId: string): Promise<Session> {
  return invoke<Session>("create_session", { projectId });
}

export function deleteSession(id: string): Promise<void> {
  return invoke("delete_session", { id });
}

export function getMessages(sessionId: string): Promise<Message[]> {
  return invoke<Message[]>("get_messages", { sessionId });
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
