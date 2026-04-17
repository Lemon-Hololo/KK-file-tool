use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
  },
};

use crate::models::{DuplicateGroup, TaskStatus};

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
}

pub struct AppState {
  pub app_data_dir: PathBuf,
  pub db_path: PathBuf,
  pub tasks: Mutex<HashMap<String, Arc<TaskRuntime>>>,
  pub task_results: Mutex<HashMap<String, Vec<DuplicateGroup>>>,
}