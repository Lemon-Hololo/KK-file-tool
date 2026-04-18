//! 不走数据库的编译期常量（队列容量、分批大小、默认值等）。

/// 哈希结果通道的缓冲区大小；更大缓冲减轻背压，提高吞吐。
pub const HASH_QUEUE_SIZE: usize = 2048;

/// 扫描结果通道的缓冲区大小（当前未使用，预留）。
pub const SCAN_QUEUE_SIZE: usize = 2048;

/// 向前端分批推送结果的批次大小；平衡实时性和事件开销。
pub const PARTIAL_BATCH_SIZE: usize = 30;

/// 暂停状态下的轮询间隔（毫秒）；平衡 CPU 占用与响应速度。
pub const PAUSE_SLEEP_MS: u64 = 100;

/// 文本预览的最大读取字节数（256 KiB）。
pub const TEXT_PREVIEW_MAX_BYTES: usize = 256 * 1024;

/// 压缩包预览时枚举的最大条目数。
pub const ZIP_PREVIEW_MAX_ENTRIES: usize = 5000;

/// 默认保留策略：保留最新文件。
pub const DEFAULT_KEEP_POLICY: &str = "newest";

/// 默认主题：跟随系统。
pub const DEFAULT_THEME_MODE: &str = "system";

/// 默认线程数；`0` 代表自动（等于 `num_cpus::get()`）。
pub const DEFAULT_THREAD_COUNT: i32 = 0;
