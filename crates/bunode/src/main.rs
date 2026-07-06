//! Bunode core
//!
//! RFC: rfcs/rust-wrapper-core.md
//! The binary used to call internal Bun.

use std::{
  env,
  ffi::OsStr,
  fmt::Write as _,
  fs,
  io::{self, IsTerminal, Read},
  path::{Path, PathBuf},
  process::{ExitCode, ExitStatus},
  time::{SystemTime, UNIX_EPOCH},
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
    cli::NodeCommand::PrintStdin => run_print_stdin(invocation),
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

fn run_print_stdin(invocation: &cli::BunodeCommandOption) -> io::Result<ExitStatus> {
  if io::stdin().is_terminal() {
    return run_bun(invocation, BunMode::Print(OsStr::new("undefined")));
  }

  let mut code = String::new();
  io::stdin().read_to_string(&mut code)?;

  if code.is_empty() {
    return run_bun(invocation, BunMode::Print(OsStr::new("undefined")));
  }

  let source_path = write_print_stdin_source(&code)?;
  let expression = build_print_stdin_expression(&source_path);

  run_bun(invocation, BunMode::Print(expression.as_os_str()))
}

fn write_print_stdin_source(code: &str) -> io::Result<PathBuf> {
  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
  let mut path = env::temp_dir();

  path.push(format!("bunode-print-stdin-{}-{timestamp}.js", std::process::id()));
  fs::write(&path, code)?;

  Ok(path)
}

fn build_print_stdin_expression(path: &Path) -> std::ffi::OsString {
  let path = escape_json_string(&path.to_string_lossy());

  // Bun's `-p` requires argv code, so stdin source moves through a temp file to avoid argv limits.
  std::ffi::OsString::from(format!(
    "try{{eval(require(\"node:fs\").readFileSync(\"{path}\",\"utf8\"))}}finally{{require(\"node:fs\").rmSync(\"{path}\",{{force:true}})}}",
  ))
}

fn run_bun(invocation: &cli::BunodeCommandOption, mode: BunMode<'_>) -> io::Result<ExitStatus> {
  let mut command = bun::command()?;
  let args = if matches!(mode, BunMode::Repl) {
    base::argv::build_repl_args(invocation)
  } else {
    let preload_path = preload::prepare()?;
    let args = base::argv::build_bun_args(invocation, &mode, &preload_path);

    // Bun sees itself as process.argv[0]; the preload patches Node-facing metadata.
    command.env(preload::EXEC_PATH_ENV, env::current_exe()?);
    command.env(preload::ARGV0_ENV, &invocation.argv0);
    command.env(preload::EXEC_ARGV_ENV, encode_exec_argv_json(&invocation.exec_argv));

    if matches!(mode, BunMode::Stdin) {
      command.env(preload::DROP_STDIN_ARGV_ENV, "1");
    }

    args
  };

  command.args(args);

  run_configured_bun(command)
}

#[cfg(unix)]
fn run_configured_bun(mut command: std::process::Command) -> io::Result<ExitStatus> {
  use std::os::unix::process::CommandExt;

  Err(command.exec())
}

#[cfg(not(unix))]
fn run_configured_bun(mut command: std::process::Command) -> io::Result<ExitStatus> {
  command.status()
}

fn encode_exec_argv_json(values: &[std::ffi::OsString]) -> String {
  let values = values
    .iter()
    .map(|value| format!("\"{}\"", escape_json_string(&value.to_string_lossy())))
    .collect::<Vec<_>>()
    .join(",");

  format!("[{values}]")
}

fn escape_json_string(value: &str) -> String {
  let mut result = String::new();

  for character in value.chars() {
    match character {
      '"' => result.push_str("\\\""),
      '\\' => result.push_str("\\\\"),
      '\n' => result.push_str("\\n"),
      '\r' => result.push_str("\\r"),
      '\t' => result.push_str("\\t"),
      character if character.is_control() => {
        let _ = write!(result, "\\u{:04x}", u32::from(character));
      }
      character => result.push(character),
    }
  }

  result
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

fn io_error_exit(error: &io::Error) -> ExitCode {
  eprintln!("bunode: {error}");
  ExitCode::from(1)
}
