//! 路径规范化与资源管理器定位命令。

use std::{collections::HashSet, path::PathBuf, process::Command};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::{
    error::{AppError, AppResult},
    models::NormalizePathResult,
    utils::path::{cmp_key_case_insensitive, is_parent_of, to_user_friendly_path},
};

fn canonicalize_path(raw: &str) -> Option<PathBuf> {
    let p = PathBuf::from(raw);
    std::fs::canonicalize(p).ok()
}

/// 去重、去不可访问、去"被父目录覆盖"的子目录，返回规范化后的路径与被剔除项。
///
/// 前端会把 `warnings` 弹窗提示用户，确保用户知情后再继续任务。
#[tauri::command]
pub fn normalize_input_paths(paths: Vec<String>) -> Result<NormalizePathResult, String> {
    normalize_input_paths_impl(paths).map_err(|e| e.to_string())
}

/// 在 Windows 资源管理器中打开目录；文件路径会高亮选中，目录路径则直接打开。
#[tauri::command]
pub fn reveal_in_explorer(file_path: String) -> Result<(), String> {
    reveal_in_explorer_impl(&file_path).map_err(|e| e.to_string())
}

fn normalize_input_paths_impl(paths: Vec<String>) -> AppResult<NormalizePathResult> {
    let mut normalized = vec![];
    let mut removed = vec![];
    let mut warnings = vec![];

    let mut seen: HashSet<String> = HashSet::new();

    for raw in paths {
        let Some(c) = canonicalize_path(&raw) else {
            warnings.push(format!("路径不可访问，已忽略: {raw}"));
            removed.push(raw);
            continue;
        };
        let key = cmp_key_case_insensitive(&c);
        if seen.contains(&key) {
            removed.push(raw);
            continue;
        }
        seen.insert(key);
        normalized.push(c);
    }

    // 先按深度升序，再用"父目录覆盖"规则过滤子目录（父目录一定先到 final_paths）。
    normalized.sort_by_key(|p| p.components().count());
    let mut final_paths: Vec<PathBuf> = vec![];

    for p in normalized {
        let covered = final_paths.iter().any(|parent| is_parent_of(parent, &p));
        if covered {
            // canonicalize 后的路径在 Windows 上带 \\?\ 前缀，提示信息走友好路径。
            warnings.push(format!(
                "子目录被父目录覆盖，已忽略: {}",
                to_user_friendly_path(&p)
            ));
            removed.push(to_user_friendly_path(&p));
        } else {
            final_paths.push(p);
        }
    }

    Ok(NormalizePathResult {
        // canonicalize 在 Windows 返回 verbatim 路径（`\\?\D:\...`），前端期待的是
        // 普通路径，需要在出口统一去掉前缀。
        normalized_paths: final_paths
            .iter()
            .map(|p| to_user_friendly_path(p))
            .collect(),
        removed_paths: removed,
        warnings,
    })
}

fn reveal_in_explorer_impl(file_path: &str) -> AppResult<()> {
    // canonicalize 在 Windows 返回 verbatim 路径，但 explorer.exe 不能解析 `\\?\` 前缀，
    // 必须用去掉前缀后的友好路径。
    let canonical = std::fs::canonicalize(file_path).map_err(|e| AppError::Io(e.to_string()))?;
    let meta = std::fs::metadata(&canonical).map_err(|e| AppError::Io(e.to_string()))?;
    let display_path = to_user_friendly_path(&canonical);

    let mut cmd = Command::new("explorer.exe");
    if meta.is_dir() {
        cmd.arg(&display_path);
    } else {
        #[cfg(windows)]
        {
            // explorer 对 `/select,<path>` 的解析很脆；路径里带空格/逗号时容易退回"文档"。
            // 用 raw_arg 直接传入完整命令行片段，保证它拿到精确的 `"绝对路径"`。
            cmd.raw_arg(format!(r#"/select,"{}""#, display_path));
        }

        #[cfg(not(windows))]
        {
            cmd.arg(format!("/select,{}", display_path));
        }
    }

    cmd.spawn().map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}
