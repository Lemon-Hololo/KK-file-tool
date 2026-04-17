use std::path::{Path, PathBuf};

use chrono::Local;
use rayon::prelude::*;
use walkdir::WalkDir;

use crate::{
    db::suffix_repo,
    models::{
        SuffixApplyItem, SuffixApplyResponse, SuffixPreviewItem, SuffixRecordDetail,
        SuffixRecordSummary, SuffixRollbackCheck, SuffixRollbackResponse,
    },
    utils::path::{to_extended_length_path, to_user_friendly_path},
};

fn normalize_suffix(input: &str) -> String {
    let mut s = input.trim().to_string();
    if s.is_empty() {
        return String::new();
    }
    if s.starts_with('.') {
        s
    } else {
        s.insert(0, '.');
        s
    }
}

fn split_name_ext(file_name: &str) -> (String, Option<String>) {
    if let Some(idx) = file_name.rfind('.') {
        if idx > 0 {
            return (
                file_name[..idx].to_string(),
                Some(file_name[idx..].to_string()),
            );
        }
    }
    (file_name.to_string(), None)
}

fn build_target_path(path: &Path, target_suffix: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_string_lossy().to_string();
    let (stem, _) = split_name_ext(&file_name);
    Some(parent.join(format!("{stem}{target_suffix}")))
}

fn resolve_conflict(target: PathBuf) -> (PathBuf, bool) {
    let exists = to_extended_length_path(&target).exists();
    if !exists {
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
        let c = parent.join(format!("{stem} ({i}){ext}"));
        if !to_extended_length_path(&c).exists() {
            return (c, true);
        }
        i += 1;
    }
}

pub fn preview_suffix_change(
    paths: &[String],
    target_suffix_input: &str,
) -> Result<Vec<SuffixPreviewItem>, String> {
    let target_suffix = normalize_suffix(target_suffix_input);
    if target_suffix.is_empty() {
        return Err("目标后缀不能为空".to_string());
    }

    let mut result = vec![];
    for root in paths {
        for e in WalkDir::new(root).into_iter().filter_map(|x| x.ok()) {
            if !e.file_type().is_file() {
                continue;
            }

            let old_path = e.path().to_path_buf();
            let Some(candidate) = build_target_path(&old_path, &target_suffix) else {
                continue;
            };

            if candidate == old_path {
                continue;
            }

            let (final_target, conflict) = resolve_conflict(candidate);

            result.push(SuffixPreviewItem {
                old_path: to_user_friendly_path(&old_path),
                new_path: to_user_friendly_path(&final_target),
                will_rename_conflict: conflict,
            });
        }
    }

    Ok(result)
}

pub fn apply_suffix_change(
    db_path: &Path,
    paths: &[String],
    target_suffix_input: &str,
    record_name: Option<String>,
    selected_old_paths: Option<Vec<String>>,
) -> Result<SuffixApplyResponse, String> {
    let target_suffix = normalize_suffix(target_suffix_input);
    if target_suffix.is_empty() {
        return Err("目标后缀不能为空".to_string());
    }

    let mut preview = preview_suffix_change(paths, &target_suffix)?;

    // 只处理选中项
    if let Some(selected) = selected_old_paths {
        let set: std::collections::HashSet<String> = selected.into_iter().collect();
        preview.retain(|x| set.contains(&x.old_path));
    }

    let created_at = Local::now().timestamp();
    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    let source_paths_json = serde_json::to_string(paths).map_err(|e| e.to_string())?;
    let record_id = suffix_repo::create_record(
        db_path,
        &name,
        &target_suffix,
        &source_paths_json,
        created_at,
    )?;

    // 并行执行文件重命名
    let rename_results: Vec<(SuffixPreviewItem, bool, Option<String>)> = preview
        .into_par_iter()
        .map(|p| {
            let old_path = PathBuf::from(&p.old_path);
            let new_path = PathBuf::from(&p.new_path);
            let r = std::fs::rename(
                to_extended_length_path(&old_path),
                to_extended_length_path(&new_path),
            );
            match r {
                Ok(_) => (p, true, None),
                Err(e) => (p, false, Some(e.to_string())),
            }
        })
        .collect();

    // 批量写入数据库
    let mut items = Vec::with_capacity(rename_results.len());
    let mut success = 0usize;
    let mut failed = 0usize;

    let now = Local::now().timestamp();
    let item_ids = suffix_repo::batch_insert_items(
        db_path,
        &record_id,
        &rename_results
            .iter()
            .map(|(p, ok, msg)| (p.old_path.as_str(), p.new_path.as_str(), *ok, msg.as_deref()))
            .collect::<Vec<_>>(),
        now,
    )?;

    for (i, (p, ok, msg)) in rename_results.into_iter().enumerate() {
        if ok {
            success += 1;
        } else {
            failed += 1;
        }
        items.push(SuffixApplyItem {
            item_id: item_ids[i],
            old_path: p.old_path,
            new_path: p.new_path,
            status: if ok { "success".into() } else { "failed".into() },
            message: msg,
        });
    }

    Ok(SuffixApplyResponse {
        record_id,
        record_name: name,
        total: items.len(),
        success,
        failed,
        items,
    })
}

pub fn list_suffix_records(db_path: &Path) -> Result<Vec<SuffixRecordSummary>, String> {
    suffix_repo::list_records(db_path)
}

pub fn get_suffix_record_detail(
    db_path: &Path,
    record_id: &str,
) -> Result<SuffixRecordDetail, String> {
    suffix_repo::get_record_detail(db_path, record_id)
}

pub fn check_rollback(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
) -> Result<SuffixRollbackCheck, String> {
    let detail = suffix_repo::get_record_detail(db_path, record_id)?;
    let selected: Vec<_> = detail
        .items
        .into_iter()
        .filter(|x| {
            item_ids
                .as_ref()
                .map(|ids| ids.contains(&x.item_id))
                .unwrap_or(true)
        })
        .filter(|x| x.apply_success)
        .collect();

    // 并行检测文件是否存在
    let checks: Vec<(bool, String)> = selected
        .par_iter()
        .map(|item| {
            let exists = to_extended_length_path(Path::new(&item.new_path)).exists();
            (exists, item.new_path.clone())
        })
        .collect();

    let mut existing = 0usize;
    let mut missing = vec![];
    for (exists, path) in checks {
        if exists {
            existing += 1;
        } else {
            missing.push(path);
        }
    }

    Ok(SuffixRollbackCheck {
        total_selected: selected.len(),
        existing_count: existing,
        missing_paths: missing,
    })
}

pub fn rollback_suffix_change(
    db_path: &Path,
    record_id: &str,
    item_ids: Option<Vec<i64>>,
    force_ignore_missing: bool,
) -> Result<SuffixRollbackResponse, String> {
    let detail = suffix_repo::get_record_detail(db_path, record_id)?;
    let selected: Vec<_> = detail
        .items
        .into_iter()
        .filter(|x| {
            item_ids
                .as_ref()
                .map(|ids| ids.contains(&x.item_id))
                .unwrap_or(true)
        })
        .filter(|x| x.apply_success)
        .collect();

    let check = check_rollback(db_path, record_id, item_ids.clone())?;
    if !force_ignore_missing && !check.missing_paths.is_empty() {
        return Err(format!(
            "存在 {} 个文件缺失，请确认后使用 forceIgnoreMissing=true 再执行",
            check.missing_paths.len()
        ));
    }

    // 并行执行文件重命名回滚
    let rollback_results: Vec<_> = selected
        .into_par_iter()
        .map(|item| {
            let current = PathBuf::from(&item.new_path);
            let old = PathBuf::from(&item.old_path);

            if !to_extended_length_path(&current).exists() {
                return (item, "skipped_missing", None::<String>);
            }

            match std::fs::rename(
                to_extended_length_path(&current),
                to_extended_length_path(&old),
            ) {
                Ok(_) => (item, "success", None),
                Err(e) => (item, "failed", Some(e.to_string())),
            }
        })
        .collect();

    // 批量更新数据库
    let now = Local::now().timestamp();
    let mut success = 0usize;
    let mut failed = 0usize;
    let mut skipped_missing = 0usize;
    let mut items = Vec::with_capacity(rollback_results.len());

    let updates: Vec<(i64, bool, Option<&str>)> = rollback_results
        .iter()
        .map(|(item, status, err)| {
            let (ok, err_msg) = match *status {
                "success" => (true, None),
                "skipped_missing" => (false, Some("SKIPPED_NOT_FOUND")),
                _ => (false, err.as_deref()),
            };
            (item.item_id, ok, err_msg)
        })
        .collect();

    suffix_repo::batch_update_rollback_results(db_path, &updates, now)?;

    for (item, status, err) in rollback_results {
        match status {
            "success" => success += 1,
            "skipped_missing" => skipped_missing += 1,
            _ => failed += 1,
        }
        items.push(SuffixApplyItem {
            item_id: item.item_id,
            old_path: item.old_path,
            new_path: item.new_path,
            status: if status == "success" {
                "success".into()
            } else {
                "failed".into()
            },
            message: match status {
                "skipped_missing" => Some("SKIPPED_NOT_FOUND".into()),
                "failed" => err,
                _ => None,
            },
        });
    }

    let status = if success > 0 && failed == 0 && skipped_missing == 0 {
        "rolled_back"
    } else if success > 0 {
        "partially_rolled_back"
    } else {
        "applied"
    };
    let _ = suffix_repo::set_record_rollback_status(db_path, record_id, status);

    Ok(SuffixRollbackResponse {
        record_id: record_id.to_string(),
        total_selected: check.total_selected,
        success,
        failed,
        skipped_missing,
        items,
    })
}

pub fn delete_suffix_record(db_path: &Path, record_id: &str) -> Result<(), String> {
    suffix_repo::delete_record(db_path, record_id)
}
