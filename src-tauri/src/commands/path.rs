//! 路径规范化命令。

use std::{collections::HashSet, path::PathBuf};

use crate::{
    error::AppResult,
    models::NormalizePathResult,
    utils::path::{cmp_key_case_insensitive, is_parent_of},
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
            warnings.push(format!("子目录被父目录覆盖，已忽略: {}", p.display()));
            removed.push(p.display().to_string());
        } else {
            final_paths.push(p);
        }
    }

    Ok(NormalizePathResult {
        normalized_paths: final_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        removed_paths: removed,
        warnings,
    })
}
