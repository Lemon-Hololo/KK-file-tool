import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export async function invokeCmd<T = any>(cmd: string, payload?: Record<string, unknown>) {
  return invoke<T>(cmd, payload);
}

export async function onEvent<T = any>(
  eventName: string,
  cb: (payload: T) => void
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => cb(event.payload));
}