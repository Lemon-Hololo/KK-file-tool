//! 路径规范化结果模型。
//!
//! `normalize_input_paths` 命令把用户输入的多个路径去重、规范化、过滤子目录，
//! 同时记录被剔除的条目与警告。

use serde::{Deserialize, Serialize};

/// 路径规范化的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizePathResult {
    /// 规范化后保留的路径（已去重、去子目录覆盖）。
    pub normalized_paths: Vec<String>,
    /// 被剔除的原始路径（不可访问、重复、或被父目录覆盖）。
    pub removed_paths: Vec<String>,
    /// 人类可读的警告信息，前端会弹窗展示。
    pub warnings: Vec<String>,
}
