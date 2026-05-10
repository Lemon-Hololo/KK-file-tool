//! 不走数据库的编译期常量（队列容量、分批大小、默认值等）。
//!
//! # 设置默认值 vs 编译期常量
//! `DEFAULT_*` 是可配置项（在 `AppSettings` 里）首次落库时写入的默认值；用户可在
//! 设置中心里改。数据库读失败时服务层也会退回到这些常量，保证即便配置表损坏
//! 功能仍然可用。剩余常量（队列大小、暂停轮询等）没有开放给用户，改它们要改
//! 这里。

/// 哈希结果通道的缓冲区大小；更大缓冲减轻背压，提高吞吐。
pub const HASH_QUEUE_SIZE: usize = 2048;

/// 向前端分批推送结果的批次大小；平衡实时性和事件开销。
pub const PARTIAL_BATCH_SIZE: usize = 30;

/// 暂停状态下的轮询间隔（毫秒）；平衡 CPU 占用与响应速度。
pub const PAUSE_SLEEP_MS: u64 = 100;

/// 默认保留策略：保留最新文件。
pub const DEFAULT_KEEP_POLICY: &str = "newest";

/// 默认主题：跟随系统。
pub const DEFAULT_THEME_MODE: &str = "system";

/// 默认线程数；`0` 代表自动（等于 `num_cpus::get()`）。
pub const DEFAULT_THREAD_COUNT: i32 = 0;

/// 默认日志保留上限（条）。前端运行时按此裁剪 `runtime.logs`。
pub const DEFAULT_LOG_MAX_LENGTH: i32 = 3000;

/// 默认 IO 并发倍率：实际 IO 并发 = `有效线程数 × 本倍率`。
///
/// SSD/NVMe 可调高（4~8），HDD 要降到 1，直接影响去重与 Mod 扫描的吞吐。
pub const DEFAULT_IO_CONCURRENCY_MULTIPLIER: i32 = 2;

/// 默认"极限模式"行数阈值；虚拟表超过此行数会降级 overscan 与分段渲染参数。
pub const DEFAULT_EXTREME_ROW_THRESHOLD: i32 = 20000;

/// 默认文本预览最大字节数，以 KB 为单位（方便 UI 展示）。
pub const DEFAULT_TEXT_PREVIEW_MAX_KB: i32 = 256;

/// 默认压缩包预览的最大条目数。
pub const DEFAULT_ZIP_PREVIEW_MAX_ENTRIES: i32 = 5000;

/// 默认 Mod 扫描关键字。
pub const DEFAULT_MOD_SCAN_KEYWORD: &str = "Koikatsu";

/// 默认后缀目标（不含点）。
pub const DEFAULT_SUFFIX_TARGET: &str = "txt";

/// 默认是否启用 Mod 工具备份/回滚机制。
///
/// 关闭后，重复删除 / 不同版本删除 / 移除版本限制三类操作不再创建备份，
/// 记录主表的 `rollback_enabled = 0`，对应记录在记录管理页的"撤回"按钮置灰。
pub const DEFAULT_MOD_ROLLBACK_ENABLED: bool = true;

/// 默认是否在去重移动时保留文件相对源根目录的子目录结构。
///
/// 关闭（默认）时所有选中文件平铺到 `<target_dir>/<task_id>/`，与历史行为一致。
/// 开启时按"文件 absPath 去掉所属任务输入根"得到相对子路径，落到
/// `<target_dir>/<task_id>/<rel_dir>/<file_name>`；找不到匹配根则降级为平铺。
pub const DEFAULT_PRESERVE_DIR_ON_MOVE: bool = false;

/// Pixiv 标签接口默认 base URL；最终请求 URL 拼接为 `<base><pid>`。
///
/// base 后是否带斜杠都接受——业务侧 [`crate::services::pixiv_tag`] 会自动补齐。
pub const DEFAULT_PIXIV_TAG_API_BASE: &str = "https://www.pixiv.net/ajax/illust/";

/// Pixiv tag 拉取的默认每分钟最大请求数。
///
/// 实际间隔 = `60s / per_minute`；60 即"每秒 1 条"，对 Pixiv 这种公开 ajax 接口
/// 是相当保守的速率，不会触发常见的 IP 限流策略。需要更激进的批量可在设置里调高，
/// 但同时要看 Cookie / 代理的状态——纯游客身份建议保留 60 或更低。
pub const DEFAULT_PIXIV_RATE_LIMIT_PER_MINUTE: i32 = 60;

/// Pixiv 增量结果在前端的合并刷新间隔（毫秒）。
///
/// `0` = 即刻：partial 一到达 store 就立刻 commit（行 UI 会跟着每条结果实时跳动，
/// 50K 张图的批量场景下视觉上比较密集）。
/// `>0` = 节流：partial 进入缓冲区，按本间隔批量 commit 一次（用户能稳稳读完每屏
/// 内容再看下一波刷新）。**间隔到了的当下立即 flush 一次，确保 done 终态不被
/// 拖延**。UI 限制最大 10000ms 兜底。
pub const DEFAULT_PIXIV_PARTIAL_FLUSH_INTERVAL_MS: i32 = 0;
