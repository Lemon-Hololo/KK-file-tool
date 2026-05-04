//! 空文件夹清理相关 DTO。
//!
//! 删除时写入 `empty_dir_records/items`，撤回时按记录重新创建空目录。

use serde::{Deserialize, Serialize};

use super::mod_tools::ModOpApplyItem;

/// 空文件夹预览项（尚未执行）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirPreviewItem {
    pub old_path: String,
    pub new_path: String,
    /// 相对任务根目录的层级；根目录为 0。
    pub depth: usize,
}

/// 应用结果 item。`status` 取值 `"success"` / `"failed"`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirApplyItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String,
    pub message: Option<String>,
}

/// 空文件夹清理直接复用 [`super::mod_tools`] 的通用 apply item，把 `services::empty_dirs`
/// 里逐字段克隆的 `to_apply_item` 帮手压缩为 `Into::into`。
impl From<ModOpApplyItem> for EmptyDirApplyItem {
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

/// `apply_empty_dir_cleanup` 的返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirApplyResponse {
    pub record_id: String,
    pub record_name: String,
    pub kind: String,
    /// 空文件夹清理一律 `true`；保留字段以与通用 DTO 对齐。
    pub rollback_enabled: bool,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<EmptyDirApplyItem>,
}

/// 列表摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub kind: String,
    pub created_at: i64,
    pub total_items: usize,
    pub success_items: usize,
    /// `"applied"` / `"partially_rolled_back"` / `"rolled_back"`。
    pub rollback_status: String,
    /// 空文件夹清理一律 `true`；保留字段以与通用 DTO 对齐。
    pub rollback_enabled: bool,
}

/// 详情中的 item。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirRecordItem {
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
pub struct EmptyDirRecordDetail {
    pub summary: EmptyDirRecordSummary,
    pub items: Vec<EmptyDirRecordItem>,
}

/// 撤回前检查。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirRollbackCheck {
    pub total_selected: usize,
    pub existing_count: usize,
    pub missing_paths: Vec<String>,
}

/// 撤回结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDirRollbackResponse {
    pub record_id: String,
    pub total_selected: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped_missing: usize,
    pub items: Vec<EmptyDirApplyItem>,
}
