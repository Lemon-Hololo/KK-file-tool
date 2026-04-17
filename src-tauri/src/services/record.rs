use std::path::Path;

use chrono::Local;

use crate::{db::hash_repo, error::AppResult, models::HashIndexEntry};

pub fn save_hash_record(
    db_path: &Path,
    record_name: &str,
    source_paths: &[String],
    entries: &[HashIndexEntry],
) -> AppResult<String> {
    let rid = hash_repo::insert_hash_record(
        db_path,
        record_name,
        source_paths,
        entries,
        Local::now().timestamp(),
    )?;
    Ok(rid)
}
