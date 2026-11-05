// Única capa que habla con el backend Rust. Los componentes no importan
// @tauri-apps directamente.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Category, CopyField, ErrorInfo, Snapshot, StartLiveRequest } from "./types";

/** Normaliza cualquier error del backend a `{ code, category, message }`. */
export function toErrorInfo(err: unknown): ErrorInfo {
  if (err && typeof err === "object" && "message" in err) {
    const e = err as { code?: string; category?: string; message: string };
    return { code: e.code ?? "UNKNOWN", category: e.category ?? "app", message: e.message };
  }
  return { code: "UNKNOWN", category: "app", message: String(err) };
}

export const api = {
  getState: () => invoke<Snapshot>("get_state"),
  login: () => invoke<Snapshot>("auth_login"),
  cancelLogin: () => invoke<void>("auth_cancel"),
  logout: () => invoke<Snapshot>("auth_logout"),
  setRemember: (remember: boolean) => invoke<Snapshot>("set_remember_session", { remember }),
  searchCategories: (query: string) => invoke<Category[]>("categories_search", { query }),
  createStream: (request: StartLiveRequest) => invoke<Snapshot>("stream_create", { request }),
  endStream: () => invoke<Snapshot>("stream_end"),
  revealKey: () => invoke<string>("stream_reveal_key"),
  copy: (field: CopyField) => invoke<void>("copy_value", { field }),
};

export function onState(cb: (s: Snapshot) => void): Promise<UnlistenFn> {
  return listen<Snapshot>("app-state", (e) => cb(e.payload));
}
