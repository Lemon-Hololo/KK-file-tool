/**
 * 后缀批量修改的命令封装。对应后端 `commands::suffix`。
 */

import { invokeCmd } from "./tauri";
import type {
  SuffixApplyResponse,
  SuffixPreviewItem,
  SuffixRecordDetail,
  SuffixRecordSummary,
  SuffixRollbackCheck,
  SuffixRollbackResponse
} from "../types/suffix";

/** 预览：按目标后缀计算每个文件的新路径。 */
export function previewSuffixChange(paths: string[], targetSuffix: string) {
  return invokeCmd<SuffixPreviewItem[]>("preview_suffix_change", { paths, targetSuffix });
}

/** 应用修改并写入记录。`selectedOldPaths` 为空时视为处理全部。 */
export function applySuffixChange(
  paths: string[],
  targetSuffix: string,
  recordName?: string | null,
  selectedOldPaths?: string[] | null
) {
  return invokeCmd<SuffixApplyResponse>("apply_suffix_change", {
    paths,
    targetSuffix,
    recordName: recordName || null,
    selectedOldPaths: selectedOldPaths || null
  });
}

/** 列出所有后缀修改记录。 */
export function listSuffixChangeRecords() {
  return invokeCmd<SuffixRecordSummary[]>("list_suffix_change_records");
}

/** 读取记录详情。 */
export function getSuffixChangeRecordDetail(recordId: string) {
  return invokeCmd<SuffixRecordDetail>("get_suffix_change_record_detail", { recordId });
}

/** 撤回前检查。 */
export function checkSuffixRollback(recordId: string, itemIds?: number[] | null) {
  return invokeCmd<SuffixRollbackCheck>("check_suffix_rollback", {
    recordId,
    itemIds: itemIds || null
  });
}

/** 执行撤回。 */
export function rollbackSuffixChange(
  recordId: string,
  itemIds?: number[] | null,
  forceIgnoreMissing = false
) {
  return invokeCmd<SuffixRollbackResponse>("rollback_suffix_change", {
    recordId,
    itemIds: itemIds || null,
    forceIgnoreMissing
  });
}

/** 删除记录（级联删除 item）。 */
export function deleteSuffixChangeRecord(recordId: string) {
  return invokeCmd<void>("delete_suffix_change_record", { recordId });
}
