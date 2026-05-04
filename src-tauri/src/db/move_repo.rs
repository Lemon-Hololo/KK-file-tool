//! 移动报告（`move_reports` / `move_report_items`）持久化。

use std::path::Path;

use rusqlite::params;

use crate::{db::open_connection, error::AppResult, models::MoveReport};

/// 事务：写入移动报告 + 所有 item + 清理被成功移动文件对应的 `hash_entries`。
///
/// 删除 `hash_entries` 是为了保证下次使用"上次记录"做跨会话比对时，
/// 已经不在原位的文件不再被当作"存在"。
pub fn save_move_report_and_cleanup_entries(
    db_path: &Path,
    report: &MoveReport,
    moved_paths: &[String],
) -> AppResult<()> {
    let mut conn = open_connection(db_path)?;
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
