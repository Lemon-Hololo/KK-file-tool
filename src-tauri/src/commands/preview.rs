//! 文件预览命令。

use crate::services::preview;

/// 前端请求预览指定路径；后端按扩展名路由到 text/image/zip 解析，
/// 不支持的类型返回 `{"type":"unsupported"}`。
#[tauri::command]
pub fn request_preview(file_path: String) -> Result<serde_json::Value, String> {
    preview::detect_preview(&file_path).map_err(|e| e.to_string())
}
