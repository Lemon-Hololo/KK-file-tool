//! 解析 .zipmod / .zip 文件内的 manifest.xml，抽取 guid / version / author / game 等元信息。

use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

use encoding_rs::{GBK, SHIFT_JIS};
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

impl ManifestMeta {
    fn identity_score(&self) -> usize {
        let mut score = 0usize;
        if !self.guid.is_empty() {
            score += 1;
        }
        if !self.version.is_empty() {
            score += 1;
        }
        if !self.author.is_empty() {
            score += 1;
        }
        score
    }
}

/// 解析 manifest.xml 字节流，抽取关键字段。
/// 默认按 UTF-8 解析；若关键字段仍为空，再依次回退到 GBK 与 Shift_JIS。
/// 未找到的字段返回空字符串（对齐 Java 原版行为）。
pub fn parse_manifest(bytes: &[u8]) -> Result<ManifestMeta, String> {
    let utf8 = std::str::from_utf8(bytes)
        .map_err(|e| format!("UTF-8 解码失败: {e}"))
        .and_then(parse_manifest_text);
    if let Ok(meta) = utf8 {
        if meta.identity_score() == 3 {
            return Ok(meta);
        }

        let mut best_meta = meta;
        for encoding in [GBK, SHIFT_JIS] {
            if let Some(decoded) = decode_manifest_bytes(bytes, encoding) {
                let fallback_meta = parse_manifest_text(decoded.as_ref())?;
                if fallback_meta.identity_score() > best_meta.identity_score() {
                    best_meta = fallback_meta;
                }
                if best_meta.identity_score() == 3 {
                    break;
                }
            }
        }
        return Ok(best_meta);
    }

    let mut last_error = None;
    let mut best_meta: Option<ManifestMeta> = None;
    for encoding in [GBK, SHIFT_JIS] {
        let Some(decoded) = decode_manifest_bytes(bytes, encoding) else {
            continue;
        };
        match parse_manifest_text(decoded.as_ref()) {
            Ok(meta) => {
                let should_replace = best_meta
                    .as_ref()
                    .map(|current| meta.identity_score() > current.identity_score())
                    .unwrap_or(true);
                if should_replace {
                    best_meta = Some(meta);
                }
                if best_meta
                    .as_ref()
                    .map(|meta| meta.identity_score() == 3)
                    .unwrap_or(false)
                {
                    break;
                }
            }
            Err(error) => last_error = Some(error),
        }
    }

    if let Some(meta) = best_meta {
        return Ok(meta);
    }

    Err(last_error.unwrap_or_else(|| "manifest 无法按 UTF-8 / GBK / Shift_JIS 解析".to_string()))
}

fn parse_manifest_text(xml: &str) -> Result<ManifestMeta, String> {
    let mut reader = Reader::from_reader(Cursor::new(xml.as_bytes()));
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
                    // quick-xml 0.37+ 移除了 `BytesText::unescape`；改用 `decode()` 做编码层
                    // 转字符串。manifest.xml 的 guid/version/author/game 文本里基本不会出现
                    // XML 实体（&amp; 等），跳过实体反转义不会影响业务。
                    let text = e
                        .decode()
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

fn decode_manifest_bytes<'a>(
    bytes: &'a [u8],
    encoding: &'static encoding_rs::Encoding,
) -> Option<std::borrow::Cow<'a, str>> {
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        None
    } else {
        Some(decoded)
    }
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

#[cfg(test)]
mod tests {
    use super::parse_manifest;
    use encoding_rs::{GBK, SHIFT_JIS};

    fn build_manifest(author: &str) -> String {
        format!(
            r#"<manifest schema-ver="1">
  <guid>com.leisehg.AndreaDoria</guid>
  <name>AndreaDoria</name>
  <version>1.0</version>
  <author>{author}</author>
  <description>snr74</description>
</manifest>"#
        )
    }

    #[test]
    fn parse_manifest_falls_back_to_gbk_when_utf8_identity_is_incomplete() {
        let xml = build_manifest("作者测试");
        let (encoded, _, had_errors) = GBK.encode(&xml);
        assert!(!had_errors);

        let meta = parse_manifest(encoded.as_ref()).unwrap();
        assert_eq!(meta.guid, "com.leisehg.AndreaDoria");
        assert_eq!(meta.version, "1.0");
        assert_eq!(meta.author, "作者测试");
    }

    #[test]
    fn parse_manifest_falls_back_to_shift_jis_when_utf8_identity_is_incomplete() {
        let xml = build_manifest("ｱ");
        let (encoded, _, had_errors) = SHIFT_JIS.encode(&xml);
        assert!(!had_errors);

        let meta = parse_manifest(encoded.as_ref()).unwrap();
        assert_eq!(meta.guid, "com.leisehg.AndreaDoria");
        assert_eq!(meta.version, "1.0");
        assert_eq!(meta.author, "ｱ");
    }
}
