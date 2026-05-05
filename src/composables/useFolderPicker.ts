/**
 * 系统目录选择对话框的薄包装。
 *
 * SettingsPage（自定义 db / 移动目标 / Mod 备份）、TaskPage（任务输入）、
 * PixivTagPanel（输出目录）原本各自写一份 `try { open(...) } catch { ElMessage.error }`，
 * 这里统一收口。
 */

import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";

export interface PickFolderOptions {
  /** 对话框标题。 */
  title: string;
  /** 是否允许多选；默认 false。 */
  multiple?: boolean;
}

/** 单选目录；用户取消或选择失败返回 null。失败会自动 toast，不向上抛错。 */
export async function pickFolder(title: string): Promise<string | null> {
  try {
    const selected = await open({ directory: true, multiple: false, title });
    if (typeof selected === "string" && selected) return selected;
    return null;
  } catch (e) {
    ElMessage.error(`打开目录选择失败：${String(e)}`);
    return null;
  }
}

/** 多选目录；返回字符串数组（取消 / 失败时返回空数组）。 */
export async function pickFolders(title: string): Promise<string[]> {
  try {
    const selected = await open({ directory: true, multiple: true, title });
    if (!selected) return [];
    const arr = Array.isArray(selected) ? selected : [selected];
    return arr.filter((x): x is string => typeof x === "string");
  } catch (e) {
    ElMessage.error(`打开目录选择失败：${String(e)}`);
    return [];
  }
}
