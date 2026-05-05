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
    let state_arc = state.inner().clone();

    events::spawn_long_task(
        app,
        state_arc,
        task_id.clone(),
        move |app, state, task_id| async move {
            // run_dedup 失败时已通过日志面板告诉用户原因；这里把 AppError 转字符串供
            // finalize_failed_long_task 发到 task_failed 事件。
            dedup::run_dedup(app, state, task_id, paths, config, runtime)
                .await
                .map_err(|e| e.to_string())
        },
        |_app, _task_id| {
            // 去重没有 partial done 协议，失败路径不需要额外推送。
        },
    );

    Ok(task_id)
}
