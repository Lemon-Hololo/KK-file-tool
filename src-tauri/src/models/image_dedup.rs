//! 图片相似度去重相关 DTO。
//!
//! 与 [`super::mod_tools`] / [`super::empty_dirs`] 同构：preview → apply → rollback
//! 走 `op_pipeline`。差异：
//! - 哈希在内存计算（不入库），扫描期间通过 `image_dedup_partial` 增量推送分组；
//! - 保留策略有 4 种（largestResolution / largestFile / newest / oldest），与 Mod
//!   去重的 newest / oldest 二选一不同；
//! - 删除走 `<backup_root>/<record_id>/<原文件名>`，与 Mod 备份共用 backup 模块。

use serde::{Deserialize, Serialize};

/// 参与相似度比较的单张图片元数据 + 哈希。
///
/// `hash` 是十六进制字符串（与算法选项无关，统一格式）；前端拿来仅作展示与
/// "组内查看"用，不参与排序。`width / height / file_size` 用于按用户选择的
/// 保留策略选 keep。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageHashFile {
    pub file_path: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub mtime: i64,
    pub ctime: i64,
    pub hash: String,
}

/// 一组相似图片：组内文件按"保留策略"已排序——`files[0]` 是默认 keep。
///
/// `similarity` 是组内最差的两两相似度（百分比，0–100）：用作 UI 上"按相似度
/// 区间筛选"的滑块下界。完全相同的组 = 100。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupGroup {
    pub group_id: String,
    pub similarity: u32,
    pub files: Vec<ImageHashFile>,
}

/// 扫描增量结果：每完成一个 chunk 推一次。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupPartialPayload {
    pub task_id: String,
    pub groups: Vec<ImageDedupGroup>,
    pub done: bool,
}

/// 应用结果 item。`status` 取值 `"success"` / `"failed"`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupApplyItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String,
    pub message: Option<String>,
}

/// `apply_image_dedup_delete` 的返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupApplyResponse {
    pub record_id: String,
    pub record_name: String,
    /// 当前固定 `"similarity_delete"`；保留字段以便未来扩展（如"按相似度合并到目录"）。
    pub kind: String,
    /// 创建时是否启用回滚；关闭时撤回按钮在 UI 上置灰，后端 rollback 也会拒绝。
    pub rollback_enabled: bool,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<ImageDedupApplyItem>,
}

/// 列表摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub kind: String,
    pub created_at: i64,
    pub total_items: usize,
    pub success_items: usize,
    /// `"applied"` / `"partially_rolled_back"` / `"rolled_back"`。
    pub rollback_status: String,
    pub rollback_enabled: bool,
}

/// 详情中的 item。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupRecordItem {
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
pub struct ImageDedupRecordDetail {
    pub summary: ImageDedupRecordSummary,
    pub items: Vec<ImageDedupRecordItem>,
}

/// 撤回前检查（与 ModOpRollbackCheck 同形）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupRollbackCheck {
    pub total_selected: usize,
    pub existing_count: usize,
    pub missing_paths: Vec<String>,
}

/// 撤回结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageDedupRollbackResponse {
    pub record_id: String,
    pub total_selected: usize,
    pub success: usize,
    pub failed: usize,
    pub skipped_missing: usize,
    pub items: Vec<ImageDedupApplyItem>,
}

/// 把 `op_pipeline` 返回的通用 apply item 映射回本业务的 item DTO。
///
/// 字段同构，纯字段转换；放在模型层避免业务侧重写一份 helper。
impl From<crate::models::ModOpApplyItem> for ImageDedupApplyItem {
    fn from(value: crate::models::ModOpApplyItem) -> Self {
        Self {
            item_id: value.item_id,
            old_path: value.old_path,
            new_path: value.new_path,
            status: value.status,
            message: value.message,
        }
    }
}
