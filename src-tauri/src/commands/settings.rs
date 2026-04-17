use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::{
    app_state::AppState,
    db::{schema, settings_repo},
    external_config,
    models::{AppSettings, TaskStatus},
};

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(state: State<'_, Arc<AppState>>, settings: AppSettings) -> Result<(), String> {
    settings_repo::save_settings(&state.db_path, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_theme_mode(state: State<'_, Arc<AppState>>, mode: String) -> Result<(), String> {
    if !matches!(mode.as_str(), "light" | "dark" | "system") {
        return Err("invalid mode".to_string());
    }
    let mut s = settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())?;
    s.theme_mode = mode;
    settings_repo::save_settings(&state.db_path, &s).map_err(|e| e.to_string())
}

// ===== 数据库路径管理 =====

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbPathInfo {
    /// 当前正在使用的数据库路径
    pub current_path: String,
    /// 默认路径（app_data_dir/fileflow.db）
    pub default_path: String,
    /// 用户设置的自定义路径（可能为 None）
    pub custom_path: Option<String>,
}

#[tauri::command]
pub fn get_db_info(state: State<'_, Arc<AppState>>) -> Result<DbPathInfo, String> {
    let ext_config = external_config::load_config(&state.app_data_dir);
    Ok(DbPathInfo {
        current_path: state.db_path.to_string_lossy().to_string(),
        default_path: state
            .app_data_dir
            .join("fileflow.db")
            .to_string_lossy()
            .to_string(),
        custom_path: ext_config.db_path,
    })
}

#[tauri::command]
pub fn set_custom_db_path(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    let trimmed = path.trim();

    if !trimmed.is_empty() {
        let p = std::path::Path::new(trimmed);
        // 校验：如果路径看起来是文件路径，检查父目录是否存在
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(format!(
                    "路径的父目录不存在: {}",
                    parent.to_string_lossy()
                ));
            }
        }
    }

    let config = external_config::ExternalConfig {
        db_path: if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        },
    };
    external_config::save_config(&state.app_data_dir, &config)
}

// ===== 删除数据库 =====

#[tauri::command]
pub fn delete_database(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    // 检查是否有任务正在运行
    {
        let tasks = state.tasks.lock().unwrap();
        for (_, runtime) in tasks.iter() {
            let status = runtime.status.lock().unwrap();
            match *status {
                TaskStatus::Running | TaskStatus::Paused => {
                    return Err("有任务正在运行，无法删除数据库".to_string());
                }
                _ => {}
            }
        }
    }

    // 删除数据库文件及 WAL/SHM 附属文件
    if state.db_path.exists() {
        std::fs::remove_file(&state.db_path).map_err(|e| format!("删除数据库失败: {e}"))?;
    }
    let _ = std::fs::remove_file(state.db_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(state.db_path.with_extension("db-shm"));

    // 重建空数据库
    schema::init_schema(&state.db_path).map_err(|e| format!("重建数据库失败: {e}"))?;

    // 清除内存中的任务结果
    {
        let mut results = state.task_results.lock().unwrap();
        results.clear();
    }

    Ok(())
}

// ===== CPU 核心数 =====

#[tauri::command]
pub fn get_cpu_count() -> usize {
    num_cpus::get()
}
