//! 应用设置与数据库路径信息模型。

use serde::{Deserialize, Serialize};

use crate::config::{DEFAULT_KEEP_POLICY, DEFAULT_THEME_MODE, DEFAULT_THREAD_COUNT};

/// 持久化到 `app_settings` 单行表的用户设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub keep_policy: String,
    pub move_target_path: Option<String>,
    pub save_record_enabled: bool,
    pub use_last_record_enabled: bool,
    pub include_current_folder_duplicates: bool,
    pub theme_mode: String,
    /// 并发处理使用的核心数；`0` 表示自动（等于 `num_cpus::get()`）。
    pub thread_count: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            keep_policy: DEFAULT_KEEP_POLICY.to_string(),
            move_target_path: None,
            save_record_enabled: true,
            use_last_record_enabled: false,
            include_current_folder_duplicates: true,
            theme_mode: DEFAULT_THEME_MODE.to_string(),
            thread_count: DEFAULT_THREAD_COUNT,
        }
    }
}

/// 返回给前端的数据库路径信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbPathInfo {
    /// 当前实际使用的数据库路径。
    pub current_path: String,
    /// 默认路径（`<app_data_dir>/fileflow.db`）。
    pub default_path: String,
    /// 用户自定义路径；未设置时为 `None`。
    pub custom_path: Option<String>,
}
