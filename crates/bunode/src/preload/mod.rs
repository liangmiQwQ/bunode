//! Generated preload support for Node-facing process metadata.

use std::{fs, io, path::PathBuf, process};

use crate::bun;

pub const EXEC_PATH_ENV: &str = "BUNODE_EXEC_PATH";
pub const ARGV0_ENV: &str = "BUNODE_ARGV0";
pub const EXEC_ARGV_ENV: &str = "BUNODE_EXEC_ARGV";
pub const DROP_STDIN_ARGV_ENV: &str = "BUNODE_DROP_STDIN_ARGV";

const PRELOAD_SOURCE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/preload.min.js"));

pub fn prepare() -> io::Result<PathBuf> {
  let bun_path = bun::path()?;
  let bun_directory = bun_path
    .parent()
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bun directory"))?;
  let preload_file_name = preload_file_name();
  let preload_path = bun_directory.join(&preload_file_name);

  // The generated file keeps released tarballs relocatable and avoids bundling JS separately.
  fs::create_dir_all(bun_directory)?;

  if fs::read(&preload_path).is_ok_and(|content| content == PRELOAD_SOURCE) {
    return Ok(preload_path);
  }

  let temporary_path = bun_directory.join(format!(".{preload_file_name}.{}.tmp", process::id()));
  fs::write(&temporary_path, PRELOAD_SOURCE)?;

  match fs::rename(&temporary_path, &preload_path) {
    Ok(()) => {}
    Err(_) if fs::read(&preload_path).is_ok_and(|content| content == PRELOAD_SOURCE) => {
      let _ = fs::remove_file(&temporary_path);
      return Ok(preload_path);
    }
    Err(error) => {
      let _ = fs::remove_file(&temporary_path);
      return Err(error);
    }
  }

  Ok(preload_path)
}

fn preload_file_name() -> String {
  format!("bunode-preload-{:016x}.js", fnv1a(PRELOAD_SOURCE))
}

fn fnv1a(value: &[u8]) -> u64 {
  let mut result = 0xcbf2_9ce4_8422_2325;

  for byte in value {
    result ^= u64::from(*byte);
    result = result.wrapping_mul(0x0000_0100_0000_01b3);
  }

  result
}
