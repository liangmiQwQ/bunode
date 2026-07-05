//! Bunode core
//!
//! RFC: rfcs/rust-wrapper-core.md
//! The binary used to call internal Bun

use std::{env, process::ExitCode};

mod bun;
mod cli;

fn main() -> ExitCode {
  match cli::parse(env::args_os()) {
    Ok(options) => run(options),
    Err(error) => error.exit(),
  }
}

fn run(invocation: cli::BunodeCommandOption) -> ExitCode {
  let cli::BunodeCommandOption { node_options, script, script_arguments } = invocation;

  if node_options.help {
    cli::print_help();
    return ExitCode::SUCCESS;
  }

  let _ = (node_options, script, script_arguments);
  ExitCode::SUCCESS
}
