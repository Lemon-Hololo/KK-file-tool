/**
 * Mod 工具（rename / organize / cleanup / modify / scan）的命令封装。
 * 对应后端 `commands::mod_tools`。
 */

import { invokeCmd, onEvent } from "./tauri";
import type {
  ModDuplicateGroup,
  ModDuplicatePartialPayload,
  ModOpApplyResponse,
  ModOpRecordDetail,
  ModOpRecordSummary,
  ModOpRollbackCheck,
  ModOpRollbackResponse,
  ModOrganizePreviewItem,
  ModRenamePreviewItem,
  ModScanCompletedPayload,
  ModVersionGroup,
  ModVersionPartialPayload
} from "../types/modTools";

/** 预览 Mod 重命名。 */
export function previewModRename(paths: string[], taskId?: string | null) {
  return invokeCmd<ModRenamePreviewItem[]>("preview_mod_rename", { paths, taskId: taskId || null });
}

/** 应用 Mod 重命名。 */
export function applyModRename(
  paths: string[],
  recordName?: string | null,
  selectedOldPaths?: string[] | null,
  taskId?: string | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_rename", {
    paths,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null,
    taskId: taskId || null
  });
}

/** 预览按 `[...]` 括号归类。 */
export function previewModOrganize(paths: string[], taskId?: string | null) {
  return invokeCmd<ModOrganizePreviewItem[]>("preview_mod_organize", { paths, taskId: taskId || null });
}

/** 应用归类。 */
export function applyModOrganize(
  paths: string[],
  recordName?: string | null,
  selectedOldPaths?: string[] | null,
  taskId?: string | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_organize", {
    paths,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null,
    taskId: taskId || null
  });
}

/** 预览 `guid + author + version` 完全相同的重复 MOD。 */
export function previewModDuplicates(paths: string[], taskId?: string | null) {
  return invokeCmd<ModDuplicateGroup[]>("preview_mod_duplicates", { paths, taskId: taskId || null });
}

/** 启动重复 MOD 检查长任务；结果通过事件增量推送。 */
export function startModDuplicateTask(paths: string[], taskId?: string | null) {
  return invokeCmd<string>("start_mod_duplicate_task", { paths, taskId: taskId || null });
}

/** 删除重复 MOD 中选中的文件，写入可撤回 Mod 操作记录。 */
export function applyModDuplicateDelete(
  paths: string[],
  selectedFilePaths: string[],
  recordName?: string | null,
  taskId?: string | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_duplicate_delete", {
    paths,
    selectedFilePaths,
    recordName: recordName || null,
    taskId: taskId || null
  });
}

/** 预览 `guid + author` 相同但版本不同的 MOD。 */
export function previewModVersions(paths: string[], taskId?: string | null) {
  return invokeCmd<ModVersionGroup[]>("preview_mod_versions", { paths, taskId: taskId || null });
}

/** 启动不同版本 MOD 检查长任务；结果通过事件增量推送。 */
export function startModVersionTask(paths: string[], taskId?: string | null) {
  return invokeCmd<string>("start_mod_version_task", { paths, taskId: taskId || null });
}

/** 删除不同版本 MOD 中选中的文件，写入可撤回 Mod 操作记录。 */
export function applyModVersionDelete(
  paths: string[],
  selectedFilePaths: string[],
  recordName?: string | null,
  taskId?: string | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_version_delete", {
    paths,
    selectedFilePaths,
    recordName: recordName || null,
    taskId: taskId || null
  });
}

/** 列出 Mod 操作记录；`kind` 可为 `"rename"` / `"organize"` / `"modify"`。 */
export function listModOpRecords(kind?: string | null) {
  return invokeCmd<ModOpRecordSummary[]>("list_mod_op_records", { kind: kind || null });
}

/** 读取记录详情。 */
export function getModOpRecordDetail(recordId: string) {
  return invokeCmd<ModOpRecordDetail>("get_mod_op_record_detail", { recordId });
}

/** 撤回前检查。 */
export function checkModOpRollback(recordId: string, itemIds?: number[] | null) {
  return invokeCmd<ModOpRollbackCheck>("check_mod_op_rollback", {
    recordId,
    itemIds: itemIds || null
  });
}

/** 执行撤回。 */
export function rollbackModOp(
  recordId: string,
  itemIds?: number[] | null,
  forceIgnoreMissing = false
) {
  return invokeCmd<ModOpRollbackResponse>("rollback_mod_op", {
    recordId,
    itemIds: itemIds || null,
    forceIgnoreMissing
  });
}

/** 删除记录。 */
export function deleteModOpRecord(recordId: string) {
  return invokeCmd<void>("delete_mod_op_record", { recordId });
}

/** 重命名记录。 */
export function renameModOpRecord(recordId: string, newName: string) {
  return invokeCmd<void>("rename_mod_op_record", { recordId, newName });
}

/** 启动扫描长任务；返回 `task_id`，取消通过通用 `stop_task`。 */
export function startModScanTask(paths: string[], keyword: string, taskId?: string | null) {
  return invokeCmd<string>("start_mod_scan_task", { paths, keyword, taskId: taskId || null });
}

/** 订阅扫描完成事件。 */
export function onModScanCompleted(cb: (p: ModScanCompletedPayload) => void) {
  return onEvent<ModScanCompletedPayload>("mod_scan_completed", cb);
}

/** 订阅重复 MOD 检查的增量结果事件。 */
export function onModDuplicatePartial(cb: (p: ModDuplicatePartialPayload) => void) {
  return onEvent<ModDuplicatePartialPayload>("mod_duplicate_partial", cb);
}

/** 订阅不同版本 MOD 检查的增量结果事件。 */
export function onModVersionPartial(cb: (p: ModVersionPartialPayload) => void) {
  return onEvent<ModVersionPartialPayload>("mod_version_partial", cb);
}

/**
 * 对选中的 `.zipmod` 文件应用"移除 `<game>KEYWORD</game>`"修改，
 * 备份原文件并把修改写入 Mod 操作记录（`kind = "modify"`，可撤回）。
 */
export function applyModModifyVersion(
  paths: string[],
  keyword: string,
  selectedFilePaths: string[],
  recordName?: string | null,
  taskId?: string | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_modify_version", {
    paths,
    keyword,
    selectedFilePaths,
    recordName: recordName || null,
    taskId: taskId || null
  });
}
