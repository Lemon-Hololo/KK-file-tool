//! 设置与数据库路径管理命令。

use std::path::Path;
use std::sync::Arc;

use tauri::State;

use crate::{
    app_state::AppState,
    constants::{db_file, theme},
    db::{schema, settings_repo},
    external_config,
    models::{AppSettings, DbPathInfo},
};

/// 读取当前用户设置。
#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())
}

/// 全量写入用户设置。
#[tauri::command]
pub fn save_settings(state: State<'_, Arc<AppState>>, settings: AppSettings) -> Result<(), String> {
    settings_repo::save_settings(&state.db_path, &settings).map_err(|e| e.to_string())
}

/// 单独更新主题模式；校验传入值是否合法。
#[tauri::command]
pub fn set_theme_mode(state: State<'_, Arc<AppState>>, mode: String) -> Result<(), String> {
    if !theme::is_valid(mode.as_str()) {
        return Err("invalid mode".to_string());
    }
    let mut s = settings_repo::get_settings(&state.db_path).map_err(|e| e.to_string())?;
    s.theme_mode = mode;
    settings_repo::save_settings(&state.db_path, &s).map_err(|e| e.to_string())
}

// ===== 数据库路径管理 =====

/// 返回当前路径 / 默认路径 / 自定义路径三元组。
#[tauri::command]
pub fn get_db_info(state: State<'_, Arc<AppState>>) -> Result<DbPathInfo, String> {
    let ext_config = external_config::load_config(&state.app_data_dir);
    Ok(DbPathInfo {
        current_path: state.db_path.to_string_lossy().to_string(),
        default_path: state.default_db_path().to_string_lossy().to_string(),
        custom_path: ext_config.db_path,
    })
}

/// 设置/清空自定义数据库路径（写入 `kk-file-tool_config.json`）。
///
/// 变更仅在下次启动时生效，避免运行中切换连接导致数据丢失。
#[tauri::command]
pub fn set_custom_db_path(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    let trimmed = path.trim();

    if !trimmed.is_empty() {
        let p = std::path::Path::new(trimmed);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                return Err(format!("路径的父目录不存在: {}", parent.to_string_lossy()));
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

/// 删除整个数据库（含 WAL / SHM 文件），然后重新初始化空 schema。
///
/// 有运行中 / 暂停中任务时拒绝执行，避免文件被占用。
#[tauri::command]
pub fn delete_database(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if state.has_active_tasks() {
        return Err("有任务正在运行，无法删除数据库".to_string());
    }

    if state.db_path.exists() {
        std::fs::remove_file(&state.db_path).map_err(|e| format!("删除数据库失败: {e}"))?;
    }
    let _ = std::fs::remove_file(state.db_path.with_extension(db_file::WAL_EXT));
    let _ = std::fs::remove_file(state.db_path.with_extension(db_file::SHM_EXT));

    schema::init_schema(&state.db_path).map_err(|e| format!("重建数据库失败: {e}"))?;

    state.clear_task_results();

    Ok(())
}

// ===== CPU 核心数 =====

/// 供前端渲染"并发核心数"可选上限。
#[tauri::command]
pub fn get_cpu_count() -> usize {
    num_cpus::get()
}

// ===== 设置导入 / 导出辅助 =====

/// 读取用户通过系统文件选择框选中的 UTF-8 文本文件。
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    std::fs::read_to_string(trimmed).map_err(|e| format!("读取文件失败: {e}"))
}

/// 把前端生成的 UTF-8 文本写入用户通过系统保存框选择的路径。
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("文件路径不能为空".to_string());
    }
    let p = Path::new(trimmed);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
    }
    std::fs::write(p, content).map_err(|e| format!("写入文件失败: {e}"))
}
