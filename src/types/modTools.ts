/**
 * Mod 工具 DTO，复用 `types/opRecord` 的通用结构。
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

/** Mod 重命名预览；`warn` 非空表示此条不参与应用。 */
export interface ModRenamePreviewItem {
  oldPath: string;
  newPath: string;
  guid: string;
  version: string;
  author: string;
  willRenameConflict: boolean;
  warn?: string | null;
}

/** 归类预览：目标子目录名 + 是否冲突。 */
export interface ModOrganizePreviewItem {
  oldPath: string;
  newPath: string;
  folderName: string;
  willConflict: boolean;
}

export type ModOpApplyItem = OpApplyItem;
export type ModOpApplyResponse = OpApplyResponse & { kind: "rename" | "organize" | string };
export type ModOpRecordItem = OpRecordItem;
export type ModOpRollbackCheck = OpRollbackCheck;
export type ModOpRollbackResponse = OpRollbackResponse;

/** 摘要额外字段：操作类型。 */
export type ModOpRecordSummary = OpRecordSummary<{ kind: "rename" | "organize" | string }>;
export type ModOpRecordDetail = OpRecordDetail<{ kind: "rename" | "organize" | string }>;

/** 扫描阶段单条匹配。 */
export interface ModScanMatch {
  filePath: string;
  guid: string;
  version: string;
  author: string;
  matchedKeyword: string;
}

/** 扫描完成事件的 payload。 */
export interface ModScanCompletedPayload {
  taskId: string;
  keyword: string;
  matches: ModScanMatch[];
  totalScanned: number;
  totalErrors: number;
  cancelled: boolean;
}
