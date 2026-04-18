/**
 * Mod 工具（rename / organize / scan）的命令封装。对应后端 `commands::mod_tools`。
 */

import { invokeCmd, onEvent } from "./tauri";
import type {
  ModOpApplyResponse,
  ModOpRecordDetail,
  ModOpRecordSummary,
  ModOpRollbackCheck,
  ModOpRollbackResponse,
  ModOrganizePreviewItem,
  ModRenamePreviewItem,
  ModScanCompletedPayload
} from "../types/modTools";

/** 预览 Mod 重命名。 */
export function previewModRename(paths: string[]) {
  return invokeCmd<ModRenamePreviewItem[]>("preview_mod_rename", { paths });
}

/** 应用 Mod 重命名。 */
export function applyModRename(
  paths: string[],
  recordName?: string | null,
  selectedOldPaths?: string[] | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_rename", {
    paths,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null
  });
}

/** 预览按 `[...]` 括号归类。 */
export function previewModOrganize(paths: string[]) {
  return invokeCmd<ModOrganizePreviewItem[]>("preview_mod_organize", { paths });
}

/** 应用归类。 */
export function applyModOrganize(
  paths: string[],
  recordName?: string | null,
  selectedOldPaths?: string[] | null
) {
  return invokeCmd<ModOpApplyResponse>("apply_mod_organize", {
    paths,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null
  });
}

/** 列出 Mod 操作记录；`kind` 可为 `"rename"` / `"organize"`。 */
export function listModOpRecords(kind?: "rename" | "organize" | null) {
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
export function startModScanTask(paths: string[], keyword: string) {
  return invokeCmd<string>("start_mod_scan_task", { paths, keyword });
}

/** 订阅扫描完成事件。 */
export function onModScanCompleted(cb: (p: ModScanCompletedPayload) => void) {
  return onEvent<ModScanCompletedPayload>("mod_scan_completed", cb);
}

/** 把扫描结果导出为 UTF-8 文本（CRLF 换行）。 */
export function exportModScanResult(targetPath: string, lines: string[]) {
  return invokeCmd<void>("export_mod_scan_result", { targetPath, lines });
}
