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
    /// 图片相似度去重的扫描增量结果（每个 chunk 完成时一次）。
    pub const IMAGE_DEDUP_PARTIAL: &str = "image_dedup_partial";
}

pub mod stages {
    pub const SCAN: &str = "scan";
    pub const HASH: &str = "hash";
    pub const MOD_SCAN: &str = "mod_scan";
    pub const MOD_DUPLICATE: &str = "mod_duplicate";
    pub const MOD_VERSION: &str = "mod_version";
    /// Pixiv 标签拉取阶段（用于进度事件 stage 字段）。
    pub const PIXIV_TAG: &str = "pixiv_tag";
    /// 图片相似度去重的扫描+哈希阶段（合并显示，分组阶段是同步的快速操作不单独发进度）。
    pub const IMAGE_DEDUP_HASH: &str = "image_dedup_hash";
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

/// 图片相似度去重的操作类型。当前只有"按相似度删除"一种，保留 `kind` 列只是与
/// `empty_dir_op_kind` 对齐的前向兼容兜底——未来若加"按相似度合并到目录"再扩
/// 一个值即可，旧记录默认值不动。
pub mod image_dedup_op_kind {
    pub const SIMILARITY_DELETE: &str = "similarity_delete";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, SIMILARITY_DELETE)
    }
}

/// 图片相似度去重支持的感知哈希算法。
///
/// `phash` 抗压缩 / 旋转最稳；`dhash` 对裁剪敏感；`ahash` 最快但精度最低。
/// 字符串值与前端 `pixivUseTranslation`-style 设置同步，落库为 `app_settings`
/// 中的 `image_dedup_algorithm` 字段。
pub mod image_hash_algorithm {
    pub const PHASH: &str = "phash";
    pub const DHASH: &str = "dhash";
    pub const AHASH: &str = "ahash";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, PHASH | DHASH | AHASH)
    }
}

/// 图片相似度去重的"每组保留哪一张"策略。
///
/// 与去重 / Mod 的 `keep_policy`（newest/oldest 二选一）不同：图片场景下
/// 同一组里可能既有高分辨率也有压缩版，单看时间戳不足以判断哪张是"原图"。
pub mod image_keep_policy {
    /// 保留分辨率最大的那张（默认推荐：通常是未压缩 / 未缩放的原图）。
    pub const LARGEST_RESOLUTION: &str = "largestResolution";
    /// 保留文件体积最大的那张（编码相同时一般也意味着信息更全）。
    pub const LARGEST_FILE: &str = "largestFile";
    pub const NEWEST: &str = "newest";
    pub const OLDEST: &str = "oldest";

    pub fn is_valid(v: &str) -> bool {
        matches!(v, LARGEST_RESOLUTION | LARGEST_FILE | NEWEST | OLDEST)
    }
}
