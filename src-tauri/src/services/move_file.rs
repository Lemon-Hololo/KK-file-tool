//! 去重后"移动重复文件"的执行层。
//!
//! 仅负责按目标目录批量 rename 文件，统计成功/失败条目；持久化为
//! `move_reports` 的工作在 [`crate::commands::move_file`] 调用后完成。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{
    models::{MoveFailureItem, MoveSuccessItem},
    services::op_pipeline,
    utils::{
        filename::resolve_conflict_with_reserved,
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

/// `move_selected_files` 的聚合结果。
pub struct MoveResult {
    pub success_items: Vec<MoveSuccessItem>,
    pub failed_items: Vec<MoveFailureItem>,
    pub released_bytes: u64,
}

/// 把 `selected_files` 全部移动到 `target_dir`；命名冲突通过
/// [`resolve_conflict_with_reserved`] 追加 ` (N)` 后缀。
///
/// 源文件缺失 / metadata 读失败 / 文件名非法 / rename 失败都记录为
/// `failed_items`，不中断批次。实际移动复用
/// [`op_pipeline::rename_or_copy_delete`]，目标目录跨卷时自动 copy + delete。
pub fn move_selected_files(target_dir: &Path, selected_files: &[String]) -> MoveResult {
    let target_ext = to_extended_length_path(target_dir);
    let _ = std::fs::create_dir_all(target_ext);

    let mut success_items = vec![];
    let mut failed_items = vec![];
    let mut released_bytes = 0u64;
    let mut reserved_targets = HashSet::new();

    for src in selected_files {
        let src_path = PathBuf::from(src);
        let src_ext = to_extended_length_path(&src_path);

        if !src_ext.exists() {
            failed_items.push(MoveFailureItem {
                src_path: src.clone(),
                error_code: "NOT_FOUND".to_string(),
                error_message: "源文件不存在".to_string(),
            });
            continue;
        }

        let meta = match std::fs::metadata(&src_ext) {
            Ok(m) => m,
            Err(e) => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "META_FAILED".to_string(),
                    error_message: e.to_string(),
                });
                continue;
            }
        };

        let file_name = match src_path.file_name().and_then(|x| x.to_str()) {
            Some(n) => n.to_string(),
            None => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "BAD_FILE_NAME".to_string(),
                    error_message: "文件名无效".to_string(),
                });
                continue;
            }
        };

        let (dst, _conflicted) =
            resolve_conflict_with_reserved(target_dir.join(&file_name), &mut reserved_targets);

        match op_pipeline::rename_or_copy_delete(&src_path, &dst) {
            Ok(_) => {
                released_bytes += meta.len();
                success_items.push(MoveSuccessItem {
                    src_path: src.clone(),
                    dst_path: to_user_friendly_path(&dst),
                    size: meta.len(),
                });
            }
            Err(e) => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "MOVE_FAILED".to_string(),
                    error_message: e.to_string(),
                });
            }
        }
    }

    MoveResult {
        success_items,
        failed_items,
        released_bytes,
    }
}
