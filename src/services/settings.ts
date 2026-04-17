import type { AppSettings, DbPathInfo } from "../types/settings";
import { invokeCmd } from "./tauri";

export async function getSettings() {
  return invokeCmd<AppSettings>("get_settings");
}

export async function saveSettings(settings: AppSettings) {
  return invokeCmd<void>("save_settings", { settings });
}

export async function setThemeMode(mode: AppSettings["themeMode"]) {
  return invokeCmd<void>("set_theme_mode", { mode });
}

export async function getDbInfo() {
  return invokeCmd<DbPathInfo>("get_db_info");
}

export async function setCustomDbPath(path: string) {
  return invokeCmd<void>("set_custom_db_path", { path });
}

export async function deleteDatabase() {
  return invokeCmd<void>("delete_database");
}

export async function getCpuCount() {
  return invokeCmd<number>("get_cpu_count");
}
