//! 图片相似度去重的扫描长任务。
//!
//! 流水线：
//! 1. WalkDir 收集候选图片（按扩展名 + 文件大小预过滤），扫描阶段发未知 total
//!    的进度事件；
//! 2. 按动态 chunk（随线程数扩张）用局部 rayon 池并行计算图片尺寸 + 感知哈希；
//!    池线程数 = `thread_count × io_multiplier`，与 dedup 对齐——图片解码 + 哈希
//!    混合 CPU/IO，超额订阅 worker 让磁盘 IO 等待时仍有任务在跑；
//! 3. **每张图哈希完成时**通过共享 `AtomicUsize` 累加 `done` 并 emit 一次
//!    `image_dedup_hash` 进度事件，进度条按张数平滑增长（之前是每 chunk 才发一次，
//!    小图库一两个 chunk 就跑完，进度条无法可见地增长）；同时通过 `TaskLogContext`
//!    打一行"哈希计算完成 path"日志（解析失败 / 尺寸过滤的会带"跳过"标签），日志
//!    与进度同时机发出，让实时日志面板能看到每张图的处理结果；
//! 4. 每个 chunk 处理完把本批次结果喂给分组逻辑——把每个新哈希与已有 head 比较
//!    Hamming 距离，命中阈值即并入；head 很多时用同一个局部 rayon 池并行查找；
//! 5. 每个 chunk 完成后发一次 `image_dedup_partial`（仅含本批次 touched 组），
//!    任务终态发一次 `done = true` 的空 partial 让前端关闭"扫描中"。
//!
//! # 关于分组算法
//! 分组使用"first-similar-head"启发式：每个新图片只跟"现有组的代表哈希"比距离，
//! 命中即并入第一个匹配组——非严格的等价类，但避免 O(N²) 全两两比较。在
//! 50K+ 图片 / N 组的实际负载下足够好；后续真有大量"链式相似"组导致拆分严重，
//! 可以再换 BK-tree / LSH。
//!
//! 取消语义：候选阶段每条 entry 检查；chunk 阶段每个 chunk 边界检查。

use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        Arc,
    },
};

use image_hasher::{Hasher, ImageHash};
use rayon::prelude::*;
use tauri::AppHandle;
use uuid::Uuid;
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    constants::{image_keep_policy, stages},
    db::settings_repo,
    error::AppResult,
    models::{ImageDedupGroup, ImageDedupPartialPayload, ImageHashFile, TaskStatus},
    services::{
        events,
        image_dedup::hash::{self, HashedImageFile, ImageHashConfig},
        logging::TaskLogContext,
        op_pipeline,
    },
    utils::{path::to_extended_length_path, time::system_time_to_secs},
};

const MIN_PROCESS_CHUNK_SIZE: usize = 256;
const PROCESS_CHUNK_PER_THREAD: usize = 16;
const MAX_PROCESS_CHUNK_SIZE: usize = 2048;
const SCAN_PROGRESS_INTERVAL: usize = 256;
const HEAD_SEARCH_PARALLEL_MIN: usize = 1024;

/// 后台运行图片相似度扫描任务（结果通过 `image_dedup_partial` 增量推送）。
pub async fn run_image_dedup_scan(
    app: AppHandle,
    app_state: Arc<AppState>,
    task_id: String,
    paths: Vec<String>,
    runtime: Arc<TaskRuntime>,
) -> Result<(), String> {
    let log = TaskLogContext::new(&app, &task_id);
    runtime.set_status(TaskStatus::Running);
    events::emit_state_changed(&app, &task_id, "Running");
    log.info("开始扫描相似图片");

    let cfg = load_image_hash_config(&app_state.db_path).map_err(|e| e.to_string())?;
    let keep_policy = read_keep_policy(&app_state.db_path);
    let threshold_pct = read_threshold(&app_state.db_path);
    let max_distance = compute_max_distance(cfg.hash_size, threshold_pct);
    log.info(&format!(
        "算法 {} / 哈希边长 {} / 阈值 {}% / 允许 Hamming 距离 ≤ {} bit",
        cfg.algorithm, cfg.hash_size, threshold_pct, max_distance
    ));

    let total_bits = hash::total_bits(cfg.hash_size);
    let hasher = Arc::new(hash::build_hasher(&cfg));
    let thread_count = op_pipeline::resolve_thread_count(&app_state.db_path);
    // 图片解码 + 哈希混合 CPU/IO（image::open 读盘 + 解码、hash_image 算 bit），
    // 与 dedup 同理：用 IO 倍率超额订阅 worker，让磁盘 IO 等待期间总有任务在跑。
    let io_multiplier = op_pipeline::resolve_io_concurrency_multiplier(&app_state.db_path);
    let concurrency = (thread_count * io_multiplier).max(1);
    let chunk_size = process_chunk_size(concurrency);
    let pool = op_pipeline::rayon_pool(concurrency).map_err(|e| e.to_string())?;
    log.info(&format!(
        "并行线程 {thread_count} × IO 倍率 {io_multiplier} = {concurrency} / 每批最多 {chunk_size} 张图片"
    ));
    events::emit_progress(&app, &task_id, stages::SCAN, 0, 0);

    let mut state = GroupingState::new(max_distance, total_bits);

    process_candidates(
        &app,
        &task_id,
        &paths,
        &cfg,
        hasher,
        &pool,
        chunk_size,
        &runtime,
        Some(&log),
        |found| {
            events::emit_progress(&app, &task_id, stages::SCAN, found, 0);
        },
        |batch| {
            // 本批每张图的 progress 已在 process_chunk 内 per-item 发，这里只做分组与 partial。
            let mut touched_ids: Vec<String> = Vec::new();
            for file in batch {
                let id = state.absorb(file, &keep_policy, &pool);
                if !touched_ids.contains(&id) {
                    touched_ids.push(id);
                }
            }

            let partial: Vec<ImageDedupGroup> = touched_ids
                .into_iter()
                .filter_map(|id| state.snapshot_group(&id))
                .collect();

            if !partial.is_empty() {
                events::emit_image_dedup_partial(
                    &app,
                    &ImageDedupPartialPayload {
                        task_id: task_id.clone(),
                        groups: partial,
                        done: false,
                    },
                );
            }
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

    events::emit_image_dedup_partial(
        &app,
        &ImageDedupPartialPayload {
            task_id: task_id.clone(),
            groups: vec![],
            done: true,
        },
    );

    if !cancelled {
        let group_count = state.committed_group_count();
        log.info(&format!(
            "图片相似度去重扫描完成：发现 {group_count} 组相似图片"
        ));
    }
    app_state.remove_task(&task_id);
    Ok(())
}

#[derive(Debug, Clone)]
struct CandidateImage {
    path: PathBuf,
    file_size: u64,
    mtime: i64,
    ctime: i64,
}

fn process_candidates<F, S>(
    app: &AppHandle,
    task_id: &str,
    paths: &[String],
    cfg: &ImageHashConfig,
    hasher: Arc<Hasher>,
    pool: &rayon::ThreadPool,
    chunk_size: usize,
    runtime: &TaskRuntime,
    log: Option<&TaskLogContext>,
    mut on_scan_progress: S,
    mut on_batch: F,
) -> Result<(), String>
where
    F: FnMut(Vec<HashedImageFile>),
    S: FnMut(usize),
{
    // 第一遍：收集候选。这里读 metadata 做文件大小预过滤，避免把明显不参与
    // 的缩略图继续送进解码/哈希阶段，同时让 hash 阶段 total 更贴近真实工作量。
    let mut candidates: Vec<CandidateImage> = Vec::new();
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
            if !hash::ext_matches(entry.path(), &cfg.extensions) {
                continue;
            }
            let metadata = match std::fs::metadata(entry.path()) {
                Ok(m) => m,
                Err(e) => {
                    if let Some(log) = log {
                        log.warn_path(&format!("读取元数据失败: {e}"), entry.path());
                    }
                    continue;
                }
            };
            if !metadata.is_file() || !hash::size_matches(metadata.len(), cfg.min_file_size_kb) {
                continue;
            }
            let mtime = system_time_to_secs(metadata.modified().ok());
            let ctime_raw = system_time_to_secs(metadata.created().ok());
            let ctime = if ctime_raw == 0 { mtime } else { ctime_raw };
            candidates.push(CandidateImage {
                path: entry.path().to_path_buf(),
                file_size: metadata.len(),
                mtime,
                ctime,
            });
            if candidates.len() % SCAN_PROGRESS_INTERVAL == 0 {
                on_scan_progress(candidates.len());
            }
        }
    }

    let total = candidates.len();
    on_scan_progress(total);
    if total == 0 {
        return Ok(());
    }
    if let Some(log) = log {
        log.info(&format!("候选图片 {total} 张，开始并行计算感知哈希"));
    }

    // 初始化哈希阶段进度（0/total）让前端进度条立刻切到"哈希与分组"。
    events::emit_progress(app, task_id, stages::IMAGE_DEDUP_HASH, 0, total);

    // per-item 共享计数器：chunk 内并发的 worker 每完成一张就 fetch_add 并 emit 进度。
    let done_counter = AtomicUsize::new(0);
    let mut iter = candidates.into_iter();
    loop {
        if runtime.is_cancelled() {
            return Ok(());
        }
        let chunk: Vec<CandidateImage> = iter.by_ref().take(chunk_size).collect();
        if chunk.is_empty() {
            break;
        }
        let result = process_chunk(
            pool,
            chunk,
            cfg,
            hasher.clone(),
            app,
            task_id,
            log,
            &done_counter,
            total,
        );
        on_batch(result);
    }

    Ok(())
}

fn process_chunk(
    pool: &rayon::ThreadPool,
    chunk: Vec<CandidateImage>,
    cfg: &ImageHashConfig,
    hasher: Arc<Hasher>,
    app: &AppHandle,
    task_id: &str,
    log: Option<&TaskLogContext>,
    done_counter: &AtomicUsize,
    total: usize,
) -> Vec<HashedImageFile> {
    pool.install(|| {
        chunk
            .into_par_iter()
            .filter_map(|candidate| {
                let result = hash::hash_one(
                    &candidate.path,
                    candidate.file_size,
                    candidate.mtime,
                    candidate.ctime,
                    &hasher,
                    cfg,
                )
                .ok()
                .flatten();
                // 即使是 None（解析失败 / 尺寸过小）也算"完成一次哈希尝试"，
                // 进度条按候选 total 而不是成功 total 推进，匹配预期 ETA。
                let done = done_counter.fetch_add(1, AtomicOrdering::Relaxed) + 1;
                events::emit_progress(app, task_id, stages::IMAGE_DEDUP_HASH, done, total);
                // 实时日志：哈希完成时发，时机与进度推进一致；解析失败的也带"跳过"标签
                // 区分开，让用户能在日志里看出哪些图被算了、哪些被丢了。
                if let Some(log) = log {
                    let label = if result.is_some() {
                        "哈希计算完成"
                    } else {
                        "哈希计算完成（跳过：解析失败或尺寸过小）"
                    };
                    log.info_path(label, &candidate.path);
                }
                result
            })
            .collect()
    })
}

fn process_chunk_size(thread_count: usize) -> usize {
    (thread_count.max(1) * PROCESS_CHUNK_PER_THREAD)
        .clamp(MIN_PROCESS_CHUNK_SIZE, MAX_PROCESS_CHUNK_SIZE)
}

/// 滚动分组状态：新文件一进来就找现有组的 head 比距离，命中即并入。
struct GroupingState {
    max_distance: u32,
    total_bits: u32,
    /// 全部组的"代表哈希"——加入新成员后不变，保持比较稳定性。
    heads: Vec<(String, ImageHash)>,
    /// 组成员（含 head 自身）。第一个元素是 keep（按 keep_policy 排序后的最优）。
    groups: Vec<(String, Vec<HashedImageFile>)>,
}

impl GroupingState {
    fn new(max_distance: u32, total_bits: u32) -> Self {
        Self {
            max_distance,
            total_bits,
            heads: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// 把一张图加入分组：找最近 head（距离 ≤ max）→ 加入；找不到 → 新建组。
    /// 返回受影响的 group_id（用于增量推送）。
    fn absorb(
        &mut self,
        file: HashedImageFile,
        keep_policy: &str,
        pool: &rayon::ThreadPool,
    ) -> String {
        if let Some(idx) = self.find_matching_head(&file.image_hash, pool) {
            let id = self.groups[idx].0.clone();
            let group = &mut self.groups[idx].1;
            group.push(file);
            sort_group(group, keep_policy);
            return id;
        }

        // 没匹配 → 新建组。
        let id = Uuid::new_v4().to_string();
        self.heads.push((id.clone(), file.image_hash.clone()));
        self.groups.push((id.clone(), vec![file]));
        id
    }

    fn find_matching_head(
        &self,
        image_hash: &ImageHash,
        pool: &rayon::ThreadPool,
    ) -> Option<usize> {
        if self.heads.len() < HEAD_SEARCH_PARALLEL_MIN {
            return self
                .heads
                .iter()
                .position(|(_, head)| head.dist(image_hash) <= self.max_distance);
        }

        pool.install(|| {
            self.heads
                .par_iter()
                .enumerate()
                .find_first(|item| {
                    let (_, (_, head)) = item;
                    head.dist(image_hash) <= self.max_distance
                })
                .map(|(idx, _)| idx)
        })
    }

    /// 取出某个 group 的当前快照——只在文件数 ≥ 2 时返回（单文件不构成"重复组"）。
    fn snapshot_group(&self, group_id: &str) -> Option<ImageDedupGroup> {
        let (_, files) = self.groups.iter().find(|(id, _)| id == group_id)?;
        if files.len() < 2 {
            return None;
        }
        // 组内"相似度"：组里距离 head 最远的那个 → 估算为整组的最低相似度。
        let head_hash = self.heads.iter().find(|(id, _)| id == group_id)?.1.clone();
        let mut max_dist = 0u32;
        for f in files {
            let d = head_hash.dist(&f.image_hash);
            if d > max_dist {
                max_dist = d;
            }
        }
        let similarity = if self.total_bits == 0 {
            100
        } else {
            let pct = 100.0 - (max_dist as f64 / self.total_bits as f64) * 100.0;
            pct.round().clamp(0.0, 100.0) as u32
        };

        Some(ImageDedupGroup {
            group_id: group_id.to_string(),
            similarity,
            files: files.iter().map(|x| x.file.clone()).collect(),
        })
    }

    fn committed_group_count(&self) -> usize {
        self.groups.iter().filter(|(_, f)| f.len() >= 2).count()
    }
}

/// 从 db 读 `AppSettings` 后构造哈希配置。
fn load_image_hash_config(db_path: &Path) -> AppResult<ImageHashConfig> {
    let s = settings_repo::get_settings(db_path)?;
    Ok(ImageHashConfig {
        algorithm: s.image_dedup_algorithm,
        hash_size: s.image_dedup_hash_size.max(4) as u32,
        extensions: s.image_dedup_extensions,
        min_file_size_kb: s.image_dedup_min_file_size_kb.max(0) as u32,
        min_dimension: s.image_dedup_min_dimension.max(0) as u32,
    })
}

fn read_keep_policy(db_path: &Path) -> String {
    settings_repo::get_settings(db_path)
        .map(|s| s.image_dedup_keep_policy)
        .unwrap_or_else(|_| image_keep_policy::LARGEST_RESOLUTION.to_string())
}

fn read_threshold(db_path: &Path) -> u32 {
    settings_repo::get_settings(db_path)
        .map(|s| s.image_dedup_similarity_threshold.clamp(0, 100) as u32)
        .unwrap_or(90)
}

/// 阈值百分比 → 允许的 Hamming 距离（向下取整）。
fn compute_max_distance(hash_size: u32, threshold_pct: u32) -> u32 {
    let bits = hash::total_bits(hash_size.max(4));
    let allowed_ratio = (100u32.saturating_sub(threshold_pct)) as f64 / 100.0;
    (bits as f64 * allowed_ratio).floor() as u32
}

/// 按 keep_policy 把组排序，让 `files[0]` 作为默认 keep。
fn sort_group(files: &mut [HashedImageFile], keep_policy: &str) {
    files.sort_by(|a, b| compare_for_keep(&a.file, &b.file, keep_policy));
}

fn compare_for_keep(a: &ImageHashFile, b: &ImageHashFile, policy: &str) -> Ordering {
    match policy {
        image_keep_policy::LARGEST_FILE => b.file_size.cmp(&a.file_size),
        image_keep_policy::NEWEST => b.mtime.cmp(&a.mtime),
        image_keep_policy::OLDEST => a.mtime.cmp(&b.mtime),
        // largestResolution 与未知值都按"宽 × 高"降序——默认策略。
        _ => {
            let area_a = (a.width as u64) * (a.height as u64);
            let area_b = (b.width as u64) * (b.height as u64);
            area_b
                .cmp(&area_a)
                // tie-breaker：分辨率相同时按文件大小降序，多半是更高质量编码。
                .then_with(|| b.file_size.cmp(&a.file_size))
        }
    }
}
