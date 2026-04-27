//! 不走数据库的编译期常量（队列容量、分批大小、默认值等）。
//!
//! # 设置默认值 vs 编译期常量
//! `DEFAULT_*` 是可配置项（在 `AppSettings` 里）首次落库时写入的默认值；用户可在
//! 设置中心里改。数据库读失败时服务层也会退回到这些常量，保证即便配置表损坏
//! 功能仍然可用。剩余常量（队列大小、暂停轮询等）没有开放给用户，改它们要改
//! 这里。

/// 哈希结果通道的缓冲区大小；更大缓冲减轻背压，提高吞吐。
pub const HASH_QUEUE_SIZE: usize = 2048;

/// 扫描结果通道的缓冲区大小（当前未使用，预留）。
pub const SCAN_QUEUE_SIZE: usize = 2048;

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
