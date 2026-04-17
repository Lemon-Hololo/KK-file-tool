use std::path::Path;

use crate::utils::path::to_extended_length_path;

pub fn metadata_len(path: &Path) -> Option<u64> {
  let ep = to_extended_length_path(path);
  std::fs::metadata(ep).ok().map(|m| m.len())
}