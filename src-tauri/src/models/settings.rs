//! 应用设置与数据库路径信息模型。

use serde::{Deserialize, Serialize};

use crate::config::{
    DEFAULT_EXTREME_ROW_THRESHOLD, DEFAULT_IO_CONCURRENCY_MULTIPLIER, DEFAULT_KEEP_POLICY,
    DEFAULT_LOG_MAX_LENGTH, DEFAULT_MOD_SCAN_KEYWORD, DEFAULT_SUFFIX_TARGET,
    DEFAULT_TEXT_PREVIEW_MAX_KB, DEFAULT_THEME_MODE, DEFAULT_THREAD_COUNT,
    DEFAULT_ZIP_PREVIEW_MAX_ENTRIES,
};

/// 持久化到 `app_settings` 单行表的用户设置。
///
/// 新增字段的通用套路：
/// 1. 在这里加字段并给 [`Default`] 加默认值（常量写在 `config.rs`）
/// 2. `db::schema::init_schema` 末尾加一条 `ALTER TABLE ADD COLUMN`
/// 3. `db::settings_repo` 的 SELECT / UPDATE 扩列
/// 4. 使用方通过 `settings_repo::get_settings(db_path)` 读（失败回退默认）
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

    // ---- 性能 ----
    /// 前端日志保留上限（条）；运行时按此裁剪 `runtime.logs`。
    pub log_max_length: i32,
    /// IO 并发倍率：实际 IO 并发 = `有效线程数 × 本倍率`，被 dedup / mod scan 共用。
    pub io_concurrency_multiplier: i32,
    /// 虚拟表进入"极限模式"的行数阈值；超过时减少 overscan 与分段渲染步长。
    pub extreme_row_threshold: i32,

    // ---- 预览 ----
    /// 文本预览最大读取字节数，以 KB 为单位（`256` 即 256 KiB）。
    pub text_preview_max_kb: i32,
    /// 压缩包预览枚举的最大条目数。
    pub zip_preview_max_entries: i32,

    // ---- 工具默认值 ----
    /// Mod 扫描关键字的默认值；空串视为未配置。
    pub mod_scan_default_keyword: String,
    /// 后缀修改的默认目标（不带点）。
    pub suffix_default_target: String,
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
            log_max_length: DEFAULT_LOG_MAX_LENGTH,
            io_concurrency_multiplier: DEFAULT_IO_CONCURRENCY_MULTIPLIER,
            extreme_row_threshold: DEFAULT_EXTREME_ROW_THRESHOLD,
            text_preview_max_kb: DEFAULT_TEXT_PREVIEW_MAX_KB,
            zip_preview_max_entries: DEFAULT_ZIP_PREVIEW_MAX_ENTRIES,
            mod_scan_default_keyword: DEFAULT_MOD_SCAN_KEYWORD.to_string(),
            suffix_default_target: DEFAULT_SUFFIX_TARGET.to_string(),
        }
    }
}

/// 返回给前端的数据库路径信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbPathInfo {
    /// 当前实际使用的数据库路径。
    pub current_path: String,
    /// 默认路径（`<app_data_dir>/kk-file-tool.db`）。
    pub default_path: String,
    /// 用户自定义路径；未设置时为 `None`。
    pub custom_path: Option<String>,
}
