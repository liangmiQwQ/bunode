//! Native Bunode prefix manager.
//!
//! RFC: rfcs/bunode-cli.md

use std::{env, fmt, process::ExitCode};

#[path = "cli/args.rs"]
mod args;
#[path = "cli/config.rs"]
mod config;
#[path = "cli/download.rs"]
mod download;
#[path = "cli/prefix.rs"]
mod prefix;

type Result<T> = std::result::Result<T, CliError>;

#[derive(Debug)]
struct CliError(String);

impl CliError {
  fn new(message: impl Into<String>) -> Self {
    Self(message.into())
  }
}

impl fmt::Display for CliError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.0)
  }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
  fn from(error: std::io::Error) -> Self {
    Self(error.to_string())
  }
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("bunode: {error}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<()> {
  match args::parse(env::args_os())? {
    args::Command::Help => args::print_help(),
    args::Command::Version => println!("bunode {}", env!("CARGO_PKG_VERSION")),
    args::Command::Patch(options) => prefix::patch(&options)?,
    args::Command::Revert(options) => prefix::revert(&options)?,
    args::Command::List => prefix::list()?,
    args::Command::Implode { yes } => prefix::implode(yes)?,
    args::Command::Update { yes } => prefix::update(yes)?,
  }

  Ok(())
}
