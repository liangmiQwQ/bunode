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
    return Err(CliError::failure(format!(
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
      args.push(normalize_eval_code(code));
      push_user_arguments(&mut args, invocation, true);
    }
    BunMode::Print(code) => {
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-p"));
      args.push(normalize_print_code(code));
      push_user_arguments(&mut args, invocation, true);
    }
    BunMode::Script(script) => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(normalize_script_name(script));
      push_user_arguments(&mut args, invocation, false);
    }
    BunMode::Stdin => {
      args.push(OsString::from("run"));
      push_runtime_flags(&mut args, preload_path);
      args.extend(invocation.bun_options.iter().cloned());
      args.push(OsString::from("-"));
      push_user_arguments(&mut args, invocation, false);
    }
    BunMode::Repl => {
      return build_repl_args(invocation);
    }
  }

  args
}

pub fn build_repl_args(invocation: &BunodeCommandOption) -> Vec<OsString> {
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

fn normalize_print_code(code: &OsStr) -> OsString {
  if code.is_empty() {
    return OsString::from("undefined");
  }

  code.to_os_string()
}

fn push_user_arguments(
  args: &mut Vec<OsString>,
  invocation: &BunodeCommandOption,
  escape_delimiters: bool,
) {
  if invocation.script_arguments.is_empty() {
    return;
  }

  // Bun consumes this separator, preserving a user-supplied `--` as argv data.
  args.push(OsString::from("--"));

  for argument in &invocation.script_arguments {
    if escape_delimiters && argument == OsStr::new("--") {
      args.push(OsString::from("--"));
    }

    args.push(argument.clone());
  }
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
    base::argv::{BunMode, build_bun_args, build_repl_args, validate_script},
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
  fn eval_and_print_modes_should_normalize_empty_code() {
    let eval_invocation = invocation(NodeCommand::Eval(OsString::new()));
    let print_invocation = invocation(NodeCommand::Print(OsString::new()));
    let empty = OsString::new();

    let eval_args = build_bun_args(
      &eval_invocation,
      &BunMode::Eval(empty.as_os_str()),
      Path::new("/tmp/preload.js"),
    );
    let print_args = build_bun_args(
      &print_invocation,
      &BunMode::Print(empty.as_os_str()),
      Path::new("/tmp/preload.js"),
    );

    assert_eq!(eval_args[5], OsString::from("void 0"));
    assert_eq!(print_args[5], OsString::from("undefined"));
  }

  #[test]
  fn eval_mode_should_escape_user_delimiters() {
    let mut invocation = invocation(NodeCommand::Print(OsString::from("process.argv")));
    invocation.script_arguments =
      vec![OsString::from("a"), OsString::from("--"), OsString::from("b")];
    let code = OsString::from("process.argv");
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
      "bunode: script `--script.js` starts with `-`; pass it with an explicit relative path like `./--script.js`.",
    );
  }
}
