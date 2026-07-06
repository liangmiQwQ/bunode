//! Bunode core
//!
//! RFC: rfcs/rust-wrapper-core.md
//! The binary used to call internal Bun.

use std::{
  env,
  process::{ExitCode, ExitStatus},
};

mod base;
mod bun;
mod cli;
mod error;
mod executor;
mod preload;
mod version;

fn main() -> ExitCode {
  match executor::run_current(env::args_os()) {
    Ok(result) => result_exit_code(&result),
    Err(error) => {
      error.print();
      error.exit_code()
    }
  }
}

fn result_exit_code(result: &executor::ExecutionResult) -> ExitCode {
  match result {
    executor::ExecutionResult::ExitCode(code) => *code,
    executor::ExecutionResult::Status(status) => status_exit_code(*status),
  }
}

fn status_exit_code(status: ExitStatus) -> ExitCode {
  #[cfg(unix)]
  {
    use std::os::unix::process::ExitStatusExt;

    if let Some(signal) = status.signal() {
      let code = 128 + signal;
      let bounded_code = code.clamp(0, i32::from(u8::MAX));

      return ExitCode::from(u8::try_from(bounded_code).unwrap_or(1));
    }
  }

  let code = status.code().map_or(1, |code| {
    let bounded_code = code.clamp(0, i32::from(u8::MAX));

    u8::try_from(bounded_code).map_or(1, |code| code)
  });

  ExitCode::from(code)
}
