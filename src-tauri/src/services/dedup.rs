use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    config::{HASH_QUEUE_SIZE, PARTIAL_BATCH_SIZE, PAUSE_SLEEP_MS},
    db::{hash_repo, settings_repo},
    error::AppResult,
    models::{
        DedupConfig, DuplicateGroup, FileEntry, HashIndexEntry, TaskLogPayload,
        TaskProgressPayload, TaskStatus,
    },
    services::record,
    utils::hash::hash_file_blake3,
    utils::path::to_extended_length_path,
};

#[derive(Clone)]
struct ScanItem {
    path: PathBuf,
    size: u64,
    mtime: i64,
    ctime: i64, // 创建时间
}

#[derive(Clone)]
struct HashItem {
    hash: String,
    path: PathBuf,
    size: u64,
    mtime: i64,
    ctime: i64, // 创建时间
}

fn emit_log(app: &AppHandle, task_id: &str, level: &str, message: &str, file_path: Option<String>) {
    let _ = app.emit(
        "task_log",
        TaskLogPayload {
            task_id: task_id.to_string(),
            level: level.to_string(),
            message: message.to_string(),
            file_path,
        },
    );
}

fn emit_progress(app: &AppHandle, task_id: &str, stage: &str, processed: usize, total: usize) {
    let percent = if total == 0 {
        0.0
    } else {
        (processed as f64 / total as f64) * 100.0
    };
    let _ = app.emit(
        "task_progress",
        TaskProgressPayload {
            task_id: task_id.to_string(),
            stage: stage.to_string(),
            processed,
            total,
            percent,
        },
    );
}

async fn pause_point(runtime: &TaskRuntime) {
    while runtime.is_paused() && !runtime.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(PAUSE_SLEEP_MS)).await;
    }
}

pub async fn run_dedup(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    config: DedupConfig,
    runtime: Arc<TaskRuntime>,
) -> AppResult<()> {
    {
        let mut s = runtime.status.lock().unwrap();
        *s = TaskStatus::Running;
    }
    let _ = app.emit(
        "task_state_changed",
        serde_json::json!({ "taskId": task_id, "status": "Running" }),
    );

    // ---------------------------------------------------------
    // 阶段1：扫描 + 哈希 流水线
    // 扫描线程发现文件后，立即投入 spawn_blocking 做哈希
    // 所有哈希任务并发执行，结果通过 result_tx 汇聚
    // ---------------------------------------------------------
    let (result_tx, mut result_rx) = mpsc::channel::<HashItem>(HASH_QUEUE_SIZE);

    // 用于统计进度的原子计数
    let scan_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hash_done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    // 扫描完成后，hash_total 才确定；先用 AtomicUsize 暂存
    let hash_total = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 判断是否需要对所有文件哈希（而不仅限于 size 重复的候选）
    let need_all_hash = config.save_record_enabled || config.use_last_record_enabled;

    // 用 semaphore 限制并发哈希任务数量，防止瞬间开太多 blocking 线程
    // 从用户设置读取核心数配置，0 = 自动使用全部可用核心
    let concurrency = {
        let thread_count = settings_repo::get_settings(&app_state.db_path)
            .map(|s| s.thread_count)
            .unwrap_or(0);
        if thread_count > 0 {
            (thread_count as usize).min(num_cpus::get()).max(1) * 2
        } else {
            num_cpus::get().max(2) * 2
        }
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    // 先做一次轻量扫描收集 size 信息（非常快，只读 metadata，不哈希）
    // 如果 need_all_hash=false，需要用 size 过滤只哈希候选文件
    // 如果 need_all_hash=true，所有文件都要哈希，size 过滤跳过
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

    // 扫描任务：在 spawn 中完成扫描+按需发起哈希
    let scan_handle: tokio::task::JoinHandle<Result<(), String>> = tokio::spawn(async move {
        // 第一步：收集所有文件元数据（快速）
        let mut all_scan: Vec<ScanItem> = vec![];
        let mut by_size: HashMap<u64, usize> = HashMap::new(); // size -> count

        for root in paths {
            for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
                // 扫描阶段只响应取消，不响应暂停
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
                        emit_log(
                            &app_scan,
                            &task_id_scan,
                            "WARN",
                            &format!("读取元数据失败: {e}"),
                            Some(p.display().to_string()),
                        );
                        continue;
                    }
                };

                // 获取修改时间（Windows 支持）
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                // 获取创建时间（Windows 支持）
                let ctime = meta
                    .created()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(mtime); // 如果获取失败，使用修改时间作为后备

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
                emit_progress(&app_scan, &task_id_scan, "scan", n, 0);
            }
        }

        if scan_runtime.is_cancelled() {
            return Ok(());
        }

        // 第二步：筛选需要哈希的文件，并发提交哈希任务
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
            // 暂停点：等待恢复后才提交新任务（哈希阶段响应暂停）
            // 已提交的哈希任务会继续执行完成
            pause_point(&scan_runtime).await;

            // 用 semaphore 限流，确保不超过 concurrency 个并发哈希
            let permit = match semaphore_c.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };

            let tx = result_tx_scan.clone();
            let runtime_w = scan_runtime.clone();
            let app_w = app_scan.clone();
            let task_w = task_id_scan.clone();
            let hash_done_w = hash_done_c.clone();
            let app_prog = app_progress.clone();
            let task_prog = task_id_progress.clone();
            let total_snap = total;

            // spawn_blocking：哈希是 CPU 密集，放到 blocking 线程池
            tokio::spawn(async move {
                let _permit = permit; // 离开作用域时自动释放 semaphore

                // 已提交的哈希任务不检查暂停状态，让它跑完
                if runtime_w.is_cancelled() {
                    return;
                }

                emit_log(
                    &app_w,
                    &task_w,
                    "INFO",
                    "哈希计算",
                    Some(item.path.display().to_string()),
                );

                let path_clone = item.path.clone();
                // 在 blocking 线程中执行哈希（BLAKE3 CPU密集）
                let hash_result =
                    tokio::task::spawn_blocking(move || hash_file_blake3(&path_clone)).await;

                let hash = match hash_result {
                    Ok(Ok(h)) => h,
                    Ok(Err(e)) => {
                        emit_log(
                            &app_w,
                            &task_w,
                            "WARN",
                            &format!("哈希失败: {e}"),
                            Some(item.path.display().to_string()),
                        );
                        // 更新进度计数，避免前端进度卡死
                        let done =
                            hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        emit_progress(&app_prog, &task_prog, "hash", done, total_snap);
                        return;
                    }
                    Err(e) => {
                        emit_log(
                            &app_w,
                            &task_w,
                            "WARN",
                            &format!("哈希任务崩溃: {e}"),
                            Some(item.path.display().to_string()),
                        );
                        // 更新进度计数，避免前端进度卡死
                        let done =
                            hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        emit_progress(&app_prog, &task_prog, "hash", done, total_snap);
                        return;
                    }
                };

                let done = hash_done_w.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                emit_progress(&app_prog, &task_prog, "hash", done, total_snap);

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
        // scan_handle 结束，result_tx_scan drop，配合外部 drop(result_tx) 关闭通道
        Ok(())
    });

    // 外层持有的 result_tx 必须 drop，否则 result_rx 不会结束
    drop(result_tx);

    // ---------------------------------------------------------
    // 阶段2：流式收集 + 增量分组
    // 边接收边分组，避免二次遍历，减少内存占用
    // ---------------------------------------------------------
    let mut by_hash: HashMap<String, Vec<FileEntry>> = HashMap::new();
    // 仅在需要保存记录时才存储原始哈希数据
    let mut hash_for_record: Vec<HashItem> = if config.save_record_enabled {
        Vec::new()
    } else {
        Vec::new()
    };

    while let Some(item) = result_rx.recv().await {
        if runtime.is_cancelled() {
            break;
        }

        // 直接分组，不存储原始 HashItem（除非需要保存记录）
        by_hash
            .entry(item.hash.clone())
            .or_default()
            .push(FileEntry {
                abs_path: item.path.display().to_string(),
                size: item.size,
                mtime: item.mtime,
                ctime: item.ctime,
                hash: Some(item.hash.clone()),
                selected_for_move: false,
                from_history: false,
            });

        // 仅在需要保存记录时才存储原始数据
        if config.save_record_enabled {
            hash_for_record.push(item);
        }
    }

    // 等待扫描+调度任务结束，并处理错误
    match scan_handle.await {
        Ok(Ok(())) => { /* 扫描成功，继续 */ }
        Ok(Err(e)) => {
            emit_log(&app, &task_id, "ERROR", &format!("扫描任务失败: {e}"), None);
            let mut s = runtime.status.lock().unwrap();
            *s = TaskStatus::Failed;
            let _ = app.emit(
                "task_state_changed",
                serde_json::json!({ "taskId": task_id, "status": "Failed" }),
            );
            return Err(crate::error::AppError::Internal(e));
        }
        Err(e) => {
            let err_msg = format!("扫描任务崩溃: {}", e);
            emit_log(&app, &task_id, "ERROR", &err_msg, None);
            let mut s = runtime.status.lock().unwrap();
            *s = TaskStatus::Failed;
            let _ = app.emit(
                "task_state_changed",
                serde_json::json!({ "taskId": task_id, "status": "Failed" }),
            );
            return Err(crate::error::AppError::Internal(err_msg));
        }
    }

    if runtime.is_cancelled() {
        // 更新状态为 Cancelled
        let mut s = runtime.status.lock().unwrap();
        *s = TaskStatus::Cancelled;
        let _ = app.emit(
            "task_state_changed",
            serde_json::json!({ "taskId": task_id, "status": "Cancelled" }),
        );
        return Ok(());
    }

    // ---------------------------------------------------------
    // 阶段3：应用策略、发送结果
    // ---------------------------------------------------------
    // 加载历史记录的完整文件信息（不仅仅是哈希）
    let history_entries: HashMap<String, Vec<FileEntry>> = if config.use_last_record_enabled {
        let record_id = if let Some(rid) = config.selected_record_id.as_deref() {
            Some(rid.to_string())
        } else {
            // 如果没有指定记录ID，获取最新的记录ID
            hash_repo::list_hash_records(&app_state.db_path)
                .ok()
                .and_then(|records| records.first().map(|r| r.record_id.clone()))
        };

        if let Some(rid) = record_id {
            match hash_repo::load_hash_record(&app_state.db_path, &rid) {
                Ok(record) => {
                    let mut map: HashMap<String, Vec<FileEntry>> = HashMap::new();
                    for entry in record.entries {
                        if entry.status == "active" {
                            map.entry(entry.hash.clone()).or_default().push(FileEntry {
                                abs_path: entry.file_path,
                                size: entry.file_size,
                                mtime: entry.mtime,
                                ctime: entry.ctime, // 使用数据库中的 ctime
                                hash: Some(entry.hash),
                                selected_for_move: false,
                                from_history: true, // 标记为来自历史
                            });
                        }
                    }
                    map
                }
                Err(e) => {
                    emit_log(
                        &app,
                        &task_id,
                        "WARN",
                        &format!("加载历史记录失败: {e}"),
                        None,
                    );
                    HashMap::new()
                }
            }
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    // 合并当前扫描结果和历史记录
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

        // 检查是否有历史文件和当前文件
        let has_history = files.iter().any(|f| f.from_history);
        let current_count = files.iter().filter(|f| !f.from_history).count();
        let history_count = files.iter().filter(|f| f.from_history).count();

        // 决定是否保留这个分组（必须有真正的重复）
        let keep = if config.use_last_record_enabled && !config.include_current_folder_duplicates {
            // 只显示与历史记录重复的文件：至少有1个历史文件和1个当前文件
            has_history && current_count > 0
        } else {
            // 显示当前重复的文件或与历史记录重复的文件
            // 当前重复：当前文件数 > 1
            // 与历史重复：至少有1个历史文件和1个当前文件
            // 历史内部重复：历史文件数 > 1（如果包含当前文件夹重复）
            let current_dup = current_count > 1;
            let cross_dup = has_history && current_count > 0;
            let history_dup = config.include_current_folder_duplicates && history_count > 1;
            current_dup || cross_dup || history_dup
        };

        if !keep || files.len() < 2 {
            continue;
        }

        if config.auto_select_enabled && files.len() > 1 {
            // 如果使用历史记录，优先保留历史文件
            if config.use_last_record_enabled && has_history {
                // 保留所有历史文件，标记所有当前文件为待移动
                for f in files.iter_mut() {
                    f.selected_for_move = !f.from_history;
                }
            } else {
                // 使用创建时间排序，按策略保留
                files.sort_by_key(|f| f.ctime);
                let keep_idx = if config.keep_policy == "newest" {
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

        // 只克隆一次，先放入 partial_buf
        partial_buf.push(g);

        if partial_buf.len() >= PARTIAL_BATCH_SIZE {
            // 批量添加到 groups
            groups.extend(partial_buf.iter().cloned());
            let _ = app.emit(
                "task_result_partial",
                serde_json::json!({
                  "taskId": task_id,
                  "groups": partial_buf,
                  "done": false
                }),
            );
            partial_buf = vec![];
        }
    }

    // 处理剩余的 partial_buf
    if !partial_buf.is_empty() {
        groups.extend(partial_buf.iter().cloned());
        let _ = app.emit(
            "task_result_partial",
            serde_json::json!({
              "taskId": task_id,
              "groups": partial_buf,
              "done": false
            }),
        );
    }

    {
        let mut lock = app_state.task_results.lock().unwrap();
        lock.insert(task_id.clone(), groups.clone());
    }

    // 在保存记录前检查取消状态
    if runtime.is_cancelled() {
        let mut s = runtime.status.lock().unwrap();
        *s = TaskStatus::Cancelled;
        let _ = app.emit(
            "task_state_changed",
            serde_json::json!({ "taskId": task_id, "status": "Cancelled" }),
        );
        return Ok(());
    }

    if config.save_record_enabled {
        let entries: Vec<HashIndexEntry> = hash_for_record
            .iter()
            .map(|x| HashIndexEntry {
                hash: x.hash.clone(),
                file_path: x.path.display().to_string(),
                file_size: x.size,
                mtime: x.mtime,
                ctime: x.ctime,
                status: "active".to_string(),
            })
            .collect();

        let name = config
            .record_name
            .clone()
            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());

        match record::save_hash_record(&app_state.db_path, &name, &paths_for_record, &entries) {
            Ok(record_id) => {
                emit_log(
                    &app,
                    &task_id,
                    "INFO",
                    &format!("保存记录成功: {} ({}条)", name, entries.len()),
                    None,
                );
                emit_log(
                    &app,
                    &task_id,
                    "INFO",
                    &format!("记录ID: {}", record_id),
                    None,
                );
            }
            Err(e) => {
                emit_log(
                    &app,
                    &task_id,
                    "ERROR",
                    &format!("保存记录失败: {}", e),
                    None,
                );
            }
        }
    }

    {
        let mut s = runtime.status.lock().unwrap();
        *s = TaskStatus::Completed;
    }

    let _ = app.emit(
        "task_result_partial",
        serde_json::json!({
          "taskId": task_id,
          "groups": [],
          "done": true
        }),
    );
    let _ = app.emit(
        "task_state_changed",
        serde_json::json!({ "taskId": task_id, "status": "Completed" }),
    );
    let _ = app.emit(
        "task_completed",
        serde_json::json!({ "taskId": task_id, "groups": groups }),
    );

    Ok(())
}
