use crate::services::preview;

#[tauri::command]
pub fn request_preview(file_path: String) -> Result<serde_json::Value, String> {
  preview::detect_preview(&file_path).map_err(|e| e.to_string())
}