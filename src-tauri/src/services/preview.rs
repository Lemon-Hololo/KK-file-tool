//! 文本 / 图片 / 压缩包 的轻量预览。
//!
//! 结果以 JSON 返回，前端根据 `type` 字段分发到不同组件渲染。
//! 文本字节数上限与压缩包条目数上限都由用户设置控制（见
//! [`op_pipeline::resolve_text_preview_max_bytes`] 与
//! [`op_pipeline::resolve_zip_preview_max_entries`]）。

use std::{fs::File, io::Read, path::Path};

use chrono::NaiveDate;
use serde_json::json;

use crate::{
    error::{AppError, AppResult},
    services::op_pipeline::{resolve_text_preview_max_bytes, resolve_zip_preview_max_entries},
    utils::path::to_extended_length_path,
};

fn preview_text(db_path: &Path, path: &str) -> AppResult<serde_json::Value> {
    let p = Path::new(path);
    let ep = to_extended_length_path(p);
    let mut f = File::open(ep).map_err(|e| AppError::Io(e.to_string()))?;

    let limit = resolve_text_preview_max_bytes(db_path);
    // 多读 3 个字节给末尾的多字节 UTF-8 字符兜底——`limit` 可能正好截在
    // 一个 4 字节字符的中间，多 3 字节让 from_utf8_lossy 不会把末尾合法
    // 字符识别成 `?`；展示长度仍按 limit 截。
    let mut buf = vec![0u8; limit + 3];
    let n = f.read(&mut buf).map_err(|e| AppError::Io(e.to_string()))?;
    buf.truncate(n);

    // 截到原始 limit 时再做一次"最近一个 UTF-8 字符边界"对齐，避免末尾被
    // 渲染为 `?`。靠 `is_char_boundary` 反查最多 3 个字节即可。
    let truncated = n > limit;
    let display_end = if truncated {
        let mut end = limit;
        while end > 0 && !is_utf8_char_boundary(&buf, end) {
            end -= 1;
        }
        end
    } else {
        n
    };
    let content = String::from_utf8_lossy(&buf[..display_end]).to_string();

    Ok(json!({
      "type": "text",
      "size": display_end,
      "truncated": truncated,
      "content": content
    }))
}

fn is_utf8_char_boundary(buf: &[u8], idx: usize) -> bool {
    if idx == 0 || idx == buf.len() {
        return true;
    }
    // UTF-8 后续字节模式 10xxxxxx；首字节绝不会是 10xxxxxx。
    (buf[idx] & 0b1100_0000) != 0b1000_0000
}

fn preview_image(path: &str) -> AppResult<serde_json::Value> {
    let p = Path::new(path);
    let ep = to_extended_length_path(p);

    let reader = image::ImageReader::open(ep).map_err(|e| AppError::Io(e.to_string()))?;
    let reader = reader
        .with_guessed_format()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let format = format!("{:?}", reader.format());
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(json!({
      "type": "image",
      "path": path,
      "width": w,
      "height": h,
      "format": format
    }))
}

fn preview_zip(db_path: &Path, path: &str) -> AppResult<serde_json::Value> {
    let p = Path::new(path);
    let ep = to_extended_length_path(p);

    let f = File::open(ep).map_err(|e| AppError::Io(e.to_string()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| AppError::Internal(e.to_string()))?;

    let limit = resolve_zip_preview_max_entries(db_path);
    let mut entries = vec![];
    let count = z.len().min(limit);
    for i in 0..count {
        let e = z
            .by_index(i)
            .map_err(|er| AppError::Internal(er.to_string()))?;
        let modified_at = e
            .last_modified()
            .and_then(zip_datetime_to_display)
            .unwrap_or_default();
        entries.push(json!({
          "name": e.name(),
          "size": e.size(),
          "isDir": e.is_dir(),
          "modifiedAt": modified_at
        }));
    }

    Ok(json!({
      "type": "archive_list",
      "truncated": z.len() > limit,
      "entries": entries
    }))
}

fn zip_datetime_to_display(value: zip::DateTime) -> Option<String> {
    let date = NaiveDate::from_ymd_opt(
        value.year().into(),
        value.month().into(),
        value.day().into(),
    )?;
    let datetime = date.and_hms_opt(
        value.hour().into(),
        value.minute().into(),
        value.second().into(),
    )?;
    Some(datetime.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// 按扩展名分派：未识别类型返回 `{"type":"unsupported"}`。
pub fn detect_preview(db_path: &Path, path: &str) -> AppResult<serde_json::Value> {
    let ext = Path::new(path)
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "json" | "csv" | "log" => preview_text(db_path, path),
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" => preview_image(path),
        "zip" | "zipmod" => preview_zip(db_path, path),
        _ => Ok(json!({ "type": "unsupported" })),
    }
}
