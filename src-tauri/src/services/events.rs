//! 前后端事件发射的统一封装。
//!
//! 所有跨组件共享的 `app.emit(...)` 调用收敛到这里，调用方无需关心事件名字面量，
//! 载荷结构也对齐 `crate::models` 中的 payload 类型。
//!
//! 注意：事件名是 IPC 契约的一部分，改动需同步 `src/` 前端监听处。

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::{
    constants::events,
    models::{DuplicateGroup, ModScanCompletedPayload, TaskLogPayload, TaskProgressPayload},
};

pub fn emit_log(
    app: &AppHandle,
    task_id: &str,
    level: &str,
    message: &str,
    file_path: Option<String>,
) {
    let _ = app.emit(
        events::TASK_LOG,
        TaskLogPayload {
            task_id: task_id.to_string(),
            level: level.to_string(),
            message: message.to_string(),
            file_path,
        },
    );
}

pub fn emit_progress(app: &AppHandle, task_id: &str, stage: &str, processed: usize, total: usize) {
    let percent = if total == 0 {
        0.0
    } else {
        (processed as f64 / total as f64) * 100.0
    };
    let _ = app.emit(
        events::TASK_PROGRESS,
        TaskProgressPayload {
            task_id: task_id.to_string(),
            stage: stage.to_string(),
            processed,
            total,
            percent,
        },
    );
}

pub fn emit_state_changed(app: &AppHandle, task_id: &str, status: &str) {
    let _ = app.emit(
        events::TASK_STATE_CHANGED,
        json!({ "taskId": task_id, "status": status }),
    );
}

pub fn emit_task_failed(app: &AppHandle, task_id: &str, message: &str) {
    let _ = app.emit(
        events::TASK_FAILED,
        json!({ "taskId": task_id, "message": message }),
    );
}

pub fn emit_result_partial(app: &AppHandle, task_id: &str, groups: &[DuplicateGroup], done: bool) {
    let _ = app.emit(
        events::TASK_RESULT_PARTIAL,
        json!({ "taskId": task_id, "groups": groups, "done": done }),
    );
}

pub fn emit_task_completed(app: &AppHandle, task_id: &str, groups: &[DuplicateGroup]) {
    let _ = app.emit(
        events::TASK_COMPLETED,
        json!({ "taskId": task_id, "groups": groups }),
    );
}

pub fn emit_mod_scan_completed(app: &AppHandle, payload: &ModScanCompletedPayload) {
    let _ = app.emit(events::MOD_SCAN_COMPLETED, payload);
}
