//! 后缀批量修改的 Tauri 命令封装。
//!
//! 仅做参数转发 / 错误映射；业务逻辑在 [`crate::services::suffix`]。

use std::sync::Arc;
use tauri::State;

use crate::{
    app_state::AppState,
    models::{
        SuffixApplyResponse, SuffixPreviewItem, SuffixRecordDetail, SuffixRecordSummary,
        SuffixRollbackCheck, SuffixRollbackResponse,
    },
    services::suffix,
};

/// 预览后缀修改。
#[tauri::command]
pub fn preview_suffix_change(
    paths: Vec<String>,
    target_suffix: String,
) -> Result<Vec<SuffixPreviewItem>, String> {
    suffix::preview_suffix_change(&paths, &target_suffix).map_err(|e| e.to_string())
}

/// 应用后缀修改并写入记录。
#[tauri::command]
pub fn apply_suffix_change(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    target_suffix: String,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> Result<SuffixApplyResponse, String> {
    suffix::apply_suffix_change(
        &state.db_path,
        &paths,
        &target_suffix,
        record_name,
        selected_old_paths,
    )
    .map_err(|e| e.to_string())
}

/// 列出全部后缀修改记录。
#[tauri::command]
pub fn list_suffix_change_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SuffixRecordSummary>, String> {
    suffix::list_suffix_records(&state.db_path).map_err(|e| e.to_string())
}

/// 读取单条记录详情。
#[tauri::command]
pub fn get_suffix_change_record_detail(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<SuffixRecordDetail, String> {
    suffix::get_suffix_record_detail(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 检查撤回的可行性（缺失文件统计）。
#[tauri::command]
pub fn check_suffix_rollback(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
) -> Result<SuffixRollbackCheck, String> {
    suffix::check_rollback(&state.db_path, &record_id, item_ids).map_err(|e| e.to_string())
}

/// 执行撤回。
#[tauri::command]
pub fn rollback_suffix_change(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<SuffixRollbackResponse, String> {
    suffix::rollback_suffix_change(&state.db_path, &record_id, item_ids, force_ignore_missing)
        .map_err(|e| e.to_string())
}

/// 删除单条记录（级联删除 item）。
#[tauri::command]
pub fn delete_suffix_change_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    suffix::delete_suffix_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}
