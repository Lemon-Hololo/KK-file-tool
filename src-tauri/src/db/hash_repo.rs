//! 哈希记录（`hash_records` / `hash_entries`）的 CRUD。
//!
//! 与 `op_record_repo` 的"可回滚操作记录"不同，哈希记录是只读索引，
//! 没有 apply/rollback 语义；故单独维护一个 repo。

use std::{collections::HashSet, path::Path};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::{AppError, AppResult},
    models::{HashIndexEntry, HashIndexRecord, HashIndexRecordSummary},
};

fn conn(db_path: &Path) -> AppResult<Connection> {
    Connection::open(db_path).map_err(|e| AppError::Db(e.to_string()))
}

/// 事务写入一条记录 + 其下所有 entries。返回新生成的 `record_id`。
pub fn insert_hash_record(
    db_path: &Path,
    record_name: &str,
    source_paths: &[String],
    entries: &[HashIndexEntry],
    created_at: i64,
) -> AppResult<String> {
    let mut conn = conn(db_path)?;
    let tx = conn.transaction()?;

    let record_id = uuid::Uuid::new_v4().to_string();
    let source_json = serde_json::to_string(source_paths)?;

    tx.execute(
    "INSERT INTO hash_records(record_id, record_name, created_at, source_paths) VALUES (?, ?, ?, ?)",
    params![record_id, record_name, created_at, source_json],
  )?;

    {
        let mut stmt = tx.prepare(
    "INSERT INTO hash_entries(record_id, hash, file_path, file_size, mtime, ctime, status) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )?;

        for e in entries {
            stmt.execute(params![
                record_id,
                e.hash,
                e.file_path,
                e.file_size as i64,
                e.mtime,
                e.ctime,
                e.status
            ])?;
        }
    }

    tx.commit()?;
    Ok(record_id)
}

/// 列出所有哈希记录的摘要（按创建时间降序，不含 entries）。
pub fn list_hash_records(db_path: &Path) -> AppResult<Vec<HashIndexRecordSummary>> {
    let conn = conn(db_path)?;
    let mut stmt = conn.prepare(
        r#"
    SELECT r.record_id, r.record_name, r.created_at, r.source_paths,
           (SELECT COUNT(1) FROM hash_entries e WHERE e.record_id = r.record_id) AS entry_count
    FROM hash_records r
    ORDER BY r.created_at DESC
    "#,
    )?;

    let rows = stmt.query_map([], |r| {
        let source_json: String = r.get(3)?;
        let source_paths: Vec<String> = serde_json::from_str(&source_json).unwrap_or_default();

        Ok(HashIndexRecordSummary {
            record_id: r.get(0)?,
            record_name: r.get(1)?,
            created_at: r.get(2)?,
            source_paths,
            entry_count: r.get::<_, i64>(4)? as usize,
        })
    })?;

    let mut out = vec![];
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// 读取单条记录的完整详情（含所有 entries）。
pub fn load_hash_record(db_path: &Path, record_id: &str) -> AppResult<HashIndexRecord> {
    let conn = conn(db_path)?;

    let (rid, name, created_at, source_json): (String, String, i64, String) = conn.query_row(
    "SELECT record_id, record_name, created_at, source_paths FROM hash_records WHERE record_id = ?",
    params![record_id],
    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
  )?;

    let source_paths: Vec<String> = serde_json::from_str(&source_json).unwrap_or_default();

    let mut stmt = conn.prepare(
    "SELECT hash, file_path, file_size, mtime, ctime, status FROM hash_entries WHERE record_id = ? ORDER BY id ASC",
  )?;

    let rows = stmt.query_map(params![record_id], |r| {
        Ok(HashIndexEntry {
            hash: r.get(0)?,
            file_path: r.get(1)?,
            file_size: r.get::<_, i64>(2)? as u64,
            mtime: r.get(3)?,
            ctime: r
                .get::<_, Option<i64>>(4)?
                .unwrap_or_else(|| r.get::<_, i64>(3).unwrap_or(0)),
            status: r.get(5)?,
        })
    })?;

    let mut entries = vec![];
    for row in rows {
        entries.push(row?);
    }

    Ok(HashIndexRecord {
        record_id: rid,
        record_name: name,
        created_at,
        source_paths,
        entries,
    })
}

/// 更新记录名；调用方确保 `new_name` 非空。
pub fn rename_hash_record(db_path: &Path, record_id: &str, new_name: &str) -> AppResult<()> {
    let conn = conn(db_path)?;
    conn.execute(
        "UPDATE hash_records SET record_name = ? WHERE record_id = ?",
        params![new_name, record_id],
    )?;
    Ok(())
}

/// 事务：删除主表 + 关联 entries。
pub fn delete_hash_record(db_path: &Path, record_id: &str) -> AppResult<()> {
    let mut conn = conn(db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM hash_entries WHERE record_id = ?",
        params![record_id],
    )?;
    tx.execute(
        "DELETE FROM hash_records WHERE record_id = ?",
        params![record_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// 从指定记录（或最新一条）加载"active"状态的哈希集合，用于跨会话去重。
pub fn load_active_hash_set(
    db_path: &Path,
    selected_record_id: Option<&str>,
    use_last_if_none: bool,
) -> AppResult<HashSet<String>> {
    let conn = conn(db_path)?;

    let rid = if let Some(r) = selected_record_id {
        Some(r.to_string())
    } else if use_last_if_none {
        conn.query_row(
            "SELECT record_id FROM hash_records ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    } else {
        None
    };

    let Some(record_id) = rid else {
        return Ok(HashSet::new());
    };

    let mut stmt =
        conn.prepare("SELECT hash FROM hash_entries WHERE record_id = ? AND status = 'active'")?;
    let rows = stmt.query_map(params![record_id], |r| r.get::<_, String>(0))?;

    let mut set = HashSet::new();
    for row in rows {
        set.insert(row?);
    }
    Ok(set)
}
