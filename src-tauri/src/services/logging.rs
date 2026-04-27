//! `task_log` 事件的通用上下文封装。
//!
//! 长任务和同步命令都可以通过本模块把日志发到前端实时日志面板：
//! - 长任务直接用既有 `task_id` 构造；
//! - 同步命令由前端传入本地 `taskId`，命令层转成上下文后继续复用同一套接口。

use std::path::Path;

use tauri::AppHandle;

use crate::{constants::log_level, services::events, utils::path::to_user_friendly_path};

/// 面向单个 `task_id` 的日志发送上下文。
#[derive(Clone)]
pub struct TaskLogContext {
    app: AppHandle,
    task_id: String,
}

impl TaskLogContext {
    /// 为已知 `task_id` 构造上下文。
    pub fn new(app: &AppHandle, task_id: &str) -> Self {
        Self {
            app: app.clone(),
            task_id: task_id.to_string(),
        }
    }

    /// 从可选 `task_id` 构造上下文；为空时返回 `None`，用于同步命令按需发日志。
    pub fn from_task(app: &AppHandle, task_id: Option<&str>) -> Option<Self> {
        let task_id = task_id?.trim();
        if task_id.is_empty() {
            return None;
        }
        Some(Self::new(app, task_id))
    }

    /// 发送任意级别日志。
    pub fn emit(&self, level: &str, message: &str, file_path: Option<String>) {
        events::emit_log(&self.app, &self.task_id, level, message, file_path);
    }

    /// 发送普通信息日志。
    pub fn info(&self, message: &str) {
        self.emit(log_level::INFO, message, None);
    }

    /// 发送普通警告日志。
    pub fn warn(&self, message: &str) {
        self.emit(log_level::WARN, message, None);
    }

    /// 发送普通错误日志。
    pub fn error(&self, message: &str) {
        self.emit(log_level::ERROR, message, None);
    }

    /// 发送带文件路径的信息日志。
    pub fn info_path(&self, message: &str, path: &Path) {
        self.emit(log_level::INFO, message, Some(to_user_friendly_path(path)));
    }

    /// 发送带文件路径的警告日志。
    pub fn warn_path(&self, message: &str, path: &Path) {
        self.emit(log_level::WARN, message, Some(to_user_friendly_path(path)));
    }

    /// 发送带文件路径的错误日志。
    pub fn error_path(&self, message: &str, path: &Path) {
        self.emit(log_level::ERROR, message, Some(to_user_friendly_path(path)));
    }

    /// 发送带已格式化文件路径的信息日志。
    pub fn info_file(&self, message: &str, file_path: String) {
        self.emit(log_level::INFO, message, Some(file_path));
    }

    /// 发送带已格式化文件路径的警告日志。
    pub fn warn_file(&self, message: &str, file_path: String) {
        self.emit(log_level::WARN, message, Some(file_path));
    }

    /// 发送带已格式化文件路径的错误日志。
    pub fn error_file(&self, message: &str, file_path: String) {
        self.emit(log_level::ERROR, message, Some(file_path));
    }
}
