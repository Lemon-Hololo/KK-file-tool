//! Pixiv 标签整理的 Tauri 命令。
//!
//! 命令层只做参数校验 / 转发，业务逻辑全部在 [`crate::services::pixiv_tag`]。

use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    app_state::AppState,
    models::{PixivImageRow, PixivTagFetchResult},
    services::pixiv_tag,
};

/// 同步扫描任务输入目录，返回所有可识别 PID 的图片候选。
///
/// 不拉网络、不读文件内容，瞬间完成。前端拿到候选后立即建出全部 pending 行，
/// 再调 [`start_pixiv_tag_scan_task`] 开长任务异步拉 tag。
#[tauri::command]
pub fn scan_pixiv_image_candidates(paths: Vec<String>) -> Result<Vec<PixivImageRow>, String> {
    pixiv_tag::scan_pixiv_images(&paths).map_err(|e| e.to_string())
}

/// 启动 Pixiv tag 拉取长任务，返回 `task_id`。
///
/// 取消通过共享的 [`crate::commands::runtime::stop_task`] 完成。
/// 终态（成功 / 失败 / 取消）会通过 `AppState::remove_task` 清理自身。
#[tauri::command]
pub async fn start_pixiv_tag_scan_task(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    pids: Vec<String>,
    task_id: Option<String>,
) -> Result<String, String> {
    let (task_id, runtime) = state.create_task(task_id);

    let state_clone = state.inner().clone();
    let app_clone = app.clone();
    let task_id_clone = task_id.clone();

    tauri::async_runtime::spawn(async move {
        let result = pixiv_tag::run_pixiv_tag_scan(
            app_clone.clone(),
            state_clone.clone(),
            task_id_clone.clone(),
            pids,
            runtime,
        )
        .await;

        let _ = result;
    });

    Ok(task_id)
}

/// 单条 PID 的同步重试拉取（给前端"重试"按钮用）。
#[tauri::command]
pub async fn fetch_pixiv_tag_single(
    state: State<'_, Arc<AppState>>,
    pid: String,
) -> Result<PixivTagFetchResult, String> {
    pixiv_tag::fetch_pixiv_tag_single(&state.db_path, &pid)
        .await
        .map_err(|e| e.to_string())
}

/// 把图片移动到 `<output_dir>/<sanitized_tag>/<basename>`，返回新路径。
///
/// 调用方一般在用户点击 tag 单元格后立即调用；这是同步轻量操作，无记录、无撤回。
#[tauri::command]
pub fn move_image_by_tag_command(
    abs_path: String,
    output_dir: String,
    tag: String,
) -> Result<String, String> {
    pixiv_tag::move_image_by_tag(&abs_path, &output_dir, &tag).map_err(|e| e.to_string())
}
