//! 空文件夹清理的 Tauri 命令封装。
//!
//! 仅做参数转发 / 错误映射；业务逻辑在 [`crate::services::empty_dirs`]。

use std::sync::Arc;

use tauri::State;

use crate::{
    app_state::AppState,
    models::{
        EmptyDirApplyResponse, EmptyDirPreviewItem, EmptyDirRecordDetail, EmptyDirRecordSummary,
        EmptyDirRollbackCheck, EmptyDirRollbackResponse,
    },
    services::empty_dirs,
};

/// 预览递归清理时可以删除的空文件夹。
#[tauri::command]
pub fn preview_empty_dirs(
    paths: Vec<String>,
    include_roots: bool,
) -> Result<Vec<EmptyDirPreviewItem>, String> {
    empty_dirs::preview_empty_dirs(&paths, include_roots).map_err(|e| e.to_string())
}

/// 删除空文件夹并写入可撤回记录。
#[tauri::command]
pub fn apply_empty_dir_cleanup(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    include_roots: bool,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> Result<EmptyDirApplyResponse, String> {
    empty_dirs::apply_empty_dir_cleanup(
        &state.db_path,
        &paths,
        include_roots,
        record_name,
        selected_old_paths,
    )
    .map_err(|e| e.to_string())
}

/// 列出全部空文件夹清理记录。
#[tauri::command]
pub fn list_empty_dir_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<EmptyDirRecordSummary>, String> {
    empty_dirs::list_empty_dir_records(&state.db_path).map_err(|e| e.to_string())
}

/// 读取单条空文件夹清理记录详情。
#[tauri::command]
pub fn get_empty_dir_record_detail(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<EmptyDirRecordDetail, String> {
    empty_dirs::get_empty_dir_record_detail(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 检查撤回的可行性。
#[tauri::command]
pub fn check_empty_dir_rollback(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
) -> Result<EmptyDirRollbackCheck, String> {
    empty_dirs::check_rollback(&state.db_path, &record_id, item_ids).map_err(|e| e.to_string())
}

/// 撤回空文件夹清理，重新创建记录中的目录。
#[tauri::command]
pub fn rollback_empty_dir_cleanup(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<EmptyDirRollbackResponse, String> {
    empty_dirs::rollback_empty_dirs(&state.db_path, &record_id, item_ids, force_ignore_missing)
        .map_err(|e| e.to_string())
}

/// 删除单条空文件夹清理记录。
#[tauri::command]
pub fn delete_empty_dir_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    empty_dirs::delete_empty_dir_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}
