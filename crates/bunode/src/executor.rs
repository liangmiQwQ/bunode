//! Version-selected Bunode executor.

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

use crate::{
  base::{self, ExecutionPlan, NodeCommand, OptionShape, argv::BunMode},
  bun,
  error::BunodeError,
  preload, version,
};

pub enum ExecutionResult {
  ExitCode(ExitCode),
  Status(ExitStatus),
}

pub fn run_current<I, T>(args: I) -> Result<ExecutionResult, BunodeError>
where
  I: IntoIterator<Item = T>,
  T: Into<std::ffi::OsString>,
{
  let versions = version::current()?;
  let executor = Executor::new(versions);
  let invocation = executor.parse(args)?;

  executor.run(&invocation)
}

struct Executor {
  versions: version::RuntimeVersions,
  shape: OptionShape,
}

impl Executor {
  const fn new(versions: version::RuntimeVersions) -> Self {
    let shape = base::option_shape_for_bun(&versions.bun);

    Self { versions, shape }
  }

  fn parse<I, T>(&self, args: I) -> Result<ExecutionPlan, BunodeError>
  where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
  {
    Ok(base::parse(args, env::var_os("NODE_OPTIONS"), &self.shape)?)
  }

  fn run(&self, invocation: &ExecutionPlan) -> Result<ExecutionResult, BunodeError> {
    match &invocation.command {
      NodeCommand::Help => {
        base::help::print(&self.shape);
        Ok(ExecutionResult::ExitCode(ExitCode::SUCCESS))
      }
      NodeCommand::Version => {
        println!("{}", self.versions.bunode_version_text());
        Ok(ExecutionResult::ExitCode(ExitCode::SUCCESS))
      }
      NodeCommand::Eval(code) => Self::run_bun(invocation, BunMode::Eval(code)),
      NodeCommand::Print(code) => Self::run_bun(invocation, BunMode::Print(code)),
      NodeCommand::PrintStdin => Self::run_print_stdin(invocation),
      NodeCommand::Script(script) => {
        base::argv::validate_script(script)?;
        Self::run_bun(invocation, BunMode::Script(script))
      }
      NodeCommand::Direct => {
        if io::stdin().is_terminal() {
          Self::run_bun(invocation, BunMode::Repl)
        } else {
          Self::run_bun(invocation, BunMode::Stdin)
        }
      }
    }
  }

  fn run_print_stdin(invocation: &ExecutionPlan) -> Result<ExecutionResult, BunodeError> {
    if io::stdin().is_terminal() {
      return Self::run_bun(invocation, BunMode::Print(OsStr::new("undefined")));
    }

    let mut code = String::new();
    io::stdin().read_to_string(&mut code)?;

    if code.is_empty() {
      return Self::run_bun(invocation, BunMode::Print(OsStr::new("undefined")));
    }

    let source_path = write_print_stdin_source(&code)?;
    let expression = build_print_stdin_expression(&source_path);

    Self::run_bun(invocation, BunMode::Print(expression.as_os_str()))
  }

  fn run_bun(
    invocation: &ExecutionPlan,
    mode: BunMode<'_>,
  ) -> Result<ExecutionResult, BunodeError> {
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

    Ok(ExecutionResult::Status(run_configured_bun(command)?))
  }
}

fn write_print_stdin_source(code: &str) -> Result<PathBuf, BunodeError> {
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
