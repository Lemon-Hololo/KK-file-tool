//! FileOrganizer: 非递归扫描目录，按文件名首个 `[...]` 括号内容建子目录并移入。

use std::path::Path;

use chrono::Local;

use crate::{
    constants::mod_op_kind,
    error::{AppError, AppResult},
    models::{ModOpApplyResponse, ModOrganizePreviewItem},
    services::{mod_tools::MOD_OP_TABLES, op_pipeline},
    utils::{
        filename::{extract_bracket, resolve_conflict},
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

/// 对 `paths` 下每个目录做一次（非递归）扫描；返回每个文件的归类预览。
pub fn preview_mod_organize(paths: &[String]) -> AppResult<Vec<ModOrganizePreviewItem>> {
    let mut result = vec![];

    for root in paths {
        let dir = Path::new(root);
        let ep = to_extended_length_path(dir);
        if !ep.is_dir() {
            return Err(AppError::InvalidInput(format!("不是有效目录: {root}")));
        }

        let entries = std::fs::read_dir(&ep).map_err(|e| AppError::Io(e.to_string()))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let file_path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let Some(folder_name_raw) = extract_bracket(&file_name) else {
                continue;
            };
            let folder_name = folder_name_raw.trim().to_string();
            if folder_name.is_empty() {
                continue;
            }

            let target_dir = dir.join(&folder_name);
            let target = target_dir.join(&file_name);

            if file_path.parent() == Some(target_dir.as_path()) {
                continue;
            }

            let (final_target, conflict) = resolve_conflict(target);

            result.push(ModOrganizePreviewItem {
                old_path: to_user_friendly_path(&file_path),
                new_path: to_user_friendly_path(&final_target),
                folder_name,
                will_conflict: conflict,
            });
        }
    }

    Ok(result)
}

/// 应用归类：按预览结果把文件 rename 到目标子目录，并持久化为 `mod_op` 记录。
pub fn apply_mod_organize(
    db_path: &Path,
    paths: &[String],
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> AppResult<ModOpApplyResponse> {
    let mut preview = preview_mod_organize(paths)?;

    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let pairs: Vec<(String, String)> = preview
        .into_iter()
        .map(|p| (p.old_path, p.new_path))
        .collect();

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());

    op_pipeline::persist_apply_rename_pairs(
        db_path,
        MOD_OP_TABLES,
        mod_op_kind::ORGANIZE,
        name,
        paths,
        pairs,
        true,
    )
}
