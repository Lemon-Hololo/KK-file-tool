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

#[tauri::command]
pub fn preview_suffix_change(
    paths: Vec<String>,
    target_suffix: String,
) -> Result<Vec<SuffixPreviewItem>, String> {
    suffix::preview_suffix_change(&paths, &target_suffix)
}

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
}

#[tauri::command]
pub fn list_suffix_change_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SuffixRecordSummary>, String> {
    suffix::list_suffix_records(&state.db_path)
}

#[tauri::command]
pub fn get_suffix_change_record_detail(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<SuffixRecordDetail, String> {
    suffix::get_suffix_record_detail(&state.db_path, &record_id)
}

#[tauri::command]
pub fn check_suffix_rollback(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
) -> Result<SuffixRollbackCheck, String> {
    suffix::check_rollback(&state.db_path, &record_id, item_ids)
}

#[tauri::command]
pub fn rollback_suffix_change(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<SuffixRollbackResponse, String> {
    suffix::rollback_suffix_change(&state.db_path, &record_id, item_ids, force_ignore_missing)
}

#[tauri::command]
pub fn delete_suffix_change_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    suffix::delete_suffix_record(&state.db_path, &record_id)
}
