//! Generated preload support for Node-facing process metadata.

use std::{
  env,
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
  let bun_path = bun::path()?;
  let bun_directory = bun_path
    .parent()
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bun directory"))?;

  prepare_in_directory(bun_directory, PRELOAD_FILE_NAME)
    .or_else(|_| prepare_temporary(PRELOAD_FILE_NAME))
    .map_err(Into::into)
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

fn prepare_temporary(preload_file_name: &str) -> io::Result<PathBuf> {
  // Some installations keep the Bun directory read-only, so fall back to a per-user stable copy.
  prepare_in_directory(&fallback_preload_directory(), preload_file_name)
}

fn fallback_preload_directory() -> PathBuf {
  if cfg!(windows) {
    return env::var_os("LOCALAPPDATA")
      .or_else(|| env::var_os("USERPROFILE"))
      .map_or_else(env::temp_dir, PathBuf::from)
      .join("bunode");
  }

  env::var_os("XDG_CACHE_HOME")
    .map(PathBuf::from)
    .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
    .unwrap_or_else(env::temp_dir)
    .join("bunode")
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
