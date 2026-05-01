//! 独立于 SQLite 的外部 JSON 配置。
//!
//! 用于在"能打开数据库之前"就需要读取的配置项——典型就是数据库路径本身
//! （鸡生蛋问题：无法把数据库路径存到数据库里）。文件位于
//! `<app_data_dir>/kk-file-tool_config.json`。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// JSON 文件的结构；缺失字段按默认值处理。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExternalConfig {
    /// 用户自定义的数据库文件路径；`None` 或空字符串时使用默认路径。
    pub db_path: Option<String>,
}

const CONFIG_FILE_NAME: &str = "kk-file-tool_config.json";

/// 返回配置文件的绝对路径（`<app_data_dir>/kk-file-tool_config.json`）。
pub fn config_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(CONFIG_FILE_NAME)
}

/// 从 JSON 文件加载配置；文件不存在或解析失败时返回 `Default`。
pub fn load_config(app_data_dir: &Path) -> ExternalConfig {
    let path = config_file_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ExternalConfig::default(),
    }
}

/// 写回 JSON 文件（美化缩进）。
pub fn save_config(app_data_dir: &Path, config: &ExternalConfig) -> Result<(), String> {
    let path = config_file_path(app_data_dir);
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// 由配置解析实际数据库路径；用户指定为目录时自动追加默认文件名。
pub fn resolve_db_path(app_data_dir: &Path, config: &ExternalConfig) -> PathBuf {
    match &config.db_path {
        Some(p) if !p.trim().is_empty() => {
            let path = PathBuf::from(p);
            if path.is_dir() {
                path.join("kk-file-tool.db")
            } else {
                path
            }
        }
        _ => app_data_dir.join("kk-file-tool.db"),
    }
}
