//! Mod 工具的 Tauri 命令封装。
//!
//! Mod 工具各功能（rename / organize / cleanup / modify / scan）与共享的记录管理命令；
//! 仅做参数转发 / 错误映射，业务在 [`crate::services::mod_tools`]。

use std::sync::Arc;

use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::{
    app_state::{AppState, TaskRuntime},
    models::{
        ModDuplicateGroup, ModDuplicatePartialPayload, ModOpApplyResponse, ModOpRecordDetail,
        ModOpRecordSummary, ModOpRollbackCheck, ModOpRollbackResponse, ModOrganizePreviewItem,
        ModRenamePreviewItem, ModVersionGroup, ModVersionPartialPayload,
    },
    services::{
        events,
        logging::TaskLogContext,
        mod_tools::{self, cleanup, modify, organize, rename, scan},
    },
};

// ---- 重命名 ----

/// 预览 Mod 重命名。
#[tauri::command]
pub fn preview_mod_rename(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<Vec<ModRenamePreviewItem>, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    rename::preview_mod_rename(&state.db_path, &paths, log)
}

/// 应用 Mod 重命名。
#[tauri::command]
pub fn apply_mod_rename(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
    task_id: Option<String>,
) -> Result<ModOpApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    rename::apply_mod_rename(&state.db_path, &paths, record_name, selected_old_paths, log)
        .map_err(|e| e.to_string())
}

// ---- 归类 ----

/// 预览按 `[...]` 括号归类。
#[tauri::command]
pub fn preview_mod_organize(
    app: AppHandle,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<Vec<ModOrganizePreviewItem>, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    organize::preview_mod_organize(&paths, log).map_err(|e| e.to_string())
}

/// 应用归类并写入记录。
#[tauri::command]
pub fn apply_mod_organize(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
    task_id: Option<String>,
) -> Result<ModOpApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    organize::apply_mod_organize(&state.db_path, &paths, record_name, selected_old_paths, log)
        .map_err(|e| e.to_string())
}

// ---- 重复 / 不同版本检查 ----

/// 预览 `guid + author + version` 完全相同的重复 MOD。
#[tauri::command]
pub fn preview_mod_duplicates(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<Vec<ModDuplicateGroup>, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    cleanup::preview_mod_duplicates(&state.db_path, &paths, log).map_err(|e| e.to_string())
}

/// 启动重复 MOD 检查长任务，结果通过 `mod_duplicate_partial` 增量推送。
#[tauri::command]
pub async fn start_mod_duplicate_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }

    let task_id = task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let runtime = Arc::new(TaskRuntime::new());
    state.insert_task(task_id.clone(), runtime.clone());

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = cleanup::run_duplicate_scan(
            app_clone.clone(),
            state_clone.clone(),
            task_id_clone.clone(),
            paths,
            runtime,
        )
        .await;

        if let Err(err) = result {
            events::emit_state_changed(&app_clone, &task_id_clone, "Failed");
            events::emit_task_failed(&app_clone, &task_id_clone, &err);
            events::emit_mod_duplicate_partial(
                &app_clone,
                &ModDuplicatePartialPayload {
                    task_id: task_id_clone.clone(),
                    groups: vec![],
                    done: true,
                },
            );
            state_clone.tasks.lock().unwrap().remove(&task_id_clone);
        }
    });

    Ok(task_id)
}

/// 删除重复 MOD 中选中的文件并写入可撤回记录。
#[tauri::command]
pub fn apply_mod_duplicate_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    task_id: Option<String>,
) -> Result<ModOpApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    cleanup::apply_mod_duplicate_delete(
        &state.db_path,
        &paths,
        selected_file_paths,
        record_name,
        log,
    )
    .map_err(|e| e.to_string())
}

/// 预览 `guid + author` 相同但版本不同的 MOD。
#[tauri::command]
pub fn preview_mod_versions(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<Vec<ModVersionGroup>, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    cleanup::preview_mod_versions(&state.db_path, &paths, log).map_err(|e| e.to_string())
}

/// 启动不同版本 MOD 检查长任务，结果通过 `mod_version_partial` 增量推送。
#[tauri::command]
pub async fn start_mod_version_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    task_id: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }

    let task_id = task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let runtime = Arc::new(TaskRuntime::new());
    state.insert_task(task_id.clone(), runtime.clone());

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = cleanup::run_version_scan(
            app_clone.clone(),
            state_clone.clone(),
            task_id_clone.clone(),
            paths,
            runtime,
        )
        .await;

        if let Err(err) = result {
            events::emit_state_changed(&app_clone, &task_id_clone, "Failed");
            events::emit_task_failed(&app_clone, &task_id_clone, &err);
            events::emit_mod_version_partial(
                &app_clone,
                &ModVersionPartialPayload {
                    task_id: task_id_clone.clone(),
                    groups: vec![],
                    done: true,
                },
            );
            state_clone.tasks.lock().unwrap().remove(&task_id_clone);
        }
    });

    Ok(task_id)
}

/// 删除不同版本 MOD 中选中的文件并写入可撤回记录。
#[tauri::command]
pub fn apply_mod_version_delete(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    task_id: Option<String>,
) -> Result<ModOpApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    cleanup::apply_mod_version_delete(
        &state.db_path,
        &paths,
        selected_file_paths,
        record_name,
        log,
    )
    .map_err(|e| e.to_string())
}

// ---- 修改版本限制 ----

/// 对选中的 `.zipmod` 从 manifest.xml 中移除 `<game>KEYWORD</game>` 标签，
/// 并把修改过程记录为 `kind = "modify"` 的 Mod 操作记录（可撤回）。
#[tauri::command]
pub fn apply_mod_modify_version(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    keyword: String,
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    task_id: Option<String>,
) -> Result<ModOpApplyResponse, String> {
    let log = TaskLogContext::from_task(&app, task_id.as_deref());
    modify::apply_mod_modify_version(
        &state.db_path,
        &paths,
        keyword,
        selected_file_paths,
        record_name,
        log,
    )
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
    task_id: Option<String>,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }
    if keyword.trim().is_empty() {
        return Err("关键字不能为空".to_string());
    }

    let task_id = task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
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
