//! 应用级共享状态：数据库路径、运行中任务、任务结果。
//!
//! `AppState::bootstrap` 封装启动期的目录/路径解析与 schema 初始化；
//! `with_task` 抽象"按 id 取任务运行时"这一高频模式。

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    constants::db_file,
    db::schema,
    error::{AppError, AppResult},
    external_config,
    models::{DuplicateGroup, TaskStatus},
};

/// 单个长任务的运行时控制状态。
pub struct TaskRuntime {
    paused: AtomicBool,
    cancelled: AtomicBool,
    status: Mutex<TaskStatus>,
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

    /// 标记任务进入暂停态；实际暂停点由任务循环自己检查 `is_paused`。
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
        self.set_status(TaskStatus::Paused);
    }

    /// 标记任务恢复运行。
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
        self.set_status(TaskStatus::Running);
    }

    /// 请求取消任务；已提交的工作单元可能仍会跑完。
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.set_status(TaskStatus::Cancelled);
    }

    /// 当前状态快照，用于需要只读检查的命令。
    pub fn status(&self) -> TaskStatus {
        self.status.lock().unwrap().clone()
    }
}

impl Default for TaskRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri 应用共享状态，集中管理数据库路径、长任务运行时和去重结果缓存。
pub struct AppState {
    pub app_data_dir: PathBuf,
    pub db_path: PathBuf,
    tasks: Mutex<HashMap<String, Arc<TaskRuntime>>>,
    task_results: Mutex<HashMap<String, Vec<DuplicateGroup>>>,
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
        schema::init_schema(&db_path)?;

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
        let runtime = self
            .tasks
            .lock()
            .unwrap()
            .get(task_id)
            .cloned()
            .ok_or(AppError::TaskNotFound)?;
        Ok(f(&runtime))
    }

    fn insert_task(&self, task_id: String, runtime: Arc<TaskRuntime>) {
        self.tasks.lock().unwrap().insert(task_id, runtime);
    }

    /// 创建并注册一个新的任务运行时，返回 `(task_id, runtime)`。
    ///
    /// 前端可预传 `task_id` 以避免事件早于监听器到达；空白 ID 会被忽略并自动生成。
    pub fn create_task(&self, preferred_task_id: Option<String>) -> (String, Arc<TaskRuntime>) {
        let task_id = preferred_task_id
            .and_then(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let runtime = Arc::new(TaskRuntime::new());
        self.insert_task(task_id.clone(), runtime.clone());
        (task_id, runtime)
    }

    /// 从运行中任务表移除任务；终态收尾统一走这里，避免外部直接碰锁。
    pub fn remove_task(&self, task_id: &str) {
        self.tasks.lock().unwrap().remove(task_id);
    }

    /// 是否存在运行中或暂停中的长任务。
    pub fn has_active_tasks(&self) -> bool {
        self.tasks
            .lock()
            .unwrap()
            .values()
            .any(|runtime| matches!(runtime.status(), TaskStatus::Running | TaskStatus::Paused))
    }

    /// 覆盖保存某个去重任务的当前结果。
    pub fn set_task_results(&self, task_id: String, groups: Vec<DuplicateGroup>) {
        self.task_results.lock().unwrap().insert(task_id, groups);
    }

    /// 原地更新某个去重任务的结果并返回更新后的快照。
    pub fn update_task_results<F>(&self, task_id: &str, f: F) -> Vec<DuplicateGroup>
    where
        F: FnOnce(&mut Vec<DuplicateGroup>),
    {
        let mut task_map = self.task_results.lock().unwrap();
        let groups = task_map.entry(task_id.to_string()).or_default();
        f(groups);
        groups.clone()
    }

    /// 清空所有去重任务结果缓存，通常用于重建数据库后同步清内存状态。
    pub fn clear_task_results(&self) {
        self.task_results.lock().unwrap().clear();
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
