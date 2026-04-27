//! 移除 `.zipmod` 内 manifest.xml 中 `<game>KEYWORD</game>` 标签的"版本限制修改"。
//!
//! # 执行流程（每个文件）
//! 1. 把原文件字节拷贝到 `backup_path`；
//! 2. 把原 zip 的所有条目重新写入一个临时文件，其中 `manifest.xml` 条目用
//!    去掉 `<game>KEYWORD</game>` 的新内容替换，其余条目用 `raw_copy_file`
//!    原样复制（保持压缩字节，零重编码）；
//! 3. 原子替换：`rename(temp → original)`。
//!
//! 失败时会清理临时文件与备份，保持原文件不动。
//!
//! # 撤回
//! 记录写入 `old_path = 原文件`, `new_path = 备份路径`；
//! `op_pipeline::rollback` 的默认行为 `rename(new_path → old_path)`
//! 正好把备份覆盖回原文件，完成撤回。

use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::Local;
use uuid::Uuid;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    constants::mod_op_kind,
    error::{AppError, AppResult},
    models::ModOpApplyResponse,
    services::{logging::TaskLogContext, mod_tools::MOD_OP_TABLES, op_pipeline},
    utils::path::to_extended_length_path,
};

/// 从 manifest.xml 字符串中删除 `<game>KEYWORD</game>` 片段。
///
/// 对齐 Java 原版：简单字面量替换，保留其他内容不变。调用方确保 `keyword`
/// 非空。
pub fn remove_game_tag(content: &str, keyword: &str) -> String {
    let tag = format!("<game>{keyword}</game>");
    content.replace(&tag, "")
}

/// 备份原 zip，再重写一份移除了指定 `<game>` 标签的 zip 替换原文件。
fn modify_zipmod(original: &Path, backup: &Path, keyword: &str) -> Result<(), String> {
    let ep_original = to_extended_length_path(original);
    let ep_backup = to_extended_length_path(backup);

    std::fs::copy(&ep_original, &ep_backup).map_err(|e| format!("备份失败: {e}"))?;

    let temp = temp_sibling_path(original);
    let ep_temp = to_extended_length_path(&temp);

    let rewrite = || -> Result<(), String> {
        let in_file = File::open(&ep_original).map_err(|e| format!("打开原文件失败: {e}"))?;
        let mut archive = ZipArchive::new(in_file).map_err(|e| format!("读取 zip 失败: {e}"))?;

        let out_file = File::create(&ep_temp).map_err(|e| format!("创建临时文件失败: {e}"))?;
        let mut writer = ZipWriter::new(out_file);

        for i in 0..archive.len() {
            let is_manifest = {
                let entry = archive
                    .by_index(i)
                    .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
                entry.name().eq_ignore_ascii_case("manifest.xml")
            };

            if is_manifest {
                let (name, content) = {
                    let mut entry = archive
                        .by_index(i)
                        .map_err(|e| format!("读取 manifest 失败: {e}"))?;
                    let name = entry.name().to_string();
                    let mut content = String::new();
                    entry
                        .read_to_string(&mut content)
                        .map_err(|e| format!("读取 manifest 内容失败: {e}"))?;
                    (name, content)
                };
                let modified = remove_game_tag(&content, keyword);

                let opts: SimpleFileOptions =
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
                writer
                    .start_file(&name, opts)
                    .map_err(|e| format!("写入 manifest 失败: {e}"))?;
                writer
                    .write_all(modified.as_bytes())
                    .map_err(|e| format!("写入 manifest 内容失败: {e}"))?;
            } else {
                let entry = archive
                    .by_index(i)
                    .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
                writer
                    .raw_copy_file(entry)
                    .map_err(|e| format!("复制条目失败: {e}"))?;
            }
        }

        writer.finish().map_err(|e| format!("关闭 zip 失败: {e}"))?;
        Ok(())
    };

    if let Err(e) = rewrite() {
        let _ = std::fs::remove_file(&ep_temp);
        let _ = std::fs::remove_file(&ep_backup);
        return Err(e);
    }

    std::fs::rename(&ep_temp, &ep_original).map_err(|e| {
        // 临时文件已是完整的 zip；替换失败时保留它供用户手动处理。
        format!("替换原文件失败（临时文件保留在 {}）: {e}", temp.display())
    })?;

    Ok(())
}

/// 生成一个与原文件同目录的临时文件名，避免跨磁盘 rename。
fn temp_sibling_path(original: &Path) -> PathBuf {
    let stem = original
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("modfile");
    let short = Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    let parent = original.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}.fileflow-tmp-{short}"))
}

/// 生成备份路径：`{原文件}.fileflow-bak-{timestamp}-{uuid8}`。
fn backup_path_for(original: &str, ts: i64) -> String {
    let short = Uuid::new_v4().simple().to_string();
    let short = &short[..8];
    format!("{original}.fileflow-bak-{ts}-{short}")
}

/// 应用"移除版本限制"并持久化为 Mod 操作记录。
///
/// - `paths`：当前任务的输入路径（仅用于记录元数据中的 `source_paths`）；
/// - `keyword`：要从 `manifest.xml` 中移除的 `<game>KEYWORD</game>` 关键字；
/// - `selected_file_paths`：要修改的具体 `.zipmod` 文件列表（由前端按用户勾选传入）。
pub fn apply_mod_modify_version(
    db_path: &Path,
    paths: &[String],
    keyword: String,
    selected_file_paths: Vec<String>,
    record_name: Option<String>,
    log: Option<TaskLogContext>,
) -> AppResult<ModOpApplyResponse> {
    let keyword_trim = keyword.trim().to_string();
    if keyword_trim.is_empty() {
        return Err(AppError::InvalidInput("关键字不能为空".to_string()));
    }
    if selected_file_paths.is_empty() {
        return Err(AppError::InvalidInput("未选择任何文件".to_string()));
    }

    let ts = Local::now().timestamp();
    let pairs: Vec<(String, String)> = selected_file_paths
        .into_iter()
        .map(|p| {
            let backup = backup_path_for(&p, ts);
            (p, backup)
        })
        .collect();

    let keyword_arc = Arc::new(keyword_trim);
    let executor = {
        let kw = keyword_arc.clone();
        let log = log.clone();
        move |original: &str, backup: &str| -> Result<(), String> {
            if let Some(log) = &log {
                log.info(&format!("正在修改版本限制: {original}"));
            }
            modify_zipmod(Path::new(original), Path::new(backup), &kw)
        }
    };

    let name = record_name.unwrap_or_else(|| Local::now().format("%Y-%m-%d_%H-%M-%S").to_string());

    op_pipeline::persist_apply_with_executor(
        db_path,
        MOD_OP_TABLES,
        mod_op_kind::MODIFY,
        name,
        paths,
        pairs,
        executor,
    )
}
