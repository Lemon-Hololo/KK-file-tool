//! 解析 .zipmod / .zip 文件内的 manifest.xml，抽取 guid / version / author / game 等元信息。

use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use zip::ZipArchive;

use crate::utils::path::to_extended_length_path;

#[derive(Debug, Default, Clone)]
pub struct ManifestMeta {
    pub guid: String,
    pub version: String,
    pub author: String,
    pub games: Vec<String>,
}

/// 解析 manifest.xml 字节流，抽取关键字段。
/// 未找到的字段返回空字符串（对齐 Java 原版行为）。
pub fn parse_manifest(bytes: &[u8]) -> Result<ManifestMeta, String> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(true);

    let mut meta = ManifestMeta::default();
    let mut current: Option<&'static str> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let tag = name.as_ref();
                current = match tag {
                    b"guid" => Some("guid"),
                    b"version" => Some("version"),
                    b"author" => Some("author"),
                    b"game" => Some("game"),
                    _ => None,
                };
            }
            Ok(Event::Text(ref e)) => {
                if let Some(tag) = current.take() {
                    let text = e
                        .unescape()
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();
                    match tag {
                        "guid" if meta.guid.is_empty() => meta.guid = text,
                        "version" if meta.version.is_empty() => meta.version = text,
                        "author" if meta.author.is_empty() => meta.author = text,
                        "game" => {
                            if !text.is_empty() {
                                meta.games.push(text);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                current = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML 解析失败: {e}")),
            _ => {}
        }
        buf.clear();
    }

    Ok(meta)
}

/// 打开 zip/zipmod，大小写不敏感查找 manifest.xml 并解析。
/// 返回 `(meta, raw_xml_bytes)`，失败时返回 Err(..)。
pub fn read_manifest_from_zip(path: &Path) -> Result<(ManifestMeta, Vec<u8>), String> {
    let ep = to_extended_length_path(path);
    let file = File::open(&ep).map_err(|e| format!("打开文件失败: {e}"))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("读取 zip 失败: {e}"))?;

    // 先找到 manifest.xml 的索引（避免借用冲突）
    let mut target_idx: Option<usize> = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
        if entry.name().eq_ignore_ascii_case("manifest.xml") {
            target_idx = Some(i);
            break;
        }
    }

    let idx = target_idx.ok_or_else(|| "未找到 manifest.xml".to_string())?;
    let mut entry = archive
        .by_index(idx)
        .map_err(|e| format!("读取 manifest 条目失败: {e}"))?;

    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| format!("读取 manifest 内容失败: {e}"))?;

    let meta = parse_manifest(&bytes)?;
    Ok((meta, bytes))
}

/// 判断文件名是否 zipmod / zip 后缀（大小写不敏感）。
pub fn is_zipmod(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".zipmod")
}
