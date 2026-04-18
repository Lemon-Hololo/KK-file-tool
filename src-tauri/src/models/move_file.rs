//! 去重后"移动重复文件"相关模型：汇总、成功/失败条目、最终报告。

use serde::{Deserialize, Serialize};

use super::task::DuplicateGroup;

/// 移动前置计算：目标目录、选中文件数、总字节数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSummary {
    pub target_dir: String,
    pub total_selected: usize,
    pub total_size: u64,
}

/// 成功移动的单条记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSuccessItem {
    pub src_path: String,
    pub dst_path: String,
    pub size: u64,
}

/// 移动失败的单条记录。`error_code` 为稳定的短串（如 `NOT_FOUND`），
/// 前端用于本地化；`error_message` 是人类可读的补充。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFailureItem {
    pub src_path: String,
    pub error_code: String,
    pub error_message: String,
}

/// 一次移动任务的完整报告（持久化到 `move_reports` 表，事件也会携带同结构）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveReport {
    pub report_id: String,
    pub task_id: String,
    pub created_at: i64,
    pub target_dir: String,
    pub total_selected: usize,
    pub total_success: usize,
    pub total_failed: usize,
    pub released_bytes: u64,
    pub success_items: Vec<MoveSuccessItem>,
    pub failed_items: Vec<MoveFailureItem>,
}

/// `apply_move_action` 命令的返回：报告 + 更新后的重复组。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveActionResponse {
    pub report: MoveReport,
    pub updated_groups: Vec<DuplicateGroup>,
}
