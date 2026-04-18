//! 哈希记录管理命令。纯转发到 [`crate::db::hash_repo`]。

use std::sync::Arc;

use tauri::State;

use crate::{
    app_state::AppState,
    db::hash_repo,
    models::{HashIndexRecord, HashIndexRecordSummary},
};

/// 列出全部哈希记录摘要。
#[tauri::command]
pub fn list_hash_records(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<HashIndexRecordSummary>, String> {
    hash_repo::list_hash_records(&state.db_path).map_err(|e| e.to_string())
}

/// 读取单条记录详情（含全部 entries）。
#[tauri::command]
pub fn load_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<HashIndexRecord, String> {
    hash_repo::load_hash_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}

/// 重命名记录。
#[tauri::command]
pub fn rename_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
    new_name: String,
) -> Result<(), String> {
    hash_repo::rename_hash_record(&state.db_path, &record_id, &new_name).map_err(|e| e.to_string())
}

/// 删除单条记录（级联删除 entries）。
#[tauri::command]
pub fn delete_hash_record(
    state: State<'_, Arc<AppState>>,
    record_id: String,
) -> Result<(), String> {
    hash_repo::delete_hash_record(&state.db_path, &record_id).map_err(|e| e.to_string())
}
