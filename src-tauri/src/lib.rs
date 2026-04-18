//! Tauri 应用入口。
//!
//! `main.rs` 仅调用本模块的 `run()`，所有启动逻辑与命令注册集中于此。

pub mod app_state;
pub mod config;
pub mod constants;
pub mod error;
pub mod external_config;
pub mod models;

pub mod commands;
pub mod db;
pub mod services;
pub mod utils;

use app_state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir error: {e}"))?;

            let state = AppState::bootstrap(app_dir).map_err(|e| e.to_string())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // path
            commands::path::normalize_input_paths,
            // dedup + runtime + move
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
            commands::suffix::rollback_suffix_change,
            // mod tools (rename / organize / scan)
            commands::mod_tools::preview_mod_rename,
            commands::mod_tools::apply_mod_rename,
            commands::mod_tools::preview_mod_organize,
            commands::mod_tools::apply_mod_organize,
            commands::mod_tools::list_mod_op_records,
            commands::mod_tools::get_mod_op_record_detail,
            commands::mod_tools::check_mod_op_rollback,
            commands::mod_tools::rollback_mod_op,
            commands::mod_tools::delete_mod_op_record,
            commands::mod_tools::rename_mod_op_record,
            commands::mod_tools::start_mod_scan_task,
            commands::mod_tools::export_mod_scan_result
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
