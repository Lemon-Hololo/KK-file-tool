//! Windows 长路径处理 + 路径展示互转。
//!
//! 后端内部一律用 `\\?\` / `\\?\UNC\` 前缀访问文件系统，以规避 MAX_PATH
//! 限制；对外（前端展示、数据库存储）再通过 [`to_user_friendly_path`] 去掉前缀。

use std::path::{Path, PathBuf};

/// 把常规路径转成扩展长度路径（Windows 独占）。
///
/// - `\\?\` 前缀已存在时原样返回；
/// - `\\server\share` UNC 路径转为 `\\?\UNC\server\share`；
/// - 普通路径加上 `\\?\` 前缀。
#[cfg(windows)]
pub fn to_extended_length_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if s.starts_with(r"\\?\") {
        return p.to_path_buf();
    }

    if s.starts_with(r"\\") {
        let unc = s.trim_start_matches(r"\\");
        return PathBuf::from(format!(r"\\?\UNC\{}", unc));
    }

    PathBuf::from(format!(r"\\?\{}", s))
}

/// 非 Windows 平台直接透传。
#[cfg(not(windows))]
pub fn to_extended_length_path(p: &Path) -> PathBuf {
    p.to_path_buf()
}

/// 把内部带 `\\?\` 前缀的路径转成前端友好的普通路径。
///
/// 例：`\\?\D:\a\b.txt` -> `D:\a\b.txt`；
/// `\\?\UNC\server\share\a.txt` -> `\\server\share\a.txt`。
pub fn to_user_friendly_path(p: &Path) -> String {
    let raw = p.to_string_lossy();

    #[cfg(windows)]
    {
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", rest);
        }
        if let Some(rest) = raw.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }

    raw.to_string()
}

/// 路径去重用的大小写不敏感 key。
pub fn cmp_key_case_insensitive(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// `child` 是否是 `parent` 的（严格）子路径。
pub fn is_parent_of(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent) && child != parent
}
