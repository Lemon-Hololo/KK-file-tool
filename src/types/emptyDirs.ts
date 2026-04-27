/**
 * 空文件夹清理 DTO，复用 `types/opRecord` 的通用可撤回记录结构。
 */

import type {
  OpApplyItem,
  OpApplyResponse,
  OpRecordDetail,
  OpRecordItem,
  OpRecordSummary,
  OpRollbackCheck,
  OpRollbackResponse
} from "./opRecord";

/** 空文件夹预览项。 */
export interface EmptyDirPreviewItem {
  oldPath: string;
  newPath: string;
  depth: number;
}

export type EmptyDirApplyItem = OpApplyItem;
export type EmptyDirApplyResponse = OpApplyResponse & { kind: "delete" | string };
export type EmptyDirRecordItem = OpRecordItem;
export type EmptyDirRollbackCheck = OpRollbackCheck;
export type EmptyDirRollbackResponse = OpRollbackResponse;

/** 摘要额外字段：操作类型。 */
export type EmptyDirRecordSummary = OpRecordSummary<{ kind: "delete" | string }>;
export type EmptyDirRecordDetail = OpRecordDetail<{ kind: "delete" | string }>;
