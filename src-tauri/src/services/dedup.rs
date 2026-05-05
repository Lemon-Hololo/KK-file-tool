use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tauri::AppHandle;
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    config::{HASH_QUEUE_SIZE, PARTIAL_BATCH_SIZE, PAUSE_SLEEP_MS},
    constants::{hash_entry_status, keep_policy, stages},
    db::hash_repo,
    error::AppResult,
    models::{DedupConfig, DuplicateGroup, FileEntry, HashIndexEntry, TaskStatus},
    services::{
        events,
        logging::TaskLogContext,
        op_pipeline::{
            record_name_or_timestamp, resolve_io_concurrency_multiplier, resolve_thread_count,
        },
    },
    utils::hash::hash_file_blake3,
    utils::path::{to_extended_length_path, to_user_friendly_path},
    utils::time::system_time_to_secs,
};

#[derive(Clone)]
struct ScanItem {
    path: PathBuf,
    size: u64,
    mtime: i64,
    ctime: i64,
}

#[derive(Clone)]
struct HashItem {
    hash: String,
    path: PathBuf,
    size: u64,
    mtime: i64,
    ctime: i64,
}

async fn pause_point(runtime: &TaskRuntime) {
    while runtime.is_paused() && !runtime.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(PAUSE_SLEEP_MS)).await;
    }
}

/// 去重任务主流程：扫描 → 并发哈希 → 流式分组 → 应用策略 → 发事件。
///
/// # 并发模型
/// - 扫描阶段 `spawn_blocking` 读取 metadata 并按 size 预过滤；
/// - 哈希阶段通过 `tokio::sync::Semaphore` 限制同时进行的 BLAKE3 任务数，
///   许可数 = 用户设置 `thread_count × 2`（0 → 全部 CPU 核心 × 2）；
/// - 结果通过 `mpsc` 通道汇入主任务，边收边分组以降低峰值内存。
///
/// # 取消 / 暂停
/// 扫描阶段只响应取消；哈希调度阶段同时响应取消和暂停（已入队的哈希任务
/// 会跑完，不再提交新任务）。
///
/// # 任务清理
/// 不论成功 / 失败 / 取消，都会通过 `AppState::remove_task` 移除自身条目，
/// 与 `mod_tools::scan` / `cleanup` 行为一致。
pub async fn run_dedup(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    config: DedupConfig,
    runtime: Arc<TaskRuntime>,
) -> AppResult<()> {
    let result = run_dedup_inner(
        app.clone(),
        app_state.clone(),
        task_id.clone(),
        paths,
        config,
        runtime,
    )
    .await;

    // 任务终态清理：从 tasks 表移除，避免 HashMap 单调增长。
    app_state.remove_task(&task_id);

    result
}

async fn run_dedup_inner(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    config: DedupConfig,
    runtime: Arc<TaskRuntime>,
) -> AppResult<()> {
    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    let log = TaskLogContext::new(&app, &task_id);

    // ---------------------------------------------------------
    // 阶段1：扫描 + 哈希 流水线
    // ---------------------------------------------------------
    let (result_tx, mut result_rx) = mpsc::channel::<HashItem>(HASH_QUEUE_SIZE);

    let scan_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hash_done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hash_total = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let need_all_hash = config.save_record_enabled || config.use_last_record_enabled;

    // 并发度统一从用户设置读取；许可数 = 有效线程数 × IO 并发倍率，保证 IO wait 期间
    // 有前台任务。倍率可在设置中心调（SSD/NVMe 可上调到 4~8，HDD 降到 1）。
    let multiplier = resolve_io_concurrency_multiplier(&app_state.db_path);
    let concurrency = resolve_thread_count(&app_state.db_path) * multiplier;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency.max(2)));

    let scan_runtime = runtime.clone();
    let app_scan = app.clone();
    let task_id_scan = task_id.clone();
    let result_tx_scan = result_tx.clone();
    let scan_count_c = scan_count.clone();
    let hash_total_c = hash_total.clone();
    let semaphore_c = semaphore.clone();
    let hash_done_c = hash_done.clone();
    let app_progress = app.clone();
    let task_id_progress = task_id.clone();
    let paths_for_record = paths.clone();
    let log_scan = log.clone();

    let scan_handle: tokio::task::JoinHandle<Result<(), String>> = tokio::spawn(async move {
        let mut all_scan: Vec<ScanItem> = vec![];
        let mut by_size: HashMap<u64, usize> = HashMap::new();

        for root in paths {
            for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                if scan_runtime.is_cancelled() {
                    return Ok(());
                }

                if !entry.file_type().is_file() {
                    continue;
                }

                let p = entry.path().to_path_buf();
                let ep = to_extended_length_path(&p);

                let meta = match std::fs::metadata(&ep) {
                    Ok(m) => m,
                    Err(e) => {
                        log_scan.warn_path(&format!("读取元数据失败: {e}"), &p);
                        continue;
                    }
                };

                let mtime = system_time_to_secs(meta.modified().ok());
                // ctime 在某些平台 / FS 上可能没有；这里按"无 ctime 时退回 mtime"约定，
                // 让保留策略仍能给出稳定的排序基准。
                let ctime_raw = system_time_to_secs(meta.created().ok());
                let ctime = if ctime_raw == 0 { mtime } else { ctime_raw };

                let size = meta.len();
                *by_size.entry(size).or_insert(0) += 1;

                let item = ScanItem {
                    path: p.clone(),
                    size,
                    mtime,
                    ctime,
                };
                all_scan.push(item);

                let n = scan_count_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                events::emit_progress(&app_scan, &task_id_scan, stages::SCAN, n, 0);
            }
        }

        if scan_runtime.is_cancelled() {
            return Ok(());
        }

        let hash_inputs: Vec<ScanItem> = if need_all_hash {
            all_scan
        } else {
            all_scan
                .into_iter()
                .filter(|item| by_size.get(&item.size).copied().unwrap_or(0) > 1)
                .collect()
        };

        let total = hash_inputs.len();
        hash_total_c.store(total, std::sync::atomic::Ordering::Relaxed);

        for item in hash_inputs {
            if scan_runtime.is_cancelled() {
                break;
            }
            pause_point(&scan_runtime).await;

            let permit = match semaphore_c.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };

            let tx = result_tx_scan.clone();
            let runtime_w = scan_runtime.clone();
            let hash_done_w = hash_done_c.clone();
            let app_prog = app_progress.clone();
            let task_prog = task_id_progress.clone();
            let total_snap = total;
            let log_hash = log_scan.clone();

            tokio::spawn(async move {
                let _permit = permit;

                if runtime_w.is_cancelled() {
                    return;
                }

                log_hash.info_path("哈希计算", &item.path);

                let path_clone = item.path.clone();
                let hash_result =
                    tokio::task::spawn_blocking(move || hash_file_blake3(&path_clone)).await;

                let hash = match hash_result {
                    Ok(Ok(h)) => h,
                    Ok(Err(e)) => {
                        log_hash.warn_path(&format!("哈希失败: {e}"), &item.path);
                        let done =
                            hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        events::emit_progress(
                            &app_prog,
                            &task_prog,
                            stages::HASH,
                            done,
                            total_snap,
                        );
                        return;
                    }
                    Err(e) => {
                        log_hash.warn_path(&format!("哈希任务崩溃: {e}"), &item.path);
                        let done =
                            hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        events::emit_progress(
                            &app_prog,
                            &task_prog,
                            stages::HASH,
                            done,
                            total_snap,
                        );
                        return;
                    }
                };

                let done = hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                events::emit_progress(&app_prog, &task_prog, stages::HASH, done, total_snap);

                let out = HashItem {
                    hash,
                    path: item.path,
                    size: item.size,
                    mtime: item.mtime,
                    ctime: item.ctime,
                };
                let _ = tx.send(out).await;
            });
        }
        Ok(())
    });

    // 外层持有的 result_tx 必须 drop，否则 result_rx 不会结束。
    drop(result_tx);

    // ---------------------------------------------------------
    // 阶段2：流式分组
    // ---------------------------------------------------------
    let mut by_hash: HashMap<String, Vec<FileEntry>> = HashMap::new();
    let mut hash_for_record: Vec<HashItem> = Vec::new();

    while let Some(item) = result_rx.recv().await {
        if runtime.is_cancelled() {
            break;
        }

        by_hash
            .entry(item.hash.clone())
            .or_default()
            .push(FileEntry {
                abs_path: to_user_friendly_path(&item.path),
                size: item.size,
                mtime: item.mtime,
                ctime: item.ctime,
                hash: Some(item.hash.clone()),
                selected_for_move: false,
                from_history: false,
            });

        if config.save_record_enabled {
            hash_for_record.push(item);
        }
    }

    match scan_handle.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            // Failed 状态由命令层 `finalize_failed_long_task` 统一发射，service 内不重复发。
            log.error(&format!("扫描任务失败: {e}"));
            return Err(crate::error::AppError::Internal(e));
        }
        Err(e) => {
            let err_msg = format!("扫描任务崩溃: {}", e);
            log.error(&err_msg);
            return Err(crate::error::AppError::Internal(err_msg));
        }
    }

    if runtime.is_cancelled() {
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(&app, &task_id, "Cancelled");
        return Ok(());
    }

    // ---------------------------------------------------------
    // 阶段3：合并历史 + 应用策略 + 发送结果
    // ---------------------------------------------------------
    let history_entries: HashMap<String, Vec<FileEntry>> = if config.use_last_record_enabled {
        let record_id = if let Some(rid) = config.selected_record_id.as_deref() {
            Some(rid.to_string())
        } else {
            hash_repo::list_hash_records(&app_state.db_path)
                .ok()
                .and_then(|records| records.first().map(|r| r.record_id.clone()))
        };

        if let Some(rid) = record_id {
            match hash_repo::load_hash_record(&app_state.db_path, &rid) {
                Ok(record) => {
                    let mut map: HashMap<String, Vec<FileEntry>> = HashMap::new();
                    for entry in record.entries {
                        if entry.status == hash_entry_status::ACTIVE {
                            map.entry(entry.hash.clone()).or_default().push(FileEntry {
                                abs_path: entry.file_path,
                                size: entry.file_size,
                                mtime: entry.mtime,
                                ctime: entry.ctime,
                                hash: Some(entry.hash),
                                selected_for_move: false,
                                from_history: true,
                            });
                        }
                    }
                    map
                }
                Err(e) => {
                    log.warn(&format!("加载历史记录失败: {e}"));
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    for (hash, history_files) in history_entries {
        by_hash.entry(hash).or_default().extend(history_files);
    }

    let mut groups = vec![];
    let mut partial_buf = vec![];
    let mut gid = 1usize;

    for (hash, mut files) in by_hash {
        if runtime.is_cancelled() {
            break;
        }

        let has_history = files.iter().any(|f| f.from_history);
        let current_count = files.iter().filter(|f| !f.from_history).count();
        let history_count = files.iter().filter(|f| f.from_history).count();

        let keep = if config.use_last_record_enabled && !config.include_current_folder_duplicates {
            has_history && current_count > 0
        } else {
            let current_dup = current_count > 1;
            let cross_dup = has_history && current_count > 0;
            let history_dup = config.include_current_folder_duplicates && history_count > 1;
            current_dup || cross_dup || history_dup
        };

        if !keep || files.len() < 2 {
            continue;
        }

        if config.auto_select_enabled && files.len() > 1 {
            if config.use_last_record_enabled && has_history {
                for f in files.iter_mut() {
                    f.selected_for_move = !f.from_history;
                }
            } else {
                files.sort_by_key(|f| f.ctime);
                let keep_idx = if config.keep_policy == keep_policy::NEWEST {
                    files.len() - 1
                } else {
                    0
                };
                for (idx, f) in files.iter_mut().enumerate() {
                    f.selected_for_move = idx != keep_idx;
                }
            }
        }

        let g = DuplicateGroup {
            group_id: format!("G{:05}", gid),
            hash,
            files,
        };
        gid += 1;

        partial_buf.push(g);

        if partial_buf.len() >= PARTIAL_BATCH_SIZE {
            groups.extend(partial_buf.iter().cloned());
            events::emit_result_partial(&app, &task_id, &partial_buf, false);
            partial_buf = vec![];
        }
    }

    if !partial_buf.is_empty() {
        groups.extend(partial_buf.iter().cloned());
        events::emit_result_partial(&app, &task_id, &partial_buf, false);
    }

    if runtime.is_cancelled() {
        // 取消时不把半截分组结果留到内存：用户随后的"重做"应当看到空缓存，而不是
        // 上次取消瞬间的不完整快照（前端可能据此误判为"还有待移动的重复组"）。
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(&app, &task_id, "Cancelled");
        return Ok(());
    }

    app_state.set_task_results(task_id.clone(), groups.clone());

    if config.save_record_enabled {
        let entries: Vec<HashIndexEntry> = hash_for_record
            .iter()
            .map(|x| HashIndexEntry {
                hash: x.hash.clone(),
                file_path: to_user_friendly_path(&x.path),
                file_size: x.size,
                mtime: x.mtime,
                ctime: x.ctime,
                status: hash_entry_status::ACTIVE.to_string(),
            })
            .collect();

        let name = record_name_or_timestamp(config.record_name.clone());

        match hash_repo::insert_hash_record(
            &app_state.db_path,
            &name,
            &paths_for_record,
            &entries,
            chrono::Local::now().timestamp(),
        ) {
            Ok(record_id) => {
                log.info(&format!("保存记录成功: {} ({}条)", name, entries.len()));
                log.info(&format!("记录ID: {}", record_id));
            }
            Err(e) => {
                log.error(&format!("保存记录失败: {}", e));
            }
        }
    }

    runtime.set_status(TaskStatus::Completed);

    events::emit_result_partial(&app, &task_id, &[], true);
    events::emit_state_changed(&app, &task_id, "Completed");
    events::emit_task_completed(&app, &task_id, &groups);

    Ok(())
}
