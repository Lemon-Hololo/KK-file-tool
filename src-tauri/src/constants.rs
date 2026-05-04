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
    pub const MOD_DUPLICATE_PARTIAL: &str = "mod_duplicate_partial";
    pub const MOD_VERSION_PARTIAL: &str = "mod_version_partial";
    /// Pixiv 标签拉取增量结果（每批若干 PID 的 tags 或 error）。
    pub const PIXIV_TAG_PARTIAL: &str = "pixiv_tag_partial";
}

pub mod stages {
    pub const SCAN: &str = "scan";
    pub const HASH: &str = "hash";
    pub const MOD_SCAN: &str = "mod_scan";
    pub const MOD_DUPLICATE: &str = "mod_duplicate";
    pub const MOD_VERSION: &str = "mod_version";
    /// Pixiv 标签拉取阶段（用于进度事件 stage 字段）。
    pub const PIXIV_TAG: &str = "pixiv_tag";
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
    pub const DEFAULT_NAME: &str = "kk-file-tool.db";
    pub const WAL_EXT: &str = "db-wal";
    pub const SHM_EXT: &str = "db-shm";
}

pub mod hash_entry_status {
    pub const ACTIVE: &str = "active";
}

pub mod mod_op_kind {
    pub const RENAME: &str = "rename";
    pub const ORGANIZE: &str = "organize";
    pub const MODIFY: &str = "modify";
    pub const DUPLICATE_DELETE: &str = "duplicate_delete";
    pub const VERSION_DELETE: &str = "version_delete";

    pub fn is_valid(v: &str) -> bool {
        matches!(
            v,
            RENAME | ORGANIZE | MODIFY | DUPLICATE_DELETE | VERSION_DELETE
        )
    }
}

pub mod empty_dir_op_kind {
    pub const DELETE: &str = "delete";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, DELETE)
    }
}
