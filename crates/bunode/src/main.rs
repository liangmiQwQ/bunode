//! Bunode core
//!
//! RFC: rfcs/rust-wrapper-core.md
//! The binary used to call internal Bun.

use std::{
  env,
  io::{self, IsTerminal},
  process::{ExitCode, ExitStatus},
};

use base::argv::BunMode;

mod base;
mod bun;
mod cli;
mod preload;
mod version;

fn main() -> ExitCode {
  let node_options = env::var_os("NODE_OPTIONS");

  match base::parse(env::args_os(), node_options) {
    Ok(options) => run(&options),
    Err(error) => error.exit(),
  }
}

fn run(invocation: &cli::BunodeCommandOption) -> ExitCode {
  let result = match &invocation.command {
    cli::NodeCommand::Help => {
      base::help::print();
      return ExitCode::SUCCESS;
    }
    cli::NodeCommand::Version => {
      return match version::bunode_version() {
        Ok(version) => {
          println!("{version}");
          ExitCode::SUCCESS
        }
        Err(error) => io_error_exit(&error),
      };
    }
    cli::NodeCommand::Eval(code) => run_bun(invocation, BunMode::Eval(code)),
    cli::NodeCommand::Print(code) => run_bun(invocation, BunMode::Print(code)),
    cli::NodeCommand::Script(script) => {
      if let Err(error) = base::argv::validate_script(script) {
        return error.exit();
      }

      run_bun(invocation, BunMode::Script(script))
    }
    cli::NodeCommand::Direct => {
      if io::stdin().is_terminal() {
        run_bun(invocation, BunMode::Repl)
      } else {
        run_bun(invocation, BunMode::Stdin)
      }
    }
  };

  match result {
    Ok(status) => status_exit_code(status),
    Err(error) => io_error_exit(&error),
  }
}

fn run_bun(invocation: &cli::BunodeCommandOption, mode: BunMode<'_>) -> io::Result<ExitStatus> {
  let mut command = bun::command()?;
  let preload_path = preload::prepare()?;
  let args = base::argv::build_bun_args(invocation, &mode, &preload_path);

  command.args(args);

  // Bun sees itself as process.argv[0]; the preload patches Node-facing metadata.
  command.env(preload::EXEC_PATH_ENV, env::current_exe()?);
  command.env(preload::ARGV0_ENV, &invocation.argv0);

  if matches!(mode, BunMode::Stdin) {
    command.env(preload::DROP_STDIN_ARGV_ENV, "1");
  }

  command.status()
}

fn status_exit_code(status: ExitStatus) -> ExitCode {
  let code = status.code().map_or(1, |code| {
    let bounded_code = code.clamp(0, i32::from(u8::MAX));

    u8::try_from(bounded_code).map_or(1, |code| code)
  });

  ExitCode::from(code)
}

fn io_error_exit(error: &io::Error) -> ExitCode {
  eprintln!("bunode: {error}");
  ExitCode::from(1)
}
