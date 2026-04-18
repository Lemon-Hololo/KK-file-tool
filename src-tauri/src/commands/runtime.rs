//! 任务运行时控制：暂停 / 恢复 / 停止。
//!
//! 三个命令通过 `AppState::with_task` 定位到 `TaskRuntime`，再切换其
//! `paused` / `cancelled` 原子位。实际的响应由各任务循环在下一次检查点完成。

use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, State};

use crate::{app_state::AppState, models::TaskStatus, services::events};

/// 暂停任务。设置 `paused = true`，任务循环在下一个 `pause_point` 暂停。
#[tauri::command]
pub fn pause_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<(), String> {
    state
        .with_task(&task_id, |runtime| {
            runtime.paused.store(true, Ordering::Relaxed);
            runtime.set_status(TaskStatus::Paused);
        })
        .map_err(|e| e.to_string())?;

    events::emit_state_changed(&app, &task_id, "Paused");
    Ok(())
}

/// 恢复任务。
#[tauri::command]
pub fn resume_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<(), String> {
    state
        .with_task(&task_id, |runtime| {
            runtime.paused.store(false, Ordering::Relaxed);
            runtime.set_status(TaskStatus::Running);
        })
        .map_err(|e| e.to_string())?;

    events::emit_state_changed(&app, &task_id, "Running");
    Ok(())
}

/// 请求取消任务；已入队的工作可能仍会执行完（不保证立即停止）。
#[tauri::command]
pub fn stop_task(state: State<'_, Arc<AppState>>, task_id: String) -> Result<(), String> {
    state
        .with_task(&task_id, |runtime| {
            runtime.cancelled.store(true, Ordering::Relaxed);
            runtime.set_status(TaskStatus::Cancelled);
        })
        .map_err(|e| e.to_string())
}
