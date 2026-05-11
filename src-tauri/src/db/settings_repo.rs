//! 单行 `app_settings` 表的读写。

use std::collections::HashMap;
use std::path::Path;

use rusqlite::params;

use crate::{db::open_connection, error::AppResult, models::AppSettings};

/// 读取用户设置；表有 `CHECK(id = 1)` 约束保证仅一行。
pub fn get_settings(db_path: &Path) -> AppResult<AppSettings> {
    let conn = open_connection(db_path)?;
    let settings = conn.query_row(
        r#"SELECT keep_policy, move_target_path, save_record_enabled, use_last_record_enabled,
                  include_current_folder_duplicates, theme_mode, thread_count,
                  log_max_length, io_concurrency_multiplier, extreme_row_threshold,
                  text_preview_max_kb, zip_preview_max_entries,
                  mod_scan_default_keyword, suffix_default_target,
                  mod_rollback_enabled, mod_backup_dir,
                  pixiv_tag_api_base, pixiv_excluded_tags, pixiv_cookie, pixiv_proxy,
                  pixiv_use_translation, pixiv_rate_limit_per_minute,
                  pixiv_partial_flush_interval_ms, pixiv_local_tag_translations,
                  preserve_dir_on_move,
                  image_dedup_algorithm, image_dedup_hash_size, image_dedup_similarity_threshold,
                  image_dedup_extensions, image_dedup_min_file_size_kb, image_dedup_min_dimension,
                  image_dedup_keep_policy, image_dedup_rollback_enabled, image_dedup_backup_dir
           FROM app_settings WHERE id = 1"#,
        [],
        |r| {
            // pixiv_excluded_tags 落库为 JSON 数组字符串；解析失败时退回空 Vec，
            // 不阻塞设置读取——历史脏数据/手改库导致的损坏应当宽容降级。
            let excluded_raw: String = r.get(17)?;
            let excluded = serde_json::from_str::<Vec<String>>(&excluded_raw).unwrap_or_default();
            let local_translations_raw: String = r.get(23)?;
            let local_translations =
                serde_json::from_str::<HashMap<String, String>>(&local_translations_raw)
                    .unwrap_or_default();
            // image_dedup_extensions 同样落库为 JSON 数组字符串；损坏时退回空 Vec
            // （上层服务再降级到 DEFAULT_IMAGE_DEDUP_EXTENSIONS）。
            let image_extensions_raw: String = r.get(28)?;
            let image_extensions =
                serde_json::from_str::<Vec<String>>(&image_extensions_raw).unwrap_or_default();

            Ok(AppSettings {
                keep_policy: r.get(0)?,
                move_target_path: r.get(1)?,
                save_record_enabled: r.get::<_, i32>(2)? != 0,
                use_last_record_enabled: r.get::<_, i32>(3)? != 0,
                include_current_folder_duplicates: r.get::<_, i32>(4)? != 0,
                theme_mode: r.get(5)?,
                thread_count: r.get(6)?,
                log_max_length: r.get(7)?,
                io_concurrency_multiplier: r.get(8)?,
                extreme_row_threshold: r.get(9)?,
                text_preview_max_kb: r.get(10)?,
                zip_preview_max_entries: r.get(11)?,
                mod_scan_default_keyword: r.get(12)?,
                suffix_default_target: r.get(13)?,
                mod_rollback_enabled: r.get::<_, i32>(14)? != 0,
                mod_backup_dir: r.get(15)?,
                pixiv_tag_api_base: r.get(16)?,
                pixiv_excluded_tags: excluded,
                pixiv_local_tag_translations: local_translations,
                pixiv_cookie: r.get(18)?,
                pixiv_proxy: r.get(19)?,
                pixiv_use_translation: r.get::<_, i32>(20)? != 0,
                pixiv_rate_limit_per_minute: r.get(21)?,
                pixiv_partial_flush_interval_ms: r.get(22)?,
                preserve_dir_on_move: r.get::<_, i32>(24)? != 0,
                image_dedup_algorithm: r.get(25)?,
                image_dedup_hash_size: r.get(26)?,
                image_dedup_similarity_threshold: r.get(27)?,
                image_dedup_extensions: image_extensions,
                image_dedup_min_file_size_kb: r.get(29)?,
                image_dedup_min_dimension: r.get(30)?,
                image_dedup_keep_policy: r.get(31)?,
                image_dedup_rollback_enabled: r.get::<_, i32>(32)? != 0,
                image_dedup_backup_dir: r.get(33)?,
            })
        },
    )?;
    Ok(settings)
}

/// 全量覆盖写入设置。
pub fn save_settings(db_path: &Path, settings: &AppSettings) -> AppResult<()> {
    let conn = open_connection(db_path)?;
    // pixiv_excluded_tags 序列化失败极少；失败时落 "[]" 兜底，避免单条设置阻塞整体保存。
    let excluded_json =
        serde_json::to_string(&settings.pixiv_excluded_tags).unwrap_or_else(|_| "[]".to_string());
    let local_translations_json = serde_json::to_string(&settings.pixiv_local_tag_translations)
        .unwrap_or_else(|_| "{}".to_string());
    let image_extensions_json = serde_json::to_string(&settings.image_dedup_extensions)
        .unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        r#"UPDATE app_settings
           SET keep_policy = ?, move_target_path = ?, save_record_enabled = ?, use_last_record_enabled = ?,
               include_current_folder_duplicates = ?, theme_mode = ?, thread_count = ?,
               log_max_length = ?, io_concurrency_multiplier = ?, extreme_row_threshold = ?,
               text_preview_max_kb = ?, zip_preview_max_entries = ?,
               mod_scan_default_keyword = ?, suffix_default_target = ?,
               mod_rollback_enabled = ?, mod_backup_dir = ?,
               pixiv_tag_api_base = ?, pixiv_excluded_tags = ?, pixiv_cookie = ?, pixiv_proxy = ?,
               pixiv_use_translation = ?, pixiv_rate_limit_per_minute = ?,
               pixiv_partial_flush_interval_ms = ?, pixiv_local_tag_translations = ?,
               preserve_dir_on_move = ?,
               image_dedup_algorithm = ?, image_dedup_hash_size = ?, image_dedup_similarity_threshold = ?,
               image_dedup_extensions = ?, image_dedup_min_file_size_kb = ?, image_dedup_min_dimension = ?,
               image_dedup_keep_policy = ?, image_dedup_rollback_enabled = ?, image_dedup_backup_dir = ?
           WHERE id = 1"#,
        params![
            settings.keep_policy,
            settings.move_target_path,
            settings.save_record_enabled as i32,
            settings.use_last_record_enabled as i32,
            settings.include_current_folder_duplicates as i32,
            settings.theme_mode,
            settings.thread_count,
            settings.log_max_length,
            settings.io_concurrency_multiplier,
            settings.extreme_row_threshold,
            settings.text_preview_max_kb,
            settings.zip_preview_max_entries,
            settings.mod_scan_default_keyword,
            settings.suffix_default_target,
            settings.mod_rollback_enabled as i32,
            settings.mod_backup_dir,
            settings.pixiv_tag_api_base,
            excluded_json,
            settings.pixiv_cookie,
            settings.pixiv_proxy,
            settings.pixiv_use_translation as i32,
            settings.pixiv_rate_limit_per_minute,
            settings.pixiv_partial_flush_interval_ms,
            local_translations_json,
            settings.preserve_dir_on_move as i32,
            settings.image_dedup_algorithm,
            settings.image_dedup_hash_size,
            settings.image_dedup_similarity_threshold,
            image_extensions_json,
            settings.image_dedup_min_file_size_kb,
            settings.image_dedup_min_dimension,
            settings.image_dedup_keep_policy,
            settings.image_dedup_rollback_enabled as i32,
            settings.image_dedup_backup_dir,
        ],
    )?;
    Ok(())
}
