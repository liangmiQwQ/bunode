//! Bun argv construction for translated Node invocations.

use std::{
  ffi::{OsStr, OsString},
  path::Path,
};

use crate::cli::{BunodeCommandOption, CliError};

#[derive(Clone, Copy)]
pub enum BunMode<'a> {
  Eval(&'a OsStr),
  Print(&'a OsStr),
  Script(&'a OsStr),
  Stdin,
  Repl,
}

pub fn validate_script(script: &OsStr) -> Result<(), CliError> {
  if script_requires_explicit_relative_path(script) {
    return Err(CliError::new(format!(
      "script `{}` starts with `-`; pass it with an explicit relative path like `./{}`.",
      script.to_string_lossy(),
      script.to_string_lossy(),
    )));
  }

  Ok(())
}

pub fn build_bun_args(
  invocation: &BunodeCommandOption,
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
      push_user_arguments(&mut args, invocation);
    }
    BunMode::Print(code) => {
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-p"));
      args.push((*code).to_os_string());
      push_user_arguments(&mut args, invocation);
    }
    BunMode::Script(script) => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(normalize_script_name(script));
      push_user_arguments(&mut args, invocation);
    }
    BunMode::Stdin => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-"));
      push_user_arguments(&mut args, invocation);
    }
    BunMode::Repl => {
      return build_repl_args();
    }
  }

  args
}

pub fn build_repl_args() -> Vec<OsString> {
  vec![OsString::from("repl")]
}

fn push_runtime_flags(args: &mut Vec<OsString>, preload_path: &Path) {
  // Defaults go before user-translated flags so explicit Bun options can override them.
  args.push(OsString::from("--no-install"));
  args.push(OsString::from("--no-env-file"));
  args.push(join_option_value("--preload", preload_path.as_os_str()));
}

fn push_user_arguments(args: &mut Vec<OsString>, invocation: &BunodeCommandOption) {
  if invocation.script_arguments.is_empty() {
    return;
  }

  // Bun consumes this separator, preserving a user-supplied `--` as argv data.
  args.push(OsString::from("--"));
  args.extend(invocation.script_arguments.iter().cloned());
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

fn script_requires_explicit_relative_path(script: &OsStr) -> bool {
  if script == OsStr::new("-") {
    return false;
  }

  let script = script.to_string_lossy();

  script.starts_with('-') && !script.contains('/') && !script.contains('\\')
}

fn join_option_value(name: &str, value: &OsStr) -> OsString {
  let mut option = OsString::from(name);
  option.push("=");
  option.push(value);
  option
}

#[cfg(test)]
mod tests {
  use std::{
    ffi::{OsStr, OsString},
    path::Path,
  };

  use crate::{
    base::argv::{BunMode, build_bun_args, validate_script},
    cli::{BunodeCommandOption, NodeCommand},
  };

  fn invocation(command: NodeCommand) -> BunodeCommandOption {
    BunodeCommandOption {
      argv0: OsString::from("node"),
      command,
      exec_argv: Vec::new(),
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
        OsString::from(if cfg!(windows) { r".\script.js" } else { "./script.js" }),
        OsString::from("--"),
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
        OsString::from("--"),
        OsString::from("--flag"),
      ],
    );
  }

  #[test]
  fn validate_script_should_require_explicit_relative_path_for_dash_script() {
    let error = validate_script(OsStr::new("--script.js")).unwrap_err();

    assert_eq!(
      error.to_string(),
      "bunode: script `--script.js` starts with `-`; pass it with an explicit relative path like `./--script.js`.",
    );
  }
}
