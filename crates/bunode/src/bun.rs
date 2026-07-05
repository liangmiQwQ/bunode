//! This module is used to call `bun` binary.
//! This module should only include `bun` binary finding, and exported `bun` function.
//! Any wrapper logic (translate, argv generation) should be put outside of this module.
//!
//! Core function: `bun` function, it receives argvs you push to Bun.

use std::{
  env,
  ffi::OsString,
  io,
  path::PathBuf,
  process::{Command, ExitStatus},
};

#[allow(dead_code)]
pub fn bun(args: &[OsString]) -> io::Result<ExitStatus> {
  // We don't modify output. (See rfcs/rust-wrapper-core.md)
  Command::new(find()?).args(args).status()
}

fn find() -> io::Result<PathBuf> {
  let executable = env::current_exe()?;
  let executable_dir = executable.parent().ok_or_else(|| {
    io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bunode executable directory")
  })?;

  #[cfg(windows)]
  let result = { executable_dir.join("bun").join("bun.exe") };
  #[cfg(not(windows))]
  let result = { executable_dir.join("..").join("bun").join("bun") };

  Ok(result)
}
