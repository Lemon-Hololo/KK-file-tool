/**
 * 空文件夹清理的命令封装。对应后端 `commands::empty_dirs`。
 */

import { invokeCmd } from "./tauri";
import type {
  EmptyDirApplyResponse,
  EmptyDirPreviewItem,
  EmptyDirRecordDetail,
  EmptyDirRecordSummary,
  EmptyDirRollbackCheck,
  EmptyDirRollbackResponse
} from "../types/emptyDirs";

/** 预览递归清理时可以删除的空文件夹。 */
export function previewEmptyDirs(paths: string[], includeRoots: boolean) {
  return invokeCmd<EmptyDirPreviewItem[]>("preview_empty_dirs", { paths, includeRoots });
}

/** 删除空文件夹并写入可撤回记录。 */
export function applyEmptyDirCleanup(
  paths: string[],
  includeRoots: boolean,
  recordName?: string | null,
  selectedOldPaths?: string[] | null
) {
  return invokeCmd<EmptyDirApplyResponse>("apply_empty_dir_cleanup", {
    paths,
    includeRoots,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null
  });
}

/** 列出全部空文件夹清理记录。 */
export function listEmptyDirRecords() {
  return invokeCmd<EmptyDirRecordSummary[]>("list_empty_dir_records");
}

/** 读取记录详情。 */
export function getEmptyDirRecordDetail(recordId: string) {
  return invokeCmd<EmptyDirRecordDetail>("get_empty_dir_record_detail", { recordId });
}

/** 撤回前检查。 */
export function checkEmptyDirRollback(recordId: string, itemIds?: number[] | null) {
  return invokeCmd<EmptyDirRollbackCheck>("check_empty_dir_rollback", {
    recordId,
    itemIds: itemIds || null
  });
}

/** 执行撤回，重新创建记录中的目录。 */
export function rollbackEmptyDirCleanup(
  recordId: string,
  itemIds?: number[] | null,
  forceIgnoreMissing = false
) {
  return invokeCmd<EmptyDirRollbackResponse>("rollback_empty_dir_cleanup", {
    recordId,
    itemIds: itemIds || null,
    forceIgnoreMissing
  });
}

/** 删除记录（级联删除 item）。 */
export function deleteEmptyDirRecord(recordId: string) {
  return invokeCmd<void>("delete_empty_dir_record", { recordId });
}
