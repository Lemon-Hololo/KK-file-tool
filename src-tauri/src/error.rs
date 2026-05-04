//! 统一错误类型。
//!
//! Tauri 命令出于 IPC 限制一般返回 `Result<T, String>`；内部函数通过
//! [`AppError`] 承载更丰富的错误分类，在命令层再 `.to_string()` 扁平化。

use thiserror::Error;

/// 领域错误分类；消息用中文便于直接展示。
#[derive(Debug, Error)]
pub enum AppError {
    /// 数据库操作失败（SQLite、序列化等）。
    #[error("db error: {0}")]
    Db(String),

    /// 文件系统 IO 错误。
    #[error("io error: {0}")]
    Io(String),

    /// 调用方输入不合法（空字段、非法枚举值等）。
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// 指定的 `task_id` 不在当前运行任务表中。
    #[error("task not found")]
    TaskNotFound,

    /// 泛化的"未找到"，比如 `record_id` 不存在。
    #[error("not found: {0}")]
    NotFound(String),

    /// 其他内部错误（线程池构建失败、序列化失败等）。
    #[error("internal: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Db(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

/// 统一的 `Result` 别名。
pub type AppResult<T> = Result<T, AppError>;
