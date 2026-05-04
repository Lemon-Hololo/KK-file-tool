//! 数据库访问层公共入口。

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub mod hash_repo;
pub mod move_repo;
pub mod op_record_repo;
pub mod schema;
pub mod settings_repo;

/// 打开一个 SQLite 连接，并统一映射为 `AppError::Db`。
///
/// 项目当前保持"每次 repo 调用独立打开连接"的简单模型；新增 repo 也应从这里
/// 取连接，避免各模块重复 `Connection::open(...).map_err(...)`。
pub fn open_connection(db_path: &Path) -> AppResult<Connection> {
    Connection::open(db_path).map_err(|e| AppError::Db(e.to_string()))
}
