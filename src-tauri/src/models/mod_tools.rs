//! Mod 工具相关 DTO。
//!
//! 结构与 [`super::suffix`] 相似，区别在于：
//! - 记录携带 `kind`（`"rename"` / `"organize"`）区分业务；
//! - 重命名预览额外暴露 `guid` / `version` / `author` 三个解析字段；
//! - 归类预览带 `folder_name`；
//! - 扫描任务有独立的 `ModScanMatch` / `ModScanCompletedPayload`。

use serde::{Deserialize, Serialize};

/// 重命名预览的单条。`warn` 非空表示该文件无法处理（如 manifest 读失败），前端应提示。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRenamePreviewItem {
    pub old_path: String,
    pub new_path: String,
    pub guid: String,
    pub version: String,
    pub author: String,
    pub will_rename_conflict: bool,
    pub warn: Option<String>,
}

/// 归类预览的单条。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOrganizePreviewItem {
    pub old_path: String,
    pub new_path: String,
    /// 目标子目录名（来自文件名首个 `[...]`）。
    pub folder_name: String,
    pub will_conflict: bool,
}

/// 应用结果 item。`status` 取值 `"success"` / `"failed"`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpApplyItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String,
    pub message: Option<String>,
}

/// `apply_mod_rename` / `apply_mod_organize` 的返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpApplyResponse {
    pub record_id: String,
    pub record_name: String,
    /// `"rename"` 或 `"organize"`（见 `constants::mod_op_kind`）。
    pub kind: String,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<ModOpApplyItem>,
}

/// 列表摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub kind: String,
    pub created_at: i64,
    pub total_items: usize,
    pub success_items: usize,
    /// `"applied"` / `"partially_rolled_back"` / `"rolled_back"`。
    pub rollback_status: String,
}

/// 详情中的 item。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpRecordItem {
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
pub struct ModOpRecordDetail {
    pub summary: ModOpRecordSummary,
    pub items: Vec<ModOpRecordItem>,
}

/// 撤回前检查。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpRollbackCheck {
    pub total_selected: usize,
    pub existing_count: usize,
    pub missing_paths: Vec<String>,
}

/// 撤回结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOpRollbackResponse {
    pub record_id: String,
    pub total_selected: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped_missing: usize,
    pub items: Vec<ModOpApplyItem>,
}

/// 扫描阶段单条匹配。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModScanMatch {
    pub file_path: String,
    pub guid: String,
    pub version: String,
    pub author: String,
    pub matched_keyword: String,
}

/// 扫描完成事件 `mod_scan_completed` 的 payload。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModScanCompletedPayload {
    pub task_id: String,
    pub keyword: String,
    pub matches: Vec<ModScanMatch>,
    pub total_scanned: usize,
    pub total_errors: usize,
    pub cancelled: bool,
}
