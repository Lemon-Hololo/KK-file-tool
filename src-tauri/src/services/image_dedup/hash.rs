//! 图片单文件感知哈希计算与候选过滤。
//!
//! 用 [`image_hasher`] v3。算法字符串到该 crate enum 的映射：
//!
//! | 用户字符串 | image_hasher::HashAlg | 备注 |
//! |-----------|----------------------|------|
//! | `phash`   | `DoubleGradient`     | crate 没有 DCT 真 pHash；`DoubleGradient` 是
//! |           |                      | 两方向梯度，对压缩 / 缩放最稳，作为 pHash 替代 |
//! | `dhash`   | `Gradient`           | 标准 dHash |
//! | `ahash`   | `Mean`               | 平均哈希（最快，精度最低） |
//!
//! 命名上仍用用户熟悉的 phash/dhash/ahash 字符串，避免暴露 crate 内部命名。

use std::path::Path;

use image_hasher::{HashAlg, Hasher, HasherConfig, ImageHash};

use crate::{
    constants::image_hash_algorithm,
    error::{AppError, AppResult},
    models::ImageHashFile,
    utils::path::to_extended_length_path,
};

/// 候选筛选 + 哈希参数：从 `AppSettings` 解析后传给扫描入口。
///
/// 把"读 settings"和"扫描"解耦：scan 不直接接 db 路径，可方便单测 / 复用。
#[derive(Debug, Clone)]
pub struct ImageHashConfig {
    /// `phash` / `dhash` / `ahash`。无效值会回落到 `phash`。
    pub algorithm: String,
    /// 边长（最终 bit 数 = size * size）。`< 4` 会被强制提升到 4。
    pub hash_size: u32,
    /// 候选扩展名（小写，不带点）。空集合视为"不过滤扩展名"——所有 image::open
    /// 能解析的都会被尝试，性能更差但兜底。
    pub extensions: Vec<String>,
    /// 跳过 < 此值的小文件（KiB）。0 视为不限。
    pub min_file_size_kb: u32,
    /// 跳过宽或高 < 此值的小图（像素）。0 视为不限。
    pub min_dimension: u32,
}

/// 单图哈希结果：对外 DTO + 已解析的 [`ImageHash`]。
///
/// 分组阶段会大量做 Hamming 距离比较，保留 `ImageHash` 可避免把 base64 字符串
/// 反复反序列化。
#[derive(Clone)]
pub struct HashedImageFile {
    pub file: ImageHashFile,
    pub image_hash: ImageHash,
}

/// 扩展名是否在候选名单内（大小写不敏感）。空白扩展名 / 没扩展名时跳过。
pub fn ext_matches(path: &Path, exts: &[String]) -> bool {
    if exts.is_empty() {
        return true;
    }
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(s) if !s.is_empty() => s.to_ascii_lowercase(),
        _ => return false,
    };
    exts.iter().any(|e| e.eq_ignore_ascii_case(&ext))
}

/// 文件大小是否达标（min_file_size_kb = 0 视为不限）。
pub fn size_matches(file_size: u64, min_file_size_kb: u32) -> bool {
    if min_file_size_kb == 0 {
        return true;
    }
    file_size >= (min_file_size_kb as u64) * 1024
}

/// 构造一个可重复使用的 [`Hasher`]；本身不持有图像数据，可在多个 worker 间共享。
///
/// `algorithm` 无效时回退 `phash`；`hash_size < 4` 时强制 4——避免用户写 1 / 2
/// 导致 `to_hasher` panic。
pub fn build_hasher(cfg: &ImageHashConfig) -> Hasher {
    let alg = match cfg.algorithm.as_str() {
        image_hash_algorithm::DHASH => HashAlg::Gradient,
        image_hash_algorithm::AHASH => HashAlg::Mean,
        // phash 与未知值都用 DoubleGradient 兜底。
        _ => HashAlg::DoubleGradient,
    };
    let size = cfg.hash_size.max(4);
    HasherConfig::new()
        .hash_alg(alg)
        .hash_size(size, size)
        .to_hasher()
}

/// 单张图片的"解码 + 计算哈希"。
///
/// 轻量候选过滤（扩展名 / 文件大小 / 文件时间）已由扫描阶段完成，这里只做
/// 兜底扩展名检查、图片解码、尺寸过滤和感知哈希。解析失败 / 尺寸不达标返回
/// `Ok(None)`，调用方按"过滤掉的"处理。
pub fn hash_one(
    path: &Path,
    file_size: u64,
    mtime: i64,
    ctime: i64,
    hasher: &Hasher,
    cfg: &ImageHashConfig,
) -> AppResult<Option<HashedImageFile>> {
    if !ext_matches(path, &cfg.extensions) {
        return Ok(None);
    }
    let ep = to_extended_length_path(path);

    let img = match image::open(&ep) {
        Ok(i) => i,
        // 解析失败（损坏 / 非图片 / 不支持的子格式）跳过，不当作错误。
        Err(_) => return Ok(None),
    };
    let (w, h) = (img.width(), img.height());
    if cfg.min_dimension > 0 && (w < cfg.min_dimension || h < cfg.min_dimension) {
        return Ok(None);
    }

    let hash = hasher.hash_image(&img);

    Ok(Some(HashedImageFile {
        file: ImageHashFile {
            file_path: crate::utils::path::to_user_friendly_path(path),
            width: w,
            height: h,
            file_size,
            mtime,
            ctime,
            hash: hash.to_base64(),
        },
        image_hash: hash,
    }))
}

/// 反序列化 base64 字符串回 `ImageHash`，用于分组阶段的 Hamming 距离比较。
///
/// 解析失败时返回 `AppError::Internal`；调用方通常应当在哈希计算阶段就保证字符串
/// 合法，所以这里出错代表数据损坏。
pub fn parse_hash(b64: &str) -> AppResult<ImageHash> {
    ImageHash::from_base64(b64).map_err(|_| AppError::Internal(format!("无效的图片哈希: {b64}")))
}

/// 给定 hash size（边长 = `size`），返回总 bit 数（= `size * size`）。
///
/// 分组阶段用 `(1.0 - threshold/100) * total_bits` 计算允许的 Hamming 距离上限。
pub fn total_bits(size: u32) -> u32 {
    let s = size.max(4);
    s * s
}
