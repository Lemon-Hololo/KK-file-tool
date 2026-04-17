use std::path::Path;

use rusqlite::{params, Connection};

use crate::{
    error::{AppError, AppResult},
    models::AppSettings,
};

fn conn(db_path: &Path) -> AppResult<Connection> {
    Connection::open(db_path).map_err(|e| AppError::Db(e.to_string()))
}

pub fn get_settings(db_path: &Path) -> AppResult<AppSettings> {
    let conn = conn(db_path)?;
    let settings = conn.query_row(
    r#"SELECT keep_policy, move_target_path, save_record_enabled, use_last_record_enabled, include_current_folder_duplicates, theme_mode, thread_count
       FROM app_settings WHERE id = 1"#,
    [],
    |r| {
      Ok(AppSettings {
        keep_policy: r.get(0)?,
        move_target_path: r.get(1)?,
        save_record_enabled: r.get::<_, i32>(2)? != 0,
        use_last_record_enabled: r.get::<_, i32>(3)? != 0,
        include_current_folder_duplicates: r.get::<_, i32>(4)? != 0,
        theme_mode: r.get(5)?,
        thread_count: r.get(6)?,
      })
    },
  )?;
    Ok(settings)
}

pub fn save_settings(db_path: &Path, settings: &AppSettings) -> AppResult<()> {
    let conn = conn(db_path)?;
    conn.execute(
    r#"UPDATE app_settings
       SET keep_policy = ?, move_target_path = ?, save_record_enabled = ?, use_last_record_enabled = ?, include_current_folder_duplicates = ?, theme_mode = ?, thread_count = ?
       WHERE id = 1"#,
    params![
      settings.keep_policy,
      settings.move_target_path,
      settings.save_record_enabled as i32,
      settings.use_last_record_enabled as i32,
      settings.include_current_folder_duplicates as i32,
      settings.theme_mode,
      settings.thread_count
    ],
  )?;
    Ok(())
}
