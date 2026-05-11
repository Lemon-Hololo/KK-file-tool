//! SQLite schema 初始化与增量列迁移。
//!
//! 采用 `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN`（忽略重复
//! 列错误）的轻量迁移策略：没有版本号表，也没有回滚，适合单用户桌面应用的
//! schema 演进节奏。SQLite 连接打开时启用 WAL + 外键约束。

use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::{
    db::open_connection,
    error::{AppError, AppResult},
    models::AppSettings,
};

/// 初始化（或迁移）数据库 schema；幂等。
///
/// - 开启 `journal_mode=WAL`、`foreign_keys=ON`；
/// - 创建所有业务表与索引（`IF NOT EXISTS`）；
/// - 保证 `app_settings` 至少有一行；
/// - 历史库补列：`thread_count` / `log_max_length` / `io_concurrency_multiplier` /
///   `extreme_row_threshold` / `text_preview_max_kb` / `zip_preview_max_entries` /
///   `mod_scan_default_keyword` / `suffix_default_target` /
///   `mod_rollback_enabled` / `mod_backup_dir` / `preserve_dir_on_move` /
///   `pixiv_*` / `image_dedup_*` 9 列（设置项）；
///   `rollback_enabled`（四类记录主表，标记单条记录创建时是否启用回滚）。
///   重复列错误忽略。
pub fn init_schema(db_path: &Path) -> AppResult<()> {
    let conn = open_connection(db_path)?;

    conn.execute_batch(
        r#"
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=NORMAL;
    PRAGMA foreign_keys=ON;

    CREATE TABLE IF NOT EXISTS app_settings (
      id INTEGER PRIMARY KEY CHECK (id = 1),
      keep_policy TEXT NOT NULL,
      move_target_path TEXT NULL,
      save_record_enabled INTEGER NOT NULL,
      use_last_record_enabled INTEGER NOT NULL,
      include_current_folder_duplicates INTEGER NOT NULL,
      theme_mode TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS hash_records (
      record_id TEXT PRIMARY KEY,
      record_name TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      source_paths TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS hash_entries (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      record_id TEXT NOT NULL,
      hash TEXT NOT NULL,
      file_path TEXT NOT NULL,
      file_size INTEGER NOT NULL,
      mtime INTEGER NOT NULL,
      ctime INTEGER,
      status TEXT NOT NULL,
      FOREIGN KEY (record_id) REFERENCES hash_records(record_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_hash_entries_hash ON hash_entries(hash);
    CREATE INDEX IF NOT EXISTS idx_hash_entries_record_id ON hash_entries(record_id);
    CREATE INDEX IF NOT EXISTS idx_hash_entries_file_path ON hash_entries(file_path);

    CREATE TABLE IF NOT EXISTS move_reports (
      report_id TEXT PRIMARY KEY,
      task_id TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      target_dir TEXT NOT NULL,
      total_selected INTEGER NOT NULL,
      total_success INTEGER NOT NULL,
      total_failed INTEGER NOT NULL,
      released_bytes INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS move_report_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      report_id TEXT NOT NULL,
      src_path TEXT NOT NULL,
      dst_path TEXT NULL,
      success INTEGER NOT NULL,
      error_code TEXT NULL,
      error_message TEXT NULL,
      size INTEGER NOT NULL,
      FOREIGN KEY (report_id) REFERENCES move_reports(report_id) ON DELETE CASCADE
    );
    CREATE INDEX IF NOT EXISTS idx_move_report_items_report_id ON move_report_items(report_id);

    -- 后缀修改记录（op_record_repo 的一个实例）
    CREATE TABLE IF NOT EXISTS suffix_change_records (
      record_id TEXT PRIMARY KEY,
      record_name TEXT NOT NULL,
      target_suffix TEXT NOT NULL,
      source_paths TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      rollback_status TEXT NOT NULL DEFAULT 'applied'
    );

    CREATE TABLE IF NOT EXISTS suffix_change_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      record_id TEXT NOT NULL,
      old_path TEXT NOT NULL,
      new_path TEXT NOT NULL,
      apply_success INTEGER NOT NULL DEFAULT 0,
      apply_error TEXT NULL,
      rollback_success INTEGER NULL,
      rollback_error TEXT NULL,
      updated_at INTEGER NOT NULL,
      FOREIGN KEY (record_id) REFERENCES suffix_change_records(record_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_suffix_items_record_id ON suffix_change_items(record_id);
    CREATE INDEX IF NOT EXISTS idx_suffix_items_old_path ON suffix_change_items(old_path);
    CREATE INDEX IF NOT EXISTS idx_suffix_items_new_path ON suffix_change_items(new_path);

    -- 空文件夹清理记录（删除后可通过 create_dir_all 撤回）
    CREATE TABLE IF NOT EXISTS empty_dir_records (
      record_id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      record_name TEXT NOT NULL,
      source_paths TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      rollback_status TEXT NOT NULL DEFAULT 'applied'
    );

    CREATE TABLE IF NOT EXISTS empty_dir_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      record_id TEXT NOT NULL,
      old_path TEXT NOT NULL,
      new_path TEXT NOT NULL,
      apply_success INTEGER NOT NULL DEFAULT 0,
      apply_error TEXT NULL,
      rollback_success INTEGER NULL,
      rollback_error TEXT NULL,
      updated_at INTEGER NOT NULL,
      FOREIGN KEY (record_id) REFERENCES empty_dir_records(record_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_empty_dir_items_record_id ON empty_dir_items(record_id);
    CREATE INDEX IF NOT EXISTS idx_empty_dir_items_old_path ON empty_dir_items(old_path);

    -- Mod 工具共享记录（op_record_repo 的另一个实例）
    CREATE TABLE IF NOT EXISTS mod_op_records (
      record_id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      record_name TEXT NOT NULL,
      source_paths TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      rollback_status TEXT NOT NULL DEFAULT 'applied'
    );

    CREATE TABLE IF NOT EXISTS mod_op_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      record_id TEXT NOT NULL,
      old_path TEXT NOT NULL,
      new_path TEXT NOT NULL,
      apply_success INTEGER NOT NULL DEFAULT 0,
      apply_error TEXT NULL,
      rollback_success INTEGER NULL,
      rollback_error TEXT NULL,
      updated_at INTEGER NOT NULL,
      FOREIGN KEY (record_id) REFERENCES mod_op_records(record_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_mod_op_items_record_id ON mod_op_items(record_id);
    CREATE INDEX IF NOT EXISTS idx_mod_op_records_kind ON mod_op_records(kind);

    -- 图片相似度去重记录（op_record_repo 的另一个实例）
    -- kind 当前固定 "similarity_delete"，与 empty_dir_op_kind 同样保留扩展位。
    CREATE TABLE IF NOT EXISTS image_dedup_op_records (
      record_id TEXT PRIMARY KEY,
      kind TEXT NOT NULL,
      record_name TEXT NOT NULL,
      source_paths TEXT NOT NULL,
      created_at INTEGER NOT NULL,
      rollback_status TEXT NOT NULL DEFAULT 'applied',
      rollback_enabled INTEGER NOT NULL DEFAULT 1
    );

    CREATE TABLE IF NOT EXISTS image_dedup_op_items (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      record_id TEXT NOT NULL,
      old_path TEXT NOT NULL,
      new_path TEXT NOT NULL,
      apply_success INTEGER NOT NULL DEFAULT 0,
      apply_error TEXT NULL,
      rollback_success INTEGER NULL,
      rollback_error TEXT NULL,
      updated_at INTEGER NOT NULL,
      FOREIGN KEY (record_id) REFERENCES image_dedup_op_records(record_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_image_dedup_items_record_id ON image_dedup_op_items(record_id);
    CREATE INDEX IF NOT EXISTS idx_image_dedup_records_kind ON image_dedup_op_records(kind);
    "#,
    )
    .map_err(|e| AppError::Db(e.to_string()))?;

    let exists: Option<i64> = conn
        .query_row("SELECT id FROM app_settings WHERE id = 1", [], |r| r.get(0))
        .optional()
        .map_err(|e| AppError::Db(e.to_string()))?;

    if exists.is_none() {
        let d = AppSettings::default();
        conn.execute(
            r#"INSERT INTO app_settings
            (id, keep_policy, move_target_path, save_record_enabled, use_last_record_enabled, include_current_folder_duplicates, theme_mode)
            VALUES (1, ?, ?, ?, ?, ?, ?)"#,
            params![
                d.keep_policy,
                d.move_target_path,
                d.save_record_enabled as i32,
                d.use_last_record_enabled as i32,
                d.include_current_folder_duplicates as i32,
                d.theme_mode
            ],
        )
        .map_err(|e| AppError::Db(e.to_string()))?;
    }

    // 历史库补列：已存在时 SQLite 返回 "duplicate column" 错误，对此忽略；
    // 其它错误（磁盘损坏 / 权限不足）也吞掉是有意为之——schema 演进失败不应阻塞启动，
    // 真正读写 settings 时还会触发底层 sqlite 错误，会被正常上抛。
    // 顺序无关紧要；DEFAULT 与 `AppSettings::Default` / `config.rs::DEFAULT_*` 保持一致。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN thread_count INTEGER NOT NULL DEFAULT 0",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN log_max_length INTEGER NOT NULL DEFAULT 3000",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN io_concurrency_multiplier INTEGER NOT NULL DEFAULT 2",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN extreme_row_threshold INTEGER NOT NULL DEFAULT 20000",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN text_preview_max_kb INTEGER NOT NULL DEFAULT 256",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN zip_preview_max_entries INTEGER NOT NULL DEFAULT 5000",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN mod_scan_default_keyword TEXT NOT NULL DEFAULT 'Koikatsu'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN suffix_default_target TEXT NOT NULL DEFAULT 'txt'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN mod_rollback_enabled INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN mod_backup_dir TEXT NULL",
        [],
    );
    // 去重移动是否保留相对源根的子目录结构；默认 0 = 平铺，与历史行为一致。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN preserve_dir_on_move INTEGER NOT NULL DEFAULT 0",
        [],
    );

    // 三类记录主表新增 rollback_enabled 列：旧记录默认 1（保持可撤回），
    // 新记录由业务层根据当时的设置决定（关闭回滚时写 0，撤回按钮置灰）。
    let _ = conn.execute(
        "ALTER TABLE mod_op_records ADD COLUMN rollback_enabled INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE suffix_change_records ADD COLUMN rollback_enabled INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE empty_dir_records ADD COLUMN rollback_enabled INTEGER NOT NULL DEFAULT 1",
        [],
    );

    // Pixiv 标签整理设置：API base / 排除 tag（JSON 数组字符串）/ 可选 Cookie。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_tag_api_base TEXT NOT NULL DEFAULT 'https://www.pixiv.net/ajax/illust/'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_excluded_tags TEXT NOT NULL DEFAULT '[]'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_local_tag_translations TEXT NOT NULL DEFAULT '{}'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_cookie TEXT NULL",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_proxy TEXT NULL",
        [],
    );
    // 是否用 translation.en 替代原 tag 显示。默认关闭，符合"无破坏性升级"原则。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_use_translation INTEGER NOT NULL DEFAULT 0",
        [],
    );
    // Pixiv 限速：每分钟最大请求数。0 = 不限速；UI 限制最小 1。默认 60（每秒 1 条）。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_rate_limit_per_minute INTEGER NOT NULL DEFAULT 60",
        [],
    );
    // Pixiv 增量结果合并刷新间隔（毫秒）。0 = 即刻应用；>0 = 节流到固定间隔。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN pixiv_partial_flush_interval_ms INTEGER NOT NULL DEFAULT 0",
        [],
    );

    // 图片相似度去重设置：算法 / 哈希边长 / 阈值 / 扩展名（JSON 数组）/ 大小过滤 /
    // 边长过滤 / 保留策略 / 回滚开关 / 备份目录。所有 DEFAULT 与 config.rs::DEFAULT_*
    // 严格对齐，新装与升级用户行为一致。
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_algorithm TEXT NOT NULL DEFAULT 'phash'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_hash_size INTEGER NOT NULL DEFAULT 16",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_similarity_threshold INTEGER NOT NULL DEFAULT 90",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_extensions TEXT NOT NULL DEFAULT '[\"jpg\",\"jpeg\",\"png\",\"webp\",\"bmp\",\"gif\"]'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_min_file_size_kb INTEGER NOT NULL DEFAULT 10",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_min_dimension INTEGER NOT NULL DEFAULT 64",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_keep_policy TEXT NOT NULL DEFAULT 'largestResolution'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_rollback_enabled INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE app_settings ADD COLUMN image_dedup_backup_dir TEXT NULL",
        [],
    );

    Ok(())
}
