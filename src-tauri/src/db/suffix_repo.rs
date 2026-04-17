use std::path::Path;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::{SuffixRecordDetail, SuffixRecordItem, SuffixRecordSummary};

fn conn(db_path: &Path) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|e| e.to_string())
}

pub fn create_record(
    db_path: &Path,
    record_name: &str,
    target_suffix: &str,
    source_paths_json: &str,
    created_at: i64,
) -> Result<String, String> {
    let conn = conn(db_path)?;
    let record_id = Uuid::new_v4().to_string();
    conn
    .execute(
      r#"INSERT INTO suffix_change_records(record_id, record_name, target_suffix, source_paths, created_at, rollback_status)
         VALUES(?, ?, ?, ?, ?, 'applied')"#,
      params![record_id, record_name, target_suffix, source_paths_json, created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(record_id)
}

pub fn insert_item(
    db_path: &Path,
    record_id: &str,
    old_path: &str,
    new_path: &str,
    apply_success: bool,
    apply_error: Option<String>,
    updated_at: i64,
) -> Result<i64, String> {
    let conn = conn(db_path)?;
    conn
    .execute(
      r#"INSERT INTO suffix_change_items(record_id, old_path, new_path, apply_success, apply_error, updated_at)
         VALUES(?, ?, ?, ?, ?, ?)"#,
      params![record_id, old_path, new_path, if apply_success { 1 } else { 0 }, apply_error, updated_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// 批量插入多个 item，使用事务提高性能，返回每条记录的 id
pub fn batch_insert_items(
    db_path: &Path,
    record_id: &str,
    items: &[(&str, &str, bool, Option<&str>)], // (old_path, new_path, success, error)
    updated_at: i64,
) -> Result<Vec<i64>, String> {
    let mut conn = conn(db_path)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let mut ids = Vec::with_capacity(items.len());
    {
        let mut stmt = tx
            .prepare(
                r#"INSERT INTO suffix_change_items(record_id, old_path, new_path, apply_success, apply_error, updated_at)
                   VALUES(?, ?, ?, ?, ?, ?)"#,
            )
            .map_err(|e| e.to_string())?;

        for &(old_path, new_path, success, error) in items {
            stmt.execute(params![
                record_id,
                old_path,
                new_path,
                if success { 1 } else { 0 },
                error,
                updated_at
            ])
            .map_err(|e| e.to_string())?;
            ids.push(tx.last_insert_rowid());
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(ids)
}

pub fn list_records(db_path: &Path) -> Result<Vec<SuffixRecordSummary>, String> {
    let conn = conn(db_path)?;
    let mut stmt = conn
    .prepare(
      r#"
      SELECT r.record_id, r.record_name, r.target_suffix, r.created_at, r.rollback_status,
             (SELECT COUNT(1) FROM suffix_change_items i WHERE i.record_id = r.record_id) AS total_items,
             (SELECT COUNT(1) FROM suffix_change_items i WHERE i.record_id = r.record_id AND i.apply_success = 1) AS success_items
      FROM suffix_change_records r
      ORDER BY r.created_at DESC
      "#,
    )
    .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |r| {
            Ok(SuffixRecordSummary {
                record_id: r.get(0)?,
                record_name: r.get(1)?,
                target_suffix: r.get(2)?,
                created_at: r.get(3)?,
                rollback_status: r.get(4)?,
                total_items: r.get::<_, i64>(5)? as usize,
                success_items: r.get::<_, i64>(6)? as usize,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = vec![];
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

pub fn get_record_detail(db_path: &Path, record_id: &str) -> Result<SuffixRecordDetail, String> {
    let conn = conn(db_path)?;

    let summary = conn
    .query_row(
      r#"
      SELECT r.record_id, r.record_name, r.target_suffix, r.created_at, r.rollback_status,
             (SELECT COUNT(1) FROM suffix_change_items i WHERE i.record_id = r.record_id) AS total_items,
             (SELECT COUNT(1) FROM suffix_change_items i WHERE i.record_id = r.record_id AND i.apply_success = 1) AS success_items
      FROM suffix_change_records r
      WHERE r.record_id = ?
      "#,
      params![record_id],
      |r| {
        Ok(SuffixRecordSummary {
          record_id: r.get(0)?,
          record_name: r.get(1)?,
          target_suffix: r.get(2)?,
          created_at: r.get(3)?,
          rollback_status: r.get(4)?,
          total_items: r.get::<_, i64>(5)? as usize,
          success_items: r.get::<_, i64>(6)? as usize,
        })
      },
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
    .prepare(
      r#"SELECT id, old_path, new_path, apply_success, apply_error, rollback_success, rollback_error
         FROM suffix_change_items WHERE record_id = ? ORDER BY id ASC"#,
    )
    .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![record_id], |r| {
            let rb: Option<i64> = r.get(5)?;
            Ok(SuffixRecordItem {
                item_id: r.get(0)?,
                old_path: r.get(1)?,
                new_path: r.get(2)?,
                apply_success: r.get::<_, i64>(3)? == 1,
                apply_error: r.get(4)?,
                rollback_success: rb.map(|x| x == 1),
                rollback_error: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut items = vec![];
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }

    Ok(SuffixRecordDetail { summary, items })
}

pub fn update_rollback_result(
    db_path: &Path,
    item_id: i64,
    success: bool,
    error: Option<String>,
    updated_at: i64,
) -> Result<(), String> {
    let conn = conn(db_path)?;
    conn.execute(
        r#"UPDATE suffix_change_items
         SET rollback_success = ?, rollback_error = ?, updated_at = ?
         WHERE id = ?"#,
        params![if success { 1 } else { 0 }, error, updated_at, item_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 批量更新回滚结果，使用事务提高性能
pub fn batch_update_rollback_results(
    db_path: &Path,
    updates: &[(i64, bool, Option<&str>)], // (item_id, success, error)
    updated_at: i64,
) -> Result<(), String> {
    let mut conn = conn(db_path)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    {
        let mut stmt = tx
            .prepare(
                r#"UPDATE suffix_change_items
                   SET rollback_success = ?, rollback_error = ?, updated_at = ?
                   WHERE id = ?"#,
            )
            .map_err(|e| e.to_string())?;

        for &(item_id, success, error) in updates {
            stmt.execute(params![
                if success { 1 } else { 0 },
                error,
                updated_at,
                item_id
            ])
            .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_record_rollback_status(
    db_path: &Path,
    record_id: &str,
    status: &str,
) -> Result<(), String> {
    let conn = conn(db_path)?;
    conn.execute(
        "UPDATE suffix_change_records SET rollback_status = ? WHERE record_id = ?",
        params![status, record_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_record(db_path: &Path, record_id: &str) -> Result<(), String> {
    let conn = conn(db_path)?;
    conn.execute(
        "DELETE FROM suffix_change_records WHERE record_id = ?",
        params![record_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
