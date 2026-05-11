//! 图片去重的备份目录解析与备份对构造。
//!
//! 与 [`super::super::mod_tools::backup`] 同构，但读取的设置项不同
//! （`image_dedup_rollback_enabled` / `image_dedup_backup_dir`），目录组织
//! 也固定为 `<backup_root>/<record_id>/<原文件名>`——图片去重不参与
//! "保留源目录结构"开关，因为图片场景下用户更在意"按记录批量清理"而不是
//! 复刻原目录树。
//!
//! 关闭备份时（`image_dedup_rollback_enabled = false`）pair 的 new_path 写
//! 空字符串，apply 阶段直接 `remove_file` 真删。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    config::DEFAULT_IMAGE_DEDUP_BACKUP_SUBDIR,
    db::settings_repo,
    error::{AppError, AppResult},
    models::AppSettings,
    utils::{filename::resolve_conflict_with_reserved, path::to_user_friendly_path},
};

/// [`prepare_backup`] 的产物——与 `mod_tools::backup::PreparedBackup` 同义，
/// 但保留独立类型避免跨业务的语义耦合。
pub struct PreparedBackup {
    /// 来自 `settings.image_dedup_rollback_enabled`。决定是否实际备份与记录。
    pub rollback_enabled: bool,
    /// 业务侧预生成的 UUID。备份目录 `<root>/<record_id>/...` 与
    /// `op_record_repo::create_record` 都共用同一个 ID。
    pub record_id: String,
    /// `(原路径, 备份路径或空串)`。
    pub pairs: Vec<(String, String)>,
}

/// 标准前置：读 settings、生成 record_id、构造 `(原路径, 备份路径)` 列表。
///
/// 启用回滚时备份路径为 `<root>/<record_id>/<filename>` 并预先建好 `record_root`；
/// 同批次同名图片用 `resolve_conflict_with_reserved` 自动加 ` (N)` 后缀避让，
/// 保证一个备份不会被另一个悄悄覆盖。
///
/// 关闭回滚时直接构造 `(原路径, 空串)` 对，不读备份目录设置也不建任何目录。
pub fn prepare_backup(
    db_path: &Path,
    selected_file_paths: Vec<String>,
) -> AppResult<PreparedBackup> {
    let settings = settings_repo::get_settings(db_path)?;
    let rollback_enabled = settings.image_dedup_rollback_enabled;
    let record_id = Uuid::new_v4().to_string();

    let pairs = if rollback_enabled {
        let root = resolve_backup_root(&settings)?;
        let record_root = root.join(&record_id);
        ensure_backup_dir(&record_root)?;

        let mut reserved: HashSet<String> = HashSet::new();
        let mut pairs: Vec<(String, String)> = Vec::with_capacity(selected_file_paths.len());

        for p in selected_file_paths {
            let intended = build_backup_path(&record_root, &p);
            let (final_backup, _) = resolve_conflict_with_reserved(intended, &mut reserved);
            pairs.push((p, to_user_friendly_path(&final_backup)));
        }

        pairs
    } else {
        selected_file_paths
            .into_iter()
            .map(|p| (p, String::new()))
            .collect()
    };

    Ok(PreparedBackup {
        rollback_enabled,
        record_id,
        pairs,
    })
}

/// 解析图片去重备份根目录。
///
/// 不在这里建目录，调用方在确定要写备份时再调 [`ensure_backup_dir`]，
/// 避免仅仅是"读设置"就触碰文件系统。
pub fn resolve_backup_root(settings: &AppSettings) -> AppResult<PathBuf> {
    if let Some(custom) = settings.image_dedup_backup_dir.as_deref() {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let exe =
        std::env::current_exe().map_err(|e| AppError::Io(format!("无法获取程序目录: {e}")))?;
    let parent = exe
        .parent()
        .ok_or_else(|| AppError::Io("程序目录无效".to_string()))?
        .to_path_buf();
    Ok(parent.join(DEFAULT_IMAGE_DEDUP_BACKUP_SUBDIR))
}

/// 构造单条备份的目标路径：`<record_root>/<原文件名>`。
///
/// `original` 没有合法 `file_name` 时（例如根目录 / 路径以分隔符结尾）
/// 用 `"unknown.image"` 作为兜底，避免备份目录里出现空文件名。
fn build_backup_path(record_root: &Path, original: &str) -> PathBuf {
    let file_name = Path::new(original)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.image".to_string());
    record_root.join(file_name)
}

/// 创建备份子目录（含父级）。已存在则视为成功。
pub fn ensure_backup_dir(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path).map_err(|e| AppError::Io(format!("创建备份目录失败: {e}")))
}
