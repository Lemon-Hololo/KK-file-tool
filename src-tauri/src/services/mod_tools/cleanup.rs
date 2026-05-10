//! Mod 重复 / 不同版本检查与删除。
//!
//! 分组规则对齐 `tmp/FindSameMod.java`：
//! - 重复 MOD：`guid + author + version` 相同且文件数大于 1；
//! - 不同版本 MOD：`guid + author` 相同且版本数大于 1。
//!
//! 检查任务采用单次 WalkDir：
//! 1. 收集候选 `.zip/.zipmod` 路径到内存（仅 PathBuf，不读 manifest）；
//! 2. 把 `len()` 作为进度的 total，按固定 chunk 分块并行读取 manifest；
//! 3. 每个 chunk 处理完立刻聚合分组 map，并通过增量事件推给前端。
//!
//! 这样既避免同时持有"全量解析结果 + 全量日志事件"，也避免之前"先扫一遍数总数、
//! 再扫一遍读 manifest"重复 IO 的浪费。
//!
//! 重复 / 不同版本两条业务的扫描骨架结构完全相同，差别只在三处：分组 key、
//! "本组是否成立"的判定（重复 ≥ 2 个文件 / 版本 > 1）、聚合后的展示结构。
//! 抽象为 [`GroupSpec`] trait 后，长任务函数 [`run_grouped_scan`] 与同步预览
//! [`preview_grouped`] 全部共享同一份实现，避免两份近乎相同的 200+ 行代码漂移。

use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, BTreeSet, HashMap, HashSet},
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};

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
            backup,
            zipmod::{is_zipmod, read_manifest_from_zip},
            MOD_OP_TABLES,
        },
        op_pipeline,
    },
    utils::{
        path::{to_extended_length_path, to_user_friendly_path},
        time::system_time_to_secs,
    },
};

const PROCESS_CHUNK_SIZE: usize = 256;

/// 描述一类"按 manifest 字段分组"的扫描业务的差异点。
///
/// `Key`：分组键（重复 = `(guid, author, version)`，版本 = `(guid, author)`）。
/// `Group`：发到前端的分组结构（重复 = [`ModDuplicateGroup`]，版本 = [`ModVersionGroup`]）。
trait GroupSpec {
    type Key: Clone + Eq + Hash;
    type Group: Clone;

    /// 从 manifest 字段计算分组 key。
    fn key(file: &ModIdentityFile) -> Self::Key;

    /// 把累积的若干文件 finalize 成对外的 group；不成立时返回 None（重复要求 ≥ 2 个文件、
    /// 版本要求 ≥ 2 个不同版本号）。返回的 `Vec` 内部已按业务约定排序。
    fn finalize(key: Self::Key, files: Vec<ModIdentityFile>) -> Option<Self::Group>;

    /// 取出对外 group 的 group_id，用作排序与稳定标识。
    fn group_id(group: &Self::Group) -> &str;

    /// 推送本批增量结果给前端。
    fn emit_partial(app: &AppHandle, task_id: &str, groups: Vec<Self::Group>, done: bool);

    /// 进度阶段标识（`stages::MOD_DUPLICATE` / `stages::MOD_VERSION`）。
    fn stage() -> &'static str;

    /// 任务完成后给日志面板的总结串。
    fn completion_summary(group_count: usize) -> String;

    /// 当前 slot 是否已经"成立"——用作 `count_groups` 决定日志面板显示的 N。
    fn slot_is_group(slot: &CollectSlot<ModIdentityFile>) -> bool;
}

struct DuplicateSpec;
struct VersionSpec;

impl GroupSpec for DuplicateSpec {
    type Key = (String, String, String);
    type Group = ModDuplicateGroup;

    fn key(file: &ModIdentityFile) -> Self::Key {
        (file.guid.clone(), file.author.clone(), file.version.clone())
    }

    fn finalize((guid, author, version): Self::Key, mut files: Vec<ModIdentityFile>) -> Option<Self::Group> {
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
    }

    fn group_id(group: &Self::Group) -> &str {
        &group.group_id
    }

    fn emit_partial(app: &AppHandle, task_id: &str, groups: Vec<Self::Group>, done: bool) {
        events::emit_mod_duplicate_partial(
            app,
            &ModDuplicatePartialPayload {
                task_id: task_id.to_string(),
                groups,
                done,
            },
        );
    }

    fn stage() -> &'static str {
        stages::MOD_DUPLICATE
    }

    fn completion_summary(group_count: usize) -> String {
        format!("重复 MOD 检查完成：匹配 {group_count} 组")
    }

    fn slot_is_group(slot: &CollectSlot<ModIdentityFile>) -> bool {
        slot.len() > 1
    }
}

impl GroupSpec for VersionSpec {
    type Key = (String, String);
    type Group = ModVersionGroup;

    fn key(file: &ModIdentityFile) -> Self::Key {
        (file.guid.clone(), file.author.clone())
    }

    fn finalize((guid, author): Self::Key, mut files: Vec<ModIdentityFile>) -> Option<Self::Group> {
        let versions: BTreeSet<String> = files.iter().map(|f| f.version.clone()).collect();
        if versions.len() <= 1 {
            return None;
        }
        files.sort_by(|a, b| {
            compare_versions(&b.version, &a.version)
                .then_with(|| b.mtime.cmp(&a.mtime))
                .then_with(|| a.file_path.cmp(&b.file_path))
        });
        let latest_version = files.first().map(|f| f.version.clone()).unwrap_or_default();
        Some(ModVersionGroup {
            group_id: version_group_id(&guid, &author),
            guid,
            author,
            latest_version,
            files,
        })
    }

    fn group_id(group: &Self::Group) -> &str {
        &group.group_id
    }

    fn emit_partial(app: &AppHandle, task_id: &str, groups: Vec<Self::Group>, done: bool) {
        events::emit_mod_version_partial(
            app,
            &ModVersionPartialPayload {
                task_id: task_id.to_string(),
                groups,
                done,
            },
        );
    }

    fn stage() -> &'static str {
        stages::MOD_VERSION
    }

    fn completion_summary(group_count: usize) -> String {
        format!("不同版本 MOD 检查完成：匹配 {group_count} 组")
    }

    fn slot_is_group(slot: &CollectSlot<ModIdentityFile>) -> bool {
        slot.version_count() > 1
    }
}

/// 同步预览重复 MOD；保留给非长任务调用路径。
pub fn preview_mod_duplicates(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<ModDuplicateGroup>> {
    preview_grouped::<DuplicateSpec>(db_path, paths, log)
}

/// 同步预览不同版本 MOD；保留给非长任务调用路径。
pub fn preview_mod_versions(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<ModVersionGroup>> {
    preview_grouped::<VersionSpec>(db_path, paths, log)
}

/// 同步预览的通用实现：扫候选 → 解析 manifest → 一次性 finalize 全部 slot。
fn preview_grouped<S: GroupSpec>(
    db_path: &Path,
    paths: &[String],
    log: Option<TaskLogContext>,
) -> AppResult<Vec<S::Group>> {
    let runtime = TaskRuntime::default();
    let mut grouped: HashMap<S::Key, CollectSlot<ModIdentityFile>> = HashMap::new();

    process_candidates(
        db_path,
        paths,
        &runtime,
        log.as_ref(),
        |batch, _processed| {
            for file in batch {
                push_collect_slot(&mut grouped, S::key(&file), file);
            }
        },
    )
    .map_err(AppError::Internal)?;

    let mut groups: Vec<S::Group> = grouped
        .into_iter()
        .filter_map(|(key, slot)| S::finalize(key, slot.into_vec()))
        .collect();
    groups.sort_by(|a, b| S::group_id(a).cmp(S::group_id(b)));
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
    run_grouped_scan::<DuplicateSpec>(app, app_state, task_id, paths, runtime, "开始检查重复 MOD")
        .await
}

/// 后台运行不同版本 MOD 检查任务。
pub async fn run_version_scan(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    runtime: Arc<TaskRuntime>,
) -> Result<(), String> {
    run_grouped_scan::<VersionSpec>(app, app_state, task_id, paths, runtime, "开始检查不同版本 MOD")
        .await
}

/// 通用分组扫描长任务实现：扫候选 → 分块解析 manifest → 增量推送 partial → 终态收尾。
///
/// 取消语义：候选阶段每条 entry 检查、分块阶段每个 chunk 边界检查；
/// 取消后会发一次 `done=true` 的空 partial 让前端关闭 running 状态。
async fn run_grouped_scan<S: GroupSpec>(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    runtime: Arc<TaskRuntime>,
    start_msg: &str,
) -> Result<(), String> {
    let log = TaskLogContext::new(&app, &task_id);
    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    log.info(start_msg);

    let mut grouped: HashMap<S::Key, CollectSlot<ModIdentityFile>> = HashMap::new();

    process_candidates(
        &app_state.db_path,
        &paths,
        &runtime,
        Some(&log),
        |batch, processed| {
            // 本批次涉及的所有 key 集合：partial 只重新构造这些组，不扫整张 map。
            let mut touched: HashSet<S::Key> = HashSet::new();
            for file in batch {
                let key = S::key(&file);
                touched.insert(key.clone());
                push_collect_slot(&mut grouped, key, file);
            }

            let partial: Vec<S::Group> = touched
                .into_iter()
                .filter_map(|key| {
                    let slot = grouped.get(&key)?;
                    S::finalize(key, slot.clone().into_vec())
                })
                .collect();

            if !partial.is_empty() {
                S::emit_partial(&app, &task_id, partial, false);
            }

            events::emit_progress(&app, &task_id, S::stage(), processed.done, processed.total);
        },
    )?;

    let cancelled = runtime.is_cancelled();
    if cancelled {
        runtime.set_status(TaskStatus::Cancelled);
        events::emit_state_changed(&app, &task_id, "Cancelled");
    } else {
        runtime.set_status(TaskStatus::Completed);
        events::emit_state_changed(&app, &task_id, "Completed");
    }
    S::emit_partial(&app, &task_id, vec![], true);
    if !cancelled {
        let count = grouped.values().filter(|s| S::slot_is_group(s)).count();
        log.info(&S::completion_summary(count));
    }
    app_state.remove_task(&task_id);
    Ok(())
}

/// 处理进度。`done` 表示已发起 manifest 解析的文件数（不区分成功/失败），
/// `total` 是预先收集到的候选总数；用作进度事件载荷。
#[derive(Debug, Clone, Copy)]
pub struct ProcessProgress {
    pub done: usize,
    pub total: usize,
}

/// 单次 WalkDir 收集所有 `.zip/.zipmod` 候选；用 `len()` 作为进度 total，
/// 然后按 chunk 并行解析 manifest，每个 chunk 完成后回调 `on_batch`，
/// 由调用方决定如何聚合 / 推送增量结果。
///
/// 取消语义：扫描候选阶段每条 entry 检查 `runtime.is_cancelled()`；
/// 处理阶段每个 chunk 边界检查；已经在 rayon 中调度的 manifest 解析会跑完。
fn process_candidates<F>(
    db_path: &Path,
    paths: &[String],
    runtime: &TaskRuntime,
    log: Option<&TaskLogContext>,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Vec<ModIdentityFile>, ProcessProgress),
{
    // 第一遍：收集候选 PathBuf。仅做轻量过滤（is_file + is_zipmod），
    // 不读取 manifest，避免占用过多内存。
    let mut candidates: Vec<PathBuf> = Vec::new();
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
            candidates.push(entry.path().to_path_buf());
        }
    }

    let total = candidates.len();
    if total == 0 {
        return Ok(());
    }

    let pool = op_pipeline::rayon_pool(op_pipeline::resolve_thread_count(db_path))
        .map_err(|e| e.to_string())?;

    // 第二遍：分块并行解析 manifest，每块完成后立刻回调聚合。
    let mut done = 0usize;
    let mut iter = candidates.into_iter();
    loop {
        if runtime.is_cancelled() {
            return Ok(());
        }
        let chunk: Vec<PathBuf> = iter.by_ref().take(PROCESS_CHUNK_SIZE).collect();
        if chunk.is_empty() {
            break;
        }
        let chunk_len = chunk.len();
        let result = process_chunk(&pool, chunk, log);
        done += chunk_len;
        on_batch(result, ProcessProgress { done, total });
    }

    Ok(())
}

fn process_chunk(
    pool: &rayon::ThreadPool,
    chunk: Vec<PathBuf>,
    log: Option<&TaskLogContext>,
) -> Vec<ModIdentityFile> {
    pool.install(|| {
        chunk
            .into_par_iter()
            .filter_map(|path| read_identity_file(&path, log))
            .collect()
    })
}

fn read_identity_file(path: &Path, log: Option<&TaskLogContext>) -> Option<ModIdentityFile> {
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
        mtime: system_time_to_secs(metadata.modified().ok()),
        ctime: system_time_to_secs(metadata.created().ok()),
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

    // 读 settings → 生成 record_id → 构造 (原路径, 备份路径或空串) 列表，全部交给
    // backup::prepare_mod_backup 处理；同批次同名 zipmod 的撞名 + 用户开启
    // "保留源目录结构"时的子目录布局也由它解决。
    let prepared = backup::prepare_mod_backup(db_path, selected_file_paths, paths)?;

    if let Some(log) = &log {
        // N 条选中 = N 条日志；之前是逐条 info！，对 1w 选择明显刷屏。改为一条总结 +
        // 抽样若干条，体现规模即可，详情走记录详情页。
        let total = prepared.pairs.len();
        let action = if prepared.rollback_enabled {
            "删除并备份"
        } else {
            "直接删除（不备份）"
        };
        log.info(&format!("准备{action} {total} 个 Mod"));
        const SAMPLE: usize = 5;
        for (old_path, _) in prepared.pairs.iter().take(SAMPLE) {
            log.info(&format!("  · {old_path}"));
        }
        if total > SAMPLE {
            log.info(&format!("  …（其余 {} 条略，详情见记录）", total - SAMPLE));
        }
    }

    let executor = move |old: &str, new: &str| -> Result<(), String> {
        if new.is_empty() {
            // 关闭回滚 → 真删
            std::fs::remove_file(to_extended_length_path(Path::new(old))).map_err(|e| e.to_string())
        } else {
            op_pipeline::rename_or_copy_delete(Path::new(old), Path::new(new))
        }
    };

    let name = op_pipeline::record_name_or_timestamp(record_name);
    op_pipeline::persist_apply_with_executor(
        db_path,
        MOD_OP_TABLES,
        &prepared.record_id,
        prepared.rollback_enabled,
        kind,
        name,
        paths,
        prepared.pairs,
        executor,
    )
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
