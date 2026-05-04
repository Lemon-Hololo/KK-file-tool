//! 应用设置与数据库路径信息模型。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::{
    DEFAULT_EXTREME_ROW_THRESHOLD, DEFAULT_IO_CONCURRENCY_MULTIPLIER, DEFAULT_KEEP_POLICY,
    DEFAULT_LOG_MAX_LENGTH, DEFAULT_MOD_ROLLBACK_ENABLED, DEFAULT_MOD_SCAN_KEYWORD,
    DEFAULT_PIXIV_PARTIAL_FLUSH_INTERVAL_MS, DEFAULT_PIXIV_RATE_LIMIT_PER_MINUTE,
    DEFAULT_PIXIV_TAG_API_BASE, DEFAULT_SUFFIX_TARGET, DEFAULT_TEXT_PREVIEW_MAX_KB,
    DEFAULT_THEME_MODE, DEFAULT_THREAD_COUNT, DEFAULT_ZIP_PREVIEW_MAX_ENTRIES,
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

    // ---- Mod 工具回滚 ----
    /// 是否启用 Mod 工具的备份/回滚机制（重复删除 / 不同版本删除 / 移除版本限制）。
    ///
    /// 关闭后这三类操作不再创建备份，操作记录的 `rollback_enabled = false`，
    /// 记录管理页的"撤回"按钮会被置灰。重命名 / 归类不受此开关影响。
    pub mod_rollback_enabled: bool,
    /// Mod 备份目录；为 `None` 或空串时使用 `<exe_dir>/mod-backups`。
    ///
    /// 同条记录的所有备份会落到 `<root>/<record_id>/<原文件名>`，
    /// 以便人工按记录批量清理。跨卷场景下备份会自动走 copy + delete 兜底。
    pub mod_backup_dir: Option<String>,

    // ---- Pixiv 标签整理 ----
    /// Pixiv 标签接口的 base URL；最终请求 `<base><pid>`。
    pub pixiv_tag_api_base: String,
    /// 排除的 tag 列表；这些 tag 不会作为虚拟表的列出现，避免噪音列。
    pub pixiv_excluded_tags: Vec<String>,
    /// 本地 tag 翻译表；key 为 Pixiv 原 tag，value 为用户维护的本地译名。
    ///
    /// 面板处于译名显示模式时，本地译名优先级高于 Pixiv 响应里的 `translation.en`。
    pub pixiv_local_tag_translations: HashMap<String, String>,
    /// 可选 Pixiv Cookie（PHPSESSID 等）；填了之后能拿到 R-18 / 关注限定等受限 tag。
    pub pixiv_cookie: Option<String>,
    /// 可选 HTTP / HTTPS / SOCKS5 代理 URL；中国大陆访问 Pixiv 一般要配。
    ///
    /// 形如 `http://127.0.0.1:7890`、`https://...`、`socks5://127.0.0.1:1080`。
    /// 留空时按 reqwest 默认行为走系统环境变量（`HTTP_PROXY` / `HTTPS_PROXY`）。
    pub pixiv_proxy: Option<String>,
    /// 是否在面板上用 `translation.en` 替代原 tag 显示（同时点击移动也用译名做目录）。
    ///
    /// 关闭时全部沿用 Pixiv 返回的原 tag（多为日文）。同一开关在配置中心和任务面板上
    /// 都暴露，用户可以在不离开任务面板的情况下切换。**Pixiv 响应没有 en 译名时
    /// 该开关对那条 tag 不生效**——回落到原 tag。
    pub pixiv_use_translation: bool,
    /// Pixiv 拉取的每分钟最大请求数（限速防黑）。
    ///
    /// 60 = 每秒 1 条；与 `Semaphore` 的并发上限正交——并发只控制"同时在飞的请求数"，
    /// 这个值控制"任意 60 秒滚动窗口内总请求数"。把每个 worker 的拉取调度排到一条共享
    /// "下一可用时刻"队列上，保证整个长任务的速率不超过 `per_minute / 60` 次/秒。
    /// `0` 视为"不限速"（仅给测试 / 高级用户用，UI 限制最小为 1）。
    pub pixiv_rate_limit_per_minute: i32,
    /// Pixiv 增量结果在前端的合并刷新间隔（毫秒）。
    ///
    /// `0` = 即刻：partial 一到达就立刻应用，UI 跟随每条结果跳动；
    /// `>0` = 节流：partial 进入缓冲区，按本间隔批量 commit。done 终态会立刻 flush
    /// 一次，不被节流拖延。UI 限制最大 10000ms。
    pub pixiv_partial_flush_interval_ms: i32,
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
            mod_rollback_enabled: DEFAULT_MOD_ROLLBACK_ENABLED,
            mod_backup_dir: None,
            pixiv_tag_api_base: DEFAULT_PIXIV_TAG_API_BASE.to_string(),
            pixiv_excluded_tags: Vec::new(),
            pixiv_local_tag_translations: HashMap::new(),
            pixiv_cookie: None,
            pixiv_proxy: None,
            pixiv_use_translation: false,
            pixiv_rate_limit_per_minute: DEFAULT_PIXIV_RATE_LIMIT_PER_MINUTE,
            pixiv_partial_flush_interval_ms: DEFAULT_PIXIV_PARTIAL_FLUSH_INTERVAL_MS,
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
