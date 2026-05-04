//! FileOrganizer: 非递归扫描目录，按文件名首个 `[...]` 括号内容建子目录并移入。

use std::{collections::HashSet, path::Path};

use chrono::Local;
use uuid::Uuid;

use crate::{
    constants::mod_op_kind,
    error::{AppError, AppResult},
    models::{ModOpApplyResponse, ModOrganizePreviewItem},
    services::{logging::TaskLogContext, mod_tools::MOD_OP_TABLES, op_pipeline},
    utils::{
        filename::{extract_bracket, resolve_conflict_with_reserved},
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

/// 对 `paths` 下每个目录做一次（非递归）扫描；返回每个文件的归类预览。
///
/// 跨多个源目录处理时，可能出现两个 `[X]foo.zipmod` 应该归类到同一个 `[X]/`
/// 子目录的情况——`reserved` 集合保证两者得到不同的最终路径（第二个会被自动
/// 标成 `[X]/foo (1).zipmod`），而不是 preview 里都标成"无冲突"、apply 时
/// 互相覆盖。
pub fn preview_mod_organize(
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<ModOrganizePreviewItem>> {
    let mut result = vec![];
    let mut reserved: HashSet<String> = HashSet::new();

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
            if let Some(log) = &log {
                log.info_path("正在检查 Mod 归类", &file_path);
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

            let (final_target, conflict) = resolve_conflict_with_reserved(target, &mut reserved);

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
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    let mut preview = preview_mod_organize(paths, log.clone())?;

    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let pairs: Vec<(String, String)> = preview
        .into_iter()
        .map(|p| (p.old_path, p.new_path))
        .collect();

    if let Some(log) = &log {
        for (old_path, _) in &pairs {
            log.info(&format!("准备归类: {old_path}"));
        }
    }

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    let record_id = Uuid::new_v4().to_string();

    // Mod 归类是纯反向 rename，不参与"启用 Mod 操作回滚"开关，永远可撤回。
    op_pipeline::persist_apply_rename_pairs(
        db_path,
        MOD_OP_TABLES,
        &record_id,
        true,
        mod_op_kind::ORGANIZE,
        name,
        paths,
        pairs,
        true,
    )
}
