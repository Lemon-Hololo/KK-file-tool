//! 去重任务启动命令。

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    app_state::AppState,
    models::DedupConfig,
    services::{dedup, events},
};

/// 启动一次去重任务；注册 `TaskRuntime` 后立即返回 `task_id`。
///
/// 后续通过 `pause_task` / `resume_task` / `stop_task` 控制，结果通过
/// `task_log` / `task_progress` / `task_result_partial` / `task_completed` 事件推送。
///
/// 前端可以通过 `task_id` 参数预先传入 ID，这样可以在派发任务前就开始监听事件，
/// 避免事件早于监听器到达造成丢失（与 mod 系列任务一致）。
#[tauri::command]
pub async fn start_dedup_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    config: DedupConfig,
    task_id: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }

    let (task_id, runtime) = state.create_task(task_id);

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = dedup::run_dedup(
            app_clone.clone(),
            state_clone,
            task_id_clone.clone(),
            paths,
            config,
            runtime,
        )
        .await;

        if let Err(err) = result {
            events::emit_state_changed(&app_clone, &task_id_clone, "Failed");
            events::emit_task_failed(&app_clone, &task_id_clone, &err.to_string());
        }
    });

    Ok(task_id)
}
