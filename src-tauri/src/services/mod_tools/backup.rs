//! Mod 备份目录解析与备份路径构造。
//!
//! 这一模块只服务于"备份型"Mod 操作（重复删除 / 不同版本删除 / 移除版本限制）。
//! 重命名 / 归类是纯反向 rename，不涉及备份概念。
//!
//! # 调用入口
//! 业务层应该直接用 [`prepare_mod_backup`]：它把"读 settings → 生成 record_id
//! → 构造 `(原路径, 备份路径)` 列表 → 处理同批次撞名"这一整套 Mod 备份操作的
//! 标准前置打成一个调用，cleanup 与 modify 共享。剩下的 [`resolve_backup_root`]
//! / [`build_backup_path`] / [`ensure_backup_dir`] 是底层原语，仅在需要绕开
//! 标准流程的特殊场景下使用。
//!
//! # 备份目录解析
//! - 用户在配置中心填了 `mod_backup_dir`（trim 非空）则直接用它。
//! - 否则取 exe 所在目录下的 `mod-backups/` 子目录。
//!   ⚠️ 当 exe 安装在 Program Files 等只读位置时创建会失败，本模块向上抛
//!   `AppError::Io`，由命令层把错误透传给前端，让用户改配置。
//!
//! # 目录组织
//! 默认平铺到 `<root>/<record_id>/<原文件名>`：每条记录一个子目录，便于人工
//! 按记录批量清理；`record_id` 是 UUID v4，零冲突可能；同一记录里如果两个源
//! 目录有同名 zipmod，由 `resolve_conflict_with_reserved` 自动加 `(N)` 后缀
//! 避让，保证一条记录里的多个备份不会互相覆盖。
//!
//! 用户在配置中心开启"保留源目录结构"（`preserve_dir_on_move`）后，再传入
//! `source_paths`（任务输入根列表）时，备份路径变成 `<root>/<record_id>/<rel>/<原文件名>`
//! ——`rel` 来自 [`crate::utils::path::relative_subdir_for`]，与去重移动落
//! `<target>/<task_id>/<rel>/` 共用同一份算法，避免两份漂移。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    db::settings_repo,
    error::{AppError, AppResult},
    models::AppSettings,
    utils::{
        filename::resolve_conflict_with_reserved,
        path::{prepare_source_roots, relative_subdir_for, to_user_friendly_path},
    },
};

const DEFAULT_BACKUP_SUBDIR: &str = "mod-backups";

/// [`prepare_mod_backup`] 的产物：准备阶段所有共享数据集中在这里。
///
/// `pairs` 是 `(原路径, 备份路径)` 列表，可以直接传给
/// `op_pipeline::persist_apply_with_executor`。`rollback_enabled = false` 时
/// 备份路径为空字符串——业务的 executor 据此切换到"真删 / in-place 改写"分支。
pub struct PreparedBackup {
    /// 来自 `settings.mod_rollback_enabled`。决定是否实际备份与记录。
    pub rollback_enabled: bool,
    /// 业务侧预生成的 UUID。备份目录 `<root>/<record_id>/...` 与
    /// `op_record_repo::create_record` 都共用同一个 ID。
    pub record_id: String,
    /// `(原路径, 备份路径或空串)`。
    pub pairs: Vec<(String, String)>,
}

/// Mod 备份型操作（cleanup / modify）的标准前置：
/// 读 settings、生成 record_id、构造 `(原路径, 备份路径)` 列表。
///
/// `source_paths` 是当前任务的输入路径（前端 `paths`），仅在用户开启
/// `preserve_dir_on_move` 时被读取——决定每个备份的相对子目录布局。空列表 +
/// 关闭开关都会直接落 `<root>/<record_id>/<filename>`。
///
/// 启用回滚时备份路径为 `<root>/<record_id>/<rel?>/<filename>` 并预先建好所有
/// 用到的子目录；同批次同名 zipmod 用 `resolve_conflict_with_reserved` 自动加
/// ` (N)` 后缀避让，保证一个备份不会被另一个悄悄覆盖。
///
/// 关闭回滚时直接构造 `(原路径, 空串)` 对，不读备份目录设置也不建任何目录，
/// 把"是否有备份"的语义压缩到 pair 的 new_path 是否为空这一个点上。
pub fn prepare_mod_backup(
    db_path: &Path,
    selected_file_paths: Vec<String>,
    source_paths: &[String],
) -> AppResult<PreparedBackup> {
    let settings = settings_repo::get_settings(db_path)?;
    let rollback_enabled = settings.mod_rollback_enabled;
    let record_id = Uuid::new_v4().to_string();

    let pairs = if rollback_enabled {
        let root = resolve_backup_root(&settings)?;
        let record_root = root.join(&record_id);
        ensure_backup_dir(&record_root)?;

        // 仅在开启"保留源目录结构"时整理 source_paths；与去重移动共享同一份
        // 纯路径算法（utils::path::prepare_source_roots / relative_subdir_for）。
        let prepared_roots = if settings.preserve_dir_on_move {
            prepare_source_roots(source_paths)
        } else {
            Vec::new()
        };

        let mut reserved: HashSet<String> = HashSet::new();
        let mut pairs: Vec<(String, String)> = Vec::with_capacity(selected_file_paths.len());

        for p in selected_file_paths {
            let rel = if settings.preserve_dir_on_move {
                relative_subdir_for(Path::new(&p), &prepared_roots)
            } else {
                None
            };
            let intended = build_backup_path(&root, &record_id, rel.as_deref(), &p);

            // 子目录可能是 `<record_root>/sub1/sub2/` 这种多层结构，先把它建出
            // 来再交给 executor。`<record_root>` 已在上面建过；其余路径
            // create_dir_all 是幂等的，不重复时立即返回。
            if let Some(parent) = intended.parent() {
                if parent != record_root {
                    ensure_backup_dir(parent)?;
                }
            }

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

/// 从设置解析 Mod 备份根目录。
///
/// 不在这里建目录，调用方在确定要写备份时再调 [`ensure_backup_dir`]，
/// 避免仅仅是"读设置"就触碰文件系统。
pub fn resolve_backup_root(settings: &AppSettings) -> AppResult<PathBuf> {
    if let Some(custom) = settings.mod_backup_dir.as_deref() {
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
    Ok(parent.join(DEFAULT_BACKUP_SUBDIR))
}

/// 构造单条备份的目标路径：`<root>/<record_id>/<rel?>/<原文件名>`。
///
/// `rel_subdir = None` 或空时落到 `<root>/<record_id>/<filename>`；
/// 传入非空相对路径时把它插在 record_id 与文件名之间，用于"保留源目录结构"。
///
/// `original` 没有合法 `file_name` 时（例如根目录 / 路径以分隔符结尾）
/// 用 `"unknown.zipmod"` 作为兜底，避免备份目录里出现空文件名。
pub fn build_backup_path(
    root: &Path,
    record_id: &str,
    rel_subdir: Option<&Path>,
    original: &str,
) -> PathBuf {
    let file_name = Path::new(original)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown.zipmod".to_string());
    let mut out = root.join(record_id);
    if let Some(rel) = rel_subdir {
        if !rel.as_os_str().is_empty() {
            out.push(rel);
        }
    }
    out.push(file_name);
    out
}

/// 创建备份子目录（含父级）。已存在则视为成功。
pub fn ensure_backup_dir(path: &Path) -> AppResult<()> {
    std::fs::create_dir_all(path).map_err(|e| AppError::Io(format!("创建备份目录失败: {e}")))
}
