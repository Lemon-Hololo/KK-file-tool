//! 文件名操作的共享工具：扩展名拆分、命名冲突解决、括号提取等。
//!
//! 本模块收敛 `services::suffix`、`services::mod_tools::*`、`services::move_file`
//! 此前各自重复实现的路径/文件名处理逻辑。所有调用方应直接复用这里的实现，
//! 避免再次在业务模块里内联同名函数。

use std::path::{Path, PathBuf};

use crate::utils::path::to_extended_length_path;

/// Windows 文件名非法字符（`\ / : * ? " < > |`）。
///
/// 构造新文件名时一般用 `-` 替换这些字符，而不是直接删除，
/// 以保留字符位置信息便于人工识别。
pub const ILLEGAL_FILENAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// 把文件名拆成 `(主干, Some(扩展名))`；无扩展名时 `扩展名` 为 `None`。
///
/// 扩展名包含前导点号，例如 `"a.txt"` -> `("a", Some(".txt"))`。
/// 以点号开头的文件名（如 `".gitignore"`）整体视为主干，返回 `(".gitignore", None)`。
pub fn split_name_ext(file_name: &str) -> (String, Option<String>) {
    if let Some(idx) = file_name.rfind('.') {
        if idx > 0 {
            return (
                file_name[..idx].to_string(),
                Some(file_name[idx..].to_string()),
            );
        }
    }
    (file_name.to_string(), None)
}

/// 目标路径冲突时追加 ` (1)`, ` (2)` ... 直到可用。
///
/// 返回 `(最终路径, 是否发生过冲突)`。检测目标存在性用 `to_extended_length_path`
/// 以兼容 Windows 超长路径。
pub fn resolve_conflict(target: PathBuf) -> (PathBuf, bool) {
    if !to_extended_length_path(&target).exists() {
        return (target, false);
    }

    let parent = target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let file_name = target.file_name().unwrap().to_string_lossy().to_string();
    let (stem, ext) = split_name_ext(&file_name);
    let ext = ext.unwrap_or_default();

    let mut i = 1usize;
    loop {
        let candidate = parent.join(format!("{stem} ({i}){ext}"));
        if !to_extended_length_path(&candidate).exists() {
            return (candidate, true);
        }
        i += 1;
    }
}

/// 去掉文件主名末尾的 ` (N)` 冲突后缀（N 为纯数字）。
///
/// 例如 `"[a] x-1.0 (2)"` -> `"[a] x-1.0"`。用于识别"已经被应用过冲突后缀"
/// 的文件名，避免重复在同一个文件上叠加编号。
pub fn strip_conflict_suffix(stem: &str) -> &str {
    if let Some(idx) = stem.rfind(" (") {
        let tail = &stem[idx + 2..];
        if let Some(inner) = tail.strip_suffix(')') {
            if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                return &stem[..idx];
            }
        }
    }
    stem
}

/// 规范化用户输入的"目标后缀"：去空格，保证以点号开头；空串返回空串。
///
/// 例如 `"txt"` -> `".txt"`，`".log"` -> `".log"`，`"  "` -> `""`。
pub fn normalize_suffix(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with('.') {
        s.to_string()
    } else {
        format!(".{s}")
    }
}

/// 提取文件名中首个 `[...]` 括号内的内容（去首尾空白前的原始片段）。
///
/// 不存在成对括号或括号内为空时返回 `None`。
pub fn extract_bracket(name: &str) -> Option<&str> {
    let start = name.find('[')?;
    let rest = &name[start + 1..];
    let end = rest.find(']')?;
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

/// 用 `-` 替换文件名中的非法字符。
pub fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if ILLEGAL_FILENAME_CHARS.contains(&c) { '-' } else { c })
        .collect()
}

/// 把全角方括号 `【】` 归一化为半角 `[]`。
pub fn normalize_brackets(s: &str) -> String {
    s.replace('【', "[").replace('】', "]")
}
