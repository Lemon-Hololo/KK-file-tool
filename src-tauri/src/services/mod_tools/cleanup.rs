//! Mod 重复 / 不同版本检查与删除。
//!
//! 分组规则对齐 `tmp/FindSameMod.java`：
//! - 重复 MOD：`guid + author + version` 相同且文件数大于 1；
//! - 不同版本 MOD：`guid + author` 相同且版本数大于 1。
//!
//! 检查任务采用两段式：
//! 1. 第一轮 WalkDir 只统计候选 `.zip/.zipmod` 数量；
//! 2. 第二轮按固定 chunk 分块并行读取 manifest，并把结果增量推给前端。
//!
//! 这样能避免同时持有"全量候选路径 + 全量解析结果 + 全量日志事件"。

use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{atomic::AtomicUsize, Arc},
    time::SystemTime,
};

use chrono::Local;
use rayon::prelude::*;
use tauri::AppHandle;
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    constants::{mod_op_kind, stages},
    error::{AppError, AppResult},
    models::{
        ModDuplicateGroup, ModDuplicatePartialPayload, ModIdentityFile, ModOpApplyResponse,
        ModVersionGroup, ModVersionPartialPayload, TaskStatus,
    },
    services::{
        events,
        logging::TaskLogContext,
        mod_tools::{
            zipmod::{is_zipmod, read_manifest_from_zip},
            MOD_OP_TABLES,
        },
        op_pipeline,
    },
    utils::path::{to_extended_length_path, to_user_friendly_path},
};

const PROCESS_CHUNK_SIZE: usize = 256;

/// 同步预览重复 MOD；保留给非长任务调用路径。
pub fn preview_mod_duplicates(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<ModDuplicateGroup>> {
    let runtime = TaskRuntime::default();
    let mut grouped: HashMap<(String, String, String), CollectSlot<ModIdentityFile>> =
        HashMap::new();

    process_candidates(db_path, paths, &runtime, log.as_ref(), |batch| {
        for file in batch {
            push_collect_slot(
                &mut grouped,
                (file.guid.clone(), file.author.clone(), file.version.clone()),
                file,
            );
        }
    })
    .map_err(AppError::Internal)?;

    let mut groups: Vec<ModDuplicateGroup> = grouped
        .into_iter()
        .filter_map(|((guid, author, version), slot)| {
            let mut files = slot.into_vec();
            if files.len() <= 1 {
                return None;
            }
            files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
            Some(ModDuplicateGroup {
                group_id: duplicate_group_id(&guid, &author, &version),
                guid,
                author,
                version,
                files,
            })
        })
        .collect();
    groups.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    Ok(groups)
}

/// 同步预览不同版本 MOD；保留给非长任务调用路径。
pub fn preview_mod_versions(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<ModVersionGroup>> {
    let runtime = TaskRuntime::default();
    let mut grouped: HashMap<(String, String), CollectSlot<ModIdentityFile>> = HashMap::new();

    process_candidates(db_path, paths, &runtime, log.as_ref(), |batch| {
        for file in batch {
            push_collect_slot(&mut grouped, (file.guid.clone(), file.author.clone()), file);
        }
    })
    .map_err(AppError::Internal)?;

    let mut groups: Vec<ModVersionGroup> = grouped
        .into_iter()
        .filter_map(|((guid, author), slot)| {
            let mut files = slot.into_vec();
            let versions: BTreeSet<String> =
                files.iter().map(|file| file.version.clone()).collect();
            if versions.len() <= 1 {
                return None;
            }
            files.sort_by(|a, b| {
                compare_versions(&b.version, &a.version)
                    .then_with(|| b.mtime.cmp(&a.mtime))
                    .then_with(|| a.file_path.cmp(&b.file_path))
            });
            let latest_version = files
                .first()
                .map(|file| file.version.clone())
                .unwrap_or_default();
            Some(ModVersionGroup {
                group_id: version_group_id(&guid, &author),
                guid,
                author,
                latest_version,
                files,
            })
        })
        .collect();
    groups.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    Ok(groups)
}

/// 删除重复 MOD 中选中的文件并写入可撤回记录。
pub fn apply_mod_duplicate_delete(
    db_path: &Path,
    paths: &[String],
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    apply_mod_delete(
        db_path,
        paths,
        selected_file_paths,
        record_name,
        mod_op_kind::DUPLICATE_DELETE,
        log,
    )
}

/// 删除不同版本 MOD 中选中的文件并写入可撤回记录。
pub fn apply_mod_version_delete(
    db_path: &Path,
    paths: &[String],
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    apply_mod_delete(
        db_path,
        paths,
        selected_file_paths,
        record_name,
        mod_op_kind::VERSION_DELETE,
        log,
    )
}

/// 后台运行重复 MOD 检查任务。
pub async fn run_duplicate_scan(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    runtime: Arc<TaskRuntime>,
) -> Result<(), String> {
    let log = TaskLogContext::new(&app, &task_id);
    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    log.info("开始检查重复 MOD");

    let total = count_candidates(&paths, &runtime);
    events::emit_progress(&app, &task_id, stages::MOD_DUPLICATE, 0, total);

    let mut grouped: HashMap<(String, String, String), CollectSlot<ModIdentityFile>> =
        HashMap::new();
    let mut processed = 0usize;

    process_candidates(&app_state.db_path, &paths, &runtime, Some(&log), |batch| {
        let mut touched = HashSet::new();
        for file in batch {
            processed += 1;
            touched.insert((file.guid.clone(), file.author.clone(), file.version.clone()));
            push_collect_slot(
                &mut grouped,
                (file.guid.clone(), file.author.clone(), file.version.clone()),
                file,
            );
        }

        let partial = touched
            .into_iter()
            .filter_map(|(guid, author, version)| {
                let slot = grouped.get(&(guid.clone(), author.clone(), version.clone()))?;
                let mut files = slot.clone().into_vec();
                if files.len() <= 1 {
                    return None;
                }
                files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
                Some(ModDuplicateGroup {
                    group_id: duplicate_group_id(&guid, &author, &version),
                    guid,
                    author,
                    version,
                    files,
                })
            })
            .collect::<Vec<_>>();

        if !partial.is_empty() {
            events::emit_mod_duplicate_partial(
                &app,
                &ModDuplicatePartialPayload {
                    task_id: task_id.clone(),
                    groups: partial,
                    done: false,
                },
            );
        }

        events::emit_progress(&app, &task_id, stages::MOD_DUPLICATE, processed, total);
    })?;

    if runtime.is_cancelled() {
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(&app, &task_id, "Cancelled");
        events::emit_mod_duplicate_partial(
            &app,
            &ModDuplicatePartialPayload {
                task_id: task_id.clone(),
                groups: vec![],
                done: true,
            },
        );
        app_state.tasks.lock().unwrap().remove(&task_id);
        return Ok(());
    }

    runtime.set_status(TaskStatus::Completed);
    events::emit_state_changed(&app, &task_id, "Completed");
    events::emit_mod_duplicate_partial(
        &app,
        &ModDuplicatePartialPayload {
            task_id: task_id.clone(),
            groups: vec![],
            done: true,
        },
    );
    log.info(&format!(
        "重复 MOD 检查完成：匹配 {} 组",
        count_duplicate_groups(&grouped)
    ));
    app_state.tasks.lock().unwrap().remove(&task_id);
    Ok(())
}

/// 后台运行不同版本 MOD 检查任务。
pub async fn run_version_scan(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    runtime: Arc<TaskRuntime>,
) -> Result<(), String> {
    let log = TaskLogContext::new(&app, &task_id);
    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    log.info("开始检查不同版本 MOD");

    let total = count_candidates(&paths, &runtime);
    events::emit_progress(&app, &task_id, stages::MOD_VERSION, 0, total);

    let mut grouped: HashMap<(String, String), CollectSlot<ModIdentityFile>> = HashMap::new();
    let mut processed = 0usize;

    process_candidates(&app_state.db_path, &paths, &runtime, Some(&log), |batch| {
        let mut touched = HashSet::new();
        for file in batch {
            processed += 1;
            touched.insert((file.guid.clone(), file.author.clone()));
            push_collect_slot(&mut grouped, (file.guid.clone(), file.author.clone()), file);
        }

        let partial = touched
            .into_iter()
            .filter_map(|(guid, author)| {
                let slot = grouped.get(&(guid.clone(), author.clone()))?;
                let mut files = slot.clone().into_vec();
                let versions: BTreeSet<String> =
                    files.iter().map(|file| file.version.clone()).collect();
                if versions.len() <= 1 {
                    return None;
                }

                files.sort_by(|a, b| {
                    compare_versions(&b.version, &a.version)
                        .then_with(|| b.mtime.cmp(&a.mtime))
                        .then_with(|| a.file_path.cmp(&b.file_path))
                });
                let latest_version = files
                    .first()
                    .map(|file| file.version.clone())
                    .unwrap_or_default();

                Some(ModVersionGroup {
                    group_id: version_group_id(&guid, &author),
                    guid,
                    author,
                    latest_version,
                    files,
                })
            })
            .collect::<Vec<_>>();

        if !partial.is_empty() {
            events::emit_mod_version_partial(
                &app,
                &ModVersionPartialPayload {
                    task_id: task_id.clone(),
                    groups: partial,
                    done: false,
                },
            );
        }

        events::emit_progress(&app, &task_id, stages::MOD_VERSION, processed, total);
    })?;

    if runtime.is_cancelled() {
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(&app, &task_id, "Cancelled");
        events::emit_mod_version_partial(
            &app,
            &ModVersionPartialPayload {
                task_id: task_id.clone(),
                groups: vec![],
                done: true,
            },
        );
        app_state.tasks.lock().unwrap().remove(&task_id);
        return Ok(());
    }

    runtime.set_status(TaskStatus::Completed);
    events::emit_state_changed(&app, &task_id, "Completed");
    events::emit_mod_version_partial(
        &app,
        &ModVersionPartialPayload {
            task_id: task_id.clone(),
            groups: vec![],
            done: true,
        },
    );
    log.info(&format!(
        "不同版本 MOD 检查完成：匹配 {} 组",
        count_version_groups(&grouped)
    ));
    app_state.tasks.lock().unwrap().remove(&task_id);
    Ok(())
}

fn count_candidates(paths: &[String], runtime: &TaskRuntime) -> usize {
    let mut total = 0usize;
    for root in paths {
        let ep = to_extended_length_path(Path::new(root));
        for entry in WalkDir::new(&ep).into_iter().filter_map(|item| item.ok()) {
            if runtime.is_cancelled() {
                return total;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if is_zipmod(&name) {
                total += 1;
            }
        }
    }
    total
}

fn process_candidates<F>(
    db_path: &Path,
    paths: &[String],
    runtime: &TaskRuntime,
    log: Option<&TaskLogContext>,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Vec<ModIdentityFile>),
{
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(op_pipeline::resolve_thread_count(db_path).max(1))
        .build()
        .map_err(|e| format!("创建线程池失败: {e}"))?;

    let counter = Arc::new(AtomicUsize::new(0));
    let mut chunk = Vec::with_capacity(PROCESS_CHUNK_SIZE);

    for root in paths {
        if let Some(log) = log {
            log.info(&format!("正在扫描路径: {root}"));
        }
        let ep = to_extended_length_path(Path::new(root));
        for entry in WalkDir::new(&ep).into_iter().filter_map(|item| item.ok()) {
            if runtime.is_cancelled() {
                return Ok(());
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if !is_zipmod(&name) {
                continue;
            }
            chunk.push(entry.path().to_path_buf());
            if chunk.len() >= PROCESS_CHUNK_SIZE {
                on_batch(process_chunk(
                    &pool,
                    std::mem::take(&mut chunk),
                    log,
                    &counter,
                ));
            }
        }
    }

    if !chunk.is_empty() {
        on_batch(process_chunk(&pool, chunk, log, &counter));
    }

    Ok(())
}

fn process_chunk(
    pool: &rayon::ThreadPool,
    chunk: Vec<PathBuf>,
    log: Option<&TaskLogContext>,
    counter: &AtomicUsize,
) -> Vec<ModIdentityFile> {
    pool.install(|| {
        chunk
            .into_par_iter()
            .filter_map(|path| read_identity_file(&path, log, counter))
            .collect()
    })
}

fn read_identity_file(
    path: &Path,
    log: Option<&TaskLogContext>,
    counter: &AtomicUsize,
) -> Option<ModIdentityFile> {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Some(log) = log {
        log.info_path("正在处理 MOD", path);
    }

    let (meta, _) = read_manifest_from_zip(path).ok()?;
    let ep = to_extended_length_path(path);
    let metadata = std::fs::metadata(&ep).ok()?;

    Some(ModIdentityFile {
        file_path: to_user_friendly_path(path),
        guid: meta.guid,
        version: meta.version,
        author: meta.author,
        size: metadata.len(),
        mtime: system_time_to_timestamp(metadata.modified().ok()),
        ctime: system_time_to_timestamp(metadata.created().ok()),
    })
}

fn apply_mod_delete(
    db_path: &Path,
    paths: &[String],
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    kind: &str,
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    if selected_file_paths.is_empty() {
        return Err(AppError::InvalidInput(
            "请先选择要删除的 MOD 文件".to_string(),
        ));
    }

    let ts = Local::now().timestamp();
    let pairs: Vec<(String, String)> = selected_file_paths
        .into_iter()
        .map(|path| {
            let backup = backup_path_for_delete(&path, ts);
            (path, backup)
        })
        .collect();

    if let Some(log) = &log {
        for (old_path, _) in &pairs {
            log.info(&format!("准备删除 Mod: {old_path}"));
        }
    }

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());
    op_pipeline::persist_apply_rename_pairs(db_path, MOD_OP_TABLES, kind, name, paths, pairs, false)
}

fn backup_path_for_delete(original: &str, ts: i64) -> String {
    let short = uuid::Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    format!("{original}.fileflow-del-{ts}-{short}")
}

fn system_time_to_timestamp(value: Option<SystemTime>) -> i64 {
    value
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts = version_parts(a);
    let b_parts = version_parts(b);
    let max_len = a_parts.len().max(b_parts.len());

    for i in 0..max_len {
        let a_part = a_parts.get(i).map(String::as_str).unwrap_or("0");
        let b_part = b_parts.get(i).map(String::as_str).unwrap_or("0");
        let ord = compare_version_part(a_part, b_part);
        if ord != Ordering::Equal {
            return ord;
        }
    }

    a.cmp(b)
}

fn version_parts(version: &str) -> Vec<String> {
    version
        .split(|c: char| !(c.is_ascii_alphanumeric()))
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn compare_version_part(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(aa), Ok(bb)) => aa.cmp(&bb),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

fn duplicate_group_id(guid: &str, author: &str, version: &str) -> String {
    format!("{guid}\u{1f}{author}\u{1f}{version}")
}

fn version_group_id(guid: &str, author: &str) -> String {
    format!("{guid}\u{1f}{author}")
}

fn count_duplicate_groups(
    grouped: &HashMap<(String, String, String), CollectSlot<ModIdentityFile>>,
) -> usize {
    grouped.values().filter(|slot| slot.len() > 1).count()
}

fn count_version_groups(
    grouped: &HashMap<(String, String), CollectSlot<ModIdentityFile>>,
) -> usize {
    grouped
        .values()
        .filter(|slot| slot.version_count() > 1)
        .count()
}

#[derive(Clone)]
enum CollectSlot<T> {
    One(T),
    Many(Vec<T>),
}

impl CollectSlot<ModIdentityFile> {
    fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(items) => items.len(),
        }
    }

    fn version_count(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(items) => items
                .iter()
                .map(|file| file.version.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
        }
    }

    fn into_vec(self) -> Vec<ModIdentityFile> {
        match self {
            Self::One(item) => vec![item],
            Self::Many(items) => items,
        }
    }
}

fn push_collect_slot<K>(
    map: &mut HashMap<K, CollectSlot<ModIdentityFile>>,
    key: K,
    file: ModIdentityFile,
) where
    K: std::cmp::Eq + std::hash::Hash,
{
    match map.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(CollectSlot::One(file));
        }
        Entry::Occupied(mut entry) => match entry.get_mut() {
            CollectSlot::One(existing) => {
                let first = existing.clone();
                *entry.get_mut() = CollectSlot::Many(vec![first, file]);
            }
            CollectSlot::Many(items) => {
                items.push(file);
            }
        },
    }
}
