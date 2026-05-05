//! 统一的"记录型操作"流水线（preview → apply → rollback）。
//!
//! `services::suffix`、`services::empty_dirs` 与 `services::mod_tools::{rename, organize}`
//! 共享相同的
//! 语义：
//!
//! 1. `preview`：业务规则计算出一批 `(old_path, new_path)`；
//! 2. `apply`：把这批 pair 写入 `op_record_repo` 所代表的记录表，再并行执行
//!    `std::fs::rename`（可选地预创建目标父目录），把结果写回 item 表；
//! 3. `rollback`：读取记录 item，对 `apply_success = true` 的项把 `new_path`
//!    重命名回 `old_path`，更新回滚字段，并维护记录整体的 `rollback_status`。
//!    空文件夹清理由业务侧自定义撤回为重新创建目录。
//!
//! 本模块是这三个动作的唯一实现，业务侧只负责产生 `pairs` 与传入
//! `OpRecordTables` / 附加列值。
//!
//! # 线程池
//! 并发度统一由 [`resolve_thread_count`] 从用户设置读取。`parallel_move` /
//! `check_rollback` / `rollback` 均构造临时 rayon 线程池执行并行操作，
//! 避免影响全局默认池。
//!
//! # 关于 `too_many_arguments`
//! 流水线核心 helper 都至少要 7+ 个参数：db 路径 + 表描述 + 业务字段 + 三类
//! 数据集合 + 各种执行控制参数。把它们打包成结构体只会让调用方把 9 个 setter
//! 写成 9 行 builder——读起来更糟。统一在模块级 allow，不再逐个加属性。
#![allow(clippy::too_many_arguments)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Local;
use rayon::prelude::*;
use rayon::ThreadPool;

use crate::{
    config::{
        DEFAULT_IO_CONCURRENCY_MULTIPLIER, DEFAULT_TEXT_PREVIEW_MAX_KB,
        DEFAULT_ZIP_PREVIEW_MAX_ENTRIES,
    },
    db::{op_record_repo, settings_repo},
    error::{AppError, AppResult},
    models::{ModOpApplyItem, ModOpApplyResponse, ModOpRollbackCheck, ModOpRollbackResponse},
    utils::{
        filename::resolve_conflict_with_reserved,
        path::{to_extended_length_path, to_user_friendly_path},
    },
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

/// 读取 IO 并发倍率（`有效线程数 × 本倍率` 作为 dedup/Mod 扫描的 Semaphore 许可）。
///
/// 用户配置无效（≤ 0 或读取失败）时退回 `DEFAULT_IO_CONCURRENCY_MULTIPLIER`；
/// 上限硬压在 16，防止意外配置把许可数撑爆。
pub fn resolve_io_concurrency_multiplier(db_path: &Path) -> usize {
    let configured = settings_repo::get_settings(db_path)
        .map(|s| s.io_concurrency_multiplier)
        .unwrap_or(DEFAULT_IO_CONCURRENCY_MULTIPLIER);
    if configured >= 1 {
        (configured as usize).min(16)
    } else {
        DEFAULT_IO_CONCURRENCY_MULTIPLIER as usize
    }
}

/// 读取文本预览最大字节数（设置里存的是 KB）。
pub fn resolve_text_preview_max_bytes(db_path: &Path) -> usize {
    let kb = settings_repo::get_settings(db_path)
        .map(|s| s.text_preview_max_kb)
        .unwrap_or(DEFAULT_TEXT_PREVIEW_MAX_KB);
    let kb = if kb >= 1 {
        kb
    } else {
        DEFAULT_TEXT_PREVIEW_MAX_KB
    };
    (kb as usize).saturating_mul(1024)
}

/// 读取压缩包预览最大条目数。
pub fn resolve_zip_preview_max_entries(db_path: &Path) -> usize {
    let v = settings_repo::get_settings(db_path)
        .map(|s| s.zip_preview_max_entries)
        .unwrap_or(DEFAULT_ZIP_PREVIEW_MAX_ENTRIES);
    if v >= 1 {
        v as usize
    } else {
        DEFAULT_ZIP_PREVIEW_MAX_ENTRIES as usize
    }
}

/// 构造一个临时 rayon 线程池用于本次并行操作；避免污染全局池。
///
/// 调用方传入已经解析好的线程数；记录流水线内部通常配合
/// [`resolve_thread_count`] 使用。
pub fn rayon_pool(n: usize) -> AppResult<ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n.max(1))
        .build()
        .map_err(|e| AppError::Internal(format!("创建线程池失败: {e}")))
}

/// 解析记录名：用户未传入时使用统一的时间戳格式。
///
/// 记录型操作和哈希记录都使用同一格式，避免各服务模块重复硬编码
/// `"%Y-%m-%d_%H-%M-%S"`。
pub fn record_name_or_timestamp(record_name: Option<String>) -> String {
    record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string())
}

/// 按 `selected_old_paths` 过滤预览列表（`None` 表示不过滤，保留全部）。
///
/// 后缀修改、Mod 重命名、Mod 归类的 apply 阶段都接受可选的
/// `selected_old_paths` 把预览结果再筛一遍——这套"none 即全选 / Some(list) 转
/// `HashSet` 后 retain"模式各模块写过三份。统一在这里处理避免漂移：未来如果
/// 改成大小写不敏感比较或归一化路径，只需要改这一处。
///
/// `get_old_path` 闭包从 item 取出"老路径"字段；调用方传入对应字段访问器
/// （`SuffixPreviewItem` / `ModRenamePreviewItem` / `ModOrganizePreviewItem`
/// 三者的字段名都叫 `old_path`，但类型不同，所以无法用统一 trait 抽象）。
pub fn filter_by_selected_old_paths<T, F>(
    items: &mut Vec<T>,
    selected_old_paths: Option<Vec<String>>,
    get_old_path: F,
) where
    F: Fn(&T) -> &str,
{
    let Some(selected) = selected_old_paths else {
        return;
    };
    let set: HashSet<String> = selected.into_iter().collect();
    items.retain(|x| set.contains(get_old_path(x)));
}

/// `parallel_move` 每条输入的执行结果：
/// `(old_path, new_path, 是否成功, 错误消息)`。
pub type MoveOutcome = (String, String, bool, Option<String>);

/// rename，跨卷失败时退化为 copy + remove。
///
/// `std::fs::rename` 跨卷会以 Windows 的 `ERROR_NOT_SAME_DEVICE`(17) /
/// Linux 的 `EXDEV`(18) 失败。Mod 备份目录可能位于和源文件不同的磁盘
/// （例如源在 D:\，备份目录配置在 C:\），需要 copy + remove 兜底。
/// 同卷场景下走 rename 仍是常数时间，性能不受影响。
pub fn rename_or_copy_delete(src: &Path, dst: &Path) -> Result<(), String> {
    let src_ep = to_extended_length_path(src);
    let dst_ep = to_extended_length_path(dst);

    match std::fs::rename(&src_ep, &dst_ep) {
        Ok(_) => Ok(()),
        Err(e) if is_cross_device_error(&e) => {
            // 拷贝成功后再删源；任何一步失败都需清理避免半成品。
            std::fs::copy(&src_ep, &dst_ep).map_err(|e| format!("跨卷复制失败: {e}"))?;
            if let Err(e) = std::fs::remove_file(&src_ep) {
                let _ = std::fs::remove_file(&dst_ep);
                return Err(format!("跨卷删除源文件失败: {e}"));
            }
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(windows)]
fn is_cross_device_error(e: &std::io::Error) -> bool {
    // Windows: ERROR_NOT_SAME_DEVICE = 17
    e.raw_os_error() == Some(17)
}

#[cfg(not(windows))]
fn is_cross_device_error(e: &std::io::Error) -> bool {
    // Unix: EXDEV = 18
    e.raw_os_error() == Some(18)
}

/// 并行执行 rename（跨卷自动退化为 copy + remove）。
/// `create_parent = true` 时在 rename 前确保目标父目录存在（用于"归类"类操作）。
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
                            if let Err(e) = std::fs::create_dir_all(to_extended_length_path(parent))
                            {
                                return (old_path, new_path, false, Some(e.to_string()));
                            }
                        }
                    }
                }

                match rename_or_copy_delete(&old_p, &new_p) {
                    Ok(_) => (old_path, new_path, true, None),
                    Err(e) => (old_path, new_path, false, Some(e)),
                }
            })
            .collect()
    }))
}

/// 并行执行自定义的 per-pair 动作（替代 `parallel_move` 的 `std::fs::rename`）。
///
/// 用于"修改"类不完全是纯 rename 的操作：执行器决定如何从 `old_path` 得到
/// `new_path`，只要最终两个路径都真实存在，后续 `op_pipeline::rollback`
/// 就能通过 `rename(new_path → old_path)` 恢复。
pub fn parallel_execute<F>(
    pairs: Vec<(String, String)>,
    thread_count: usize,
    executor: F,
) -> AppResult<Vec<MoveOutcome>>
where
    F: Fn(&str, &str) -> Result<(), String> + Send + Sync,
{
    let pool = rayon_pool(thread_count)?;
    Ok(pool.install(|| {
        pairs
            .into_par_iter()
            .map(
                |(old_path, new_path)| match executor(&old_path, &new_path) {
                    Ok(_) => (old_path, new_path, true, None),
                    Err(e) => (old_path, new_path, false, Some(e)),
                },
            )
            .collect()
    }))
}

/// 写入 `op_records`，并行执行 rename，最后把每条结果写回 `op_items`。
///
/// 返回的 `ModOpApplyResponse` 既适用于 Mod 工具也适用于 Suffix（字段同构；
/// `kind` 字段在 Suffix 场景下由调用方填入空字符串即可，前端模型已忽略）。
///
/// # 参数
/// - `record_id`：业务侧预生成（Mod 备份型操作需要在 apply 前用 record_id
///   构造备份子目录路径）。其它业务可用 `Uuid::new_v4().to_string()`。
/// - `rollback_enabled`：写入记录主表的回滚开关；Mod 备份型操作根据用户设置
///   传入，其它业务一律 `true`。
/// - `extra_value`：`tables.extra_summary_column` 对应列要写入的值；
///   如 Mod 工具传 `"rename"` / `"organize"`，Suffix 传 `.txt` 这样的后缀串。
/// - `create_parent`：rename 前是否自动创建目标父目录；Suffix 不需要，
///   Organize 需要。
pub fn persist_apply_rename_pairs(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    rollback_enabled: bool,
    extra_value: &str,
    record_name: String,
    source_paths: &[String],
    pairs: Vec<(String, String)>,
    create_parent: bool,
) -> AppResult<ModOpApplyResponse> {
    let thread_count = resolve_thread_count(db_path);
    let results = parallel_move(pairs, create_parent, thread_count)?;
    persist_apply_results(
        db_path,
        tables,
        record_id,
        rollback_enabled,
        extra_value,
        record_name,
        source_paths,
        results,
    )
}

/// 通用入口：用自定义执行器跑一批 `(old, new)`，然后把结果持久化为记录。
pub fn persist_apply_with_executor<F>(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    rollback_enabled: bool,
    extra_value: &str,
    record_name: String,
    source_paths: &[String],
    pairs: Vec<(String, String)>,
    executor: F,
) -> AppResult<ModOpApplyResponse>
where
    F: Fn(&str, &str) -> Result<(), String> + Send + Sync,
{
    let thread_count = resolve_thread_count(db_path);
    let results = parallel_execute(pairs, thread_count, executor)?;
    persist_apply_results(
        db_path,
        tables,
        record_id,
        rollback_enabled,
        extra_value,
        record_name,
        source_paths,
        results,
    )
}

/// 把已执行完成的一批结果写入记录表，并映射为 `ModOpApplyResponse`。
///
/// 大多数业务应优先用 `persist_apply_rename_pairs` 或
/// `persist_apply_with_executor`；只有执行顺序必须由业务控制时（如空目录清理要先删
/// 深层目录再删父目录）才直接调用本函数。
pub fn persist_apply_results(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    rollback_enabled: bool,
    extra_value: &str,
    record_name: String,
    source_paths: &[String],
    results: Vec<MoveOutcome>,
) -> AppResult<ModOpApplyResponse> {
    let created_at = Local::now().timestamp();
    let source_paths_json =
        serde_json::to_string(source_paths).map_err(|e| AppError::Internal(e.to_string()))?;
    op_record_repo::create_record(
        db_path,
        tables,
        record_id,
        &record_name,
        extra_value,
        rollback_enabled,
        &source_paths_json,
        created_at,
    )?;

    // 同一调用里 record 的 created_at 与 items 的 updated_at 取同一时刻：差几毫秒
    // 在业务上没区别，但是统一基准让"列表里看到的 record 时间 == items 的初始时间"
    // 这一直觉成立。
    let item_ids = op_record_repo::batch_insert_items(
        db_path,
        tables,
        record_id,
        &results
            .iter()
            .map(|(o, n, ok, msg)| (o.as_str(), n.as_str(), *ok, msg.as_deref()))
            .collect::<Vec<_>>(),
        created_at,
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
            status: if ok {
                "success".into()
            } else {
                "failed".into()
            },
            message: msg,
        });
    }

    Ok(ModOpApplyResponse {
        record_id: record_id.to_string(),
        record_name,
        kind: extra_value.to_string(),
        rollback_enabled,
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
    check_rollback_with_detail(db_path, &detail, item_ids.as_ref())
}

/// 与 [`check_rollback`] 等价，但接受调用方已经查到的 detail，避免重复读库。
pub fn check_rollback_with_detail(
    db_path: &Path,
    detail: &op_record_repo::OpRecordDetail,
    item_ids: Option<&Vec<i64>>,
) -> AppResult<ModOpRollbackCheck> {
    let selected: Vec<&op_record_repo::OpRecordItem> = detail
        .items
        .iter()
        .filter(|x| item_ids.map(|ids| ids.contains(&x.item_id)).unwrap_or(true))
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
///
/// 若记录创建时 `rollback_enabled = false`（例如用户关闭了 Mod 工具的回滚开关），
/// 直接返回 `AppError::InvalidInput`，后端拒绝执行。
///
/// # 冲突处理
/// 当原路径上已经被新文件占用（用户在 apply 之后又往那里放了同名文件）时，
/// 撤回不会覆盖该文件，而是把恢复目标加 ` (1)` / ` (2)` ... 后缀避让。决议过程
/// 在并行 rename 之前**顺序**完成（见 `resolve_conflict_with_reserved`），避免两个
/// rayon worker 同时撞上同一个目标各自分到 `(1)` 然后互相覆盖的 race。
/// 冲突时仍记 `apply_success = true`，并把"已恢复到 X"写入 `rollback_error`
/// 字段（该列在记录详情里既显示错误也显示备注）。
pub fn rollback(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> AppResult<ModOpRollbackResponse> {
    // 一次查 detail，给 check_rollback 与 selected 逻辑共用，避免重复读库。
    let detail = op_record_repo::get_record_detail(db_path, tables, record_id)?;

    if !detail.summary.rollback_enabled {
        return Err(AppError::InvalidInput(
            "该记录创建时未启用回滚，无法撤回".to_string(),
        ));
    }

    let selected: Vec<op_record_repo::OpRecordItem> = detail
        .items
        .iter()
        .filter(|x| {
            item_ids
                .as_ref()
                .map(|ids| ids.contains(&x.item_id))
                .unwrap_or(true)
        })
        .filter(|x| x.apply_success)
        .cloned()
        .collect();

    let check = check_rollback_with_detail(db_path, &detail, item_ids.as_ref())?;
    if !force_ignore_missing && !check.missing_paths.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "存在 {} 个文件缺失，请确认后使用 forceIgnoreMissing=true 再执行",
            check.missing_paths.len()
        )));
    }

    // 阶段 1：顺序解析每条 item 的最终目标路径。`reserved` 避免并行时两个 worker
    // 都看到 X 不存在 → 都解析为 X (1)，互相覆盖。new_path 缺失的项不参与
    // reserved（也不会执行 rename）。
    let mut reserved: HashSet<String> = HashSet::new();
    let resolved: Vec<RollbackPlan> = selected
        .into_iter()
        .map(|item| {
            let current = PathBuf::from(&item.new_path);
            if item.new_path.is_empty() || !to_extended_length_path(&current).exists() {
                return RollbackPlan {
                    item,
                    current,
                    target: None,
                    conflict: false,
                };
            }
            let old = PathBuf::from(&item.old_path);
            let (final_old, conflict) = resolve_conflict_with_reserved(old, &mut reserved);
            RollbackPlan {
                item,
                current,
                target: Some(final_old),
                conflict,
            }
        })
        .collect();

    // 阶段 2：并行执行 rename。冲突预解析已经完成，这里只负责 IO。
    let pool = rayon_pool(resolve_thread_count(db_path))?;
    let results: Vec<RollbackOutcome> = pool.install(|| {
        resolved
            .into_par_iter()
            .map(|plan| {
                let RollbackPlan {
                    item,
                    current,
                    target,
                    conflict,
                } = plan;
                let Some(final_old) = target else {
                    return RollbackOutcome {
                        item,
                        status: "skipped_missing",
                        message: None,
                    };
                };

                // 兜底建父目录（归类撤回时子目录可能已被外部清掉；同卷 rename
                // 不需要这一步，但跨卷 copy 一定要）
                if let Some(parent) = final_old.parent() {
                    if !to_extended_length_path(parent).exists() {
                        if let Err(e) = std::fs::create_dir_all(to_extended_length_path(parent)) {
                            return RollbackOutcome {
                                item,
                                status: "failed",
                                message: Some(e.to_string()),
                            };
                        }
                    }
                }

                // 跨卷场景由 rename_or_copy_delete 兜底——例如用户把备份目录
                // 配置在另一块盘，撤回时备份要从那盘拷回源盘。
                match rename_or_copy_delete(&current, &final_old) {
                    Ok(_) => {
                        let message = if conflict {
                            Some(format!(
                                "目标已存在，已恢复到: {}",
                                to_user_friendly_path(&final_old)
                            ))
                        } else {
                            None
                        };
                        RollbackOutcome {
                            item,
                            status: "success",
                            message,
                        }
                    }
                    Err(e) => RollbackOutcome {
                        item,
                        status: "failed",
                        message: Some(e),
                    },
                }
            })
            .collect()
    });

    let now = Local::now().timestamp();
    let mut success = 0usize;
    let mut failed = 0usize;
    let mut skipped_missing = 0usize;
    let mut items = Vec::with_capacity(results.len());

    // rollback_error 列同时承载错误与"已恢复到 X"备注；前端"撤回错误"列按
    // 字符串原样展示，不再额外区分。
    let updates: Vec<(i64, bool, Option<&str>)> = results
        .iter()
        .map(|outcome| {
            let (ok, msg) = match outcome.status {
                "success" => (true, outcome.message.as_deref()),
                "skipped_missing" => (false, Some("SKIPPED_NOT_FOUND")),
                _ => (false, outcome.message.as_deref()),
            };
            (outcome.item.item_id, ok, msg)
        })
        .collect();

    op_record_repo::batch_update_rollback_results(db_path, tables, &updates, now)?;

    for outcome in results {
        match outcome.status {
            "success" => success += 1,
            "skipped_missing" => skipped_missing += 1,
            _ => failed += 1,
        }
        items.push(ModOpApplyItem {
            item_id: outcome.item.item_id,
            old_path: outcome.item.old_path,
            new_path: outcome.item.new_path,
            status: if outcome.status == "success" {
                "success".into()
            } else {
                "failed".into()
            },
            message: match outcome.status {
                "skipped_missing" => Some("SKIPPED_NOT_FOUND".into()),
                "success" => outcome.message,
                _ => outcome.message,
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

/// 顺序解析阶段产出的每条 item 计划：阶段 2 用它直接执行。
struct RollbackPlan {
    item: op_record_repo::OpRecordItem,
    /// 备份/搬走的文件当前位置（item.new_path）。
    current: PathBuf,
    /// 解析后的最终落点；`None` 表示 `current` 缺失或 new_path 为空，本条会被
    /// 标记为 `skipped_missing` 而不参与 rename。
    target: Option<PathBuf>,
    /// 是否因目标已被占用而做了 ` (N)` 后缀。
    conflict: bool,
}

/// 阶段 2 并行执行后的结果，整理给后续写库 / 构造响应使用。
struct RollbackOutcome {
    item: op_record_repo::OpRecordItem,
    status: &'static str,
    message: Option<String>,
}
