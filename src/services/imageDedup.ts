/**
 * 图片相似度去重的命令封装；对应后端 `commands::image_dedup`。
 */

import { invokeCmd, onEvent } from "./tauri";
import type {
  ImageDedupApplyResponse,
  ImageDedupPartialPayload,
  ImageDedupRecordDetail,
  ImageDedupRecordSummary,
  ImageDedupRollbackCheck,
  ImageDedupRollbackResponse
} from "../types/imageDedup";

/** 启动图片相似度扫描长任务，结果通过 `image_dedup_partial` 事件增量推送。 */
export function startImageDedupTask(paths: string[], taskId?: string | null) {
  return invokeCmd<string>("start_image_dedup_task", { paths, taskId: taskId || null });
}

/** 删除选中的相似图片（前端已排除 keep），写入可撤回记录。 */
export function applyImageDedupDelete(
  paths: string[],
  selectedFilePaths: string[],
  recordName?: string | null,
  taskId?: string | null
) {
  return invokeCmd<ImageDedupApplyResponse>("apply_image_dedup_delete", {
    paths,
    selectedFilePaths,
    recordName: recordName || null,
    taskId: taskId || null
  });
}

/** 列出图片去重记录。 */
export function listImageDedupRecords() {
  return invokeCmd<ImageDedupRecordSummary[]>("list_image_dedup_records", {});
}

/** 读取记录详情。 */
export function getImageDedupRecordDetail(recordId: string) {
  return invokeCmd<ImageDedupRecordDetail>("get_image_dedup_record_detail", { recordId });
}

/** 撤回前检查。 */
export function checkImageDedupRollback(recordId: string, itemIds?: number[] | null) {
  return invokeCmd<ImageDedupRollbackCheck>("check_image_dedup_rollback", {
    recordId,
    itemIds: itemIds || null
  });
}

/** 执行撤回。 */
export function rollbackImageDedup(
  recordId: string,
  itemIds?: number[] | null,
  forceIgnoreMissing = false
) {
  return invokeCmd<ImageDedupRollbackResponse>("rollback_image_dedup", {
    recordId,
    itemIds: itemIds || null,
    forceIgnoreMissing
  });
}

/** 删除单条记录。 */
export function deleteImageDedupRecord(recordId: string) {
  return invokeCmd<void>("delete_image_dedup_record", { recordId });
}

/** 重命名单条记录。 */
export function renameImageDedupRecord(recordId: string, newName: string) {
  return invokeCmd<void>("rename_image_dedup_record", { recordId, newName });
}

/** 订阅图片相似度扫描的增量结果事件。 */
export function onImageDedupPartial(cb: (p: ImageDedupPartialPayload) => void) {
  return onEvent<ImageDedupPartialPayload>("image_dedup_partial", cb);
}
