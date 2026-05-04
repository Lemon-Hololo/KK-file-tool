//! Pixiv 标签整理相关 DTO。
//!
//! 一次扫描的生命周期：
//! 1. 后端扫描任务输入目录得到 `Vec<PixivImageRow>`（含 PID）；
//! 2. 后端长任务并发拉 tag，分批通过 `pixiv_tag_partial` 事件推回 [`PixivTagPartialPayload`]；
//! 3. 前端用 `pid` 索引把每条 [`PixivTagPartialItem`] 写回行内的 `tags` / `error` / `status`；
//! 4. 用户点击 tag 单元格时调 `move_image_by_tag_command` 把图移到 `<output>/<tag>/`。
//!
//! 单条命令 `fetch_pixiv_tag_single` 的返回值用 [`PixivTagFetchResult`]，给"重试"按钮用。
//!
//! 关于 `translations`：Pixiv 响应里每个 tag 项可能带 `translation.en`，这是社区译名
//! （日文 → 中/英 的人工对应，例如 "コイカツ!" → "Koikatsu"、"恋活" → "Koikatsu"）。
//! 后端把所有"有 en 译名的 tag"汇总成 `original → en` 的 Map，前端在开关打开时
//! 用译名替代原 tag 显示与点击落盘 —— **不展开为新行**，保持 tags 数组与原始顺序一一对应。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 扫描候选行：从任务输入文件夹里识别出的、文件名包含合法 PID 的图片。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixivImageRow {
    /// 文件绝对路径，已通过 `to_user_friendly_path` 去掉 `\\?\` 前缀。
    pub abs_path: String,
    /// 仅用于显示的文件名（`Path::file_name`）。
    pub file_name: String,
    /// 文件名中提取的 8~9 位 PID。
    pub pid: String,
}

/// 单个 PID 的拉取结果。`tags` 与 `error` 互斥：
/// - `tags = Some(_)` 表示成功，`error` 为 `None`；
/// - `error = Some(_)` 表示失败（HTTP / Pixiv `error: true` / 解析失败）。
///
/// `translations` 仅在 `tags` 有值时携带，记录"原 tag → en 译名"，未提供译名的 tag 不出现。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixivTagPartialItem {
    pub pid: String,
    pub tags: Option<Vec<String>>,
    pub translations: Option<HashMap<String, String>>,
    pub error: Option<String>,
}

/// `pixiv_tag_partial` 事件载荷。`done = true` 时 `items` 可能为空，仅作为终态信号。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixivTagPartialPayload {
    pub task_id: String,
    pub items: Vec<PixivTagPartialItem>,
    pub done: bool,
}

/// 单条 PID 拉取的返回值（重试按钮）；失败时命令直接返回 `Err(String)`。
///
/// `translations` 是 `tag → en` 映射；没有 `translation.en` 的 tag 不会出现在该 Map 里。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixivTagFetchResult {
    pub tags: Vec<String>,
    pub translations: HashMap<String, String>,
}
