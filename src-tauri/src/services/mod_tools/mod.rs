//! Mod 工具服务层。
//!
//! 三个功能共享 `op_pipeline` 流水线：
//! - [`rename`]：读取 zipmod 内 `manifest.xml` 的 `guid/author/version`，生成 `[author] guid-version.zipmod`。
//! - [`organize`]：按文件名首个 `[...]` 建子目录并归类（非递归）。
//! - [`cleanup`]：按 `guid/author/version` 检查重复与不同版本，并把删除移动到可回滚备份。
//! - [`scan`]：长任务，扫描 zipmod 查找含指定 `<game>` 关键字的条目。
//!
//! 记录管理（list / detail / rollback / delete / rename）统一通过
//! [`records`] 模块转发到 `op_record_repo`。

use crate::{
    constants::mod_op_kind,
    db::op_record_repo::OpRecordTables,
    error::{AppError, AppResult},
    models::{
        ModOpApplyItem, ModOpRecordDetail, ModOpRecordItem, ModOpRecordSummary, ModOpRollbackCheck,
        ModOpRollbackResponse,
    },
    services::op_pipeline,
};

pub mod cleanup;
pub mod modify;
pub mod organize;
pub mod rename;
pub mod scan;
pub mod zipmod;

/// Mod 工具所使用的记录表描述符。
pub const MOD_OP_TABLES: OpRecordTables = OpRecordTables {
    record_table: "mod_op_records",
    item_table: "mod_op_items",
    extra_summary_column: Some("kind"),
};

/// 列出 Mod 操作记录；`kind` 可传 `Some("rename")` / `Some("organize")` 过滤。
pub fn list_records(
    db_path: &std::path::Path,
    kind: Option<&str>,
) -> AppResult<Vec<ModOpRecordSummary>> {
    if let Some(k) = kind {
        if !mod_op_kind::is_valid(k) {
            return Err(AppError::InvalidInput(format!("非法 kind: {k}")));
        }
    }
    let rows = crate::db::op_record_repo::list_records(db_path, MOD_OP_TABLES, kind)?;
    Ok(rows.into_iter().map(to_summary).collect())
}

/// 读取单个 Mod 操作记录的详情（含全部 item）。
pub fn get_record_detail(
    db_path: &std::path::Path,
    record_id: &str,
) -> AppResult<ModOpRecordDetail> {
    let d = crate::db::op_record_repo::get_record_detail(db_path, MOD_OP_TABLES, record_id)?;
    Ok(ModOpRecordDetail {
        summary: to_summary(d.summary),
        items: d.items.into_iter().map(to_record_item).collect(),
    })
}

/// 检查撤回：返回 `new_path` 仍存在 / 缺失的统计。
pub fn check_rollback(
    db_path: &std::path::Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<ModOpRollbackCheck> {
    op_pipeline::check_rollback(db_path, MOD_OP_TABLES, record_id, item_ids)
}

/// 执行撤回：把已成功条目 `new_path → old_path` 回滚。
pub fn rollback(
    db_path: &std::path::Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<ModOpRollbackResponse> {
    op_pipeline::rollback(
        db_path,
        MOD_OP_TABLES,
        record_id,
        item_ids,
        force_ignore_missing,
    )
}

/// 删除记录；相关 item 通过 FK CASCADE 自动清除。
pub fn delete_record(db_path: &std::path::Path, record_id: &str) -> AppResult<()> {
    crate::db::op_record_repo::delete_record(db_path, MOD_OP_TABLES, record_id)
}

/// 重命名记录。
pub fn rename_record(db_path: &std::path::Path, record_id: &str, new_name: &str) -> AppResult<()> {
    crate::db::op_record_repo::rename_record(db_path, MOD_OP_TABLES, record_id, new_name)
}

fn to_summary(s: crate::db::op_record_repo::OpRecordSummary) -> ModOpRecordSummary {
    ModOpRecordSummary {
        record_id: s.record_id,
        record_name: s.record_name,
        kind: s.extra.unwrap_or_default(),
        created_at: s.created_at,
        total_items: s.total_items,
        success_items: s.success_items,
        rollback_status: s.rollback_status,
    }
}

fn to_record_item(i: crate::db::op_record_repo::OpRecordItem) -> ModOpRecordItem {
    ModOpRecordItem {
        item_id: i.item_id,
        old_path: i.old_path,
        new_path: i.new_path,
        apply_success: i.apply_success,
        apply_error: i.apply_error,
        rollback_success: i.rollback_success,
        rollback_error: i.rollback_error,
    }
}

/// 把 apply 结果中保留的通用 `ModOpApplyItem` 直接透传（当前与流水线输出一致）。
pub type ApplyItem = ModOpApplyItem;
