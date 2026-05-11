//! 图片相似度去重的 Tauri 命令封装。
//!
//! 仅做参数转发 / 错误映射，业务在 [`crate::services::image_dedup`]。
//! 长任务签名与 mod_tools 的 `start_*_task` 完全对齐——前端预生成 `task_id`，
//! 命令层 `state.create_task(task_id)` + `events::spawn_long_task`，失败时
//! 用 `on_failure_extra` 推一次空 `image_dedup_partial`(done=true) 让前端关闭 spinner。

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    app_state::AppState,
    models::{
        ImageDedupApplyResponse, ImageDedupPartialPayload, ImageDedupRecordDetail,
        ImageDedupRecordSummary, ImageDedupRollbackCheck, ImageDedupRollbackResponse,
    },
    services::{
        events,
        image_dedup::{self, apply, scan},
        logging::TaskLogContext,
    },
};

/// 启动图片相似度去重扫描长任务，结果通过 `image_dedup_partial` 增量推送。
#[tauri::command]
pub async fn start_image_dedup_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }

    let (task_id, runtime) = state.create_task(task_id);
    let state_arc = state.inner().clone();

    events::spawn_long_task(
        app,
        state_arc,
        task_id.clone(),
        move |app, state, task_id| async move {
            scan::run_image_dedup_scan(app, state, task_id, paths, runtime).await
        },
        |app, task_id| {
            events::emit_image_dedup_partial(
                app,
                &ImageDedupPartialPayload {
                    task_id: task_id.to_string(),
                    groups: vec![],
                    done: true,
                },
            );
        },
    );

    Ok(task_id)
}

/// 删除选中的相似图片并写入可撤回记录。
#[tauri::command]
pub fn apply_image_dedup_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    task_id: Option<String>,
) -> Result<ImageDedupApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    apply::apply_image_dedup_delete(
        &state.db_path,
        &paths,
        selected_file_paths,
        record_name,
        log,
    )
    .map_err(|e| e.to_string())
}

/// 列出图片去重记录。
#[tauri::command]
pub fn list_image_dedup_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<ImageDedupRecordSummary>, String> {
    image_dedup::list_records(&state.db_path).map_err(|e| e.to_string())
}

/// 读取单条图片去重记录的详情。
#[tauri::command]
pub fn get_image_dedup_record_detail(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<ImageDedupRecordDetail, String> {
    image_dedup::get_record_detail(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 检查撤回的可行性。
#[tauri::command]
pub fn check_image_dedup_rollback(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
) -> Result<ImageDedupRollbackCheck, String> {
    image_dedup::check_rollback(&state.db_path, &record_id, item_ids).map_err(|e| e.to_string())
}

/// 执行撤回。
#[tauri::command]
pub fn rollback_image_dedup(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<ImageDedupRollbackResponse, String> {
    image_dedup::rollback(&state.db_path, &record_id, item_ids, force_ignore_missing)
        .map_err(|e| e.to_string())
}

/// 删除单条记录（item 通过 FK CASCADE 自动清除）。
#[tauri::command]
pub fn delete_image_dedup_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    image_dedup::delete_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 重命名记录。
#[tauri::command]
pub fn rename_image_dedup_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    new_name: String,
) -> Result<(), String> {
    image_dedup::rename_record(&state.db_path, &record_id, &new_name).map_err(|e| e.to_string())
}
