pub mod app_state;
pub mod config;
pub mod error;
pub mod external_config;
pub mod models;

pub mod commands;
pub mod db;
pub mod services;
pub mod utils;

use std::sync::{Arc, Mutex};

use app_state::AppState;
use tauri::Manager;

/// 构建并返回 Tauri Builder（供 main.rs 调用）
///
/// 这样做的好处：
/// 1) main.rs 变得极薄，只负责 run
/// 2) 后续可以在 lib 层做测试与复用
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir error: {e}"))?;

            if !app_dir.exists() {
                std::fs::create_dir_all(&app_dir)
                    .map_err(|e| format!("create app_data_dir failed: {e}"))?;
            }

            let db_path = {
                let ext_config = external_config::load_config(&app_dir);
                let resolved = external_config::resolve_db_path(&app_dir, &ext_config);
                // 若自定义路径的父目录不存在，回退到默认路径
                if let Some(parent) = resolved.parent() {
                    if !parent.exists() {
                        eprintln!(
                            "Custom db_path parent dir does not exist: {:?}, falling back to default",
                            parent
                        );
                        app_dir.join("fileflow.db")
                    } else {
                        resolved
                    }
                } else {
                    app_dir.join("fileflow.db")
                }
            };
            db::schema::init_schema(&db_path).map_err(|e| e.to_string())?;

            let state = Arc::new(AppState {
                app_data_dir: app_dir,
                db_path,
                tasks: Mutex::new(std::collections::HashMap::new()),
                task_results: Mutex::new(std::collections::HashMap::new()),
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // path
            commands::path::normalize_input_paths,
            // dedup + runtime + move (批次B拆分后)
            commands::dedup::start_dedup_task,
            commands::runtime::pause_task,
            commands::runtime::resume_task,
            commands::runtime::stop_task,
            commands::move_file::get_move_summary,
            commands::move_file::apply_move_action,
            // preview
            commands::preview::request_preview,
            // settings
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::set_theme_mode,
            commands::settings::get_db_info,
            commands::settings::set_custom_db_path,
            commands::settings::delete_database,
            commands::settings::get_cpu_count,
            // hash records
            commands::records::list_hash_records,
            commands::records::load_hash_record,
            commands::records::rename_hash_record,
            commands::records::delete_hash_record,
            // suffix change
            commands::suffix::preview_suffix_change,
            commands::suffix::apply_suffix_change,
            commands::suffix::list_suffix_change_records,
            commands::suffix::get_suffix_change_record_detail,
            commands::suffix::check_suffix_rollback,
            commands::suffix::delete_suffix_change_record,
            commands::suffix::rollback_suffix_change
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
