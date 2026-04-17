use std::path::Path;

use rusqlite::{params, Connection};

use crate::{
    error::{AppError, AppResult},
    models::MoveReport,
};

fn conn(db_path: &Path) -> AppResult<Connection> {
    Connection::open(db_path).map_err(|e| AppError::Db(e.to_string()))
}

/// 事务：写 move report + 删除成功移动文件的 hash_entries
pub fn save_move_report_and_cleanup_entries(
    db_path: &Path,
    report: &MoveReport,
    moved_paths: &[String],
) -> AppResult<()> {
    let mut conn = conn(db_path)?;
    let tx = conn.transaction()?;

    tx.execute(
    r#"INSERT INTO move_reports
    (report_id, task_id, created_at, target_dir, total_selected, total_success, total_failed, released_bytes)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    params![
      report.report_id,
      report.task_id,
      report.created_at,
      report.target_dir,
      report.total_selected as i64,
      report.total_success as i64,
      report.total_failed as i64,
      report.released_bytes as i64
    ],
  )?;

    {
        let mut item_stmt = tx.prepare(
    "INSERT INTO move_report_items(report_id, src_path, dst_path, success, error_code, error_message, size) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )?;

        for s in &report.success_items {
            item_stmt.execute(params![
                report.report_id,
                s.src_path,
                s.dst_path,
                1i32,
                Option::<String>::None,
                Option::<String>::None,
                s.size as i64
            ])?;
        }

        for f in &report.failed_items {
            item_stmt.execute(params![
                report.report_id,
                f.src_path,
                Option::<String>::None,
                0i32,
                f.error_code,
                f.error_message,
                0i64
            ])?;
        }
    }

    {
        let mut del_stmt = tx.prepare("DELETE FROM hash_entries WHERE file_path = ?")?;
        for p in moved_paths {
            del_stmt.execute(params![p])?;
        }
    }

    tx.commit()?;
    Ok(())
}
