import { invokeCmd } from "./tauri";
import type {
  SuffixApplyResponse,
  SuffixPreviewItem,
  SuffixRecordDetail,
  SuffixRecordSummary,
  SuffixRollbackCheck,
  SuffixRollbackResponse
} from "../types/suffix";

export function previewSuffixChange(paths: string[], targetSuffix: string) {
  return invokeCmd<SuffixPreviewItem[]>("preview_suffix_change", { paths, targetSuffix });
}

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

export function listSuffixChangeRecords() {
  return invokeCmd<SuffixRecordSummary[]>("list_suffix_change_records");
}

export function getSuffixChangeRecordDetail(recordId: string) {
  return invokeCmd<SuffixRecordDetail>("get_suffix_change_record_detail", { recordId });
}

export function checkSuffixRollback(recordId: string, itemIds?: number[] | null) {
  return invokeCmd<SuffixRollbackCheck>("check_suffix_rollback", {
    recordId,
    itemIds: itemIds || null
  });
}

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

export function deleteSuffixChangeRecord(recordId: string) {
  return invokeCmd<void>("delete_suffix_change_record", { recordId });
}
