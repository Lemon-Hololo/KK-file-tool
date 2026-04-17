use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    app_state::{AppState, TaskRuntime},
    models::DedupConfig,
    services::dedup,
};

#[tauri::command]
pub async fn start_dedup_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
    config: DedupConfig,
) -> Result<String, String> {
    if paths.is_empty() {
        return Err("至少需要一个路径".to_string());
    }

    let task_id = Uuid::new_v4().to_string();
    let runtime = Arc::new(TaskRuntime::new());

    {
        let mut tasks = state.tasks.lock().unwrap();
        tasks.insert(task_id.clone(), runtime.clone());
    }

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = dedup::run_dedup(
            app_clone.clone(),
            state_clone.clone(),
            task_id_clone.clone(),
            paths,
            config,
            runtime,
        )
        .await;

        if let Err(err) = result {
            let _ = app_clone.emit(
                "task_state_changed",
                serde_json::json!({ "taskId": task_id_clone, "status": "Failed" }),
            );
            let _ = app_clone.emit(
                "task_failed",
                serde_json::json!({ "taskId": task_id_clone, "message": err.to_string() }),
            );
        }
    });

    Ok(task_id)
}
