/**
 * 通用"可撤回操作记录"类型（后端 `db::op_record_repo` + `services::op_pipeline`
 * 的前端映射）。后缀修改与 Mod 工具共享这些结构，业务差异由 `Extra` 泛型
 * 参数承载。
 */

/** 单条 apply 结果 item。 */
export interface OpApplyItem {
  itemId: number;
  oldPath: string;
  newPath: string;
  status: "success" | "failed";
  message?: string | null;
}

/** apply 的聚合返回。 */
export interface OpApplyResponse {
  recordId: string;
  recordName: string;
  total: number;
  success: number;
  failed: number;
  items: OpApplyItem[];
}

/** 记录列表摘要，`Extra` 承载业务差异字段（如 `targetSuffix` / `kind`）。 */
export type OpRecordSummary<Extra = Record<string, never>> = {
  recordId: string;
  recordName: string;
  createdAt: number;
  totalItems: number;
  successItems: number;
  /** `"applied"` / `"partially_rolled_back"` / `"rolled_back"`。 */
  rollbackStatus: string;
} & Extra;

/** 记录详情里的单条 item。 */
export interface OpRecordItem {
  itemId: number;
  oldPath: string;
  newPath: string;
  applySuccess: boolean;
  applyError?: string | null;
  rollbackSuccess?: boolean | null;
  rollbackError?: string | null;
}

/** 记录详情 = 摘要 + 全部 item。 */
export interface OpRecordDetail<Extra = Record<string, never>> {
  summary: OpRecordSummary<Extra>;
  items: OpRecordItem[];
}

/** 撤回前检查：仍存在 / 缺失的统计。 */
export interface OpRollbackCheck {
  totalSelected: number;
  existingCount: number;
  missingPaths: string[];
}

/** 撤回结果。 */
export interface OpRollbackResponse {
  recordId: string;
  totalSelected: number;
  success: number;
  failed: number;
  skippedMissing: number;
  items: OpApplyItem[];
}
