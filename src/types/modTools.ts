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

/** 重复 / 不同版本检查中的单个 Mod 文件。 */
export interface ModIdentityFile {
  filePath: string;
  guid: string;
  version: string;
  author: string;
  size: number;
  mtime: number;
  ctime: number;
  selectedForDelete?: boolean;
}

/** `guid + author + version` 完全相同的重复 MOD 分组。 */
export interface ModDuplicateGroup {
  groupId: string;
  guid: string;
  author: string;
  version: string;
  files: ModIdentityFile[];
}

/** 重复 MOD 检查的增量结果事件。 */
export interface ModDuplicatePartialPayload {
  taskId: string;
  groups: ModDuplicateGroup[];
  done: boolean;
}

/** `guid + author` 相同但版本不同的 MOD 分组。 */
export interface ModVersionGroup {
  groupId: string;
  guid: string;
  author: string;
  latestVersion: string;
  files: ModIdentityFile[];
}

/** 不同版本 MOD 检查的增量结果事件。 */
export interface ModVersionPartialPayload {
  taskId: string;
  groups: ModVersionGroup[];
  done: boolean;
}

export type ModOpApplyItem = OpApplyItem;
export type ModOpKind =
  | "rename"
  | "organize"
  | "modify"
  | "duplicate_delete"
  | "version_delete"
  | string;
export type ModOpApplyResponse = OpApplyResponse & { kind: ModOpKind };
export type ModOpRecordItem = OpRecordItem;
export type ModOpRollbackCheck = OpRollbackCheck;
export type ModOpRollbackResponse = OpRollbackResponse;

/** 摘要额外字段：操作类型。 */
export type ModOpRecordSummary = OpRecordSummary<{ kind: ModOpKind }>;
export type ModOpRecordDetail = OpRecordDetail<{ kind: ModOpKind }>;

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
