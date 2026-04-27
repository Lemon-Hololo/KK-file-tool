//! 空文件夹清理服务。
//!
//! 预览采用后序递归：只有当一个目录自身没有文件，且所有子目录也都是
//! 可清理的空目录时，它才会出现在候选列表中。删除时先删深层目录再删父目录；
//! 撤回时按记录重新创建目录，因此只能恢复空目录结构，不能恢复目录中后来出现的文件。

use std::{
    cmp::Reverse,
    collections::HashSet,
    path::{Path, PathBuf},
};

use chrono::Local;

use crate::{
    constants::empty_dir_op_kind,
    db::{op_record_repo, op_record_repo::OpRecordTables},
    error::{AppError, AppResult},
    models::{
        EmptyDirApplyItem, EmptyDirApplyResponse, EmptyDirPreviewItem, EmptyDirRecordDetail,
        EmptyDirRecordItem, EmptyDirRecordSummary, EmptyDirRollbackCheck, EmptyDirRollbackResponse,
    },
    services::op_pipeline::{self, MoveOutcome},
    utils::path::{cmp_key_case_insensitive, to_extended_length_path, to_user_friendly_path},
};

/// 空文件夹清理所使用的记录表描述符。
pub const EMPTY_DIR_TABLES: OpRecordTables = OpRecordTables {
    record_table: "empty_dir_records",
    item_table: "empty_dir_items",
    extra_summary_column: Some("kind"),
};

/// 递归预览可删除的空文件夹。
///
/// `include_roots = false` 时只返回任务输入路径的子目录；这是默认安全策略，
/// 避免用户只是想清理工作目录时把工作目录本身删掉。
pub fn preview_empty_dirs(
    paths: &[String],
    include_roots: bool,
) -> AppResult<Vec<EmptyDirPreviewItem>> {
    if paths.is_empty() {
        return Err(AppError::InvalidInput("至少需要一个路径".to_string()));
    }

    let mut result = vec![];
    let mut seen_roots = HashSet::new();

    for root in paths {
        let root_path = Path::new(root);
        let root_ep = to_extended_length_path(root_path);
        let metadata = std::fs::metadata(&root_ep)
            .map_err(|e| AppError::Io(format!("无法访问路径 {root}: {e}")))?;
        if !metadata.is_dir() {
            return Err(AppError::InvalidInput(format!("不是有效目录: {root}")));
        }

        let root_key = cmp_key_case_insensitive(&root_ep);
        if !seen_roots.insert(root_key) {
            continue;
        }

        collect_empty_dirs(&root_ep, 0, include_roots, &mut result);
    }

    let mut seen = HashSet::new();
    result.retain(|item| seen.insert(item.old_path.to_lowercase()));
    Ok(result)
}

/// 删除空文件夹并写入可撤回记录。
pub fn apply_empty_dir_cleanup(
    db_path: &Path,
    paths: &[String],
    include_roots: bool,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> AppResult<EmptyDirApplyResponse> {
    let mut preview = preview_empty_dirs(paths, include_roots)?;

    if let Some(selected) = selected_old_paths {
        preview.retain(|item| {
            selected
                .iter()
                .any(|root| is_same_or_child(root, &item.old_path))
        });
    }

    preview.sort_by_key(|item| Reverse(item.depth));

    let results: Vec<MoveOutcome> = preview
        .into_iter()
        .map(|item| {
            let old_path = item.old_path;
            let new_path = item.new_path;
            match delete_empty_dir(&old_path) {
                Ok(_) => (old_path, new_path, true, None),
                Err(err) => (old_path, new_path, false, Some(err)),
            }
        })
        .collect();

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    let response = op_pipeline::persist_apply_results(
        db_path,
        EMPTY_DIR_TABLES,
        empty_dir_op_kind::DELETE,
        name,
        paths,
        results,
    )?;

    Ok(EmptyDirApplyResponse {
        record_id: response.record_id,
        record_name: response.record_name,
        kind: response.kind,
        total: response.total,
        success: response.success,
        failed: response.failed,
        items: response.items.into_iter().map(to_apply_item).collect(),
    })
}

/// 列出全部空文件夹清理记录。
pub fn list_empty_dir_records(db_path: &Path) -> AppResult<Vec<EmptyDirRecordSummary>> {
    let rows = op_record_repo::list_records(db_path, EMPTY_DIR_TABLES, None)?;
    Ok(rows.into_iter().map(to_summary).collect())
}

/// 读取单个空文件夹清理记录的详情。
pub fn get_empty_dir_record_detail(
    db_path: &Path,
    record_id: &str,
) -> AppResult<EmptyDirRecordDetail> {
    let detail = op_record_repo::get_record_detail(db_path, EMPTY_DIR_TABLES, record_id)?;
    Ok(EmptyDirRecordDetail {
        summary: to_summary(detail.summary),
        items: detail.items.into_iter().map(to_record_item).collect(),
    })
}

/// 检查撤回可行性。
///
/// 空目录清理的撤回是 `create_dir_all(old_path)`，路径缺失反而是正常状态；
/// 只有目标路径已被非目录文件占用时才作为不可自动处理的路径返回。
pub fn check_rollback(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<EmptyDirRollbackCheck> {
    let selected = selected_success_items(db_path, record_id, item_ids)?;
    let mut blocked = vec![];
    let mut existing = 0usize;

    for item in &selected {
        let path = Path::new(&item.old_path);
        let ep = to_extended_length_path(path);
        if ep.is_dir() {
            existing += 1;
        } else if ep.exists() {
            blocked.push(item.old_path.clone());
        }
    }

    Ok(EmptyDirRollbackCheck {
        total_selected: selected.len(),
        existing_count: existing,
        missing_paths: blocked,
    })
}

/// 撤回空文件夹清理：按记录重新创建目录。
pub fn rollback_empty_dirs(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<EmptyDirRollbackResponse> {
    let selected = selected_success_items(db_path, record_id, item_ids.clone())?;
    let check = check_rollback(db_path, record_id, item_ids)?;
    if !force_ignore_missing && !check.missing_paths.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "存在 {} 个路径被非目录文件占用，请确认后使用 forceIgnoreMissing=true 再执行",
            check.missing_paths.len()
        )));
    }

    let mut results = Vec::with_capacity(selected.len());
    for item in selected {
        let path = PathBuf::from(&item.old_path);
        let ep = to_extended_length_path(&path);
        let result = if ep.is_dir() {
            Ok(())
        } else if ep.exists() {
            Err("目标路径已存在且不是目录".to_string())
        } else {
            std::fs::create_dir_all(&ep).map_err(|e| e.to_string())
        };
        results.push((item, result));
    }

    let now = Local::now().timestamp();
    let updates: Vec<(i64, bool, Option<&str>)> = results
        .iter()
        .map(|(item, result)| {
            (
                item.item_id,
                result.is_ok(),
                result.as_ref().err().map(String::as_str),
            )
        })
        .collect();
    op_record_repo::batch_update_rollback_results(db_path, EMPTY_DIR_TABLES, &updates, now)?;

    let mut success = 0usize;
    let mut failed = 0usize;
    let mut items = Vec::with_capacity(results.len());

    for (item, result) in results {
        match result {
            Ok(_) => {
                success += 1;
                items.push(EmptyDirApplyItem {
                    item_id: item.item_id,
                    old_path: item.old_path,
                    new_path: item.new_path,
                    status: "success".to_string(),
                    message: None,
                });
            }
            Err(err) => {
                failed += 1;
                items.push(EmptyDirApplyItem {
                    item_id: item.item_id,
                    old_path: item.old_path,
                    new_path: item.new_path,
                    status: "failed".to_string(),
                    message: Some(err),
                });
            }
        }
    }

    let status = if success > 0 && failed == 0 {
        "rolled_back"
    } else if success > 0 {
        "partially_rolled_back"
    } else {
        "applied"
    };
    let _ =
        op_record_repo::set_record_rollback_status(db_path, EMPTY_DIR_TABLES, record_id, status);

    Ok(EmptyDirRollbackResponse {
        record_id: record_id.to_string(),
        total_selected: check.total_selected,
        success,
        failed,
        skipped_missing: 0,
        items,
    })
}

/// 删除记录。
pub fn delete_empty_dir_record(db_path: &Path, record_id: &str) -> AppResult<()> {
    op_record_repo::delete_record(db_path, EMPTY_DIR_TABLES, record_id)
}

fn collect_empty_dirs(
    dir: &Path,
    depth: usize,
    include_current: bool,
    result: &mut Vec<EmptyDirPreviewItem>,
) -> bool {
    let dir_ep = to_extended_length_path(dir);
    let entries = match std::fs::read_dir(&dir_ep) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    let mut empty_after_cleanup = true;

    for entry in entries {
        let Ok(entry) = entry else {
            empty_after_cleanup = false;
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            empty_after_cleanup = false;
            continue;
        };

        if file_type.is_dir() {
            let child_empty = collect_empty_dirs(&entry.path(), depth + 1, true, result);
            if !child_empty {
                empty_after_cleanup = false;
            }
        } else {
            empty_after_cleanup = false;
        }
    }

    if empty_after_cleanup && include_current {
        let path = to_user_friendly_path(dir);
        result.push(EmptyDirPreviewItem {
            old_path: path.clone(),
            new_path: path,
            depth,
        });
    }

    empty_after_cleanup
}

fn delete_empty_dir(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    std::fs::remove_dir(to_extended_length_path(&path)).map_err(|e| e.to_string())
}

fn selected_success_items(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<Vec<op_record_repo::OpRecordItem>> {
    let detail = op_record_repo::get_record_detail(db_path, EMPTY_DIR_TABLES, record_id)?;
    let mut items: Vec<_> = detail
        .items
        .into_iter()
        .filter(|item| item.apply_success)
        .collect();

    let Some(ids) = item_ids else {
        return Ok(items);
    };

    let selected_paths: Vec<String> = items
        .iter()
        .filter(|item| ids.contains(&item.item_id))
        .map(|item| item.old_path.clone())
        .collect();

    items.retain(|item| {
        selected_paths
            .iter()
            .any(|root| is_same_or_child(root, &item.old_path))
    });
    Ok(items)
}

fn path_key(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn is_same_or_child(parent: &str, child: &str) -> bool {
    let parent_key = path_key(parent);
    let child_key = path_key(child);
    child_key == parent_key || child_key.starts_with(&format!("{parent_key}/"))
}

fn to_apply_item(item: crate::models::ModOpApplyItem) -> EmptyDirApplyItem {
    EmptyDirApplyItem {
        item_id: item.item_id,
        old_path: item.old_path,
        new_path: item.new_path,
        status: item.status,
        message: item.message,
    }
}

fn to_summary(summary: op_record_repo::OpRecordSummary) -> EmptyDirRecordSummary {
    EmptyDirRecordSummary {
        record_id: summary.record_id,
        record_name: summary.record_name,
        kind: summary.extra.unwrap_or_default(),
        created_at: summary.created_at,
        total_items: summary.total_items,
        success_items: summary.success_items,
        rollback_status: summary.rollback_status,
    }
}

fn to_record_item(item: op_record_repo::OpRecordItem) -> EmptyDirRecordItem {
    EmptyDirRecordItem {
        item_id: item.item_id,
        old_path: item.old_path,
        new_path: item.new_path,
        apply_success: item.apply_success,
        apply_error: item.apply_error,
        rollback_success: item.rollback_success,
        rollback_error: item.rollback_error,
    }
}
