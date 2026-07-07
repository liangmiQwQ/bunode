//! Generated preload support for Node-facing process metadata.

use std::{
  fs::{self, OpenOptions},
  io::{self, Write},
  path::{Path, PathBuf},
  process,
  time::{SystemTime, UNIX_EPOCH},
};

use crate::{bun, error::BunodeError};

pub const EXEC_PATH_ENV: &str = "BUNODE_EXEC_PATH";
pub const ARGV0_ENV: &str = "BUNODE_ARGV0";
pub const EXEC_ARGV_ENV: &str = "BUNODE_EXEC_ARGV";
pub const ARGV_ENV: &str = "BUNODE_ARGV";
pub const REQUIRE_ENV: &str = "BUNODE_REQUIRE";

const PRELOAD_FILE_NAME: &str = "bunode-preload.cjs";
const PRELOAD_SOURCE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/preload.min.cjs"));

pub fn prepare() -> Result<PathBuf, BunodeError> {
  let directory = bun::bun_binary_directory()?;
  let preload_path = directory.join(PRELOAD_FILE_NAME);

  prepare_in_directory(&directory, PRELOAD_FILE_NAME)
    .map_err(|source| BunodeError::PreloadPreparation { path: preload_path, source })
}

fn prepare_in_directory(directory: &Path, preload_file_name: &str) -> io::Result<PathBuf> {
  let preload_path = directory.join(preload_file_name);

  // The generated file keeps released tarballs relocatable and avoids bundling JS separately.
  fs::create_dir_all(directory)?;

  if fs::read(&preload_path).is_ok_and(|content| content == PRELOAD_SOURCE) {
    return Ok(preload_path);
  }

  let temporary_path =
    write_private_preload_file(directory, &format!(".{preload_file_name}"), ".tmp")?;

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

fn write_private_preload_file(directory: &Path, prefix: &str, suffix: &str) -> io::Result<PathBuf> {
  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();

  for attempt in 0..32 {
    let path = directory.join(format!("{prefix}.{}.{timestamp}.{attempt}{suffix}", process::id()));
    let mut options = OpenOptions::new();

    options.write(true).create_new(true);

    #[cfg(unix)]
    {
      use std::os::unix::fs::OpenOptionsExt;

      options.mode(0o600);
    }

    match options.open(&path) {
      Ok(mut file) => {
        file.write_all(PRELOAD_SOURCE)?;
        return Ok(path);
      }
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
      Err(error) => return Err(error),
    }
  }

  Err(io::Error::new(io::ErrorKind::AlreadyExists, "failed to create Bunode preload file"))
}
