use std::{fs::File, io::Read, path::Path};

use crate::utils::path::to_extended_length_path;

pub fn hash_file_blake3(path: &Path) -> Result<String, String> {
  let ep = to_extended_length_path(path);
  let mut file = File::open(ep).map_err(|e| e.to_string())?;
  let mut hasher = blake3::Hasher::new();
  let mut buf = [0u8; 1024 * 1024];

  loop {
    let n = file.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
      break;
    }
    hasher.update(&buf[..n]);
  }

  Ok(hasher.finalize().to_hex().to_string())
}