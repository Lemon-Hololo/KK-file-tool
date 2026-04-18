//! 任务域模型：任务状态、去重配置、文件条目、重复组以及 IPC payload。

use serde::{Deserialize, Serialize};

/// 任务生命周期状态。IPC 事件 `task_state_changed` 会把变体名（驼峰）作为字符串下发。
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

/// 去重任务配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupConfig {
    /// 自动选择保留文件的策略：`"newest"` / `"oldest"`（详见 `constants::keep_policy`）。
    pub keep_policy: String,
    /// 被移动文件的目标目录；未设置时每次任务生成一个子目录。
    pub move_target_path: Option<String>,
    /// 是否启用"自动勾选"——对每组自动标记待移动文件。
    pub auto_select_enabled: bool,
    /// 扫描完成后是否把本次结果保存为哈希记录。
    pub save_record_enabled: bool,
    /// 是否加载上一次的哈希记录做跨会话去重比对。
    pub use_last_record_enabled: bool,
    /// 指定使用的记录 ID；为空时取最新一条。
    pub selected_record_id: Option<String>,
    /// 是否把"当前扫描路径内部的重复"也算进结果。
    pub include_current_folder_duplicates: bool,
    /// 保存记录时使用的名字；不指定则用当前时间格式化。
    pub record_name: Option<String>,
}

/// 去重结果中的单个文件条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub abs_path: String,
    pub size: u64,
    pub mtime: i64,
    /// 创建时间，用于保留策略（`newest` / `oldest` 排序依据）。
    pub ctime: i64,
    pub hash: Option<String>,
    /// 是否勾选为"待移动"。自动策略或用户操作都会写此字段。
    pub selected_for_move: bool,
    /// 是否来自历史哈希记录（跨会话比对时标记）。
    pub from_history: bool,
}

/// 一个哈希值对应的重复文件组。`files.len() >= 2` 才会被视为"组"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// 人类可读的组编号，形如 `"G00001"`。
    pub group_id: String,
    pub hash: String,
    pub files: Vec<FileEntry>,
}

/// IPC 事件 `task_log` 的 payload。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLogPayload {
    pub task_id: String,
    /// 等级字符串（`INFO` / `WARN` / `ERROR`，见 `constants::log_level`）。
    pub level: String,
    pub message: String,
    /// 可选关联文件路径，便于前端悬浮预览。
    pub file_path: Option<String>,
}

/// IPC 事件 `task_progress` 的 payload。`total = 0` 表示阶段还未确定总量。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressPayload {
    pub task_id: String,
    /// 阶段标识（见 `constants::stages`）。
    pub stage: String,
    pub processed: usize,
    pub total: usize,
    pub percent: f64,
}
