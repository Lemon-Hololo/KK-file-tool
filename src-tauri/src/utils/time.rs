//! 时间相关的纯函数工具。

use std::time::{SystemTime, UNIX_EPOCH};

/// 把 `SystemTime`（通常来自 `Metadata::modified()` / `created()`）转换为
/// 秒级 Unix 时间戳。`None` 或读取失败时返回 `0`。
///
/// 项目里 mtime / ctime 落库一律用秒级 i64；从 `Metadata` 拿到的
/// `SystemTime` 都要走这套语义化处理，避免 `dedup.rs` / `mod_tools::cleanup`
/// 各自手写一份 `duration_since(UNIX_EPOCH)` 链。
///
/// # 与 ctime 兜底约定
/// `ctime` 在 Linux 老内核 / 特殊 FS 上可能不可读；调用方需要"`ctime` 失败时
/// 回落到 mtime"语义，请直接 `system_time_to_secs(meta.created().ok()).max(...)`
/// 或在外部判空，本函数本身只做单次转换。
pub fn system_time_to_secs(value: Option<SystemTime>) -> i64 {
    value
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
