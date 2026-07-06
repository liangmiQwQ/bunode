//! Universal Node command data shared by wrapper layers.

use std::{ffi::OsString, fmt, process::ExitCode};

use clap::Command;

pub trait CliOptionSchema {
  fn augment_command(command: Command) -> Command;
}

pub fn option_command<Schema>() -> Command
where
  Schema: CliOptionSchema,
{
  Schema::augment_command(Command::new("node").disable_help_flag(true).disable_version_flag(true))
}

#[derive(Debug, PartialEq, Eq)]
pub struct BunodeCommandOption {
  pub argv0: OsString,
  pub command: NodeCommand,
  pub exec_argv: Vec<OsString>,
  pub bun_options: Vec<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCommand {
  Help,
  Version,
  Eval(OsString),
  Print(OsString),
  PrintStdin,
  Script(OsString),
  Direct,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
  message: String,
  exit_code: u8,
}

impl CliError {
  pub fn new(message: impl Into<String>) -> Self {
    Self { message: format!("bunode: {}", message.into()), exit_code: 9 }
  }

  pub fn failure(message: impl Into<String>) -> Self {
    Self { message: format!("bunode: {}", message.into()), exit_code: 1 }
  }

  pub fn exit(&self) -> ExitCode {
    eprintln!("{self}");
    ExitCode::from(self.exit_code)
  }
}

impl fmt::Display for CliError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}
