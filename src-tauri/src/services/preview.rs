//! 文本 / 图片 / 压缩包 的轻量预览。
//!
//! 结果以 JSON 返回，前端根据 `type` 字段分发到不同组件渲染。

use std::{fs::File, io::Read, path::Path};

use serde_json::json;

use crate::{
    config::{TEXT_PREVIEW_MAX_BYTES, ZIP_PREVIEW_MAX_ENTRIES},
    error::{AppError, AppResult},
    utils::path::to_extended_length_path,
};

fn preview_text(path: &str) -> AppResult<serde_json::Value> {
    let p = Path::new(path);
    let ep = to_extended_length_path(p);
    let mut f = File::open(ep).map_err(|e| AppError::Io(e.to_string()))?;

    let mut buf = vec![0u8; TEXT_PREVIEW_MAX_BYTES];
    let n = f.read(&mut buf).map_err(|e| AppError::Io(e.to_string()))?;
    buf.truncate(n);

    let content = String::from_utf8_lossy(&buf).to_string();

    Ok(json!({
      "type": "text",
      "size": n,
      "truncated": n >= TEXT_PREVIEW_MAX_BYTES,
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

fn preview_zip(path: &str) -> AppResult<serde_json::Value> {
    let p = Path::new(path);
    let ep = to_extended_length_path(p);

    let f = File::open(ep).map_err(|e| AppError::Io(e.to_string()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| AppError::Internal(e.to_string()))?;

    let mut entries = vec![];
    let count = z.len().min(ZIP_PREVIEW_MAX_ENTRIES);
    for i in 0..count {
        let e = z
            .by_index(i)
            .map_err(|er| AppError::Internal(er.to_string()))?;
        entries.push(json!({
          "name": e.name(),
          "size": e.size(),
          "isDir": e.is_dir()
        }));
    }

    Ok(json!({
      "type": "archive_list",
      "truncated": z.len() > ZIP_PREVIEW_MAX_ENTRIES,
      "entries": entries
    }))
}

/// 按扩展名分派：未识别类型返回 `{"type":"unsupported"}`。
pub fn detect_preview(path: &str) -> AppResult<serde_json::Value> {
    let ext = Path::new(path)
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "json" | "csv" | "log" => preview_text(path),
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" => preview_image(path),
        "zip" | "zipmod" => preview_zip(path),
        _ => Ok(json!({ "type": "unsupported" })),
    }
}
