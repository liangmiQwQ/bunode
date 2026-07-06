use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BunodeError {
  // ---- Finding and execute bun binary ----------------------
  #[error("Error to execute bun binary: {0}")]
  CommandExecution(#[from] io::Error),

  #[error("Bun binary failed to run with code {0}")]
  CommandExecutionWithExitCode(i32),

  #[error("Bun binary not found.")]
  BunBinaryNotFound(),

  #[error("Bun binary not found at {0}.")]
  BunBinaryNotFoundWithPath(PathBuf),

  // ---- Version handling ----------------------
  #[error("Failed to parse Bun's version {0}.")]
  BadBunVersion(String),

  #[error("Failed to parse Bun's masqueraded Node.js version {0}.")]
  BadNodeCompatibleShimVersion(String),

  #[error("Bun {0} is not supported. Please use a stable Bun >=1.4.0.")]
  UnsupporttedBunVersion(String),
}
