use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 外部配置（存储在 JSON 文件中，独立于 SQLite）
/// 用于需要在打开数据库之前就读取的配置项（如数据库路径本身）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExternalConfig {
    /// 自定义数据库文件路径，None 或空字符串时使用默认路径
    pub db_path: Option<String>,
}

const CONFIG_FILE_NAME: &str = "fileflow_config.json";

pub fn config_file_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(CONFIG_FILE_NAME)
}

pub fn load_config(app_data_dir: &Path) -> ExternalConfig {
    let path = config_file_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ExternalConfig::default(),
    }
}

pub fn save_config(app_data_dir: &Path, config: &ExternalConfig) -> Result<(), String> {
    let path = config_file_path(app_data_dir);
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// 根据外部配置解析实际的数据库路径
pub fn resolve_db_path(app_data_dir: &Path, config: &ExternalConfig) -> PathBuf {
    match &config.db_path {
        Some(p) if !p.trim().is_empty() => {
            let path = PathBuf::from(p);
            // 如果用户指定的是目录，自动追加文件名
            if path.is_dir() {
                path.join("fileflow.db")
            } else {
                path
            }
        }
        _ => app_data_dir.join("fileflow.db"),
    }
}
