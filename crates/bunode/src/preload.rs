//! Generated preload support for Node-facing process metadata.

use std::{fs, io, path::PathBuf};

use crate::bun;

pub(crate) const EXEC_PATH_ENV: &str = "BUNODE_EXEC_PATH";
pub(crate) const ARGV0_ENV: &str = "BUNODE_ARGV0";
pub(crate) const DROP_STDIN_ARGV_ENV: &str = "BUNODE_DROP_STDIN_ARGV";

const PRELOAD_FILE_NAME: &str = "bunode-preload.js";
const PRELOAD_SOURCE: &[u8] = include_bytes!("preload.js");

pub(crate) fn prepare() -> io::Result<PathBuf> {
  let bun_path = bun::path()?;
  let bun_directory = bun_path
    .parent()
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bun directory"))?;
  let preload_path = bun_directory.join(PRELOAD_FILE_NAME);

  // The generated file keeps released tarballs relocatable and avoids bundling JS separately.
  fs::create_dir_all(bun_directory)?;

  if fs::read(&preload_path).is_ok_and(|content| content == PRELOAD_SOURCE) {
    return Ok(preload_path);
  }

  fs::write(&preload_path, PRELOAD_SOURCE)?;
  Ok(preload_path)
}
