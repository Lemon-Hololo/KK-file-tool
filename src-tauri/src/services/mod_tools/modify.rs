//! 移除 `.zipmod` 内 manifest.xml 中 `<game>KEYWORD</game>` 标签的"版本限制修改"。
//!
//! # 执行流程（每个文件，启用回滚时）
//! 1. 把原文件字节拷贝到 `backup_path`；
//! 2. 把原 zip 的所有条目重新写入一个临时文件，其中 `manifest.xml` 条目用
//!    去掉 `<game>KEYWORD</game>` 的新内容替换，其余条目用 `raw_copy_file`
//!    原样复制（保持压缩字节，零重编码）；
//! 3. 原子替换：`rename(temp → original)`。
//!
//! 关闭回滚时跳过第 1 步（不创建备份），直接走第 2/3 步。
//!
//! 失败时会清理临时文件与备份，保持原文件不动。
//!
//! # 撤回
//! 启用回滚时：记录写入 `old_path = 原文件`, `new_path = 备份路径`；
//! `op_pipeline::rollback` 的默认行为 `rename(new_path → old_path)`
//! 正好把备份覆盖回原文件，完成撤回。
//!
//! 关闭回滚时：item 的 `new_path` 写空字符串，记录主表的
//! `rollback_enabled = false`，撤回会被后端 / 前端共同拒绝。
//!
//! # 备份目录
//! 备份位置由 [`crate::services::mod_tools::backup`] 解析；用户配置优先，
//! 否则落在 `<exe_dir>/mod-backups/<record_id>/<原文件名>`。
//! 临时文件 [`temp_sibling_path`] 故意保留在原文件同目录——`rename(temp → original)`
//! 必须同卷才能原子替换，备份位置可以在别处但临时文件不行。

use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use uuid::Uuid;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::{
    constants::mod_op_kind,
    error::{AppError, AppResult},
    models::ModOpApplyResponse,
    services::{
        logging::TaskLogContext,
        mod_tools::{backup, MOD_OP_TABLES},
        op_pipeline,
    },
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

/// 重写 zip 文件移除指定 `<game>` 标签。
///
/// `backup` 为 `Some(p)` 时把原文件 copy 到 `p`（用于撤回）；为 `None` 时
/// 跳过备份，节约一次 IO 与磁盘空间。无论哪种模式，临时文件都在原文件同
/// 目录以保 `rename(temp → original)` 原子替换。
fn modify_zipmod(original: &Path, backup: Option<&Path>, keyword: &str) -> Result<(), String> {
    let ep_original = to_extended_length_path(original);

    if let Some(backup_path) = backup {
        // 启用回滚：先备份。备份路径可能跨卷，std::fs::copy 天然支持。
        if let Some(parent) = backup_path.parent() {
            if !to_extended_length_path(parent).exists() {
                std::fs::create_dir_all(to_extended_length_path(parent))
                    .map_err(|e| format!("创建备份目录失败: {e}"))?;
            }
        }
        let ep_backup = to_extended_length_path(backup_path);
        std::fs::copy(&ep_original, &ep_backup).map_err(|e| format!("备份失败: {e}"))?;
    }

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
                let (name, content, compression) = {
                    let mut entry = archive
                        .by_index(i)
                        .map_err(|e| format!("读取 manifest 失败: {e}"))?;
                    let name = entry.name().to_string();
                    // 保留原条目的压缩方式，避免把 Stored/Bzip2 等强行改成 Deflated。
                    let compression = entry.compression();
                    let mut content = String::new();
                    entry
                        .read_to_string(&mut content)
                        .map_err(|e| format!("读取 manifest 内容失败: {e}"))?;
                    (name, content, compression)
                };
                let modified = remove_game_tag(&content, keyword);

                let opts: SimpleFileOptions =
                    SimpleFileOptions::default().compression_method(compression);
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
        if let Some(backup_path) = backup {
            let _ = std::fs::remove_file(to_extended_length_path(backup_path));
        }
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
    parent.join(format!("{stem}.kk-file-tool-tmp-{short}"))
}

/// 应用"移除版本限制"并持久化为 Mod 操作记录。
///
/// - `paths`：当前任务的输入路径（仅用于记录元数据中的 `source_paths`）；
/// - `keyword`：要从 `manifest.xml` 中移除的 `<game>KEYWORD</game>` 关键字；
/// - `selected_file_paths`：要修改的具体 `.zipmod` 文件列表（由前端按用户勾选传入）。
///
/// 行为分支：
/// - 用户开启"启用 Mod 操作回滚"（默认）：备份到 `<backup_root>/<record_id>/<filename>`，
///   写入可撤回记录；
/// - 关闭：in-place 改写不留备份，记录主表 `rollback_enabled = false`，撤回按钮置灰。
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

    // 同 cleanup：读 settings → 生成 record_id → 算备份对，全部交给
    // backup::prepare_mod_backup 处理。多个源目录同名 zipmod 撞名也由它兜底。
    let prepared = backup::prepare_mod_backup(db_path, selected_file_paths)?;

    let keyword_arc = Arc::new(keyword_trim);
    let executor = {
        let kw = keyword_arc.clone();
        let log = log.clone();
        let with_backup = prepared.rollback_enabled;
        move |original: &str, backup_path: &str| -> Result<(), String> {
            if let Some(log) = &log {
                if with_backup {
                    log.info(&format!("正在修改版本限制（备份）: {original}"));
                } else {
                    log.info(&format!("正在修改版本限制（不备份）: {original}"));
                }
            }
            let backup_opt = if backup_path.is_empty() {
                None
            } else {
                Some(PathBuf::from(backup_path))
            };
            modify_zipmod(Path::new(original), backup_opt.as_deref(), &kw)
        }
    };

    let name = op_pipeline::record_name_or_timestamp(record_name);

    op_pipeline::persist_apply_with_executor(
        db_path,
        MOD_OP_TABLES,
        &prepared.record_id,
        prepared.rollback_enabled,
        mod_op_kind::MODIFY,
        name,
        paths,
        prepared.pairs,
        executor,
    )
}
