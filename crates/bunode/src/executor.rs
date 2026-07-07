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
    base::parse(args, env::var_os("NODE_OPTIONS"), &self.shape)
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
      NodeCommand::Eval(code) => {
        let code = build_eval_expression(code);

        Self::run_bun(invocation, BunMode::Eval(code.as_os_str()))
      }
      NodeCommand::Print(code) => {
        let code = build_print_expression(code);

        Self::run_bun(invocation, BunMode::Eval(code.as_os_str()))
      }
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

    let expression = build_print_stdin_expression();

    Self::run_bun(invocation, BunMode::Eval(expression.as_os_str()))
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
      let exec_path = env::current_exe()?;
      let process_argv = build_process_argv(invocation, &mode, exec_path.as_os_str());

      // Bun sees itself as process.argv[0]; the preload patches Node-facing metadata.
      command.env(preload::EXEC_PATH_ENV, &exec_path);
      command.env(preload::ARGV0_ENV, &invocation.argv0);
      command.env(preload::EXEC_ARGV_ENV, encode_os_string_json(&invocation.exec_argv));
      command.env(preload::ARGV_ENV, encode_os_string_json(&process_argv));
      command.env(preload::REQUIRE_ENV, encode_os_string_json(&invocation.common_js_preloads));

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

fn build_process_argv(
  invocation: &ExecutionPlan,
  mode: &BunMode<'_>,
  exec_path: &OsStr,
) -> Vec<OsString> {
  let mut argv = vec![exec_path.to_os_string()];

  if let BunMode::Script(script) = mode {
    argv.push(script.to_os_string());
  }

  argv.extend(invocation.script_arguments.iter().cloned());
  argv
}

fn build_eval_expression(code: &OsStr) -> std::ffi::OsString {
  let source = js_string_literal(&code.to_string_lossy());
  let globals = node_globals_setup("[eval]");

  std::ffi::OsString::from(format!(
    "await(async()=>{{const __bunodeSource={source};let __bunodeScript=true;try{{new Function(__bunodeSource)}}catch(__bunodeError){{if(!(__bunodeError instanceof SyntaxError))throw __bunodeError;__bunodeScript=false}}if(__bunodeScript){{{globals}return globalThis.eval(__bunodeSource)}}const __bunodeUrl=URL.createObjectURL(new Blob([__bunodeSource],{{type:\"text/javascript\"}}));try{{await import(__bunodeUrl)}}finally{{URL.revokeObjectURL(__bunodeUrl)}}}})()",
  ))
}

fn build_print_expression(code: &OsStr) -> std::ffi::OsString {
  let source = js_string_literal(&code.to_string_lossy());
  let globals = node_globals_setup("[eval]");
  let helpers = print_helpers();

  std::ffi::OsString::from(format!(
    "(()=>{{{helpers}try{{const __bunodeSource={source};try{{new Function(__bunodeStripHashbang(__bunodeSource))}}catch(__bunodeError){{__bunodeRejectPrintModuleSource(__bunodeSource,__bunodeError)}}{globals}__bunodePrint(globalThis.eval(__bunodeSource))}}catch(__bunodeError){{__bunodeReportError(__bunodeError)}}}})()",
  ))
}

fn build_print_stdin_expression() -> std::ffi::OsString {
  // Read fd 0 inside Bun so preloads can exit before an unbounded pipe is drained.
  let globals = node_globals_setup("[stdin]");
  let helpers = print_helpers();

  std::ffi::OsString::from(format!(
    "(()=>{{{helpers}try{{const __bunodeFs=require(\"node:fs\");const __bunodeSource=__bunodeFs.readFileSync(0,\"utf8\");try{{new Function(__bunodeStripHashbang(__bunodeSource))}}catch(__bunodeError){{__bunodeRejectPrintModuleSource(__bunodeSource,__bunodeError)}}{globals}__bunodePrint(globalThis.eval(__bunodeSource))}}catch(__bunodeError){{__bunodeReportError(__bunodeError)}}}})()",
  ))
}

fn build_stdin_module_expression() -> std::ffi::OsString {
  // Parse first: plain stdin keeps Node's script-like globals, while ESM stdin uses a Blob module.
  let globals = node_globals_setup("[stdin]");

  std::ffi::OsString::from(format!(
    "await(async()=>{{const __bunodeStripHashbang=(source)=>{{if(!source.startsWith(\"#!\"))return source;const index=source.indexOf(String.fromCharCode(10));return source.slice(index===-1?source.length:index+1)}};const __bunodeFs=require(\"node:fs\");const __bunodeSource=__bunodeFs.readFileSync(0,\"utf8\");if(__bunodeSource.length===0)return;let __bunodeScript=true;try{{new Function(__bunodeStripHashbang(__bunodeSource))}}catch(__bunodeError){{if(!(__bunodeError instanceof SyntaxError))throw __bunodeError;__bunodeScript=false}}if(__bunodeScript){{{globals}return globalThis.eval(__bunodeSource)}}const __bunodeUrl=URL.createObjectURL(new Blob([__bunodeSource],{{type:\"text/javascript\"}}));try{{await import(__bunodeUrl)}}finally{{URL.revokeObjectURL(__bunodeUrl)}}}})()",
  ))
}

fn node_globals_setup(filename: &str) -> String {
  let filename = js_string_literal(filename);

  format!(
    "const __bunodeModule={{id:{filename},path:\".\",exports:{{}},filename:require(\"node:path\").resolve({filename}),loaded:false,children:[],paths:globalThis.module?.paths??[]}};__bunodeModule.require=require;Object.assign(globalThis,{{__filename:{filename},__dirname:\".\",require,module:__bunodeModule,exports:__bunodeModule.exports}});",
  )
}

fn js_string_literal(value: &str) -> String {
  format!("\"{}\"", escape_json_string(value))
}

const fn print_helpers() -> &'static str {
  "const __bunodeStripHashbang=(source)=>{if(!source.startsWith(\"#!\"))return source;const index=source.indexOf(String.fromCharCode(10));return source.slice(index===-1?source.length:index+1)};const __bunodeLooksLikeModuleSource=(source)=>/\\b(?:import|export|await)\\b/.test(source);const __bunodeRejectPrintModuleSource=(source,error)=>{if(error instanceof SyntaxError&&__bunodeLooksLikeModuleSource(source)){console.error(\"Error [ERR_EVAL_ESM_CANNOT_PRINT]: --print cannot be used with ESM input\");process.exit(1)}throw error};const __bunodeReportError=(error)=>{console.error(error?.stack??error);process.exit(1)};const __bunodeInspect=require(\"node:util\").inspect;const __bunodeUseColor=()=>!process.env.NO_COLOR&&process.env.FORCE_COLOR!==\"0\"&&(!!process.env.FORCE_COLOR||!!process.env.CLICOLOR_FORCE);const __bunodePrint=(value)=>{console.log(typeof value===\"string\"?value:__bunodeInspect(value,{colors:__bunodeUseColor()}))};"
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

fn encode_os_string_json(values: &[std::ffi::OsString]) -> String {
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
