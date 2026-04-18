/** 应用设置与数据库路径管理的命令封装。对应后端 `commands::settings`。 */

import type { AppSettings, DbPathInfo } from "../types/settings";
import { invokeCmd } from "./tauri";

/** 读取用户设置。 */
export async function getSettings() {
  return invokeCmd<AppSettings>("get_settings");
}

/** 保存用户设置。 */
export async function saveSettings(settings: AppSettings) {
  return invokeCmd<void>("save_settings", { settings });
}

/** 单独更新主题模式。 */
export async function setThemeMode(mode: AppSettings["themeMode"]) {
  return invokeCmd<void>("set_theme_mode", { mode });
}

/** 返回当前 / 默认 / 自定义三类数据库路径信息。 */
export async function getDbInfo() {
  return invokeCmd<DbPathInfo>("get_db_info");
}

/** 设置自定义数据库路径（下次启动生效）。 */
export async function setCustomDbPath(path: string) {
  return invokeCmd<void>("set_custom_db_path", { path });
}

/** 删除数据库文件（含 WAL / SHM）并重建 schema。 */
export async function deleteDatabase() {
  return invokeCmd<void>("delete_database");
}

/** 返回 CPU 核心数，供前端限制"线程数"输入上限。 */
export async function getCpuCount() {
  return invokeCmd<number>("get_cpu_count");
}
