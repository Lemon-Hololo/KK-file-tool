//! Pixiv 标签整理业务逻辑。
//!
//! 负责：
//! 1. 从任务输入文件夹同步扫描出含 PID 的图片（[`scan_pixiv_images`]）；
//! 2. 长任务并发拉取每个 PID 的 tag 列表，分批通过 `pixiv_tag_partial` 增量推回（[`run_pixiv_tag_scan`]）；
//! 3. 单条 PID 的同步重试拉取（[`fetch_pixiv_tag_single`]）；
//! 4. 把图片移动到 `<output>/<sanitized_tag>/<basename>`（[`move_image_by_tag`]）。
//!
//! 实现要点：
//! - HTTP：`reqwest` + `Referer: https://www.pixiv.net/` + 浏览器 UA + 可选 Cookie。
//! - 并发：`tokio::sync::Semaphore`，许可数 = `min(thread × io_mult, 8)`。
//!   Pixiv 对 ajax 接口存在限流，硬上限 8 已足够吃满本地带宽，再高也只是触发 429。
//! - 取消：通过 [`crate::app_state::TaskRuntime::is_cancelled`] 读取，每个并发任务起手与拿到 permit 后双重检查。
//! - 终态：无论成功 / 失败 / 取消，最后必须发一条 `done = true` 的 partial 并把任务从
//!   `AppState` 中移除，否则前端的 running 状态永远卡住、HashMap 也会泄漏。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::{mpsc, Mutex as AsyncMutex, Semaphore};
use walkdir::WalkDir;

use crate::{
    app_state::{AppState, TaskRuntime},
    constants::{log_level, stages},
    db::settings_repo,
    error::{AppError, AppResult},
    models::{PixivImageRow, PixivTagFetchResult, PixivTagPartialItem, PixivTagPartialPayload},
    services::{events, op_pipeline},
    utils::{
        filename::{resolve_conflict_with_reserved, sanitize_filename, ILLEGAL_FILENAME_CHARS},
        path::{to_extended_length_path, to_user_friendly_path},
    },
};

/// 支持的图片扩展名（小写比较）。
const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// 单次拉取 PID 的并发硬上限，避免触发 Pixiv 限流。
const FETCH_CONCURRENCY_HARD_CAP: usize = 8;

/// HTTP 请求超时（连接 + 整体）。
const HTTP_TIMEOUT_SECS: u64 = 20;

/// 增量批次最大条数：满则立即 flush。
const PARTIAL_FLUSH_BATCH: usize = 30;

/// 增量批次最大延迟：超过此时间也立即 flush（让 UI 即使在慢响应下也能感受到进度）。
const PARTIAL_FLUSH_INTERVAL: Duration = Duration::from_millis(250);

/// 浏览器 UA：Pixiv ajax 对 UA 做粗略筛查，curl 默认 UA 会被拒。
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Pixiv ajax 在浏览器请求里会带语言偏好；补上它能让 tag 翻译字段更接近页面实际返回。
const ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en;q=0.8,ja;q=0.7";

/// PID 提取正则：8~9 位连续数字，前后是非数字（用 `(?:^|\D)` / `(?:$|\D)` 模拟单词边界，
/// 因为 `\b` 在 Unicode 模式下对 ASCII 数字仍然成立，但显式更稳）。
fn pid_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\D)(\d{8,9})(?:\D|$)").unwrap())
}

/// 进程级共享的"下一次允许发请求的时刻"。
///
/// 长任务和单条重试都会去 [`acquire_rate_slot`] 排队领号；这样并发 worker、
/// 后台跑的长任务、用户手点的重试不会各自把 60 req/min 各吃一份导致总速率
/// 翻倍——共享的 next-slot 把整体钉死在 `60s / per_minute` 的间隔上。
///
/// `Mutex` 只在"读 + 写下次时刻"那一瞬持有，sleep 在锁外完成；
/// 所以哪怕 1000 个 worker 排队也不会卡死锁。
fn next_request_slot() -> &'static AsyncMutex<Instant> {
    static SLOT: OnceLock<AsyncMutex<Instant>> = OnceLock::new();
    SLOT.get_or_init(|| AsyncMutex::new(Instant::now()))
}

/// 在共享 next-slot 上排队领一个发送时刻并睡到那一刻。
///
/// `per_minute` ≤ 0 视为不限速（直接返回）。否则间隔 = `60s / per_minute`，
/// 把当前 slot 推后一个 interval、记下原值作为本次的发送时刻。多个 worker
/// 同时调用时，第 N 个会得到 `start + N*interval`，整体平均速率精准等于
/// `per_minute / 60` req/sec。
///
/// **取消语义**：sleep 期间如果 [`TaskRuntime::is_cancelled`] 翻成 true，
/// 函数会从 sleep 中被打断后下一轮检查时直接返回（短 chunk 轮询）。返回
/// `false` 表示"被取消而没拉到 slot"，调用方应当跳过本次拉取。
async fn acquire_rate_slot(per_minute: i32, runtime: &TaskRuntime) -> bool {
    if per_minute <= 0 {
        return !runtime.is_cancelled();
    }
    // i32 -> u32 安全：上面已经 ≤ 0 早退；最大值 i32::MAX 远超合理上限。
    let per_minute_u = per_minute as u32;
    // 用 nanos 算 interval 才能在 per_minute > 60 时拿到 sub-second 间隔。
    let interval = Duration::from_nanos(60_000_000_000 / per_minute_u as u64);

    let wait_until = {
        let mut slot = next_request_slot().lock().await;
        let now = Instant::now();
        let target = if *slot > now { *slot } else { now };
        *slot = target + interval;
        target
    };

    // 如果已经到时间，直接返回；否则按 100ms chunk 睡，便于响应取消。
    loop {
        if runtime.is_cancelled() {
            return false;
        }
        let now = Instant::now();
        if now >= wait_until {
            return true;
        }
        let remaining = wait_until - now;
        let chunk = remaining.min(Duration::from_millis(100));
        tokio::time::sleep(chunk).await;
    }
}

/// 提取文件名（不含路径）中第一个 8~9 位数字段；找不到返回 `None`。
///
/// 例如 `144285190_p0.png` -> `Some("144285190")`、`2025_99349202.jpg` -> `Some("99349202")`、
/// `123_short.png` -> `None`。
pub fn extract_pid(file_name: &str) -> Option<String> {
    pid_regex()
        .captures(file_name)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 文件后缀是否属于支持的图片格式。
fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| IMAGE_EXTS.iter().any(|e| s.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// 同步扫描任务输入目录，返回所有可识别 PID 的图片。
///
/// 仅做"快速过滤" —— 不拉网络、不读文件内容，瞬间完成；调用方再用结果驱动长任务。
pub fn scan_pixiv_images(paths: &[String]) -> AppResult<Vec<PixivImageRow>> {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    for raw in paths {
        let root = to_extended_length_path(Path::new(raw));
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            if !is_image(p) {
                continue;
            }
            let file_name = match p.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            let pid = match extract_pid(&file_name) {
                Some(p) => p,
                None => continue,
            };
            let abs = to_user_friendly_path(p);
            // 多个输入路径可能覆盖同一文件，去重避免表里出现重复行。
            let key = abs.to_lowercase();
            if !seen.insert(key) {
                continue;
            }
            rows.push(PixivImageRow {
                abs_path: abs,
                file_name,
                pid,
            });
        }
    }

    Ok(rows)
}

/// 构造 reqwest 客户端：默认 headers 带 Referer / UA / 可选 Cookie；可选代理。
///
/// Pixiv ajax 接口要求 `Referer: https://www.pixiv.net/`，否则常被拦截或返回乱页。
/// 代理为空时按 reqwest 默认行为读 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量；
/// 大陆环境一般要在设置里显式配置 `http://127.0.0.1:7890` 之类的代理。
fn build_http_client(cookie: Option<&str>, proxy: Option<&str>) -> AppResult<Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_static("https://www.pixiv.net/"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(BROWSER_UA),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json, text/javascript, */*; q=0.01"),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        reqwest::header::HeaderValue::from_static(ACCEPT_LANGUAGE),
    );
    if let Some(raw) = cookie.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        let value = reqwest::header::HeaderValue::from_str(raw)
            .map_err(|e| AppError::InvalidInput(format!("Cookie 不合法：{e}")))?;
        headers.insert(reqwest::header::COOKIE, value);
    }

    let mut builder = Client::builder()
        .default_headers(headers)
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .gzip(true);

    if let Some(p) = proxy.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }) {
        let proxy_obj = reqwest::Proxy::all(p)
            .map_err(|e| AppError::InvalidInput(format!("代理 URL 不合法：{e}")))?;
        builder = builder.proxy(proxy_obj);
    }

    builder
        .build()
        .map_err(|e| AppError::Internal(format!("构建 HTTP 客户端失败：{e}")))
}

/// 把 reqwest 错误及其整条 source 链拼成一行可读字符串。
///
/// reqwest 的 `Display` 只输出顶层（"error sending request for url ..."），
/// 真正的根因藏在 `.source()` 链里（"connection refused" / "TLS handshake failed" 等）。
/// 直接报顶层会让用户摸不着头脑，所以这里把整条链拼出来。
fn format_error_chain(e: &reqwest::Error) -> String {
    use std::error::Error;
    let mut out = format!("{e}");
    let mut cur: Option<&dyn Error> = e.source();
    while let Some(src) = cur {
        out.push_str(" -> ");
        out.push_str(&format!("{src}"));
        cur = src.source();
    }
    out
}

/// 给"error sending request"类错误加上"建议配代理"的友好提示，便于用户自助排错。
fn classify_send_error(e: &reqwest::Error) -> String {
    let chain = format_error_chain(e);
    if e.is_connect() {
        format!(
            "无法连接到服务器：{chain}\n\
             如在中国大陆访问 Pixiv，请在设置中配置 HTTP/SOCKS5 代理（如 http://127.0.0.1:7890）"
        )
    } else if e.is_timeout() {
        format!("连接超时：{chain}\n如经常超时，可在设置中调整代理或重试")
    } else if e.is_request() {
        format!("请求构造失败：{chain}")
    } else {
        format!("网络错误：{chain}")
    }
}

/// 拼接最终请求 URL：`<base><pid>`，自动在 base 末尾补斜杠。
fn build_request_url(base: &str, pid: &str) -> String {
    let trimmed = base.trim();
    if trimmed.ends_with('/') {
        format!("{trimmed}{pid}")
    } else {
        format!("{trimmed}/{pid}")
    }
}

/// 从 Pixiv ajax JSON 中提取原 tag 列表与 `translation.en` 映射。
///
/// Pixiv 的真实结构是 `body.tags.tags[*]`：每项的 `tag` 是原始标签字符串，
/// 可选的 `translation.en` 是该 tag 的译名。没有 `translation` 或 `en`
/// 为空时只保留原 tag，不往映射里写空值。
fn parse_pixiv_tag_payload(json: &Value) -> AppResult<(Vec<String>, HashMap<String, String>)> {
    let tags = json
        .pointer("/body/tags/tags")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::Internal("响应缺少 body.tags.tags 数组".to_string()))?;

    let mut out_tags = Vec::with_capacity(tags.len());
    let mut translations = HashMap::<String, String>::new();
    for item in tags {
        let tag = match item.get("tag").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => continue,
        };
        // translation.en 是社区译名；可能不存在 / 是空字符串 / 不是字符串。
        // 只接收非空字符串，省得前端再判空。
        if let Some(en) = item
            .pointer("/translation/en")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            translations.insert(tag.clone(), en.to_string());
        }
        out_tags.push(tag);
    }
    Ok((out_tags, translations))
}

/// 单条 PID 的实际拉取实现：HTTP GET → 解析 JSON → 取 `body.tags.tags[*].tag` 与 `translation.en`。
///
/// 返回 `(tags, translations)`：
/// - `tags`：保留响应里的原始顺序（前端用它生成 chip 列）；
/// - `translations`：仅含有 `translation.en` 字符串的 tag，键是原 tag、值是 en 译名。
///
/// 错误情况：网络层（连接失败 / 超时 / TLS）→ 走 [`classify_send_error`] 给出含建议的友好消息；
/// HTTP 非 2xx → 按状态码翻译；Pixiv `error: true` → 取响应里的 `message`；JSON 结构异常 → 报错。
pub async fn fetch_tag_for_pid(
    client: &Client,
    base: &str,
    pid: &str,
) -> AppResult<(Vec<String>, HashMap<String, String>)> {
    let url = build_request_url(base, pid);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Internal(classify_send_error(&e)))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("读取响应失败：{}", format_error_chain(&e))))?;

    if !status.is_success() {
        // 对常见状态给出更直观的中文消息，其它原样附上状态码。
        let msg = match status.as_u16() {
            404 => "PID 不存在或作品已被删除".to_string(),
            403 => "无权访问（可能需要 Cookie）".to_string(),
            429 => "请求过快，已被限流".to_string(),
            _ => format!("HTTP {status}"),
        };
        return Err(AppError::Internal(msg));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("响应不是合法 JSON：{e}")))?;

    if json.get("error").and_then(Value::as_bool).unwrap_or(false) {
        let msg = json
            .get("message")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("Pixiv 返回 error: true")
            .to_string();
        return Err(AppError::Internal(msg));
    }

    parse_pixiv_tag_payload(&json)
}

/// 单条 PID 同步拉取（给前端"重试"按钮用）。
///
/// 内部读 settings 拿 base / cookie / proxy / rate_limit_per_minute，构造一个一次性 Client；
/// 不复用长任务的 Client，避免重试阻塞在长任务的连接池上。
///
/// 复用全局共享的 [`acquire_rate_slot`]——重试也算 Pixiv 调用，必须排进同一条限速队列，
/// 否则用户在长任务跑的同时狂按重试可以把瞬时速率打到 2× per_minute，把"防黑"目的搞穿。
/// 这里没有 TaskRuntime 上下文，造一个一次性的 cancellable runtime 充作"永不取消"哨兵。
pub async fn fetch_pixiv_tag_single(db_path: &Path, pid: &str) -> AppResult<PixivTagFetchResult> {
    if pid.trim().is_empty() {
        return Err(AppError::InvalidInput("PID 不能为空".to_string()));
    }
    let settings = settings_repo::get_settings(db_path)
        .map_err(|e| AppError::Internal(format!("读取设置失败：{e}")))?;
    let client = build_http_client(
        settings.pixiv_cookie.as_deref(),
        settings.pixiv_proxy.as_deref(),
    )?;
    let dummy_runtime = TaskRuntime::new();
    let _ = acquire_rate_slot(settings.pixiv_rate_limit_per_minute, &dummy_runtime).await;
    let (tags, translations) =
        fetch_tag_for_pid(&client, &settings.pixiv_tag_api_base, pid.trim()).await?;
    Ok(PixivTagFetchResult { tags, translations })
}

/// 长任务主体：并发拉 tag → 增量推送 → 终态清理。
///
/// 入参 `pids` 由调用方先通过 [`scan_pixiv_images`] 同步扫描得到——这样前端能在
/// 长任务启动前就立刻渲染 pending 行，避免"扫描+拉取一体"模式下表格一直空白
/// 直到第一条 partial 到达。
///
/// `runtime.cancelled` 一旦置位，正在排队等 permit 的任务会快速退出，
/// 已经在跑的请求会跑完（不强制 abort）。
pub async fn run_pixiv_tag_scan(
    app: tauri::AppHandle,
    state: Arc<AppState>,
    task_id: String,
    pids: Vec<String>,
    runtime: Arc<TaskRuntime>,
) -> AppResult<()> {
    events::emit_state_changed(&app, &task_id, "Running");
    events::emit_log(
        &app,
        &task_id,
        log_level::INFO,
        &format!("开始拉取 {} 个 PID 的 tag", pids.len()),
        None,
    );
    events::emit_progress(&app, &task_id, stages::PIXIV_TAG, 0, pids.len());

    if pids.is_empty() {
        finalize_done(&app, &state, &task_id, false);
        return Ok(());
    }

    // 1) 读 settings + 构造 client + 算并发许可
    let settings = match settings_repo::get_settings(&state.db_path) {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("读取设置失败：{e}");
            finalize_failed(&app, &state, &task_id, &msg);
            return Err(AppError::Internal(msg));
        }
    };
    let client = match build_http_client(
        settings.pixiv_cookie.as_deref(),
        settings.pixiv_proxy.as_deref(),
    ) {
        Ok(c) => c,
        Err(e) => {
            finalize_failed(&app, &state, &task_id, &e.to_string());
            return Err(e);
        }
    };
    let base = settings.pixiv_tag_api_base.clone();
    let per_minute = settings.pixiv_rate_limit_per_minute;

    let thread_count = op_pipeline::resolve_thread_count(&state.db_path);
    let io_mult = op_pipeline::resolve_io_concurrency_multiplier(&state.db_path);
    let permits = thread_count
        .saturating_mul(io_mult)
        .clamp(1, FETCH_CONCURRENCY_HARD_CAP);
    let semaphore = Arc::new(Semaphore::new(permits));

    events::emit_log(
        &app,
        &task_id,
        log_level::INFO,
        &format!(
            "并发拉取 tags（许可 {permits}，限速 {} req/min）",
            if per_minute > 0 {
                per_minute.to_string()
            } else {
                "∞".to_string()
            }
        ),
        None,
    );

    // 2) spawn 一组任务，每个完成后通过 mpsc 推回主聚合循环
    let total = pids.len();
    let (tx, mut rx) = mpsc::channel::<PixivTagPartialItem>(permits.max(8) * 4);

    for pid in pids {
        let permit_holder = semaphore.clone();
        let runtime = runtime.clone();
        let tx = tx.clone();
        let client = client.clone();
        let base = base.clone();
        // 把 AppHandle + task_id 克隆给 worker 用于打日志：每条 PID 完成都要发
        // 一行 task_log，让用户在日志面板上能看到具体哪个 PID 出错 / 成功了多少 tag。
        let app_for_worker = app.clone();
        let task_id_for_worker = task_id.clone();

        tokio::spawn(async move {
            let permit = match permit_holder.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };
            if runtime.is_cancelled() {
                drop(permit);
                let _ = tx
                    .send(PixivTagPartialItem {
                        pid: pid.clone(),
                        tags: None,
                        translations: None,
                        error: Some("已取消".to_string()),
                    })
                    .await;
                return;
            }

            // 排队领发送时刻：所有并发 worker 共享同一条 next-slot 队列，
            // 整体速率被锁定在 per_minute / 60 req/sec。被取消时直接退出，
            // 不发起 HTTP；预订掉的 slot 也"作废"——下一个 worker 直接拿到
            // 当前时间对齐的 slot，浪费的只是一段已过期的 sleep 时间。
            if !acquire_rate_slot(per_minute, &runtime).await {
                drop(permit);
                let _ = tx
                    .send(PixivTagPartialItem {
                        pid: pid.clone(),
                        tags: None,
                        translations: None,
                        error: Some("已取消".to_string()),
                    })
                    .await;
                return;
            }

            let item = match fetch_tag_for_pid(&client, &base, &pid).await {
                Ok((tags, translations)) => {
                    // 成功打 INFO 日志：tag 数 + 译名数。译名数 = 0 时省略尾巴，
                    // 避免长任务里 50K 张图每条都拖一句"0 译名"显得啰嗦。
                    let trans_n = translations.len();
                    let msg = if trans_n > 0 {
                        format!(
                            "PID {} ✓ 取到 {} 个 tag（其中 {} 个有英文译名）",
                            pid,
                            tags.len(),
                            trans_n
                        )
                    } else {
                        format!("PID {} ✓ 取到 {} 个 tag", pid, tags.len())
                    };
                    events::emit_log(
                        &app_for_worker,
                        &task_id_for_worker,
                        log_level::INFO,
                        &msg,
                        None,
                    );
                    PixivTagPartialItem {
                        pid: pid.clone(),
                        tags: Some(tags),
                        // 没有任何 en 译名时不发空 Map，省点 IPC 字节，前端按 None 兜底空对象。
                        translations: if translations.is_empty() {
                            None
                        } else {
                            Some(translations)
                        },
                        error: None,
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    // 失败打 WARN 日志：把 PID 和错误一起写出来，便于用户对照
                    // 文件名定位；错误首行已经够用，多行被压成 inline。
                    let first_line = err_msg.lines().next().unwrap_or(&err_msg).to_string();
                    events::emit_log(
                        &app_for_worker,
                        &task_id_for_worker,
                        log_level::WARN,
                        &format!("PID {} ✗ 失败：{}", pid, first_line),
                        None,
                    );
                    PixivTagPartialItem {
                        pid: pid.clone(),
                        tags: None,
                        translations: None,
                        error: Some(err_msg),
                    }
                }
            };
            drop(permit);
            let _ = tx.send(item).await;
        });
    }
    drop(tx);

    // 3) 主聚合循环：批量 flush
    let mut buffer: Vec<PixivTagPartialItem> = Vec::with_capacity(PARTIAL_FLUSH_BATCH);
    let mut last_flush = Instant::now();
    let mut processed: usize = 0;

    loop {
        let next = tokio::time::timeout(PARTIAL_FLUSH_INTERVAL, rx.recv()).await;
        match next {
            Ok(Some(item)) => {
                buffer.push(item);
                processed += 1;
                if buffer.len() >= PARTIAL_FLUSH_BATCH
                    || last_flush.elapsed() >= PARTIAL_FLUSH_INTERVAL
                {
                    flush_partial(&app, &task_id, &mut buffer, false);
                    last_flush = Instant::now();
                    events::emit_progress(&app, &task_id, stages::PIXIV_TAG, processed, total);
                }
            }
            Ok(None) => break,
            Err(_) => {
                if !buffer.is_empty() {
                    flush_partial(&app, &task_id, &mut buffer, false);
                    last_flush = Instant::now();
                    events::emit_progress(&app, &task_id, stages::PIXIV_TAG, processed, total);
                }
            }
        }
    }

    // 4) 终态：flush 残留 + 标记 done
    if !buffer.is_empty() {
        flush_partial(&app, &task_id, &mut buffer, false);
    }
    events::emit_progress(&app, &task_id, stages::PIXIV_TAG, processed, total);
    // 终态总结日志：跑完 / 取消时给出"成功 X / 失败 Y / 总计 Z"，方便用户对完
    // 一眼判定是不是要按"重试失败"按钮。统计走"前面 worker 已发的 partial item"
    // 的 error 字段而不是这里再次维护——但 mpsc 已关，这里靠 `processed` 总数；
    // 如果需要精细统计，前端自己用 store 的 errorCount 显示就够了。
    if runtime.is_cancelled() {
        events::emit_log(
            &app,
            &task_id,
            log_level::WARN,
            &format!("Pixiv tag 拉取已取消（已处理 {} / {}）", processed, total),
            None,
        );
    } else {
        events::emit_log(
            &app,
            &task_id,
            log_level::INFO,
            &format!("Pixiv tag 拉取完成（共处理 {}）", processed),
            None,
        );
    }
    finalize_done(&app, &state, &task_id, runtime.is_cancelled());
    Ok(())
}

/// 把 buffer 里的 items 推到前端，清空 buffer。
fn flush_partial(
    app: &tauri::AppHandle,
    task_id: &str,
    buffer: &mut Vec<PixivTagPartialItem>,
    done: bool,
) {
    let payload = PixivTagPartialPayload {
        task_id: task_id.to_string(),
        items: std::mem::take(buffer),
        done,
    };
    events::emit_pixiv_tag_partial(app, &payload);
}

/// 任务正常结束（含取消、空候选）：发空 `done = true` partial、改状态、清 task 表。
fn finalize_done(app: &tauri::AppHandle, state: &Arc<AppState>, task_id: &str, cancelled: bool) {
    events::emit_pixiv_tag_partial(
        app,
        &PixivTagPartialPayload {
            task_id: task_id.to_string(),
            items: Vec::new(),
            done: true,
        },
    );
    events::emit_state_changed(
        app,
        task_id,
        if cancelled { "Cancelled" } else { "Completed" },
    );
    state.remove_task(task_id);
}

/// 任务失败：发 `done = true` partial（前端能解锁 running 状态）+ 失败事件 + 清 task 表。
fn finalize_failed(app: &tauri::AppHandle, state: &Arc<AppState>, task_id: &str, message: &str) {
    events::emit_pixiv_tag_partial(
        app,
        &PixivTagPartialPayload {
            task_id: task_id.to_string(),
            items: Vec::new(),
            done: true,
        },
    );
    events::finalize_failed_long_task(app, state, task_id, message);
}

/// 把单张图片移动到 `<output_dir>/<sanitized_tag>/<basename>`。返回新的用户友好路径。
///
/// - tag 需经 [`sanitize_filename`] 清理 Windows 非法字符；首尾留空的也会被截掉，
///   完全清理为空时报错（无法生成有效目录名）。
/// - 同名冲突走 [`resolve_conflict_with_reserved`] 兜底（叠加 ` (N)` 后缀）。
/// - 实际移动用 [`op_pipeline::rename_or_copy_delete`]，跨卷自动 copy + delete。
pub fn move_image_by_tag(abs_path: &str, output_dir: &str, tag: &str) -> AppResult<String> {
    let abs_path = abs_path.trim();
    let output_dir = output_dir.trim();
    if abs_path.is_empty() {
        return Err(AppError::InvalidInput("源路径不能为空".to_string()));
    }
    if output_dir.is_empty() {
        return Err(AppError::InvalidInput("输出目录不能为空".to_string()));
    }
    if tag.trim().is_empty() {
        return Err(AppError::InvalidInput("tag 不能为空".to_string()));
    }

    let cleaned_tag = clean_tag_for_dir_name(tag);
    if cleaned_tag.is_empty() {
        return Err(AppError::InvalidInput(
            "tag 全是非法字符，无法用作目录名".to_string(),
        ));
    }

    let src = PathBuf::from(abs_path);
    let basename = src
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("源路径没有文件名".to_string()))?
        .to_owned();

    let target_dir = PathBuf::from(output_dir).join(&cleaned_tag);
    std::fs::create_dir_all(to_extended_length_path(&target_dir))
        .map_err(|e| AppError::Io(format!("创建目录失败：{e}")))?;

    let initial = target_dir.join(&basename);
    let mut reserved = std::collections::HashSet::new();
    let (final_path, _had_conflict) = resolve_conflict_with_reserved(initial, &mut reserved);

    op_pipeline::rename_or_copy_delete(&src, &final_path)
        .map_err(|e| AppError::Io(format!("移动失败：{e}")))?;

    Ok(to_user_friendly_path(&final_path))
}

/// 把 tag 清理为合法目录名：
/// 1. 替换 Windows 非法字符（`\ / : * ? " < > |`）为 `-`；
/// 2. 截掉首尾空白与首尾点号（Windows 不允许目录名以 `.` 结尾）；
/// 3. 把内部连续空白压成单空格（避免 `"  abc"` 这种诡异目录名）。
fn clean_tag_for_dir_name(tag: &str) -> String {
    let replaced = sanitize_filename(tag);
    // 二次过滤：非法字符变 `-` 后还要去掉首尾空白和首尾点号。
    let trimmed = replaced
        .trim()
        .trim_matches(|c: char| c == '.' || ILLEGAL_FILENAME_CHARS.contains(&c));
    // 内部连续空白压一压。
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_first_8_or_9_digits() {
        assert_eq!(
            extract_pid("144285190_p0.png"),
            Some("144285190".to_string())
        );
        assert_eq!(extract_pid("99349202.jpg"), Some("99349202".to_string()));
        assert_eq!(
            extract_pid("2025_99349202_p0.png"),
            Some("99349202".to_string()) // "2025" 只有 4 位被跳过
        );
        assert_eq!(extract_pid("123_short.png"), None);
        assert_eq!(extract_pid("1234567890_p0.png"), None); // 10 位不算
    }

    #[test]
    fn build_url_handles_trailing_slash() {
        assert_eq!(
            build_request_url("https://www.pixiv.net/ajax/illust/", "12345678"),
            "https://www.pixiv.net/ajax/illust/12345678"
        );
        assert_eq!(
            build_request_url("https://www.pixiv.net/ajax/illust", "12345678"),
            "https://www.pixiv.net/ajax/illust/12345678"
        );
    }

    #[test]
    fn parses_translation_en_from_pixiv_payload() {
        let json = serde_json::json!({
            "error": false,
            "body": {
                "tags": {
                    "tags": [
                        {
                            "tag": "コイカツ",
                            "translation": { "en": "恋活" }
                        },
                        {
                            "tag": "キャラ配布(コイカツ)",
                            "translation": { "en": "人物卡（恋活）" }
                        },
                        {
                            "tag": "Koikatsu"
                        }
                    ]
                }
            }
        });

        let (tags, translations) = parse_pixiv_tag_payload(&json).unwrap();

        assert_eq!(
            tags,
            vec![
                "コイカツ".to_string(),
                "キャラ配布(コイカツ)".to_string(),
                "Koikatsu".to_string(),
            ]
        );
        assert_eq!(
            translations.get("キャラ配布(コイカツ)").map(String::as_str),
            Some("人物卡（恋活）")
        );
        assert_eq!(
            translations.get("コイカツ").map(String::as_str),
            Some("恋活")
        );
        assert!(!translations.contains_key("Koikatsu"));
    }

    #[test]
    fn clean_tag_strips_illegal_and_trims() {
        assert_eq!(clean_tag_for_dir_name(" コイカツ! "), "コイカツ!");
        assert_eq!(clean_tag_for_dir_name("a/b:c"), "a-b-c");
        assert_eq!(clean_tag_for_dir_name("..."), "");
        assert_eq!(clean_tag_for_dir_name("  a   b  "), "a b");
    }
}
