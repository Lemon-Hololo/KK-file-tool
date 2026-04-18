//! 文件系统小工具。

use std::path::Path;

use crate::utils::path::to_extended_length_path;

/// 读取文件大小；失败或文件不存在时返回 `None`。
pub fn metadata_len(path: &Path) -> Option<u64> {
    let ep = to_extended_length_path(path);
    std::fs::metadata(ep).ok().map(|m| m.len())
}
