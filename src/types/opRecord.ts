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
  /**
   * 创建记录时是否启用回滚。
   *
   * 目前只有 Mod 工具的备份型操作（重复删除 / 不同版本删除 / 移除版本限制）
   * 在用户关闭"启用 Mod 操作回滚"设置时会写入 `false`，记录管理页 / 面板里
   * 的"撤回"按钮应据此置灰。其它业务（后缀 / 空目录清理 / Mod 重命名 /
   * Mod 归类）一律 `true`。
   */
  rollbackEnabled: boolean;
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
  /** 创建记录时是否启用回滚；`false` 时该记录的"撤回"按钮置灰。 */
  rollbackEnabled: boolean;
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
