//! 文件预览命令。

use std::sync::Arc;

use tauri::State;

use crate::{app_state::AppState, services::preview};

/// 前端请求预览指定路径；后端按扩展名路由到 text/image/zip 解析，
/// 不支持的类型返回 `{"type":"unsupported"}`。
#[tauri::command]
pub fn request_preview(
    state: State<'_, Arc<AppState>>,
    file_path: String,
) -> Result<serde_json::Value, String> {
    preview::detect_preview(&state.db_path, &file_path).map_err(|e| e.to_string())
}
