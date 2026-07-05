//! Universal Node command data shared by wrapper layers.

use std::{ffi::OsString, fmt, process::ExitCode};

#[derive(Debug, PartialEq, Eq)]
pub struct BunodeCommandOption {
  pub argv0: OsString,
  pub command: NodeCommand,
  pub bun_options: Vec<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCommand {
  Help,
  Version,
  Eval(OsString),
  Print(OsString),
  Script(OsString),
  Direct,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
  message: String,
}

impl CliError {
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: format!("bunode: {}", message.into()) }
  }

  pub fn exit(&self) -> ExitCode {
    eprintln!("{self}");
    ExitCode::from(1)
  }
}

impl fmt::Display for CliError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}
