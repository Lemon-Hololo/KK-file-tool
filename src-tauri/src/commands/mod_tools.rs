//! Mod 工具的 Tauri 命令封装。
//!
//! 三个功能（rename / organize / scan）与共享的记录管理命令；
//! 仅做参数转发 / 错误映射，业务在 [`crate::services::mod_tools`]。

use std::sync::Arc;

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    app_state::{AppState, TaskRuntime},
    models::{
        ModOpApplyResponse, ModOpRecordDetail, ModOpRecordSummary, ModOpRollbackCheck,
        ModOpRollbackResponse, ModOrganizePreviewItem, ModRenamePreviewItem,
    },
    services::{
        events,
        mod_tools::{self, organize, rename, scan},
    },
};

// ---- 重命名 ----

/// 预览 Mod 重命名。
#[tauri::command]
pub fn preview_mod_rename(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
) -> Result<Vec<ModRenamePreviewItem>, String> {
    rename::preview_mod_rename(&state.db_path, &paths)
}

/// 应用 Mod 重命名。
#[tauri::command]
pub fn apply_mod_rename(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> Result<ModOpApplyResponse, String> {
    rename::apply_mod_rename(&state.db_path, &paths, record_name, selected_old_paths)
        .map_err(|e| e.to_string())
}

// ---- 归类 ----

/// 预览按 `[...]` 括号归类。
#[tauri::command]
pub fn preview_mod_organize(paths: Vec<String>) -> Result<Vec<ModOrganizePreviewItem>, String> {
    organize::preview_mod_organize(&paths).map_err(|e| e.to_string())
}

/// 应用归类并写入记录。
#[tauri::command]
pub fn apply_mod_organize(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> Result<ModOpApplyResponse, String> {
    organize::apply_mod_organize(&state.db_path, &paths, record_name, selected_old_paths)
        .map_err(|e| e.to_string())
}

// ---- 记录管理 ----

/// 列出 Mod 操作记录；可按 `kind` 过滤（`rename` / `organize`）。
#[tauri::command]
pub fn list_mod_op_records(
    state: State<'_, Arc<AppState>>,
    kind: Option<String>,
) -> Result<Vec<ModOpRecordSummary>, String> {
    mod_tools::list_records(&state.db_path, kind.as_deref()).map_err(|e| e.to_string())
}

/// 读取单条 Mod 操作记录的详情。
#[tauri::command]
pub fn get_mod_op_record_detail(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<ModOpRecordDetail, String> {
    mod_tools::get_record_detail(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 检查撤回的可行性。
#[tauri::command]
pub fn check_mod_op_rollback(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
) -> Result<ModOpRollbackCheck, String> {
    mod_tools::check_rollback(&state.db_path, &record_id, item_ids).map_err(|e| e.to_string())
}

/// 执行撤回。
#[tauri::command]
pub fn rollback_mod_op(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<ModOpRollbackResponse, String> {
    mod_tools::rollback(&state.db_path, &record_id, item_ids, force_ignore_missing)
        .map_err(|e| e.to_string())
}

/// 删除单条记录。
#[tauri::command]
pub fn delete_mod_op_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    mod_tools::delete_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 重命名记录。
#[tauri::command]
pub fn rename_mod_op_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    new_name: String,
) -> Result<(), String> {
    mod_tools::rename_record(&state.db_path, &record_id, &new_name).map_err(|e| e.to_string())
}

/// 导出扫描结果为 UTF-8 文本（CRLF 换行）。
#[tauri::command]
pub fn export_mod_scan_result(target_path: String, lines: Vec<String>) -> Result<(), String> {
    use std::io::Write;

    let ep = crate::utils::path::to_extended_length_path(std::path::Path::new(&target_path));
    let mut f = std::fs::File::create(&ep).map_err(|e| e.to_string())?;
    for l in &lines {
        f.write_all(l.as_bytes()).map_err(|e| e.to_string())?;
        f.write_all(b"\r\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---- 扫描长任务 ----

/// 启动 Mod 关键字扫描长任务，返回 `task_id`。
///
/// 取消通过共享的 [`crate::commands::runtime::stop_task`] 命令完成
/// （扫描任务也插入到 `AppState.tasks`）。
#[tauri::command]
pub async fn start_mod_scan_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    keyword: String,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }
    if keyword.trim().is_empty() {
        return Err("关键字不能为空".to_string());
    }

    let task_id = Uuid::new_v4().to_string();
    let runtime = Arc::new(TaskRuntime::new());
    state.insert_task(task_id.clone(), runtime.clone());

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = scan::run_scan(
            app_clone.clone(),
            state_clone,
            task_id_clone.clone(),
            paths,
            keyword,
            runtime,
        )
        .await;

        if let Err(err) = result {
            events::emit_state_changed(&app_clone, &task_id_clone, "Failed");
            events::emit_task_failed(&app_clone, &task_id_clone, &err);
        }
    });

    Ok(task_id)
}
