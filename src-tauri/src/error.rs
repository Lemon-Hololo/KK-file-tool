use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("db error: {0}")]
    Db(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("task not found")]
    TaskNotFound,

    #[error("not found: {0}")]
    NotFound(String),

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

pub type AppResult<T> = Result<T, AppError>;
