/**
 * 后缀批量修改的 DTO，复用 `types/opRecord` 的通用结构。
 *
 * 保留 `Suffix*` 前缀是为了让调用点显式知道自己在处理哪类业务。
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

/** 预览项：旧路径 / 新路径 / 冲突标记。 */
export interface SuffixPreviewItem {
  oldPath: string;
  newPath: string;
  willRenameConflict: boolean;
}

export type SuffixApplyItem = OpApplyItem;
export type SuffixApplyResponse = OpApplyResponse;
export type SuffixRecordItem = OpRecordItem;
export type SuffixRollbackCheck = OpRollbackCheck;
export type SuffixRollbackResponse = OpRollbackResponse;

/** 摘要额外字段：目标后缀。 */
export type SuffixRecordSummary = OpRecordSummary<{ targetSuffix: string }>;
export type SuffixRecordDetail = OpRecordDetail<{ targetSuffix: string }>;
