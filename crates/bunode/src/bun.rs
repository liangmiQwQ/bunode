//! This module is used to call `bun` binary.
//! This module should only include `bun` binary finding, and command construction.
//! Any wrapper logic (translate, argv generation) should be put outside of this module.

use std::{env, path::PathBuf, process::Command};

use crate::error::BunodeError;

pub fn command() -> Result<Command, BunodeError> {
  let bun_path = path()?;

  if bun_path.exists() {
    Ok(Command::new(bun_path))
  } else {
    Err(BunodeError::BunBinaryNotFoundWithPath(bun_path))
  }
}

pub fn bun_binary_directory() -> Result<PathBuf, BunodeError> {
  let executable = env::current_exe()?;
  let executable_dir = executable.parent().ok_or(BunodeError::BunBinaryNotFound())?;

  #[cfg(windows)]
  let result = { executable_dir.join("bun") };
  #[cfg(not(windows))]
  let result = { executable_dir.join("..").join("bun") };

  Ok(result)
}

pub fn path() -> Result<PathBuf, BunodeError> {
  let bun_binary_directory = bun_binary_directory()?;

  #[cfg(windows)]
  let result = { bun_binary_directory.join("bun.exe") };
  #[cfg(not(windows))]
  let result = { bun_binary_directory.join("bun") };

  Ok(result)
}
