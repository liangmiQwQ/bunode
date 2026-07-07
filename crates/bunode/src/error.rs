use std::{io, path::PathBuf, process::ExitCode};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BunodeError {
  #[error(transparent)]
  CliUsage(#[from] CliUsageError),

  #[error(transparent)]
  CliFailure(#[from] CliFailureError),

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

#[derive(Error, Debug)]
pub enum CliUsageError {
  #[error("{0}")]
  ArgumentParse(String),

  #[error("unsupported Node.js option `{0}`")]
  UnsupportedNodeOption(String),

  #[error("option `{0}` does not take a value")]
  OptionDoesNotTakeValue(String),

  #[error("option `{0}` requires a value")]
  OptionRequiresValue(String),

  #[error("`{0}` is not allowed in NODE_OPTIONS")]
  NodeOptionsDisallowed(String),

  #[error("{}: not found", .0.display())]
  FileNotFound(PathBuf),

  #[error("unterminated quote in NODE_OPTIONS")]
  UnterminatedNodeOptionsQuote,

  #[error("data URL imports passed to --import are not supported")]
  UnsupportedDataUrlImport,

  #[error(
    "`node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.\nUse `node --inspect` / `node --inspect-brk` compatible flags instead."
  )]
  UnsupportedNodeInspect,
}

#[derive(Error, Debug)]
pub enum CliFailureError {
  #[error("script `{0}` starts with `-`; pass it with an explicit relative path like `./{0}`.")]
  DashScriptRequiresExplicitRelativePath(String),

  #[error("failed to read {}: {source}", path.display())]
  ReadEnvFile {
    path: PathBuf,
    #[source]
    source: io::Error,
  },
}

impl BunodeError {
  pub fn exit_code(&self) -> ExitCode {
    match self {
      Self::CliUsage(_) => ExitCode::from(9),
      _ => ExitCode::from(1),
    }
  }

  pub fn print(&self) {
    eprintln!("bunode: {self}");
  }
}
