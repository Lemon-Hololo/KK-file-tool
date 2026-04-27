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
    let mut buf = vec![0u8; limit];
    let n = f.read(&mut buf).map_err(|e| AppError::Io(e.to_string()))?;
    buf.truncate(n);

    let content = String::from_utf8_lossy(&buf).to_string();

    Ok(json!({
      "type": "text",
      "size": n,
      "truncated": n >= limit,
      "content": content
    }))
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
