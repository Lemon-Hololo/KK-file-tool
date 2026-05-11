/**
 * 图片相似度去重 DTO，复用 `types/opRecord` 的通用结构。
 *
 * 与 `modTools` 的差异：
 * - 单个图片元数据多了 `width / height / hash`（参与 keep 排序与展示）；
 * - `ImageDedupGroup` 多了 `similarity` 字段（百分比，0–100），用于按"相似度区间"
 *   过滤分组的滑块；
 * - kind 当前固定 `"similarity_delete"`，保留扩展位。
 */

import type {
  OpApplyResponse,
  OpRecordDetail,
  OpRecordSummary,
  OpRollbackCheck,
  OpRollbackResponse
} from "./opRecord";

/** 单张参与相似度比较的图片元数据 + 哈希。 */
export interface ImageHashFile {
  filePath: string;
  width: number;
  height: number;
  fileSize: number;
  mtime: number;
  ctime: number;
  /** base64 编码的感知哈希；前端不参与比较，只展示。 */
  hash: string;
  /** UI 本地状态：是否被勾选删除。后端不读此字段。 */
  selectedForDelete?: boolean;
}

/** 一组相似图片：`files[0]` 是按 keepPolicy 排好序后的默认 keep。 */
export interface ImageDedupGroup {
  groupId: string;
  /** 组内最差两两相似度（百分比，0–100）；用于 UI 滑块筛选下界。 */
  similarity: number;
  files: ImageHashFile[];
}

/** 扫描增量结果事件。 */
export interface ImageDedupPartialPayload {
  taskId: string;
  groups: ImageDedupGroup[];
  done: boolean;
}

export type ImageDedupKind = "similarity_delete" | string;
export type ImageDedupApplyResponse = OpApplyResponse & { kind: ImageDedupKind };
export type ImageDedupRecordSummary = OpRecordSummary<{ kind: ImageDedupKind }>;
export type ImageDedupRecordDetail = OpRecordDetail<{ kind: ImageDedupKind }>;
export type ImageDedupRollbackCheck = OpRollbackCheck;
export type ImageDedupRollbackResponse = OpRollbackResponse;
