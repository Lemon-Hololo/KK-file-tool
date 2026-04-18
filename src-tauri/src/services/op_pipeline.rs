//! 统一的"记录型操作"流水线（preview → apply → rollback）。
//!
//! `services::suffix` 与 `services::mod_tools::{rename, organize}` 共享相同的
//! 语义：
//!
//! 1. `preview`：业务规则计算出一批 `(old_path, new_path)`；
//! 2. `apply`：把这批 pair 写入 `op_record_repo` 所代表的记录表，再并行执行
//!    `std::fs::rename`（可选地预创建目标父目录），把结果写回 item 表；
//! 3. `rollback`：读取记录 item，对 `apply_success = true` 的项把 `new_path`
//!    重命名回 `old_path`，更新回滚字段，并维护记录整体的 `rollback_status`。
//!
//! 本模块是这三个动作的唯一实现，业务侧只负责产生 `pairs` 与传入
//! `OpRecordTables` / 附加列值。
//!
//! # 线程池
//! 并发度统一由 [`resolve_thread_count`] 从用户设置读取。`parallel_move` /
//! `check_rollback` / `rollback` 均构造临时 rayon 线程池执行并行操作，
//! 避免影响全局默认池。

use std::path::{Path, PathBuf};

use chrono::Local;
use rayon::prelude::*;
use rayon::ThreadPool;

use crate::{
    db::{op_record_repo, settings_repo},
    error::{AppError, AppResult},
    models::{ModOpApplyItem, ModOpApplyResponse, ModOpRollbackCheck, ModOpRollbackResponse},
    utils::path::to_extended_length_path,
};

pub use crate::db::op_record_repo::OpRecordTables;

/// 读取用户设置解析有效线程数；`0 / 未配置 → num_cpus`，且至少为 1。
pub fn resolve_thread_count(db_path: &Path) -> usize {
    let configured = settings_repo::get_settings(db_path)
        .map(|s| s.thread_count)
        .unwrap_or(0);
    if configured > 0 {
        (configured as usize).min(num_cpus::get()).max(1)
    } else {
        num_cpus::get().max(1)
    }
}

/// 构造一个临时 rayon 线程池用于本次并行操作；避免污染全局池。
fn rayon_pool(n: usize) -> AppResult<ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n.max(1))
        .build()
        .map_err(|e| AppError::Internal(format!("创建线程池失败: {e}")))
}

/// `parallel_move` 每条输入的执行结果：
/// `(old_path, new_path, 是否成功, 错误消息)`。
pub type MoveOutcome = (String, String, bool, Option<String>);

/// 并行执行 `std::fs::rename`。`create_parent = true` 时在 rename 前确保
/// 目标父目录存在（用于"归类"类操作）。
pub fn parallel_move(
    pairs: Vec<(String, String)>,
    create_parent: bool,
    thread_count: usize,
) -> AppResult<Vec<MoveOutcome>> {
    let pool = rayon_pool(thread_count)?;
    Ok(pool.install(|| {
        pairs
            .into_par_iter()
            .map(|(old_path, new_path)| {
                let old_p = PathBuf::from(&old_path);
                let new_p = PathBuf::from(&new_path);

                if create_parent {
                    if let Some(parent) = new_p.parent() {
                        if !to_extended_length_path(parent).exists() {
                            if let Err(e) =
                                std::fs::create_dir_all(to_extended_length_path(parent))
                            {
                                return (old_path, new_path, false, Some(e.to_string()));
                            }
                        }
                    }
                }

                match std::fs::rename(
                    to_extended_length_path(&old_p),
                    to_extended_length_path(&new_p),
                ) {
                    Ok(_) => (old_path, new_path, true, None),
                    Err(e) => (old_path, new_path, false, Some(e.to_string())),
                }
            })
            .collect()
    }))
}

/// 写入 `op_records`，并行执行 rename，最后把每条结果写回 `op_items`。
///
/// 返回的 `ModOpApplyResponse` 既适用于 Mod 工具也适用于 Suffix（字段同构；
/// `kind` 字段在 Suffix 场景下由调用方填入空字符串即可，前端模型已忽略）。
///
/// # 参数
/// - `extra_value`：`tables.extra_summary_column` 对应列要写入的值；
///   如 Mod 工具传 `"rename"` / `"organize"`，Suffix 传 `.txt` 这样的后缀串。
/// - `create_parent`：rename 前是否自动创建目标父目录；Suffix 不需要，
///   Organize 需要。
pub fn persist_apply_rename_pairs(
    db_path: &Path,
    tables: OpRecordTables,
    extra_value: &str,
    record_name: String,
    source_paths: &[String],
    pairs: Vec<(String, String)>,
    create_parent: bool,
) -> AppResult<ModOpApplyResponse> {
    let thread_count = resolve_thread_count(db_path);

    let created_at = Local::now().timestamp();
    let source_paths_json =
        serde_json::to_string(source_paths).map_err(|e| AppError::Internal(e.to_string()))?;
    let record_id = op_record_repo::create_record(
        db_path,
        tables,
        &record_name,
        extra_value,
        &source_paths_json,
        created_at,
    )?;

    let results = parallel_move(pairs, create_parent, thread_count)?;

    let now = Local::now().timestamp();
    let item_ids = op_record_repo::batch_insert_items(
        db_path,
        tables,
        &record_id,
        &results
            .iter()
            .map(|(o, n, ok, msg)| (o.as_str(), n.as_str(), *ok, msg.as_deref()))
            .collect::<Vec<_>>(),
        now,
    )?;

    let mut items = Vec::with_capacity(results.len());
    let mut success = 0usize;
    let mut failed = 0usize;

    for (i, (old_path, new_path, ok, msg)) in results.into_iter().enumerate() {
        if ok {
            success += 1;
        } else {
            failed += 1;
        }
        items.push(ModOpApplyItem {
            item_id: item_ids[i],
            old_path,
            new_path,
            status: if ok { "success".into() } else { "failed".into() },
            message: msg,
        });
    }

    Ok(ModOpApplyResponse {
        record_id,
        record_name,
        kind: extra_value.to_string(),
        total: items.len(),
        success,
        failed,
        items,
    })
}

/// 检查 `record_id` 下（可选按 `item_ids` 过滤）的记录项对应的 `new_path`
/// 是否仍存在于文件系统，用于撤回前的风险提示。
pub fn check_rollback(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> AppResult<ModOpRollbackCheck> {
    let detail = op_record_repo::get_record_detail(db_path, tables, record_id)?;
    let selected: Vec<_> = detail
        .items
        .into_iter()
        .filter(|x| {
            item_ids
                .as_ref()
                .map(|ids| ids.contains(&x.item_id))
                .unwrap_or(true)
        })
        .filter(|x| x.apply_success)
        .collect();

    let pool = rayon_pool(resolve_thread_count(db_path))?;
    let checks: Vec<(bool, String)> = pool.install(|| {
        selected
            .par_iter()
            .map(|item| {
                let exists = to_extended_length_path(Path::new(&item.new_path)).exists();
                (exists, item.new_path.clone())
            })
            .collect()
    });

    let mut existing = 0usize;
    let mut missing = vec![];
    for (exists, path) in checks {
        if exists {
            existing += 1;
        } else {
            missing.push(path);
        }
    }

    Ok(ModOpRollbackCheck {
        total_selected: selected.len(),
        existing_count: existing,
        missing_paths: missing,
    })
}

/// 把 `record_id` 下（可选按 `item_ids` 过滤）的已成功条目按 `new_path → old_path`
/// 回滚。`force_ignore_missing = false` 时若存在缺失路径会拒绝执行。
///
/// 同步更新每条 item 的 `rollback_success / rollback_error`，并根据整体
/// 成功率设置记录的 `rollback_status`。
pub fn rollback(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<ModOpRollbackResponse> {
    let detail = op_record_repo::get_record_detail(db_path, tables, record_id)?;
    let selected: Vec<_> = detail
        .items
        .into_iter()
        .filter(|x| {
            item_ids
                .as_ref()
                .map(|ids| ids.contains(&x.item_id))
                .unwrap_or(true)
        })
        .filter(|x| x.apply_success)
        .collect();

    let check = check_rollback(db_path, tables, record_id, item_ids.clone())?;
    if !force_ignore_missing && !check.missing_paths.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "存在 {} 个文件缺失，请确认后使用 forceIgnoreMissing=true 再执行",
            check.missing_paths.len()
        )));
    }

    let pool = rayon_pool(resolve_thread_count(db_path))?;
    let results: Vec<_> = pool.install(|| {
        selected
            .into_par_iter()
            .map(|item| {
                let current = PathBuf::from(&item.new_path);
                let old = PathBuf::from(&item.old_path);

                if !to_extended_length_path(&current).exists() {
                    return (item, "skipped_missing", None::<String>);
                }

                // 回滚时也需要保证老的父目录存在（归类回滚时子目录已删除的可能性较低，
                // 但 rename 场景老目录一定存在；统一处理更稳妥）
                if let Some(parent) = old.parent() {
                    if !to_extended_length_path(parent).exists() {
                        if let Err(e) = std::fs::create_dir_all(to_extended_length_path(parent)) {
                            return (item, "failed", Some(e.to_string()));
                        }
                    }
                }

                match std::fs::rename(
                    to_extended_length_path(&current),
                    to_extended_length_path(&old),
                ) {
                    Ok(_) => (item, "success", None),
                    Err(e) => (item, "failed", Some(e.to_string())),
                }
            })
            .collect()
    });

    let now = Local::now().timestamp();
    let mut success = 0usize;
    let mut failed = 0usize;
    let mut skipped_missing = 0usize;
    let mut items = Vec::with_capacity(results.len());

    let updates: Vec<(i64, bool, Option<&str>)> = results
        .iter()
        .map(|(item, status, err)| {
            let (ok, err_msg) = match *status {
                "success" => (true, None),
                "skipped_missing" => (false, Some("SKIPPED_NOT_FOUND")),
                _ => (false, err.as_deref()),
            };
            (item.item_id, ok, err_msg)
        })
        .collect();

    op_record_repo::batch_update_rollback_results(db_path, tables, &updates, now)?;

    for (item, status, err) in results {
        match status {
            "success" => success += 1,
            "skipped_missing" => skipped_missing += 1,
            _ => failed += 1,
        }
        items.push(ModOpApplyItem {
            item_id: item.item_id,
            old_path: item.old_path,
            new_path: item.new_path,
            status: if status == "success" {
                "success".into()
            } else {
                "failed".into()
            },
            message: match status {
                "skipped_missing" => Some("SKIPPED_NOT_FOUND".into()),
                "failed" => err,
                _ => None,
            },
        });
    }

    let status = if success > 0 && failed == 0 && skipped_missing == 0 {
        "rolled_back"
    } else if success > 0 {
        "partially_rolled_back"
    } else {
        "applied"
    };
    let _ = op_record_repo::set_record_rollback_status(db_path, tables, record_id, status);

    Ok(ModOpRollbackResponse {
        record_id: record_id.to_string(),
        total_selected: check.total_selected,
        success,
        failed,
        skipped_missing,
        items,
    })
}
