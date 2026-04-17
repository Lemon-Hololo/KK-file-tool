// 哈希结果通道的缓冲区大小，用于异步传输哈希结果
// 较大的缓冲区可以减少背压，提高吞吐量
pub const HASH_QUEUE_SIZE: usize = 2048;

// 扫描结果通道的缓冲区大小（当前未使用）
pub const SCAN_QUEUE_SIZE: usize = 2048;

// 分批发送结果到前端的批次大小
// 每 N 个重复组就向前端发送一次进度，平衡实时性和性能
pub const PARTIAL_BATCH_SIZE: usize = 30;

// 暂停状态下的轮询间隔（毫秒）
// 平衡 CPU 占用和响应速度，100ms 是合理的折中值
pub const PAUSE_SLEEP_MS: u64 = 100;

// 文本预览的最大字节数（256KB）
pub const TEXT_PREVIEW_MAX_BYTES: usize = 256 * 1024;

// 压缩包预览的最大条目数
pub const ZIP_PREVIEW_MAX_ENTRIES: usize = 5000;

// 默认保留策略：保留最新文件
pub const DEFAULT_KEEP_POLICY: &str = "newest";

// 默认主题模式：跟随系统
pub const DEFAULT_THEME_MODE: &str = "system";

// 默认多线程核心数：0 表示自动使用全部可用核心
pub const DEFAULT_THREAD_COUNT: i32 = 0;