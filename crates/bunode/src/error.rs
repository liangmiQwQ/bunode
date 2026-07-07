use std::{io, path::PathBuf, process::ExitCode};

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
#[error("bunode: {message}")]
pub struct CliError {
  message: String,
  exit_code: u8,
}

impl CliError {
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: message.into(), exit_code: 9 }
  }

  pub fn failure(message: impl Into<String>) -> Self {
    Self { message: message.into(), exit_code: 1 }
  }

  pub fn exit_code(&self) -> ExitCode {
    ExitCode::from(self.exit_code)
  }
}

#[derive(Error, Debug)]
pub enum BunodeError {
  #[error(transparent)]
  Cli(#[from] CliError),

  // ---- Finding and execute bun binary ----------------------
  #[error("Error to execute bun binary: {0}")]
  CommandExecution(#[from] io::Error),

  #[error("Bun binary failed to run with code {0}")]
  CommandExecutionWithExitCode(i32),

  #[error("Bun binary not found.")]
  BunBinaryNotFound(),

  #[error("Bun binary not found at {0}.")]
  BunBinaryNotFoundWithPath(PathBuf),

  // ---- Preload handling ----------------------
  #[error("Failed to prepare Bunode preload at {path}: {source}")]
  PreloadPreparation {
    path: PathBuf,
    #[source]
    source: io::Error,
  },

  // ---- Version handling ----------------------
  #[error("Failed to parse Bun's version {0}.")]
  BadBunVersion(String),

  #[error("Failed to parse Bun's masqueraded Node.js version {0}.")]
  BadNodeCompatibleShimVersion(String),

  #[error("Bun {0} is not supported. Please use a stable Bun >=1.4.0.")]
  UnsupporttedBunVersion(String),
}

impl BunodeError {
  pub fn exit_code(&self) -> ExitCode {
    match self {
      Self::Cli(error) => error.exit_code(),
      _ => ExitCode::from(1),
    }
  }

  pub fn print(&self) {
    match self {
      Self::Cli(error) => eprintln!("{error}"),
      _ => eprintln!("bunode: {self}"),
    }
  }
}
