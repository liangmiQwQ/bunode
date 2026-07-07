//! Bun argv construction for translated Node invocations.

use std::{
  ffi::{OsStr, OsString},
  path::Path,
};

use crate::{
  base::ExecutionPlan,
  error::{BunodeError, CliFailureError},
};

#[derive(Clone, Copy)]
pub enum BunMode<'a> {
  Eval(&'a OsStr),
  Script(&'a OsStr),
  Repl,
}

pub fn validate_script(script: &OsStr) -> Result<(), BunodeError> {
  if script_requires_explicit_relative_path(script) {
    return Err(
      CliFailureError::DashScriptRequiresExplicitRelativePath(
        script.to_string_lossy().into_owned(),
      )
      .into(),
    );
  }

  Ok(())
}

pub fn build_bun_args(
  invocation: &ExecutionPlan,
  mode: &BunMode<'_>,
  preload_path: &Path,
) -> Vec<OsString> {
  let mut args = Vec::new();

  match mode {
    BunMode::Eval(code) => {
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-e"));
      args.push(normalize_eval_code(code));
      push_user_arguments(&mut args, invocation, true);
    }
    BunMode::Script(script) => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(normalize_script_name(script));
      push_user_arguments(&mut args, invocation, false);
    }
    BunMode::Repl => {
      return build_repl_args(invocation);
    }
  }

  args
}

pub fn build_repl_args(invocation: &ExecutionPlan) -> Vec<OsString> {
  let mut args = invocation.bun_options.clone();

  args.push(OsString::from("repl"));
  args
}

fn push_runtime_flags(args: &mut Vec<OsString>, preload_path: &Path) {
  // Defaults go before user-translated flags so explicit Bun options can override them.
  args.push(OsString::from("--no-install"));
  args.push(OsString::from("--no-env-file"));
  args.push(join_option_value("--preload", preload_path.as_os_str()));
}

fn normalize_eval_code(code: &OsStr) -> OsString {
  if code.is_empty() {
    return OsString::from("void 0");
  }

  code.to_os_string()
}

fn push_user_arguments(
  args: &mut Vec<OsString>,
  invocation: &ExecutionPlan,
  escape_delimiters: bool,
) {
  if invocation.script_arguments.is_empty() {
    return;
  }

  // Bun consumes this separator, preserving a user-supplied `--` as argv data.
  args.push(OsString::from("--"));

  let mut previous_argument_was_delimiter = false;

  for argument in &invocation.script_arguments {
    if escape_delimiters && argument == OsStr::new("--") && !previous_argument_was_delimiter {
      args.push(OsString::from("--"));
    }

    previous_argument_was_delimiter = argument == OsStr::new("--");
    args.push(argument.clone());
  }
}

fn normalize_script_name(script: &OsStr) -> OsString {
  if script == OsStr::new("-") {
    return OsString::from("-");
  }

  let script_text = script.to_string_lossy();

  if has_path_separator(&script_text) {
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

  script.starts_with('-') && !has_path_separator(&script)
}

fn has_path_separator(value: &str) -> bool {
  value.contains('/') || (cfg!(windows) && value.contains('\\'))
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
    base::argv::{BunMode, build_bun_args, build_repl_args, validate_script},
    base::{ExecutionPlan, NodeCommand},
  };

  fn invocation(command: NodeCommand) -> ExecutionPlan {
    ExecutionPlan {
      argv0: OsString::from("node"),
      command,
      exec_argv: Vec::new(),
      bun_options: vec![OsString::from("--conditions=node")],
      common_js_preloads: Vec::new(),
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
    let invocation = invocation(NodeCommand::Eval(OsString::from("1 + 1")));
    let code = OsString::from("1 + 1");
    let args =
      build_bun_args(&invocation, &BunMode::Eval(code.as_os_str()), Path::new("/tmp/preload.js"));

    assert_eq!(
      args,
      vec![
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("--preload=/tmp/preload.js"),
        OsString::from("--conditions=node"),
        OsString::from("-e"),
        OsString::from("1 + 1"),
        OsString::from("--"),
        OsString::from("--flag"),
      ],
    );
  }

  #[test]
  fn eval_mode_should_normalize_empty_code() {
    let eval_invocation = invocation(NodeCommand::Eval(OsString::new()));
    let empty = OsString::new();

    let eval_args = build_bun_args(
      &eval_invocation,
      &BunMode::Eval(empty.as_os_str()),
      Path::new("/tmp/preload.js"),
    );

    assert_eq!(eval_args[5], OsString::from("void 0"));
  }

  #[test]
  fn eval_mode_should_escape_user_delimiters() {
    let mut invocation = invocation(NodeCommand::Print(OsString::from("process.argv")));
    invocation.script_arguments =
      vec![OsString::from("a"), OsString::from("--"), OsString::from("b")];
    let code = OsString::from("process.argv");
    let args =
      build_bun_args(&invocation, &BunMode::Eval(code.as_os_str()), Path::new("/tmp/preload.js"));

    assert_eq!(
      args,
      vec![
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("--preload=/tmp/preload.js"),
        OsString::from("--conditions=node"),
        OsString::from("-e"),
        OsString::from("process.argv"),
        OsString::from("--"),
        OsString::from("a"),
        OsString::from("--"),
        OsString::from("--"),
        OsString::from("b"),
      ],
    );
  }

  #[test]
  fn eval_mode_should_escape_consecutive_user_delimiters_once() {
    let mut invocation = invocation(NodeCommand::Print(OsString::from("process.argv")));
    invocation.script_arguments =
      vec![OsString::from("--"), OsString::from("--"), OsString::from("b")];
    let code = OsString::from("process.argv");
    let args =
      build_bun_args(&invocation, &BunMode::Eval(code.as_os_str()), Path::new("/tmp/preload.js"));

    assert_eq!(
      args,
      vec![
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("--preload=/tmp/preload.js"),
        OsString::from("--conditions=node"),
        OsString::from("-e"),
        OsString::from("process.argv"),
        OsString::from("--"),
        OsString::from("--"),
        OsString::from("--"),
        OsString::from("--"),
        OsString::from("b"),
      ],
    );
  }

  #[test]
  fn repl_mode_should_forward_runtime_flags() {
    let invocation = invocation(NodeCommand::Direct);

    assert_eq!(
      build_repl_args(&invocation),
      vec![OsString::from("--conditions=node"), OsString::from("repl")],
    );
  }

  #[test]
  fn validate_script_should_require_explicit_relative_path_for_dash_script() {
    let error = validate_script(OsStr::new("--script.js")).unwrap_err();

    assert_eq!(
      error.to_string(),
      "script `--script.js` starts with `-`; pass it with an explicit relative path like `./--script.js`.",
    );
  }

  #[cfg(not(windows))]
  #[test]
  fn script_mode_should_prefix_unix_script_names_with_backslashes() {
    let invocation = invocation(NodeCommand::Script(OsString::from(r"foo\bar.js")));
    let script = OsString::from(r"foo\bar.js");
    let args = build_bun_args(
      &invocation,
      &BunMode::Script(script.as_os_str()),
      Path::new("/tmp/preload.js"),
    );

    assert_eq!(args[5], OsString::from(r"./foo\bar.js"));
  }
}
