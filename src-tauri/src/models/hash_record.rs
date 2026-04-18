//! 哈希索引记录相关模型。
//!
//! `hash_records` 表保存去重扫描产生的历史哈希索引，用于下次去重时
//! 快速比对"新旧"之间的重复关系，避免重复计算整份目录。

use serde::{Deserialize, Serialize};

/// 单条哈希条目：去重时每个参与哈希的文件都会写一条。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexEntry {
    pub hash: String,
    pub file_path: String,
    pub file_size: u64,
    /// 文件修改时间（秒级 Unix 时间戳）。
    pub mtime: i64,
    /// 文件创建时间（秒级 Unix 时间戳，Windows 可用）。
    pub ctime: i64,
    /// 条目状态；当前只有 `"active"`，为未来的"软删除"等状态预留。
    pub status: String,
}

/// 哈希记录（一次去重任务的完整输出），含所有条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexRecord {
    pub record_id: String,
    pub record_name: String,
    pub created_at: i64,
    pub source_paths: Vec<String>,
    pub entries: Vec<HashIndexEntry>,
}

/// 列表视图：不加载全部条目，仅返回条目总数用于前端摘要展示。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub created_at: i64,
    pub source_paths: Vec<String>,
    pub entry_count: usize,
}
