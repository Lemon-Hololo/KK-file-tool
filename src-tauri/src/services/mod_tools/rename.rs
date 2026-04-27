//! Mod 重命名：扫描 `.zip/.zipmod`，读取 manifest，生成 `[author] guid-version.zipmod`。

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use chrono::Local;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    constants::mod_op_kind,
    error::AppResult,
    models::{ModOpApplyResponse, ModRenamePreviewItem},
    services::{
        logging::TaskLogContext,
        mod_tools::{
            zipmod::{is_zipmod, read_manifest_from_zip, ManifestMeta},
            MOD_OP_TABLES,
        },
        op_pipeline,
    },
    utils::{
        filename::{
            normalize_brackets, sanitize_filename, split_name_ext, strip_conflict_suffix,
        },
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

const TARGET_EXT: &str = ".zipmod";

struct PendingRenamePreviewItem {
    old_path: String,
    target_path: PathBuf,
    guid: String,
    version: String,
    author: String,
}

enum PreviewSeed {
    Pending(PendingRenamePreviewItem),
    Final(ModRenamePreviewItem),
}

/// 按 Java 原版规则构造新文件主名（不含扩展名）。
///
/// - `guid` 以 `[...]` 或 `【...】` 开头：`newName = guid`（`【】` 归一化为 `[]`）
/// - 否则：`[author] guid`（`author` 空 → `[unknown]`，`guid` 空 → `unknown`）
/// - 非法字符替换为 `-`
/// - `version` 非空 → 追加 `-version`
pub fn build_new_name(meta: &ManifestMeta) -> String {
    let guid = normalize_brackets(meta.guid.trim());
    let author = normalize_brackets(meta.author.trim());
    let version = meta.version.trim();

    let starts_with_bracket = guid.starts_with('[');

    let mut new_name = if starts_with_bracket {
        if guid.is_empty() {
            "unknown".to_string()
        } else {
            guid.clone()
        }
    } else {
        let author_part = if author.is_empty() {
            "[unknown] ".to_string()
        } else {
            format!("[{author}] ")
        };
        let guid_part = if guid.is_empty() {
            "unknown".to_string()
        } else {
            guid.clone()
        };
        format!("{author_part}{guid_part}")
    };

    new_name = sanitize_filename(&new_name);

    if !version.is_empty() {
        new_name.push('-');
        new_name.push_str(version);
    }

    new_name
}

/// 预览重命名：递归扫描 `paths`，对每个 zipmod 生成一条预览项。
///
/// 并行读取 zip manifest（IO + 解压 CPU 混合，多线程受用户 `thread_count` 控制）。
/// 当文件当前名已满足目标命名（忽略 `(N)` 冲突后缀）时从结果中剔除，避免
/// 重复预览永远命中自己。
pub fn preview_mod_rename(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> Result<Vec<ModRenamePreviewItem>, String> {
    let mut candidates: Vec<PathBuf> = vec![];
    for root in paths {
        let ep = to_extended_length_path(Path::new(root));
        for e in WalkDir::new(&ep).into_iter().filter_map(|x| x.ok()) {
            if !e.file_type().is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy();
            if is_zipmod(&name) {
                candidates.push(e.path().to_path_buf());
            }
        }
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(op_pipeline::resolve_thread_count(db_path).max(1))
        .build()
        .map_err(|e| format!("创建线程池失败: {e}"))?;

    let mut seeds: Vec<PreviewSeed> = pool.install(|| {
        candidates
            .into_par_iter()
            .filter_map(|path| build_preview_seed(&path, log.as_ref()))
            .collect()
    });

    // 并行结果顺序不稳定，按 `old_path` 排序保证同批次冲突后缀分配稳定。
    seeds.sort_by(|a, b| preview_seed_old_path(a).cmp(preview_seed_old_path(b)));

    let mut reserved_targets = HashSet::new();
    let mut result = Vec::with_capacity(seeds.len());
    for seed in seeds {
        match seed {
            PreviewSeed::Pending(item) => {
                let (final_target, conflict) =
                    resolve_conflict_for_batch(item.target_path, &mut reserved_targets);
                result.push(ModRenamePreviewItem {
                    old_path: item.old_path,
                    new_path: to_user_friendly_path(&final_target),
                    guid: item.guid,
                    version: item.version,
                    author: item.author,
                    will_rename_conflict: conflict,
                    warn: None,
                });
            }
            PreviewSeed::Final(item) => result.push(item),
        }
    }

    Ok(result)
}

/// 对单个候选文件生成预览种子。返回 `None` 表示应从结果中剔除（例如与目标同名）。
fn build_preview_seed(path: &Path, log: Option<&TaskLogContext>) -> Option<PreviewSeed> {
    if let Some(log) = log {
        log.info_path("正在读取 Mod manifest", path);
    }
    match read_manifest_from_zip(path) {
        Ok((meta, _)) => {
            let base = build_new_name(&meta);
            if base.is_empty() {
                return Some(PreviewSeed::Final(ModRenamePreviewItem {
                    old_path: to_user_friendly_path(path),
                    new_path: to_user_friendly_path(path),
                    guid: meta.guid,
                    version: meta.version,
                    author: meta.author,
                    will_rename_conflict: false,
                    warn: Some("无法生成新文件名".to_string()),
                }));
            }
            let parent = path.parent()?;
            let target = parent.join(format!("{base}{TARGET_EXT}"));

            let current_name = path
                .file_name()
                .map(|x| x.to_string_lossy().to_string())
                .unwrap_or_default();
            let (current_stem, current_ext) = split_name_ext(&current_name);
            let current_ext = current_ext.unwrap_or_default();
            if current_ext.eq_ignore_ascii_case(TARGET_EXT)
                && strip_conflict_suffix(&current_stem) == base
            {
                return None;
            }

            Some(PreviewSeed::Pending(PendingRenamePreviewItem {
                old_path: to_user_friendly_path(path),
                target_path: target,
                guid: meta.guid,
                version: meta.version,
                author: meta.author,
            }))
        }
        Err(e) => Some(PreviewSeed::Final(ModRenamePreviewItem {
            old_path: to_user_friendly_path(path),
            new_path: to_user_friendly_path(path),
            guid: String::new(),
            version: String::new(),
            author: String::new(),
            will_rename_conflict: false,
            warn: Some(e),
        })),
    }
}

fn preview_seed_old_path(seed: &PreviewSeed) -> &str {
    match seed {
        PreviewSeed::Pending(item) => &item.old_path,
        PreviewSeed::Final(item) => &item.old_path,
    }
}

/// 同时考虑磁盘现状与本批次已保留目标名，为后续条目分配稳定的 ` (N)` 后缀。
fn resolve_conflict_for_batch(
    target: PathBuf,
    reserved_targets: &mut HashSet<String>,
) -> (PathBuf, bool) {
    if !path_exists_or_reserved(&target, reserved_targets) {
        reserve_target(&target, reserved_targets);
        return (target, false);
    }

    let parent = target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let file_name = target.file_name().unwrap().to_string_lossy().to_string();
    let (stem, ext) = split_name_ext(&file_name);
    let ext = ext.unwrap_or_default();

    let mut i = 1usize;
    loop {
        let candidate = parent.join(format!("{stem} ({i}){ext}"));
        if !path_exists_or_reserved(&candidate, reserved_targets) {
            reserve_target(&candidate, reserved_targets);
            return (candidate, true);
        }
        i += 1;
    }
}

fn path_exists_or_reserved(path: &Path, reserved_targets: &HashSet<String>) -> bool {
    reserved_targets.contains(&batch_target_key(path)) || to_extended_length_path(path).exists()
}

fn reserve_target(path: &Path, reserved_targets: &mut HashSet<String>) {
    reserved_targets.insert(batch_target_key(path));
}

fn batch_target_key(path: &Path) -> String {
    to_extended_length_path(path)
        .to_string_lossy()
        .to_lowercase()
}

/// 应用重命名：按预览结果把文件批量 rename，并持久化为 `mod_op` 记录。
pub fn apply_mod_rename(
    db_path: &Path,
    paths: &[String],
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    let mut preview = preview_mod_rename(db_path, paths, log.clone())
        .map_err(crate::error::AppError::Internal)?;

    preview.retain(|x| x.warn.is_none() && x.old_path != x.new_path);

    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let pairs: Vec<(String, String)> = preview
        .into_iter()
        .map(|p| (p.old_path, p.new_path))
        .collect();

    if let Some(log) = &log {
        for (old_path, _) in &pairs {
            log.info(&format!("准备重命名: {old_path}"));
        }
    }

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());

    op_pipeline::persist_apply_rename_pairs(
        db_path,
        MOD_OP_TABLES,
        mod_op_kind::RENAME,
        name,
        paths,
        pairs,
        false,
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::resolve_conflict_for_batch;

    fn temp_dir_path(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("fileflow-{prefix}-{nonce}"))
    }

    #[test]
    fn batch_conflicts_append_incrementing_suffixes() {
        let dir = temp_dir_path("mod-rename-batch");
        fs::create_dir_all(&dir).unwrap();

        let mut reserved = HashSet::new();
        let (first, first_conflict) =
            resolve_conflict_for_batch(dir.join("same.zipmod"), &mut reserved);
        let (second, second_conflict) =
            resolve_conflict_for_batch(dir.join("same.zipmod"), &mut reserved);
        let (third, third_conflict) =
            resolve_conflict_for_batch(dir.join("same.zipmod"), &mut reserved);

        assert_eq!(first.file_name().unwrap().to_string_lossy(), "same.zipmod");
        assert!(!first_conflict);
        assert_eq!(second.file_name().unwrap().to_string_lossy(), "same (1).zipmod");
        assert!(second_conflict);
        assert_eq!(third.file_name().unwrap().to_string_lossy(), "same (2).zipmod");
        assert!(third_conflict);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_conflicts_skip_existing_files_before_reserved_names() {
        let dir = temp_dir_path("mod-rename-existing");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("same.zipmod"), b"1").unwrap();
        fs::write(dir.join("same (1).zipmod"), b"2").unwrap();

        let mut reserved = HashSet::new();
        let (resolved, conflict) =
            resolve_conflict_for_batch(dir.join("same.zipmod"), &mut reserved);

        assert!(conflict);
        assert_eq!(
            resolved.file_name().unwrap().to_string_lossy(),
            "same (2).zipmod"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
