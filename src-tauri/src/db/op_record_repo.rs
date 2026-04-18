//! 通用"操作记录"表访问层。
//!
//! FileFlow 中目前有两种记录型操作：后缀修改 (`suffix_change_records`) 与
//! Mod 工具 (`mod_op_records`)。两者共享相同的流水线（preview → apply → rollback）
//! 与 item 表 schema，只在 `records` 主表的"附加字段"上不同（前者是 `target_suffix`，
//! 后者是 `kind`）。
//!
//! 本模块用参数化表描述符 [`OpRecordTables`] 统一两者的 CRUD，
//! 避免在 `suffix_repo.rs` / `mod_tools_repo.rs` 维护两份几乎相同的 SQL。
//!
//! # 调用约定
//! - 每次调用独立开 `Connection::open`，与项目其余 repo 一致。
//! - 所有 item 表结构必须为：
//!   `id / record_id / old_path / new_path / apply_success / apply_error /`
//!   `rollback_success / rollback_error / updated_at`
//!   任何额外字段由业务侧另建辅助表承载，不在此处。

use std::path::Path;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// 描述一对"记录主表 + item 子表"的表名和附加列。
///
/// `extra_summary_column` 为 `None` 时，`records` 主表仅有基础字段；
/// 为 `Some("kind")` 或 `Some("target_suffix")` 时，会在 `INSERT / SELECT`
/// 中自动拼接该列。
#[derive(Debug, Clone, Copy)]
pub struct OpRecordTables {
    /// 记录主表名，如 `"suffix_change_records"`。
    pub record_table: &'static str,
    /// item 子表名，如 `"suffix_change_items"`。
    pub item_table: &'static str,
    /// 主表中除基础字段外的附加列（如 `"kind"` / `"target_suffix"`）。
    ///
    /// 基础字段为：`record_id, record_name, source_paths, created_at, rollback_status`。
    pub extra_summary_column: Option<&'static str>,
}

/// 中性"记录摘要"视图，上层可再映射为 `SuffixRecordSummary` / `ModOpRecordSummary`。
#[derive(Debug, Clone)]
pub struct OpRecordSummary {
    pub record_id: String,
    pub record_name: String,
    /// `extra_summary_column` 对应列的值（未配置附加列时为 `None`）。
    pub extra: Option<String>,
    pub created_at: i64,
    pub rollback_status: String,
    pub total_items: usize,
    pub success_items: usize,
}

/// 中性"记录条目"视图。
#[derive(Debug, Clone)]
pub struct OpRecordItem {
    pub item_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub apply_success: bool,
    pub apply_error: Option<String>,
    pub rollback_success: Option<bool>,
    pub rollback_error: Option<String>,
}

/// 记录详情 = 摘要 + 全部 item。
#[derive(Debug, Clone)]
pub struct OpRecordDetail {
    pub summary: OpRecordSummary,
    pub items: Vec<OpRecordItem>,
}

fn conn(db_path: &Path) -> AppResult<Connection> {
    Connection::open(db_path).map_err(|e| AppError::Db(e.to_string()))
}

/// 插入记录主表；自动生成 UUID 作为 `record_id` 返回。
///
/// `extra_value` 仅在 `tables.extra_summary_column` 为 `Some` 时写入；
/// 否则必须传入空字符串或调用方不传（签名上仍接收 `&str` 以简化 API）。
pub fn create_record(
    db_path: &Path,
    tables: OpRecordTables,
    record_name: &str,
    extra_value: &str,
    source_paths_json: &str,
    created_at: i64,
) -> AppResult<String> {
    let conn = conn(db_path)?;
    let record_id = Uuid::new_v4().to_string();

    let sql = if let Some(col) = tables.extra_summary_column {
        format!(
            "INSERT INTO {t}(record_id, record_name, {col}, source_paths, created_at, rollback_status) \
             VALUES(?, ?, ?, ?, ?, 'applied')",
            t = tables.record_table,
            col = col
        )
    } else {
        format!(
            "INSERT INTO {t}(record_id, record_name, source_paths, created_at, rollback_status) \
             VALUES(?, ?, ?, ?, 'applied')",
            t = tables.record_table
        )
    };

    if tables.extra_summary_column.is_some() {
        conn.execute(
            &sql,
            params![record_id, record_name, extra_value, source_paths_json, created_at],
        )
        .map_err(|e| AppError::Db(e.to_string()))?;
    } else {
        conn.execute(
            &sql,
            params![record_id, record_name, source_paths_json, created_at],
        )
        .map_err(|e| AppError::Db(e.to_string()))?;
    }

    Ok(record_id)
}

/// 批量插入 item（事务）。返回每条插入后的 `id`（顺序与输入一致）。
///
/// `items` 元组语义：`(old_path, new_path, apply_success, apply_error_msg)`。
pub fn batch_insert_items(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    items: &[(&str, &str, bool, Option<&str>)],
    updated_at: i64,
) -> AppResult<Vec<i64>> {
    let mut conn = conn(db_path)?;
    let tx = conn
        .transaction()
        .map_err(|e| AppError::Db(e.to_string()))?;

    let sql = format!(
        "INSERT INTO {t}(record_id, old_path, new_path, apply_success, apply_error, updated_at) \
         VALUES(?, ?, ?, ?, ?, ?)",
        t = tables.item_table
    );

    let mut ids = Vec::with_capacity(items.len());
    {
        let mut stmt = tx.prepare(&sql).map_err(|e| AppError::Db(e.to_string()))?;

        for &(old_path, new_path, success, error) in items {
            stmt.execute(params![
                record_id,
                old_path,
                new_path,
                if success { 1 } else { 0 },
                error,
                updated_at
            ])
            .map_err(|e| AppError::Db(e.to_string()))?;
            ids.push(tx.last_insert_rowid());
        }
    }

    tx.commit().map_err(|e| AppError::Db(e.to_string()))?;
    Ok(ids)
}

/// 列出所有记录摘要，按 `created_at` 降序。
///
/// `filter_extra_eq` 为 `Some(v)` 时，仅返回附加列等于 `v` 的记录
/// （要求 `tables.extra_summary_column` 也为 `Some`）。
pub fn list_records(
    db_path: &Path,
    tables: OpRecordTables,
    filter_extra_eq: Option<&str>,
) -> AppResult<Vec<OpRecordSummary>> {
    let conn = conn(db_path)?;

    let extra_col = tables.extra_summary_column.unwrap_or("NULL");
    let where_clause = match (tables.extra_summary_column, filter_extra_eq) {
        (Some(col), Some(_)) => format!(" WHERE r.{col} = ?"),
        _ => String::new(),
    };

    let sql = format!(
        r#"
        SELECT r.record_id, r.record_name, r.{extra}, r.created_at, r.rollback_status,
               (SELECT COUNT(1) FROM {it} i WHERE i.record_id = r.record_id) AS total_items,
               (SELECT COUNT(1) FROM {it} i WHERE i.record_id = r.record_id AND i.apply_success = 1) AS success_items
        FROM {rt} r{where_clause}
        ORDER BY r.created_at DESC
        "#,
        extra = extra_col,
        it = tables.item_table,
        rt = tables.record_table,
        where_clause = where_clause
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Db(e.to_string()))?;

    let mapper = |r: &rusqlite::Row| {
        Ok(OpRecordSummary {
            record_id: r.get(0)?,
            record_name: r.get(1)?,
            extra: r.get::<_, Option<String>>(2)?,
            created_at: r.get(3)?,
            rollback_status: r.get(4)?,
            total_items: r.get::<_, i64>(5)? as usize,
            success_items: r.get::<_, i64>(6)? as usize,
        })
    };

    let rows_iter = match (tables.extra_summary_column, filter_extra_eq) {
        (Some(_), Some(v)) => stmt.query_map(params![v], mapper),
        _ => stmt.query_map([], mapper),
    }
    .map_err(|e| AppError::Db(e.to_string()))?;

    let mut result = vec![];
    for row in rows_iter {
        result.push(row.map_err(|e| AppError::Db(e.to_string()))?);
    }
    Ok(result)
}

/// 读取记录详情（摘要 + 所有 item，按 `id` 升序）。
pub fn get_record_detail(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
) -> AppResult<OpRecordDetail> {
    let conn = conn(db_path)?;

    let extra_col = tables.extra_summary_column.unwrap_or("NULL");
    let summary_sql = format!(
        r#"
        SELECT r.record_id, r.record_name, r.{extra}, r.created_at, r.rollback_status,
               (SELECT COUNT(1) FROM {it} i WHERE i.record_id = r.record_id) AS total_items,
               (SELECT COUNT(1) FROM {it} i WHERE i.record_id = r.record_id AND i.apply_success = 1) AS success_items
        FROM {rt} r
        WHERE r.record_id = ?
        "#,
        extra = extra_col,
        it = tables.item_table,
        rt = tables.record_table
    );

    let summary = conn
        .query_row(&summary_sql, params![record_id], |r| {
            Ok(OpRecordSummary {
                record_id: r.get(0)?,
                record_name: r.get(1)?,
                extra: r.get::<_, Option<String>>(2)?,
                created_at: r.get(3)?,
                rollback_status: r.get(4)?,
                total_items: r.get::<_, i64>(5)? as usize,
                success_items: r.get::<_, i64>(6)? as usize,
            })
        })
        .map_err(|e| AppError::Db(e.to_string()))?;

    let items_sql = format!(
        r#"SELECT id, old_path, new_path, apply_success, apply_error, rollback_success, rollback_error
           FROM {it} WHERE record_id = ? ORDER BY id ASC"#,
        it = tables.item_table
    );

    let mut stmt = conn
        .prepare(&items_sql)
        .map_err(|e| AppError::Db(e.to_string()))?;

    let rows = stmt
        .query_map(params![record_id], |r| {
            let rb: Option<i64> = r.get(5)?;
            Ok(OpRecordItem {
                item_id: r.get(0)?,
                old_path: r.get(1)?,
                new_path: r.get(2)?,
                apply_success: r.get::<_, i64>(3)? == 1,
                apply_error: r.get(4)?,
                rollback_success: rb.map(|x| x == 1),
                rollback_error: r.get(6)?,
            })
        })
        .map_err(|e| AppError::Db(e.to_string()))?;

    let mut items = vec![];
    for row in rows {
        items.push(row.map_err(|e| AppError::Db(e.to_string()))?);
    }

    Ok(OpRecordDetail { summary, items })
}

/// 批量更新回滚结果（事务）。`updates` 元组语义：`(item_id, success, error_msg)`。
pub fn batch_update_rollback_results(
    db_path: &Path,
    tables: OpRecordTables,
    updates: &[(i64, bool, Option<&str>)],
    updated_at: i64,
) -> AppResult<()> {
    let mut conn = conn(db_path)?;
    let tx = conn
        .transaction()
        .map_err(|e| AppError::Db(e.to_string()))?;

    let sql = format!(
        "UPDATE {it} SET rollback_success = ?, rollback_error = ?, updated_at = ? WHERE id = ?",
        it = tables.item_table
    );

    {
        let mut stmt = tx.prepare(&sql).map_err(|e| AppError::Db(e.to_string()))?;
        for &(item_id, success, error) in updates {
            stmt.execute(params![
                if success { 1 } else { 0 },
                error,
                updated_at,
                item_id
            ])
            .map_err(|e| AppError::Db(e.to_string()))?;
        }
    }

    tx.commit().map_err(|e| AppError::Db(e.to_string()))?;
    Ok(())
}

/// 设置记录整体的 `rollback_status`（`applied` / `partially_rolled_back` / `rolled_back`）。
pub fn set_record_rollback_status(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    status: &str,
) -> AppResult<()> {
    let conn = conn(db_path)?;
    let sql = format!(
        "UPDATE {rt} SET rollback_status = ? WHERE record_id = ?",
        rt = tables.record_table
    );
    conn.execute(&sql, params![status, record_id])
        .map_err(|e| AppError::Db(e.to_string()))?;
    Ok(())
}

/// 按 `record_id` 删除记录；相关 item 通过 FK `ON DELETE CASCADE` 自动清除。
pub fn delete_record(db_path: &Path, tables: OpRecordTables, record_id: &str) -> AppResult<()> {
    let conn = conn(db_path)?;
    let sql = format!(
        "DELETE FROM {rt} WHERE record_id = ?",
        rt = tables.record_table
    );
    conn.execute(&sql, params![record_id])
        .map_err(|e| AppError::Db(e.to_string()))?;
    Ok(())
}

/// 更新记录名。`new_name` 前后空白会被 trim；为空时返回 `InvalidInput`。
pub fn rename_record(
    db_path: &Path,
    tables: OpRecordTables,
    record_id: &str,
    new_name: &str,
) -> AppResult<()> {
    let name = new_name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("记录名不能为空".to_string()));
    }
    let conn = conn(db_path)?;
    let sql = format!(
        "UPDATE {rt} SET record_name = ? WHERE record_id = ?",
        rt = tables.record_table
    );
    let affected = conn
        .execute(&sql, params![name, record_id])
        .map_err(|e| AppError::Db(e.to_string()))?;
    if affected == 0 {
        return Err(AppError::NotFound(format!("记录不存在: {record_id}")));
    }
    Ok(())
}
