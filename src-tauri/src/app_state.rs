//! 应用级共享状态：数据库路径、运行中任务、任务结果。
//!
//! `AppState::bootstrap` 封装启动期的目录/路径解析与 schema 初始化；
//! `with_task` 抽象"按 id 取任务运行时"这一高频模式。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use crate::{
    constants::db_file,
    db::schema,
    error::{AppError, AppResult},
    external_config,
    models::{DuplicateGroup, TaskStatus},
};

pub struct TaskRuntime {
    pub paused: AtomicBool,
    pub cancelled: AtomicBool,
    pub status: Mutex<TaskStatus>,
}

impl TaskRuntime {
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            status: Mutex::new(TaskStatus::Idle),
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn set_status(&self, status: TaskStatus) {
        *self.status.lock().unwrap() = status;
    }
}

impl Default for TaskRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub db_path: PathBuf,
    pub tasks: Mutex<HashMap<String, Arc<TaskRuntime>>>,
    pub task_results: Mutex<HashMap<String, Vec<DuplicateGroup>>>,
}

impl AppState {
    /// 启动期初始化：创建数据目录、解析 db 路径（带父目录回退）、初始化 schema。
    ///
    /// 返回 `Arc<Self>` 方便直接交给 `app.manage(...)`。
    pub fn bootstrap(app_data_dir: PathBuf) -> AppResult<Arc<Self>> {
        if !app_data_dir.exists() {
            std::fs::create_dir_all(&app_data_dir)?;
        }

        let db_path = resolve_db_path_with_fallback(&app_data_dir);
        schema::init_schema(&db_path).map_err(AppError::Db)?;

        Ok(Arc::new(Self {
            app_data_dir,
            db_path,
            tasks: Mutex::new(HashMap::new()),
            task_results: Mutex::new(HashMap::new()),
        }))
    }

    /// 按 id 取任务运行时并执行闭包；任务不存在时返回 `AppError::TaskNotFound`。
    pub fn with_task<F, R>(&self, task_id: &str, f: F) -> AppResult<R>
    where
        F: FnOnce(&Arc<TaskRuntime>) -> R,
    {
        let tasks = self.tasks.lock().unwrap();
        let runtime = tasks.get(task_id).ok_or(AppError::TaskNotFound)?;
        Ok(f(runtime))
    }

    pub fn insert_task(&self, task_id: String, runtime: Arc<TaskRuntime>) {
        self.tasks.lock().unwrap().insert(task_id, runtime);
    }

    pub fn default_db_path(&self) -> PathBuf {
        self.app_data_dir.join(db_file::DEFAULT_NAME)
    }
}

/// 解析数据库路径：若自定义路径父目录不存在则回退到默认路径。
fn resolve_db_path_with_fallback(app_data_dir: &std::path::Path) -> PathBuf {
    let ext_config = external_config::load_config(app_data_dir);
    let resolved = external_config::resolve_db_path(app_data_dir, &ext_config);
    match resolved.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && !parent.exists() => {
            // bootstrap 阶段尚未初始化日志系统，只能直接打印到 stderr。
            eprintln!(
                "Custom db_path parent dir does not exist: {:?}, falling back to default",
                parent
            );
            app_data_dir.join(db_file::DEFAULT_NAME)
        }
        _ => resolved,
    }
}
