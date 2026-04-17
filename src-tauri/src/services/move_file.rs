use std::path::{Path, PathBuf};

use crate::{
    models::{MoveFailureItem, MoveSuccessItem},
    utils::path::to_extended_length_path,
};

pub struct MoveResult {
    pub success_items: Vec<MoveSuccessItem>,
    pub failed_items: Vec<MoveFailureItem>,
    pub released_bytes: u64,
}

/// Windows 重名策略：name.ext -> name (1).ext
fn resolve_conflict_name(dst_dir: &Path, original_name: &str) -> PathBuf {
    let mut stem = original_name.to_string();
    let mut ext = String::new();

    if let Some(pos) = original_name.rfind('.') {
        if pos > 0 {
            stem = original_name[..pos].to_string();
            ext = original_name[pos..].to_string();
        }
    }

    let mut idx = 1usize;
    loop {
        let candidate = dst_dir.join(format!("{} ({}){}", stem, idx, ext));
        let ec = to_extended_length_path(&candidate);
        if !ec.exists() {
            return candidate;
        }
        idx += 1;
    }
}

pub fn move_selected_files(target_dir: &Path, selected_files: &[String]) -> MoveResult {
    let target_ext = to_extended_length_path(target_dir);
    let _ = std::fs::create_dir_all(target_ext);

    let mut success_items = vec![];
    let mut failed_items = vec![];
    let mut released_bytes = 0u64;

    for src in selected_files {
        let src_path = PathBuf::from(src);
        let src_ext = to_extended_length_path(&src_path);

        if !src_ext.exists() {
            failed_items.push(MoveFailureItem {
                src_path: src.clone(),
                error_code: "NOT_FOUND".to_string(),
                error_message: "源文件不存在".to_string(),
            });
            continue;
        }

        let meta = match std::fs::metadata(&src_ext) {
            Ok(m) => m,
            Err(e) => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "META_FAILED".to_string(),
                    error_message: e.to_string(),
                });
                continue;
            }
        };

        let file_name = match src_path.file_name().and_then(|x| x.to_str()) {
            Some(n) => n.to_string(),
            None => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "BAD_FILE_NAME".to_string(),
                    error_message: "文件名无效".to_string(),
                });
                continue;
            }
        };

        let mut dst = target_dir.join(&file_name);
        let mut dst_ext = to_extended_length_path(&dst);

        if dst_ext.exists() {
            dst = resolve_conflict_name(target_dir, &file_name);
            dst_ext = to_extended_length_path(&dst);
        }

        match std::fs::rename(&src_ext, &dst_ext) {
            Ok(_) => {
                released_bytes += meta.len();
                success_items.push(MoveSuccessItem {
                    src_path: src.clone(),
                    dst_path: dst.display().to_string(),
                    size: meta.len(),
                });
            }
            Err(e) => {
                failed_items.push(MoveFailureItem {
                    src_path: src.clone(),
                    error_code: "MOVE_FAILED".to_string(),
                    error_message: e.to_string(),
                });
            }
        }
    }

    MoveResult {
        success_items,
        failed_items,
        released_bytes,
    }
}
