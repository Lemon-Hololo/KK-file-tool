//! Windows 长路径处理 + 路径展示互转 + "保留源目录结构"用的相对路径计算。
//!
//! 后端内部一律用 `\\?\` / `\\?\UNC\` 前缀访问文件系统，以规避 MAX_PATH
//! 限制；对外（前端展示、数据库存储）再通过 [`to_user_friendly_path`] 去掉前缀。
//!
//! [`prepare_source_roots`] / [`relative_subdir_for`] 是"保留源目录结构"开关
//! （`preserve_dir_on_move`）的纯路径实现：去重移动落到 `<target>/<task_id>/<rel>/`、
//! Mod 备份落到 `<backup_root>/<record_id>/<rel>/` 都共用这套算法，避免两份漂移。

use std::path::{Component, Path, PathBuf};

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

/// 把"源根列表"预处理成 `(PathBuf, lowercase_string)` 并按字符串长度降序排列，
/// 用于 [`relative_subdir_for`] 的"最长前缀优先"匹配——避免 `D:\a` 把本应归到
/// `D:\a\b` 的文件错认为根。空字符串自动跳过。
///
/// 调用方一次预处理、循环匹配：去重的 `move_selected_files` 与 Mod 备份的
/// `prepare_mod_backup` 在启用 `preserve_dir_on_move` 时都用本函数的结果。
pub fn prepare_source_roots(source_roots: &[String]) -> Vec<(PathBuf, String)> {
    let mut v: Vec<(PathBuf, String)> = source_roots
        .iter()
        .filter_map(|r| {
            let trimmed = r.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some((PathBuf::from(trimmed), trimmed.to_lowercase()))
            }
        })
        .collect();
    v.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    v
}

/// 在已按长度降序的 `prepared_roots` 中找到 `file` 所属源根，返回相对其的子目录
/// （不含文件名）；找不到任何匹配返回 `None`，由调用方决定降级策略（去重移动
/// 直接落 target_dir 根、Mod 备份直接落 record_id 根）。
///
/// 字面 [`Path::strip_prefix`] 优先（canonicalize 后大小写一致即可命中，最快）；
/// 命中失败时回退到大小写不敏感的字符串前缀比较——前端 `paths` 的大小写经过
/// `canonicalize` 已对齐磁盘真实形式，但用户从 useStorage 中拿出的旧路径偶尔
/// 会与扫描结果存在大小写差异，留个降级保平安。
pub fn relative_subdir_for(file: &Path, prepared_roots: &[(PathBuf, String)]) -> Option<PathBuf> {
    for (root, root_lower) in prepared_roots {
        if let Ok(rel) = file.strip_prefix(root) {
            return Some(parent_components_of(rel));
        }

        // 降级：大小写不敏感的字面前缀比较；再对齐到下一段分隔符避免 `D:\a` 误命中 `D:\ab\c`。
        let file_str = file.to_string_lossy();
        let file_lower = file_str.to_lowercase();
        if !file_lower.starts_with(root_lower) {
            continue;
        }
        let rest = &file_str[root_lower.len()..];
        let rest = rest.trim_start_matches(['/', '\\']);
        if rest.is_empty() || rest.len() == file_str.len() - root_lower.len() {
            // 没有分隔符就匹配上的，说明根名只是文件路径前缀的子串而不是真正父目录。
            // 例：root = "D:\\a", file = "D:\\ab\\x.txt"。
            if file_str.len() > root_lower.len()
                && !matches!(file_str.as_bytes()[root_lower.len()], b'/' | b'\\')
            {
                continue;
            }
        }
        return Some(parent_components_of(Path::new(rest)));
    }
    None
}

/// 取相对路径中除文件名外的父目录部分；过滤掉 `.` / `..` 与 prefix/root，避免
/// 拼接到 target_dir / 备份目录时跳出预期范围。
pub fn parent_components_of(rel: &Path) -> PathBuf {
    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let mut out = PathBuf::new();
    for comp in parent.components() {
        if let Component::Normal(seg) = comp {
            out.push(seg);
        }
    }
    out
}
