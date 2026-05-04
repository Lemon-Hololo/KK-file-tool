//! 后缀批量修改相关 DTO。
//!
//! 字段结构与 [`super::mod_tools`] 的 Mod 操作模型高度相似——两者都是
//! "以 `(old_path, new_path)` 为核心的可撤回记录"。两者底层共用
//! [`crate::db::op_record_repo`] / [`crate::services::op_pipeline`]，但
//! 为了让前端显式知道"这条记录属于哪类业务"，模型仍保留两套独立命名。

use serde::{Deserialize, Serialize};

use super::mod_tools::ModOpApplyItem;

/// 单条预览项（尚未执行）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixPreviewItem {
    pub old_path: String,
    pub new_path: String,
    /// 是否因同名冲突被自动加了 ` (N)` 后缀。
    pub will_rename_conflict: bool,
}

/// 应用结果 item。`status` 取值 `"success"` / `"failed"`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixApplyItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String,
    pub message: Option<String>,
}

/// 后缀业务直接复用 [`super::mod_tools`] 的通用 apply item，避免 `services::suffix`
/// 在响应映射阶段写一份逐字段克隆——两者结构完全一致，仅字段名相同的复制粘贴。
impl From<ModOpApplyItem> for SuffixApplyItem {
    fn from(value: ModOpApplyItem) -> Self {
        Self {
            item_id: value.item_id,
            old_path: value.old_path,
            new_path: value.new_path,
            status: value.status,
            message: value.message,
        }
    }
}

/// `apply_suffix_change` 的返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixApplyResponse {
    pub record_id: String,
    pub record_name: String,
    /// 后缀业务一律 `true`（不参与 Mod 工具回滚开关）；保留字段以与通用 DTO 对齐。
    pub rollback_enabled: bool,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<SuffixApplyItem>,
}

/// 列表摘要。`rollback_status` 取值 `"applied"` / `"partially_rolled_back"` / `"rolled_back"`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub target_suffix: String,
    pub created_at: i64,
    pub total_items: usize,
    pub success_items: usize,
    pub rollback_status: String,
    /// 后缀业务一律 `true`；保留字段以与通用 DTO 对齐。
    pub rollback_enabled: bool,
}

/// 详情中的 item。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRecordItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub apply_success: bool,
    pub apply_error: Option<String>,
    pub rollback_success: Option<bool>,
    pub rollback_error: Option<String>,
}

/// 详情 = 摘要 + 全部 item。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRecordDetail {
    pub summary: SuffixRecordSummary,
    pub items: Vec<SuffixRecordItem>,
}

/// 撤回前检查：`new_path` 仍存在 / 缺失的统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRollbackCheck {
    pub total_selected: usize,
    pub existing_count: usize,
    pub missing_paths: Vec<String>,
}

/// 撤回结果。`skipped_missing` 是执行过程中被发现缺失而跳过的条目数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRollbackResponse {
    pub record_id: String,
    pub total_selected: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped_missing: usize,
    pub items: Vec<SuffixApplyItem>,
}
