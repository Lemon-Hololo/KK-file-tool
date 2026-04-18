//! Mod 重命名：扫描 `.zip/.zipmod`，读取 manifest，生成 `[author] guid-version.zipmod`。

use std::path::{Path, PathBuf};

use chrono::Local;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    constants::mod_op_kind,
    error::AppResult,
    models::{ModOpApplyResponse, ModRenamePreviewItem},
    services::{
        mod_tools::{
            zipmod::{is_zipmod, read_manifest_from_zip, ManifestMeta},
            MOD_OP_TABLES,
        },
        op_pipeline,
    },
    utils::{
        filename::{
            normalize_brackets, resolve_conflict, sanitize_filename, split_name_ext,
            strip_conflict_suffix,
        },
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

const TARGET_EXT: &str = ".zipmod";

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

    let mut result: Vec<ModRenamePreviewItem> = pool.install(|| {
        candidates
            .into_par_iter()
            .filter_map(|path| build_preview_item(&path))
            .collect()
    });

    // 并行结果顺序不稳定，按 `old_path` 排序保证多次预览列表一致。
    result.sort_by(|a, b| a.old_path.cmp(&b.old_path));
    Ok(result)
}

/// 对单个候选文件生成预览项。返回 `None` 表示应从结果中剔除（例如与目标同名）。
fn build_preview_item(path: &Path) -> Option<ModRenamePreviewItem> {
    match read_manifest_from_zip(path) {
        Ok((meta, _)) => {
            let base = build_new_name(&meta);
            if base.is_empty() {
                return Some(ModRenamePreviewItem {
                    old_path: to_user_friendly_path(path),
                    new_path: to_user_friendly_path(path),
                    guid: meta.guid,
                    version: meta.version,
                    author: meta.author,
                    will_rename_conflict: false,
                    warn: Some("无法生成新文件名".to_string()),
                });
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

            let (final_target, conflict) = resolve_conflict(target);
            Some(ModRenamePreviewItem {
                old_path: to_user_friendly_path(path),
                new_path: to_user_friendly_path(&final_target),
                guid: meta.guid,
                version: meta.version,
                author: meta.author,
                will_rename_conflict: conflict,
                warn: None,
            })
        }
        Err(e) => Some(ModRenamePreviewItem {
            old_path: to_user_friendly_path(path),
            new_path: to_user_friendly_path(path),
            guid: String::new(),
            version: String::new(),
            author: String::new(),
            will_rename_conflict: false,
            warn: Some(e),
        }),
    }
}

/// 应用重命名：按预览结果把文件批量 rename，并持久化为 `mod_op` 记录。
pub fn apply_mod_rename(
    db_path: &Path,
    paths: &[String],
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> AppResult<ModOpApplyResponse> {
    let mut preview = preview_mod_rename(db_path, paths).map_err(crate::error::AppError::Internal)?;

    preview.retain(|x| x.warn.is_none() && x.old_path != x.new_path);

    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let pairs: Vec<(String, String)> = preview
        .into_iter()
        .map(|p| (p.old_path, p.new_path))
        .collect();

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
