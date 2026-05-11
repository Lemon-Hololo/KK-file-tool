//! 应用图片相似度去重（删除选中并写记录）。
//!
//! 与 `mod_tools::cleanup::apply_mod_delete` 同型：
//! - 启用回滚：把选中图复制到 `<backup_root>/<record_id>/<原文件名>`，再 rename
//!   原图到备份位置（`op_pipeline::rename_or_copy_delete` 跨卷自动退化为
//!   copy + delete）；item 记 `(原路径, 备份路径)`，`op_pipeline::rollback`
//!   通过 `rename(backup → original)` 自动撤回。
//! - 关闭回滚：直接 `remove_file` 真删；item.new_path 写空字符串，记录主表
//!   `rollback_enabled = 0`，前后端双重拒绝撤回。

use std::path::Path;

use crate::{
    constants::image_dedup_op_kind,
    error::{AppError, AppResult},
    models::{ImageDedupApplyItem, ImageDedupApplyResponse},
    services::{
        image_dedup::{backup, IMAGE_DEDUP_TABLES},
        logging::TaskLogContext,
        op_pipeline,
    },
    utils::path::to_extended_length_path,
};

/// 删除选中的相似图片并写入可撤回记录。
///
/// `paths`：当前任务的输入路径（仅用于记录元数据中的 `source_paths`）；
/// `selected_file_paths`：要删除的具体图片路径列表（前端按用户勾选传入，**已经
/// 排除每组的 keep**——后端不参与"哪一张是 keep"的判定）。
pub fn apply_image_dedup_delete(
    db_path: &Path,
    paths: &[String],
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    log: Option<TaskLogContext>,
) -> AppResult<ImageDedupApplyResponse> {
    if selected_file_paths.is_empty() {
        return Err(AppError::InvalidInput("请先选择要删除的图片".to_string()));
    }

    let prepared = backup::prepare_backup(db_path, selected_file_paths)?;

    if let Some(log) = &log {
        let total = prepared.pairs.len();
        let action = if prepared.rollback_enabled {
            "删除并备份"
        } else {
            "直接删除（不备份）"
        };
        log.info(&format!("准备{action} {total} 张图片"));
        const SAMPLE: usize = 5;
        for (old_path, _) in prepared.pairs.iter().take(SAMPLE) {
            log.info(&format!("  · {old_path}"));
        }
        if total > SAMPLE {
            log.info(&format!("  …（其余 {} 条略，详情见记录）", total - SAMPLE));
        }
    }

    let executor = move |old: &str, new: &str| -> Result<(), String> {
        if new.is_empty() {
            // 关闭回滚 → 真删
            std::fs::remove_file(to_extended_length_path(Path::new(old))).map_err(|e| e.to_string())
        } else {
            op_pipeline::rename_or_copy_delete(Path::new(old), Path::new(new))
        }
    };

    let name = op_pipeline::record_name_or_timestamp(record_name);
    let response = op_pipeline::persist_apply_with_executor(
        db_path,
        IMAGE_DEDUP_TABLES,
        &prepared.record_id,
        prepared.rollback_enabled,
        image_dedup_op_kind::SIMILARITY_DELETE,
        name,
        paths,
        prepared.pairs,
        executor,
    )?;

    Ok(ImageDedupApplyResponse {
        record_id: response.record_id,
        record_name: response.record_name,
        kind: response.kind,
        rollback_enabled: response.rollback_enabled,
        total: response.total,
        success: response.success,
        failed: response.failed,
        items: response
            .items
            .into_iter()
            .map(ImageDedupApplyItem::from)
            .collect(),
    })
}
