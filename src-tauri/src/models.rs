use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Idle,
    Running,
    Paused,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupConfig {
    pub keep_policy: String,
    pub move_target_path: Option<String>,
    pub auto_select_enabled: bool,
    pub save_record_enabled: bool,
    pub use_last_record_enabled: bool,
    pub selected_record_id: Option<String>,
    pub include_current_folder_duplicates: bool,
    pub record_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub abs_path: String,
    pub size: u64,
    pub mtime: i64,
    pub ctime: i64, // 创建时间，用于保留策略
    pub hash: Option<String>,
    pub selected_for_move: bool,
    pub from_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub group_id: String,
    pub hash: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPayload {
    pub task_id: String,
    pub level: String,
    pub message: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressPayload {
    pub task_id: String,
    pub stage: String,
    pub processed: usize,
    pub total: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub keep_policy: String,
    pub move_target_path: Option<String>,
    pub save_record_enabled: bool,
    pub use_last_record_enabled: bool,
    pub include_current_folder_duplicates: bool,
    pub theme_mode: String,
    /// 多线程处理核心数，0 = 自动（使用全部可用核心）
    pub thread_count: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            keep_policy: crate::config::DEFAULT_KEEP_POLICY.to_string(),
            move_target_path: None,
            save_record_enabled: true,
            use_last_record_enabled: false,
            include_current_folder_duplicates: true,
            theme_mode: crate::config::DEFAULT_THEME_MODE.to_string(),
            thread_count: crate::config::DEFAULT_THREAD_COUNT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexEntry {
    pub hash: String,
    pub file_path: String,
    pub file_size: u64,
    pub mtime: i64,
    pub ctime: i64, // 创建时间
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexRecord {
    pub record_id: String,
    pub record_name: String,
    pub created_at: i64,
    pub source_paths: Vec<String>,
    pub entries: Vec<HashIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashIndexRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub created_at: i64,
    pub source_paths: Vec<String>,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizePathResult {
    pub normalized_paths: Vec<String>,
    pub removed_paths: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSummary {
    pub target_dir: String,
    pub total_selected: usize,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveSuccessItem {
    pub src_path: String,
    pub dst_path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFailureItem {
    pub src_path: String,
    pub error_code: String,
    pub error_message: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveActionResponse {
    pub report: MoveReport,
    pub updated_groups: Vec<DuplicateGroup>,
}

// ===== Suffix Change =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixPreviewItem {
    pub old_path: String,
    pub new_path: String,
    pub will_rename_conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixApplyItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub status: String, // success / failed
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixApplyResponse {
    pub record_id: String,
    pub record_name: String,
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub items: Vec<SuffixApplyItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRecordSummary {
    pub record_id: String,
    pub record_name: String,
    pub target_suffix: String,
    pub created_at: i64,
    pub total_items: usize,
    pub success_items: usize,
    pub rollback_status: String, // applied / partially_rolled_back / rolled_back
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRecordDetail {
    pub summary: SuffixRecordSummary,
    pub items: Vec<SuffixRecordItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuffixRollbackCheck {
    pub total_selected: usize,
    pub existing_count: usize,
    pub missing_paths: Vec<String>,
}

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
