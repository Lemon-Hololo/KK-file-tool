//! 后缀批量修改服务。
//!
//! 业务规则很薄：递归扫描 → 生成 `(old_path, new_path)` → 委托给
//! [`op_pipeline`] 执行写库 + 并行 rename。记录管理 (list / detail /
//! rollback / delete) 同样通过 [`op_pipeline`] 与 `op_record_repo` 完成。

use std::path::{Path, PathBuf};

use chrono::Local;
use walkdir::WalkDir;

use crate::{
    db::{op_record_repo, op_record_repo::OpRecordTables},
    error::{AppError, AppResult},
    models::{
        SuffixApplyItem, SuffixApplyResponse, SuffixPreviewItem, SuffixRecordDetail,
        SuffixRecordItem, SuffixRecordSummary, SuffixRollbackCheck, SuffixRollbackResponse,
    },
    services::op_pipeline,
    utils::{
        filename::{normalize_suffix, resolve_conflict, split_name_ext},
        path::to_user_friendly_path,
    },
};

/// 后缀修改所使用的记录表描述符。
pub const SUFFIX_TABLES: OpRecordTables = OpRecordTables {
    record_table: "suffix_change_records",
    item_table: "suffix_change_items",
    extra_summary_column: Some("target_suffix"),
};

fn build_target_path(path: &Path, target_suffix: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let (stem, _) = split_name_ext(&file_name);
    Some(parent.join(format!("{stem}{target_suffix}")))
}

/// 预览：对 `paths` 下每个文件计算新路径，并标记是否会与现有文件冲突。
pub fn preview_suffix_change(
    paths: &[String],
    target_suffix_input: &str,
) -> AppResult<Vec<SuffixPreviewItem>> {
    let target_suffix = normalize_suffix(target_suffix_input);
    if target_suffix.is_empty() {
        return Err(AppError::InvalidInput("目标后缀不能为空".to_string()));
    }

    let mut result = vec![];
    for root in paths {
        for e in WalkDir::new(root).into_iter().filter_map(|x| x.ok()) {
            if !e.file_type().is_file() {
                continue;
            }

            let old_path = e.path().to_path_buf();
            let Some(candidate) = build_target_path(&old_path, &target_suffix) else {
                continue;
            };

            if candidate == old_path {
                continue;
            }

            let (final_target, conflict) = resolve_conflict(candidate);

            result.push(SuffixPreviewItem {
                old_path: to_user_friendly_path(&old_path),
                new_path: to_user_friendly_path(&final_target),
                will_rename_conflict: conflict,
            });
        }
    }

    Ok(result)
}

/// 应用：按预览结果批量 rename，写入记录表。
pub fn apply_suffix_change(
    db_path: &Path,
    paths: &[String],
    target_suffix_input: &str,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> AppResult<SuffixApplyResponse> {
    let target_suffix = normalize_suffix(target_suffix_input);
    if target_suffix.is_empty() {
        return Err(AppError::InvalidInput("目标后缀不能为空".to_string()));
    }

    let mut preview = preview_suffix_change(paths, &target_suffix)?;

    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let pairs: Vec<(String, String)> = preview
        .into_iter()
        .map(|p| (p.old_path, p.new_path))
        .collect();

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());

    let resp = op_pipeline::persist_apply_rename_pairs(
        db_path,
        SUFFIX_TABLES,
        &target_suffix,
        name,
        paths,
        pairs,
        false,
    )?;

    Ok(SuffixApplyResponse {
        record_id: resp.record_id,
        record_name: resp.record_name,
        total: resp.total,
        success: resp.success,
        failed: resp.failed,
        items: resp
            .items
            .into_iter()
            .map(|i| SuffixApplyItem {
                item_id: i.item_id,
                old_path: i.old_path,
                new_path: i.new_path,
                status: i.status,
                message: i.message,
            })
            .collect(),
    })
}

/// 列出所有后缀修改记录。
pub fn list_suffix_records(db_path: &Path) -> AppResult<Vec<SuffixRecordSummary>> {
    let rows = op_record_repo::list_records(db_path, SUFFIX_TABLES, None)?;
    Ok(rows.into_iter().map(to_summary).collect())
}

/// 读取单个后缀修改记录的详情。
pub fn get_suffix_record_detail(
    db_path: &Path,
    record_id: &str,
) -> AppResult<SuffixRecordDetail> {
    let d = op_record_repo::get_record_detail(db_path, SUFFIX_TABLES, record_id)?;
    Ok(SuffixRecordDetail {
        summary: to_summary(d.summary),
        items: d
            .items
            .into_iter()
            .map(|i| SuffixRecordItem {
                item_id: i.item_id,
                old_path: i.old_path,
                new_path: i.new_path,
                apply_success: i.apply_success,
                apply_error: i.apply_error,
                rollback_success: i.rollback_success,
                rollback_error: i.rollback_error,
            })
            .collect(),
    })
}

/// 检查撤回：返回 `new_path` 仍存在 / 缺失的统计。
pub fn check_rollback(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<SuffixRollbackCheck> {
    let c = op_pipeline::check_rollback(db_path, SUFFIX_TABLES, record_id, item_ids)?;
    Ok(SuffixRollbackCheck {
        total_selected: c.total_selected,
        existing_count: c.existing_count,
        missing_paths: c.missing_paths,
    })
}

/// 执行撤回。
pub fn rollback_suffix_change(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<SuffixRollbackResponse> {
    let r = op_pipeline::rollback(
        db_path,
        SUFFIX_TABLES,
        record_id,
        item_ids,
        force_ignore_missing,
    )?;
    Ok(SuffixRollbackResponse {
        record_id: r.record_id,
        total_selected: r.total_selected,
        success: r.success,
        failed: r.failed,
        skipped_missing: r.skipped_missing,
        items: r
            .items
            .into_iter()
            .map(|i| SuffixApplyItem {
                item_id: i.item_id,
                old_path: i.old_path,
                new_path: i.new_path,
                status: i.status,
                message: i.message,
            })
            .collect(),
    })
}

/// 删除记录。
pub fn delete_suffix_record(db_path: &Path, record_id: &str) -> AppResult<()> {
    op_record_repo::delete_record(db_path, SUFFIX_TABLES, record_id)
}

fn to_summary(s: op_record_repo::OpRecordSummary) -> SuffixRecordSummary {
    SuffixRecordSummary {
        record_id: s.record_id,
        record_name: s.record_name,
        target_suffix: s.extra.unwrap_or_default(),
        created_at: s.created_at,
        total_items: s.total_items,
        success_items: s.success_items,
        rollback_status: s.rollback_status,
    }
}
