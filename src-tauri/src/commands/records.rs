use std::sync::Arc;
use tauri::State;

use crate::{
    app_state::AppState,
    db::hash_repo,
    models::{HashIndexRecord, HashIndexRecordSummary},
};

#[tauri::command]
pub fn list_hash_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<HashIndexRecordSummary>, String> {
    hash_repo::list_hash_records(&state.db_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<HashIndexRecord, String> {
    hash_repo::load_hash_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    new_name: String,
) -> Result<(), String> {
    hash_repo::rename_hash_record(&state.db_path, &record_id, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    hash_repo::delete_hash_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}
