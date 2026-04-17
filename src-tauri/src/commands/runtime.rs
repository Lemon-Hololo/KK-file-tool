use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::{app_state::AppState, models::TaskStatus};

#[tauri::command]
pub fn pause_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<(), String> {
    let tasks = state.tasks.lock().unwrap();
    let task = tasks.get(&task_id).ok_or("task not found")?;
    task.paused
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *task.status.lock().unwrap() = TaskStatus::Paused;

    // 通知前端状态变化
    let _ = app.emit(
        "task_state_changed",
        serde_json::json!({ "taskId": task_id, "status": "Paused" }),
    );

    Ok(())
}

#[tauri::command]
pub fn resume_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<(), String> {
    let tasks = state.tasks.lock().unwrap();
    let task = tasks.get(&task_id).ok_or("task not found")?;
    task.paused
        .store(false, std::sync::atomic::Ordering::Relaxed);
    *task.status.lock().unwrap() = TaskStatus::Running;

    // 通知前端状态变化
    let _ = app.emit(
        "task_state_changed",
        serde_json::json!({ "taskId": task_id, "status": "Running" }),
    );

    Ok(())
}

#[tauri::command]
pub fn stop_task(state: State<'_, Arc<AppState>>, task_id: String) -> Result<(), String> {
    let tasks = state.tasks.lock().unwrap();
    let task = tasks.get(&task_id).ok_or("task not found")?;
    task.cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *task.status.lock().unwrap() = TaskStatus::Cancelled;
    Ok(())
}
