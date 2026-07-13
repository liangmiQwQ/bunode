//! Option behavior for parsed Node and Bunode flags.
//!
//! Token parsing decides how argv is consumed; this module decides what each supported
//! option does to parser state and Bun invocation segments.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::error::{BunodeError, CliUsageError};

use super::super::options::OptionSpec;
use super::super::{builtins, env_file};
use super::NodeCommand;
use super::state::{
  CommandMode, InvocationSegments, ParseState, PrintMode, PrintOperandMode, Source,
};

pub(super) fn apply_option(
  option: &str,
  spec: &OptionSpec,
  value: Option<OsString>,
  source: Source,
  original: Vec<OsString>,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
) -> Result<(), BunodeError> {
  ensure_source_allowed(spec, source)?;

  if state.print_mode == PrintMode::Enabled && !matches!(option, "--print" | "-p") {
    state.print_operand_mode = PrintOperandMode::Script;
  }

  if source == Source::CommandLine && spec.exec_argv {
    builder.exec_argv.extend(original);
  }

  match option {
    "--help" | "-h" => {
      if state.command_mode != CommandMode::Version {
        state.command_mode = CommandMode::Help;
      }
    }
    "--version" | "-v" => state.command_mode = CommandMode::Version,
    "--eval" | "-e" => {
      state.inline_command = Some(NodeCommand::Eval(required_option_value(value, option)?));
    }
    "--print" | "-p" => {
      // Node accepts `--print=<value>` but still reads the expression from argv operands.
      state.print_mode = PrintMode::Enabled;
      state.print_operand_mode = PrintOperandMode::Expression;
    }
    "--require" | "-r" => push_common_js_preload(builder, required_option_value(value, option)?),
    "--import" => push_es_module_preload(builder, required_option_value(value, option)?)?,
    "--experimental-import-meta-resolve" => {
      push_forward_flag(builder, "--experimental-import-meta-resolve");
    }
    "--inspect" => push_optional_forward(builder, "--inspect", value, Some("127.0.0.1:9229")),
    "--inspect-brk" => {
      push_optional_forward(builder, "--inspect-brk", value, Some("127.0.0.1:9229"));
    }
    "--inspect-wait" => {
      push_optional_forward(builder, "--inspect-wait", value, Some("127.0.0.1:9229"));
    }
    "--conditions" | "-C" => push_forward_value(builder, "--conditions", value, option)?,
    "--cpu-prof" => push_forward_flag(builder, "--cpu-prof"),
    "--cpu-prof-dir" => push_forward_value(builder, "--cpu-prof-dir", value, option)?,
    "--cpu-prof-interval" => push_forward_value(builder, "--cpu-prof-interval", value, option)?,
    "--cpu-prof-name" => push_forward_value(builder, "--cpu-prof-name", value, option)?,
    "--heap-prof" => push_forward_flag(builder, "--heap-prof"),
    "--heap-prof-dir" => push_forward_value(builder, "--heap-prof-dir", value, option)?,
    "--heap-prof-name" => push_forward_value(builder, "--heap-prof-name", value, option)?,
    "--dns-result-order" => push_forward_value(builder, "--dns-result-order", value, option)?,
    "--env-file" => push_node_env_file(builder, state, value, option, source)?,
    "--expose-gc" => push_forward_flag(builder, "--expose-gc"),
    "--no-addons" => push_forward_flag(builder, "--no-addons"),
    "--no-deprecation" => push_forward_flag(builder, "--no-deprecation"),
    "--throw-deprecation" => push_forward_flag(builder, "--throw-deprecation"),
    "--title" => push_forward_value(builder, "--title", value, option)?,
    "--unhandled-rejections" => {
      push_forward_value(builder, "--unhandled-rejections", value, option)?;
    }
    "--use-bundled-ca" => push_forward_flag(builder, "--use-bundled-ca"),
    "--use-openssl-ca" => push_forward_flag(builder, "--use-openssl-ca"),
    "--use-system-ca" => push_forward_flag(builder, "--use-system-ca"),
    "--zero-fill-buffers" => push_forward_flag(builder, "--zero-fill-buffers"),
    "--bun-config" => push_forward_value(builder, "--config", value, option)?,
    "--bun-console-depth" => push_forward_value(builder, "--console-depth", value, option)?,
    "--bun-env-file" => push_bun_env_file(builder, value, option)?,
    "--bun-fetch-preconnect" => push_forward_value(builder, "--fetch-preconnect", value, option)?,
    "--bun-hot" => push_forward_flag(builder, "--hot"),
    "--bun-install" => push_forward_value(builder, "--install", value, option)?,
    "--bun-no-clear-screen" => push_forward_flag(builder, "--no-clear-screen"),
    "--bun-no-env-file" => push_forward_flag(builder, "--no-env-file"),
    "--bun-port" => push_forward_value(builder, "--port", value, option)?,
    "--bun-prefer-latest" => push_forward_flag(builder, "--prefer-latest"),
    "--bun-prefer-offline" => push_forward_flag(builder, "--prefer-offline"),
    "--bun-preload" => push_bun_preload(builder, value, option)?,
    "--bun-smol" => push_forward_flag(builder, "--smol"),
    "--bun-user-agent" => push_forward_value(builder, "--user-agent", value, option)?,
    "--bun-watch" => push_forward_flag(builder, "--watch"),
    _ => return Err(unsupported_option(option)),
  }

  Ok(())
}

pub(super) fn ensure_source_allowed(spec: &OptionSpec, source: Source) -> Result<(), BunodeError> {
  if source == Source::NodeOptions && !spec.node_options_allowed {
    let name = spec.long.first().copied().unwrap_or("option");
    return Err(CliUsageError::NodeOptionsDisallowed(name.to_string()).into());
  }

  Ok(())
}

fn required_option_value(value: Option<OsString>, option: &str) -> Result<OsString, BunodeError> {
  value.ok_or_else(|| option_requires_value(option))
}

pub(super) fn option_requires_value(option: &str) -> BunodeError {
  CliUsageError::OptionRequiresValue(option.to_string()).into()
}

fn push_common_js_preload(builder: &mut InvocationSegments, value: OsString) {
  if !builtins::is_builtin_module(&value.to_string_lossy()) {
    builder.common_js_preloads.push(value);
  }
}

fn push_es_module_preload(
  builder: &mut InvocationSegments,
  value: OsString,
) -> Result<(), BunodeError> {
  if is_data_specifier(&value.to_string_lossy()) {
    return Err(CliUsageError::UnsupportedDataUrlImport.into());
  }

  if !builtins::is_builtin_module(&value.to_string_lossy()) {
    builder.es_module_preloads.push(value);
  }

  Ok(())
}

fn is_data_specifier(value: &str) -> bool {
  value.split_once(':').is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("data"))
}

fn push_forward_flag(builder: &mut InvocationSegments, name: &str) {
  builder.bun_options.push(OsString::from(name));
}

fn push_forward_value(
  builder: &mut InvocationSegments,
  name: &str,
  value: Option<OsString>,
  option: &str,
) -> Result<(), BunodeError> {
  builder.bun_options.push(join_option_value(name, required_option_value(value, option)?));

  Ok(())
}

fn push_optional_forward(
  builder: &mut InvocationSegments,
  name: &str,
  value: Option<OsString>,
  default: Option<&str>,
) {
  let value = value.or_else(|| default.map(OsString::from));

  builder
    .bun_options
    .push(value.map_or_else(|| OsString::from(name), |value| join_option_value(name, value)));
}

fn push_node_env_file(
  builder: &mut InvocationSegments,
  state: &mut ParseState,
  value: Option<OsString>,
  option: &str,
  source: Source,
) -> Result<(), BunodeError> {
  let value = required_option_value(value, option)?;
  validate_env_file(&value)?;

  if source == Source::CommandLine
    && state.read_env_file_node_options
    && let Some(node_options) = env_file::read_node_options(&value)?
  {
    state.env_file_node_options = Some(node_options);
  }

  builder.bun_options.push(join_option_value("--env-file", value));

  Ok(())
}

fn push_bun_env_file(
  builder: &mut InvocationSegments,
  value: Option<OsString>,
  option: &str,
) -> Result<(), BunodeError> {
  let value = required_option_value(value, option)?;
  validate_env_file(&value)?;
  builder.bun_options.push(join_option_value("--env-file", value));

  Ok(())
}

fn push_bun_preload(
  builder: &mut InvocationSegments,
  value: Option<OsString>,
  option: &str,
) -> Result<(), BunodeError> {
  builder.bun_preloads.push(join_option_value("--preload", required_option_value(value, option)?));

  Ok(())
}

pub(super) fn join_option_value(name: &str, value: OsString) -> OsString {
  let mut option = OsString::from(name);
  option.push("=");
  option.push(value);
  option
}

fn validate_env_file(path: &OsStr) -> Result<(), BunodeError> {
  let path = Path::new(path);

  if path.is_file() {
    return Ok(());
  }

  Err(CliUsageError::FileNotFound(path.to_path_buf()).into())
}

pub(super) fn unsupported_option(option: impl AsRef<OsStr>) -> BunodeError {
  CliUsageError::UnsupportedNodeOption(option.as_ref().to_string_lossy().into_owned()).into()
}
