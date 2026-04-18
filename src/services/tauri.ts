/**
 * Tauri IPC 基础封装。
 *
 * 所有业务 service 模块统一通过 `invokeCmd` 调用后端命令，通过 `onEvent`
 * 订阅后端发射的事件，避免散落的 `invoke` / `listen` 直调。
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** 调用后端 `#[tauri::command]`。参数键名使用驼峰，由 serde 自动对应 snake_case。 */
export async function invokeCmd<T = unknown>(cmd: string, payload?: Record<string, unknown>) {
  return invoke<T>(cmd, payload);
}

/** 订阅后端发射的事件；返回 unlisten 函数。 */
export async function onEvent<T = unknown>(
  eventName: string,
  cb: (payload: T) => void
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => cb(event.payload));
}
