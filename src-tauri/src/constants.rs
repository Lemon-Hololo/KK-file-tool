//! 集中管理跨模块复用的字面量常量（事件名、枚举值等）。
//!
//! 这些值是前后端 IPC 契约的一部分，修改需同步前端。

pub mod events {
    pub const TASK_LOG: &str = "task_log";
    pub const TASK_PROGRESS: &str = "task_progress";
    pub const TASK_STATE_CHANGED: &str = "task_state_changed";
    pub const TASK_FAILED: &str = "task_failed";
    pub const TASK_RESULT_PARTIAL: &str = "task_result_partial";
    pub const TASK_COMPLETED: &str = "task_completed";
    pub const MOVE_REPORT_READY: &str = "move_report_ready";
    pub const MOD_SCAN_COMPLETED: &str = "mod_scan_completed";
}

pub mod stages {
    pub const SCAN: &str = "scan";
    pub const HASH: &str = "hash";
    pub const MOD_SCAN: &str = "mod_scan";
}

pub mod log_level {
    pub const INFO: &str = "INFO";
    pub const WARN: &str = "WARN";
    pub const ERROR: &str = "ERROR";
}

pub mod keep_policy {
    pub const NEWEST: &str = "newest";
    pub const OLDEST: &str = "oldest";
}

pub mod theme {
    pub const LIGHT: &str = "light";
    pub const DARK: &str = "dark";
    pub const SYSTEM: &str = "system";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, LIGHT | DARK | SYSTEM)
    }
}

pub mod db_file {
    pub const DEFAULT_NAME: &str = "fileflow.db";
    pub const WAL_EXT: &str = "db-wal";
    pub const SHM_EXT: &str = "db-shm";
}

pub mod hash_entry_status {
    pub const ACTIVE: &str = "active";
}

pub mod mod_op_kind {
    pub const RENAME: &str = "rename";
    pub const ORGANIZE: &str = "organize";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, RENAME | ORGANIZE)
    }
}
