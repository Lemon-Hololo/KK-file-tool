//! 图片相似度去重服务层。
//!
//! 与 [`super::mod_tools`] 同构：preview / apply / rollback 走 `op_pipeline`，
//! 备份目录解析有自己的 `backup` 子模块（与 mod_tools 备份目录设置相互独立），
//! 删除走"备份到 `<backup_root>/<record_id>/<原文件名>` 后真删原图"。
//!
//! 子模块：
//! - [`hash`]：单张图片的元数据 + 感知哈希计算（用 `image_hasher` v3）。
//! - [`scan`]：长任务入口，walk 候选 → 并行哈希 → 滚动分组 → 增量推送 partial。
//! - [`backup`]：备份根目录解析 + `(原路径, 备份路径)` 对的构造。
//! - [`apply`]：删除选中并写记录、检查撤回、执行撤回。
//!
//! 记录管理（list / detail / delete / rename）走 `op_record_repo` 通用 CRUD。

use crate::{
    db::op_record_repo::OpRecordTables,
    error::{AppError, AppResult},
    models::{
        ImageDedupRecordDetail, ImageDedupRecordItem, ImageDedupRecordSummary,
        ImageDedupRollbackCheck, ImageDedupRollbackResponse,
    },
    services::op_pipeline,
};

pub mod apply;
pub mod backup;
pub mod hash;
pub mod scan;

/// 图片相似度去重所使用的记录表描述符。
///
/// `kind` 当前固定 `"similarity_delete"`；保留 extra column 给未来扩展（如
/// "按相似度合并到目录"），与 `empty_dir_op_kind` 保持同样的"先单值、后扩展"风格。
pub const IMAGE_DEDUP_TABLES: OpRecordTables = OpRecordTables {
    record_table: "image_dedup_op_records",
    item_table: "image_dedup_op_items",
    extra_summary_column: Some("kind"),
};

/// 列出图片去重记录。
pub fn list_records(db_path: &std::path::Path) -> AppResult<Vec<ImageDedupRecordSummary>> {
    let rows = crate::db::op_record_repo::list_records(db_path, IMAGE_DEDUP_TABLES, None)?;
    Ok(rows.into_iter().map(to_summary).collect())
}

/// 读取单条图片去重记录的详情（含全部 item）。
pub fn get_record_detail(
    db_path: &std::path::Path,
    record_id: &str,
) -> AppResult<ImageDedupRecordDetail> {
    let d = crate::db::op_record_repo::get_record_detail(db_path, IMAGE_DEDUP_TABLES, record_id)?;
    Ok(ImageDedupRecordDetail {
        summary: to_summary(d.summary),
        items: d.items.into_iter().map(to_record_item).collect(),
    })
}

/// 检查撤回：返回 `new_path` 仍存在 / 缺失的统计。
///
/// 记录创建时若 `rollback_enabled = false`（用户关闭备份），立即返回错误避免做
/// 无意义的存在性检查。
pub fn check_rollback(
    db_path: &std::path::Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<ImageDedupRollbackCheck> {
    let detail =
        crate::db::op_record_repo::get_record_detail(db_path, IMAGE_DEDUP_TABLES, record_id)?;
    if !detail.summary.rollback_enabled {
        return Err(AppError::InvalidInput(
            "该记录创建时未启用备份，无法撤回".to_string(),
        ));
    }
    let r = op_pipeline::check_rollback_with_detail(db_path, &detail, item_ids.as_ref())?;
    Ok(ImageDedupRollbackCheck {
        total_selected: r.total_selected,
        existing_count: r.existing_count,
        missing_paths: r.missing_paths,
    })
}

/// 执行撤回：把已成功条目 `new_path → old_path` 回滚（即把备份覆盖回原图位置）。
pub fn rollback(
    db_path: &std::path::Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<ImageDedupRollbackResponse> {
    let r = op_pipeline::rollback(
        db_path,
        IMAGE_DEDUP_TABLES,
        record_id,
        item_ids,
        force_ignore_missing,
    )?;
    Ok(ImageDedupRollbackResponse {
        record_id: r.record_id,
        total_selected: r.total_selected,
        success: r.success,
        failed: r.failed,
        skipped_missing: r.skipped_missing,
        items: r.items.into_iter().map(Into::into).collect(),
    })
}

/// 删除记录；相关 item 通过 FK CASCADE 自动清除。
pub fn delete_record(db_path: &std::path::Path, record_id: &str) -> AppResult<()> {
    crate::db::op_record_repo::delete_record(db_path, IMAGE_DEDUP_TABLES, record_id)
}

/// 重命名记录。
pub fn rename_record(db_path: &std::path::Path, record_id: &str, new_name: &str) -> AppResult<()> {
    crate::db::op_record_repo::rename_record(db_path, IMAGE_DEDUP_TABLES, record_id, new_name)
}

fn to_summary(s: crate::db::op_record_repo::OpRecordSummary) -> ImageDedupRecordSummary {
    ImageDedupRecordSummary {
        record_id: s.record_id,
        record_name: s.record_name,
        kind: s.extra.unwrap_or_default(),
        created_at: s.created_at,
        total_items: s.total_items,
        success_items: s.success_items,
        rollback_status: s.rollback_status,
        rollback_enabled: s.rollback_enabled,
    }
}

fn to_record_item(i: crate::db::op_record_repo::OpRecordItem) -> ImageDedupRecordItem {
    ImageDedupRecordItem {
        item_id: i.item_id,
        old_path: i.old_path,
        new_path: i.new_path,
        apply_success: i.apply_success,
        apply_error: i.apply_error,
        rollback_success: i.rollback_success,
        rollback_error: i.rollback_error,
    }
}
