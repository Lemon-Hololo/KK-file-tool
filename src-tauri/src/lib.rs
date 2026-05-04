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
            commands::path::reveal_in_explorer,
            // dedup + runtime + move
            commands::dedup::start_dedup_task,
            commands::runtime::pause_task,
            commands::runtime::resume_task,
            commands::runtime::stop_task,
            commands::move_file::get_move_summary,
            commands::move_file::apply_move_action,
            // empty folder cleanup
            commands::empty_dirs::preview_empty_dirs,
            commands::empty_dirs::apply_empty_dir_cleanup,
            commands::empty_dirs::list_empty_dir_records,
            commands::empty_dirs::get_empty_dir_record_detail,
            commands::empty_dirs::check_empty_dir_rollback,
            commands::empty_dirs::rollback_empty_dir_cleanup,
            commands::empty_dirs::delete_empty_dir_record,
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
            commands::settings::read_text_file,
            commands::settings::write_text_file,
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
            // mod tools (rename / organize / cleanup / modify / scan)
            commands::mod_tools::preview_mod_rename,
            commands::mod_tools::apply_mod_rename,
            commands::mod_tools::preview_mod_organize,
            commands::mod_tools::apply_mod_organize,
            commands::mod_tools::preview_mod_duplicates,
            commands::mod_tools::start_mod_duplicate_task,
            commands::mod_tools::apply_mod_duplicate_delete,
            commands::mod_tools::preview_mod_versions,
            commands::mod_tools::start_mod_version_task,
            commands::mod_tools::apply_mod_version_delete,
            commands::mod_tools::apply_mod_modify_version,
            commands::mod_tools::list_mod_op_records,
            commands::mod_tools::get_mod_op_record_detail,
            commands::mod_tools::check_mod_op_rollback,
            commands::mod_tools::rollback_mod_op,
            commands::mod_tools::delete_mod_op_record,
            commands::mod_tools::rename_mod_op_record,
            commands::mod_tools::start_mod_scan_task,
            // pixiv tag tools
            commands::pixiv_tag::scan_pixiv_image_candidates,
            commands::pixiv_tag::start_pixiv_tag_scan_task,
            commands::pixiv_tag::fetch_pixiv_tag_single,
            commands::pixiv_tag::move_image_by_tag_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}
