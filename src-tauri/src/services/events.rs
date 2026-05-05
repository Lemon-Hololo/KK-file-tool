//! 前后端事件发射的统一封装。
//!
//! 所有跨组件共享的 `app.emit(...)` 调用收敛到这里，调用方无需关心事件名字面量，
//! 载荷结构也对齐 `crate::models` 中的 payload 类型。
//!
//! 注意：事件名是 IPC 契约的一部分，改动需同步 `src/` 前端监听处。

use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::{
    app_state::AppState,
    constants::events,
    models::{
        DuplicateGroup, ModDuplicatePartialPayload, ModScanCompletedPayload,
        ModVersionPartialPayload, PixivTagPartialPayload, TaskLogPayload, TaskProgressPayload,
    },
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

pub fn emit_mod_duplicate_partial(app: &AppHandle, payload: &ModDuplicatePartialPayload) {
    let _ = app.emit(events::MOD_DUPLICATE_PARTIAL, payload);
}

pub fn emit_mod_version_partial(app: &AppHandle, payload: &ModVersionPartialPayload) {
    let _ = app.emit(events::MOD_VERSION_PARTIAL, payload);
}

/// Pixiv 标签拉取的增量事件。
///
/// 一次扫描会发多次 `done = false` 的 partial（每批若干 PID 完成），
/// 任务终态时发一次 `done = true`（`items` 可能为空）。前端依靠 `done`
/// 关闭 running 状态。
pub fn emit_pixiv_tag_partial(app: &AppHandle, payload: &PixivTagPartialPayload) {
    let _ = app.emit(events::PIXIV_TAG_PARTIAL, payload);
}

/// 长任务失败时的统一收尾：发"失败"状态事件 + `task_failed` 事件，并把任务从
/// `AppState` 的运行表中移除。
///
/// 各 `start_*_task` 命令在 spawn 的闭包里收到 `Err` 后调本函数即可——避免每个
/// 命令模块都写一份 emit + remove_task 的样板。**partial done 信号需要业务侧
/// 自行先发**（不同长任务的 partial payload 类型不同，本函数不便强行抽象），
/// 然后再调本函数完成"状态切到 Failed + 解锁运行表"两步。
pub fn finalize_failed_long_task(
    app: &AppHandle,
    state: &Arc<AppState>,
    task_id: &str,
    err: &str,
) {
    emit_state_changed(app, task_id, "Failed");
    emit_task_failed(app, task_id, err);
    state.remove_task(task_id);
}

/// 启动一个长任务并安装统一的失败收尾。
///
/// 这是命令层的薄包装：克隆 `state` / `app` / `task_id` 给 spawn 闭包，
/// 等业务 future 返回 `Err(err)` 时先调可选的 `on_failure_extra`（用来发该业务
/// 自己的 partial done 信号），再调 [`finalize_failed_long_task`] 把状态切到
/// Failed 并清理运行表。成功路径业务函数自己负责终态收尾。
///
/// 使用场景：dedup、mod_scan、mod_duplicate、mod_version、pixiv_tag 的
/// `start_*_task` 命令都走相同模板——先 `state.create_task(task_id)` 拿到
/// `(task_id, runtime)`，再把 `move ||` 的业务闭包 `fut` 喂给本 helper。
///
/// `fut` 的类型签名意味着业务函数必须接受 `(AppHandle, Arc<AppState>, String)` 三元组，
/// 把它们和自己额外的参数（paths / keyword 等）一起 capture 进去。命令层只剩"组装
/// 参数 + create_task + 调 spawn_long_task"三行。
pub fn spawn_long_task<Fut, Make, Extra>(
    app: AppHandle,
    state: Arc<AppState>,
    task_id: String,
    make_future: Make,
    on_failure_extra: Extra,
) where
    Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    Make: FnOnce(AppHandle, Arc<AppState>, String) -> Fut + Send + 'static,
    Extra: FnOnce(&AppHandle, &str) + Send + 'static,
{
    let app_clone = app.clone();
    let state_clone = state.clone();
    let task_id_clone = task_id.clone();
    tauri::async_runtime::spawn(async move {
        let result = make_future(app_clone.clone(), state_clone.clone(), task_id_clone.clone()).await;
        if let Err(err) = result {
            on_failure_extra(&app_clone, &task_id_clone);
            finalize_failed_long_task(&app_clone, &state_clone, &task_id_clone, &err);
        }
    });
}
