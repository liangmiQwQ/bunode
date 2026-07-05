//! Bunode core
//!
//! RFC: rfcs/rust-wrapper-core.md
//! The binary used to call internal Bun.

use std::{
  env,
  ffi::{OsStr, OsString},
  io::{self, IsTerminal},
  path::Path,
  process::{ExitCode, ExitStatus},
};

mod bun;
mod cli;
mod preload;
mod version;

fn main() -> ExitCode {
  let node_options = env::var_os("NODE_OPTIONS");

  match cli::parse(env::args_os(), node_options) {
    Ok(options) => run(&options),
    Err(error) => error.exit(),
  }
}

fn run(invocation: &cli::BunodeCommandOption) -> ExitCode {
  let result = match &invocation.command {
    cli::NodeCommand::Help => {
      cli::print_help();
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
    cli::NodeCommand::Script(script) => run_bun(invocation, BunMode::Script(script)),
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

#[derive(Clone, Copy)]
enum BunMode<'a> {
  Eval(&'a OsStr),
  Print(&'a OsStr),
  Script(&'a OsStr),
  Stdin,
  Repl,
}

fn run_bun(invocation: &cli::BunodeCommandOption, mode: BunMode<'_>) -> io::Result<ExitStatus> {
  let mut command = bun::command()?;
  let preload_path = preload::prepare()?;
  let args = build_bun_args(invocation, &mode, &preload_path);

  command.args(args);

  // Bun sees itself as process.argv[0]; the preload patches Node-facing metadata.
  command.env(preload::EXEC_PATH_ENV, env::current_exe()?);
  command.env(preload::ARGV0_ENV, &invocation.argv0);

  if matches!(mode, BunMode::Stdin) {
    command.env(preload::DROP_STDIN_ARGV_ENV, "1");
  }

  command.status()
}

fn build_bun_args(
  invocation: &cli::BunodeCommandOption,
  mode: &BunMode<'_>,
  preload_path: &Path,
) -> Vec<OsString> {
  let mut args = Vec::new();

  match mode {
    BunMode::Eval(code) => {
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-e"));
      args.push((*code).to_os_string());
      args.extend(invocation.script_arguments.iter().cloned());
    }
    BunMode::Print(code) => {
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-p"));
      args.push((*code).to_os_string());
      args.extend(invocation.script_arguments.iter().cloned());
    }
    BunMode::Script(script) => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(normalize_script_name(script));
      args.extend(invocation.script_arguments.iter().cloned());
    }
    BunMode::Stdin => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-"));
      args.extend(invocation.script_arguments.iter().cloned());
    }
    BunMode::Repl => {
      args.push(OsString::from("repl"));
    }
  }

  args
}

fn push_runtime_flags(args: &mut Vec<OsString>, preload_path: &Path) {
  // Defaults go before user-translated flags so explicit Bun options can override them.
  args.push(OsString::from("--no-install"));
  args.push(OsString::from("--no-env-file"));
  args.push(join_option_value("--preload", preload_path.as_os_str()));
}

fn normalize_script_name(script: &OsStr) -> OsString {
  if script == OsStr::new("-") {
    return OsString::from("-");
  }

  let script_text = script.to_string_lossy();

  if script_text.contains('/') || script_text.contains('\\') {
    return script.to_os_string();
  }

  let mut normalized = OsString::from(if cfg!(windows) { r".\" } else { "./" });
  normalized.push(script);
  normalized
}

fn join_option_value(name: &str, value: &OsStr) -> OsString {
  let mut option = OsString::from(name);
  option.push("=");
  option.push(value);
  option
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

#[cfg(test)]
mod tests {
  use std::{ffi::OsString, path::Path};

  use super::{BunMode, build_bun_args};
  use crate::cli::{BunodeCommandOption, NodeCommand};

  fn invocation(command: NodeCommand) -> BunodeCommandOption {
    BunodeCommandOption {
      argv0: OsString::from("node"),
      command,
      bun_options: vec![OsString::from("--conditions=node")],
      script_arguments: vec![OsString::from("--flag")],
    }
  }

  #[test]
  fn script_mode_should_use_bun_run_with_safe_defaults() {
    let invocation = invocation(NodeCommand::Script(OsString::from("script.js")));
    let script = OsString::from("script.js");
    let args = build_bun_args(
      &invocation,
      &BunMode::Script(script.as_os_str()),
      Path::new("/tmp/preload.js"),
    );

    assert_eq!(
      args,
      vec![
        OsString::from("run"),
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("--preload=/tmp/preload.js"),
        OsString::from("--conditions=node"),
        OsString::from("./script.js"),
        OsString::from("--flag"),
      ],
    );
  }

  #[test]
  fn eval_mode_should_not_use_bun_run() {
    let invocation = invocation(NodeCommand::Print(OsString::from("1 + 1")));
    let code = OsString::from("1 + 1");
    let args =
      build_bun_args(&invocation, &BunMode::Print(code.as_os_str()), Path::new("/tmp/preload.js"));

    assert_eq!(
      args,
      vec![
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("--preload=/tmp/preload.js"),
        OsString::from("--conditions=node"),
        OsString::from("-p"),
        OsString::from("1 + 1"),
        OsString::from("--flag"),
      ],
    );
  }
}
