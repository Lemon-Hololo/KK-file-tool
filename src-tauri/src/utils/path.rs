use std::path::{Path, PathBuf};

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

#[cfg(not(windows))]
pub fn to_extended_length_path(p: &Path) -> PathBuf {
    p.to_path_buf()
}

/// 把内部文件系统路径转换为前端展示/存储使用的普通路径
/// 例如:
/// \\?\D:\a\b.txt -> D:\a\b.txt
/// \\?\UNC\server\share\a.txt -> \\server\share\a.txt
pub fn to_user_friendly_path(p: &Path) -> String {
    let raw = p.to_string_lossy();

    #[cfg(windows)]
    {
        if raw.starts_with(r"\\?\UNC\") {
            return format!(r"\\{}", &raw[8..]);
        }
        if raw.starts_with(r"\\?\") {
            return raw[4..].to_string();
        }
    }

    raw.to_string()
}

pub fn cmp_key_case_insensitive(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

pub fn is_parent_of(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent) && child != parent
}
