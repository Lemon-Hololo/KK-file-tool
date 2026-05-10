//! 去重结果移动命令。

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use chrono::Local;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    app_state::AppState,
    constants::events,
    db::{move_repo, settings_repo},
    models::{MoveActionResponse, MoveReport, MoveSummary},
    services::move_file,
    utils::path::to_extended_length_path,
};

/// 用户未指定目标目录且设置里也没配置时使用的默认移动目录：
/// `<exe 同级>/temp_moved_files`。
fn default_move_dir() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let parent = exe.parent().ok_or("无法获取程序目录")?;
    Ok(parent.join("temp_moved_files").display().to_string())
}

/// 预估移动：目标目录 + 选中文件数 + 总字节数；不执行。
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

/// 执行移动：把 `selected_files` 移动到 `<target_dir>/<task_id>/`，写入报告，
/// 更新内存中的重复组，发送 `move_report_ready` 事件。
///
/// `source_paths` 是当前任务的输入路径（前端 `paths`），仅在用户开启了
/// `preserve_dir_on_move` 设置时被读取——决定每个文件落地的相对子目录。
#[tauri::command]
pub fn apply_move_action(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    task_id: String,
    selected_files: Vec<String>,
    move_target_path: Option<String>,
    source_paths: Option<Vec<String>>,
) -> Result<MoveActionResponse, String> {
    let settings = settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())?;
    let target_dir = match move_target_path {
        Some(v) if !v.trim().is_empty() => v,
        _ => match settings.move_target_path.as_ref() {
            Some(v) if !v.trim().is_empty() => v.clone(),
            _ => default_move_dir()?,
        },
    };

    let task_target_dir = PathBuf::from(&target_dir).join(&task_id);
    // 没传 source_paths 等于空切片；service 在 preserve_structure = false 时不读它。
    let roots = source_paths.unwrap_or_default();
    let move_result = move_file::move_selected_files(
        &task_target_dir,
        &selected_files,
        &roots,
        settings.preserve_dir_on_move,
    );

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

    // 更新内存中的任务结果：把已成功移动的文件从 groups 里剔除，
    // 仅剩 1 个文件的组也移除（不再算重复）。
    let moved_set: HashSet<String> = moved_paths.into_iter().collect();
    let updated_groups = state.update_task_results(&task_id, |groups| {
        for g in groups.iter_mut() {
            g.files.retain(|f| !moved_set.contains(&f.abs_path));
        }
        groups.retain(|g| g.files.len() > 1);
    });

    let _ = app.emit(
        events::MOVE_REPORT_READY,
        json!({
            "taskId": task_id,
            "report": report,
            "updatedGroups": updated_groups,
        }),
    );

    Ok(MoveActionResponse {
        report,
        updated_groups,
    })
}
