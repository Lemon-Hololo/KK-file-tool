use std::{collections::HashSet, path::PathBuf, sync::Arc};

use chrono::Local;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    db::{move_repo, settings_repo},
    models::{MoveActionResponse, MoveReport, MoveSummary},
    services::move_file,
    utils::path::to_extended_length_path,
};

fn default_move_dir() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe.parent().ok_or("无法获取程序目录")?;
    Ok(parent.join("temp_moved_files").display().to_string())
}

#[tauri::command]
pub fn get_move_summary(
    selected_files: Vec<String>,
    move_target_path: Option<String>,
) -> Result<MoveSummary, String> {
    let target_dir = match move_target_path {
        Some(v) if !v.trim().is_empty() => v,
        _ => default_move_dir()?,
    };

    let mut total_size = 0u64;
    for path in &selected_files {
        let p = PathBuf::from(path);
        let ep = to_extended_length_path(&p);
        if let Ok(meta) = std::fs::metadata(ep) {
            total_size += meta.len();
        }
    }

    Ok(MoveSummary {
        target_dir,
        total_selected: selected_files.len(),
        total_size,
    })
}

#[tauri::command]
pub fn apply_move_action(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
    selected_files: Vec<String>,
    move_target_path: Option<String>,
) -> Result<MoveActionResponse, String> {
    let target_dir = match move_target_path {
        Some(v) if !v.trim().is_empty() => v,
        _ => {
            let settings = settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())?;
            match settings.move_target_path {
                Some(v) if !v.trim().is_empty() => v,
                _ => default_move_dir()?,
            }
        }
    };

    let task_target_dir = PathBuf::from(&target_dir).join(&task_id);
    let move_result = move_file::move_selected_files(&task_target_dir, &selected_files);

    let report = MoveReport {
        report_id: Uuid::new_v4().to_string(),
        task_id: task_id.clone(),
        created_at: Local::now().timestamp(),
        target_dir: task_target_dir.display().to_string(),
        total_selected: selected_files.len(),
        total_success: move_result.success_items.len(),
        total_failed: move_result.failed_items.len(),
        released_bytes: move_result.released_bytes,
        success_items: move_result.success_items.clone(),
        failed_items: move_result.failed_items.clone(),
    };

    let moved_paths: Vec<String> = move_result
        .success_items
        .iter()
        .map(|x| x.src_path.clone())
        .collect();
    move_repo::save_move_report_and_cleanup_entries(&state.db_path, &report, &moved_paths)
        .map_err(|e| e.to_string())?;

    let updated_groups = {
        let moved_set: HashSet<String> = moved_paths.into_iter().collect();
        let mut task_map = state.task_results.lock().unwrap();
        let groups = task_map.entry(task_id.clone()).or_default();

        for g in groups.iter_mut() {
            g.files.retain(|f| !moved_set.contains(&f.abs_path));
        }
        groups.retain(|g| g.files.len() > 1);
        groups.clone()
    };

    let _ = app.emit(
        "move_report_ready",
        serde_json::json!({
          "taskId": task_id,
          "report": report,
          "updatedGroups": updated_groups
        }),
    );

    Ok(MoveActionResponse {
        report,
        updated_groups,
    })
}
