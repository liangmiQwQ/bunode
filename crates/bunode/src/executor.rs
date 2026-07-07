//! Version-selected Bunode executor.

use std::{
  env,
  ffi::{OsStr, OsString},
  fmt::Write as _,
  io::{self, IsTerminal},
  process::{Command, ExitCode, ExitStatus},
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
      NodeCommand::Script(script) if script == OsStr::new("-") => {
        Self::run_script_stdin(invocation)
      }
      NodeCommand::Script(script) => {
        base::argv::validate_script(script)?;
        Self::run_bun(invocation, BunMode::Script(script))
      }
      NodeCommand::Direct => {
        if io::stdin().is_terminal() {
          Self::run_bun(invocation, BunMode::Repl)
        } else {
          Self::run_stdin(invocation)
        }
      }
    }
  }

  fn run_stdin(invocation: &ExecutionPlan) -> Result<ExecutionResult, BunodeError> {
    let code = build_stdin_module_expression();

    Self::run_bun(invocation, BunMode::Eval(code.as_os_str()))
  }

  fn run_script_stdin(invocation: &ExecutionPlan) -> Result<ExecutionResult, BunodeError> {
    let invocation = invocation_with_script_argument(invocation, OsString::from("-"));
    let code = build_stdin_module_expression();

    Self::run_bun(&invocation, BunMode::Eval(code.as_os_str()))
  }

  fn run_print_stdin(invocation: &ExecutionPlan) -> Result<ExecutionResult, BunodeError> {
    if io::stdin().is_terminal() {
      return Self::run_bun(invocation, BunMode::Repl);
    }

    let expression = build_stdin_eval_expression();

    Self::run_bun(invocation, BunMode::Print(expression.as_os_str()))
  }

  fn run_bun(
    invocation: &ExecutionPlan,
    mode: BunMode<'_>,
  ) -> Result<ExecutionResult, BunodeError> {
    let command = Self::configure_bun(invocation, mode)?;

    Ok(ExecutionResult::Status(run_configured_bun(command)?))
  }

  fn configure_bun(invocation: &ExecutionPlan, mode: BunMode<'_>) -> Result<Command, BunodeError> {
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

      args
    };

    command.args(args);

    Ok(command)
  }
}

fn invocation_with_script_argument(
  invocation: &ExecutionPlan,
  argument: OsString,
) -> ExecutionPlan {
  let mut invocation = invocation.clone();

  invocation.script_arguments.insert(0, argument);
  invocation
}

fn build_stdin_eval_expression() -> std::ffi::OsString {
  // Read fd 0 inside Bun so preloads can exit before an unbounded pipe is drained.
  std::ffi::OsString::from(
    "(()=>{const __bunodeFs=require(\"node:fs\");const __bunodeModule=globalThis.module??{exports:{}};const __bunodeExports=globalThis.exports??__bunodeModule.exports;Object.assign(globalThis,{__filename:\"[stdin]\",__dirname:\".\",require,module:__bunodeModule,exports:__bunodeExports});return globalThis.eval(__bunodeFs.readFileSync(0,\"utf8\"))})()",
  )
}

fn build_stdin_module_expression() -> std::ffi::OsString {
  // Parse first: plain stdin keeps Node's script-like globals, while ESM stdin uses a Blob module.
  std::ffi::OsString::from(
    "await(async()=>{const __bunodeFs=require(\"node:fs\");const __bunodeSource=__bunodeFs.readFileSync(0,\"utf8\");if(__bunodeSource.length===0)return;try{new Function(__bunodeSource);const __bunodeModule=globalThis.module??{exports:{}};const __bunodeExports=globalThis.exports??__bunodeModule.exports;Object.assign(globalThis,{__filename:\"[stdin]\",__dirname:\".\",require,module:__bunodeModule,exports:__bunodeExports});return globalThis.eval(__bunodeSource)}catch(__bunodeError){if(!(__bunodeError instanceof SyntaxError))throw __bunodeError;const __bunodeUrl=URL.createObjectURL(new Blob([__bunodeSource],{type:\"text/javascript\"}));try{await import(__bunodeUrl)}finally{URL.revokeObjectURL(__bunodeUrl)}}})()",
  )
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
