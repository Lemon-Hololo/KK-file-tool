//! 扫描 .zip/.zipmod 中 manifest.xml 含指定关键字的文件。
//!
//! 作为长任务运行，共用 `AppState` 注册的 `TaskRuntime`，支持取消。

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicUsize},
};

use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    constants::stages,
    models::{ModScanCompletedPayload, ModScanMatch, TaskStatus},
    services::mod_tools::zipmod::{is_zipmod, read_manifest_from_zip},
    services::op_pipeline::{resolve_io_concurrency_multiplier, resolve_thread_count},
    services::{events, logging::TaskLogContext},
    utils::path::{to_extended_length_path, to_user_friendly_path},
};

pub async fn run_scan(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    keyword: String,
    runtime: Arc<TaskRuntime>,
) -> Result<(), String> {
    let keyword_trim = keyword.trim().to_string();
    if keyword_trim.is_empty() {
        return Err("关键字不能为空".to_string());
    }

    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    let log = TaskLogContext::new(&app, &task_id);

    log.info(&format!("开始扫描，关键字: {keyword_trim}"));

    // 1) 收集候选文件
    let mut candidates: Vec<PathBuf> = vec![];
    for root in &paths {
        let ep = to_extended_length_path(std::path::Path::new(root));
        for entry in WalkDir::new(&ep).into_iter().filter_map(|x| x.ok()) {
            if runtime.is_cancelled() {
                break;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if is_zipmod(&name) {
                candidates.push(entry.path().to_path_buf());
            }
        }
    }

    if runtime.is_cancelled() {
        emit_final(&app, &task_id, &runtime, &keyword_trim, vec![], 0, 0, true);
        app_state.remove_task(&task_id);
        return Ok(());
    }

    let total = candidates.len();
    events::emit_progress(&app, &task_id, stages::MOD_SCAN, 0, total);

    // 2) 并行读取 manifest；semaphore 控并发（IO + 少量 CPU）
    // 与 dedup 一致：有效线程数 × 用户配置的 IO 倍率（默认 2）
    let multiplier = resolve_io_concurrency_multiplier(&app_state.db_path);
    let concurrency = (resolve_thread_count(&app_state.db_path) * multiplier).max(2);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let processed = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let matches: Arc<Mutex<Vec<ModScanMatch>>> = Arc::new(Mutex::new(vec![]));
    let log_scan = log.clone();

    let mut handles = Vec::with_capacity(candidates.len());

    for path in candidates {
        if runtime.is_cancelled() {
            break;
        }

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break,
        };

        let app_c = app.clone();
        let task_id_c = task_id.clone();
        let runtime_c = runtime.clone();
        let keyword_c = keyword_trim.clone();
        let processed_c = processed.clone();
        let errors_c = errors.clone();
        let matches_c = matches.clone();
        let total_c = total;
        let log_c = log_scan.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit;
            if runtime_c.is_cancelled() {
                return;
            }

            let path_for_blocking = path.clone();
            let result =
                tokio::task::spawn_blocking(move || read_manifest_from_zip(&path_for_blocking))
                    .await;

            let path_display = to_user_friendly_path(&path);

            match result {
                Ok(Ok((meta, _raw))) => {
                    if meta
                        .games
                        .iter()
                        .any(|g| g.eq_ignore_ascii_case(&keyword_c))
                    {
                        log_c.info_file(&format!("匹配: {path_display}"), path_display.clone());
                        let m = ModScanMatch {
                            file_path: path_display,
                            guid: meta.guid,
                            version: meta.version,
                            author: meta.author,
                            matched_keyword: keyword_c.clone(),
                        };
                        matches_c.lock().unwrap().push(m);
                    }
                }
                Ok(Err(e)) => {
                    errors_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    log_c.warn_file(&format!("读取失败: {e}"), path_display);
                }
                Err(e) => {
                    errors_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    log_c.warn_file(&format!("任务崩溃: {e}"), path_display);
                }
            }

            let done = processed_c.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            events::emit_progress(&app_c, &task_id_c, stages::MOD_SCAN, done, total_c);
        });

        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }

    let cancelled = runtime.is_cancelled();
    let matches_final = std::mem::take(&mut *matches.lock().unwrap());
    let total_errors = errors.load(std::sync::atomic::Ordering::Relaxed);

    emit_final(
        &app,
        &task_id,
        &runtime,
        &keyword_trim,
        matches_final,
        total,
        total_errors,
        cancelled,
    );

    // 清理 tasks 表
    app_state.remove_task(&task_id);

    Ok(())
}

// emit_final 把"扫描完成"前后要发的 4 个事件 + 1 条总结日志收敛到一处，参数
// 自然超 7 个；包成结构体只是把 8 个 setter 换成 8 个 setter，没有可读性收益。
#[allow(clippy::too_many_arguments)]
fn emit_final(
    app: &AppHandle,
    task_id: &str,
    runtime: &Arc<TaskRuntime>,
    keyword: &str,
    matches: Vec<ModScanMatch>,
    total: usize,
    errors: usize,
    cancelled: bool,
) {
    let log = TaskLogContext::new(app, task_id);
    if cancelled {
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(app, task_id, "Cancelled");
    } else {
        runtime.set_status(TaskStatus::Completed);
        events::emit_state_changed(app, task_id, "Completed");
    }

    log.info(&format!(
        "扫描完成：匹配 {}，扫描 {}，错误 {}{}",
        matches.len(),
        total,
        errors,
        if cancelled { "（已取消）" } else { "" }
    ));

    events::emit_mod_scan_completed(
        app,
        &ModScanCompletedPayload {
            task_id: task_id.to_string(),
            keyword: keyword.to_string(),
            matches,
            total_scanned: total,
            total_errors: errors,
            cancelled,
        },
    );
}
