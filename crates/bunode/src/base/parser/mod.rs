//! Node option parsing and translation into Bun runtime flags.
//!
//! `lexopt` owns argv tokenization. Bunode keeps Node-specific CLI semantics explicit
//! and produces an execution plan for the executor.

use std::ffi::OsString;

use crate::error::BunodeError;

use super::options::OptionShape;

// Option effects: update parser state and translated Bun invocation segments.
mod actions;
// Environment source preprocessing: split NODE_OPTIONS before token parsing.
mod node_options;
// Parser state: track command selection, operands, preloads, and output segments.
mod state;
// Token stream driver: consume lexopt tokens according to Node CLI rules.
mod tokens;

use node_options::split_node_options;
use state::{InvocationSegments, ParseState, Source};
use tokens::parse_tokens;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionPlan {
  pub argv0: OsString,
  pub command: NodeCommand,
  pub exec_argv: Vec<OsString>,
  pub bun_options: Vec<OsString>,
  pub common_js_preloads: Vec<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCommand {
  Help,
  Version,
  Eval(OsString),
  Print(OsString),
  PrintStdin,
  Script(OsString),
  Direct,
}

pub fn parse<I, T>(
  args: I,
  node_options: Option<OsString>,
  shape: &OptionShape,
) -> Result<ExecutionPlan, BunodeError>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString>,
{
  let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

  if node_options.is_none() {
    let (invocation, env_file_node_options) = parse_once(&args, None, shape, true)?;

    if env_file_node_options.as_ref().is_some_and(|value| !value.is_empty()) {
      return parse_once(&args, env_file_node_options, shape, false)
        .map(|(invocation, _)| invocation);
    }

    return Ok(invocation);
  }

  parse_once(&args, node_options, shape, false).map(|(invocation, _)| invocation)
}

fn parse_once(
  args: &[OsString],
  node_options: Option<OsString>,
  shape: &OptionShape,
  read_env_file_node_options: bool,
) -> Result<(ExecutionPlan, Option<OsString>), BunodeError> {
  // 1. Keep argv0 for process.argv0 correction in the generated preload.
  let argv0 = args.first().cloned().unwrap_or_else(|| OsString::from("node"));
  let mut state = ParseState { read_env_file_node_options, ..ParseState::default() };
  let mut builder = InvocationSegments::default();

  // 2. NODE_OPTIONS behaves as if it appears before CLI flags.
  if let Some(node_options) = node_options.filter(|value| !value.is_empty()) {
    let node_options = split_node_options(&node_options)?;
    parse_tokens(&node_options, Source::NodeOptions, &mut state, &mut builder, shape)?;
  }

  // 3. CLI operands stop option parsing once the script position is reached.
  parse_tokens(
    args.get(1..).unwrap_or_default(),
    Source::CommandLine,
    &mut state,
    &mut builder,
    shape,
  )?;

  state.finish(argv0, builder)
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use semver::Version;

  use super::{ExecutionPlan, NodeCommand};

  use super::parse;

  fn parse_cli(args: &[&str]) -> Result<ExecutionPlan, crate::error::BunodeError> {
    let shape = super::super::options::option_shape_for_bun(&Version::new(1, 3, 14));

    parse(args, None, &shape)
  }

  fn parse_with_node_options(
    args: &[&str],
    node_options: &str,
  ) -> Result<ExecutionPlan, crate::error::BunodeError> {
    let shape = super::super::options::option_shape_for_bun(&Version::new(1, 3, 14));

    parse(args, Some(OsString::from(node_options)), &shape)
  }

  fn assert_common_js_preloads(actual: &[OsString], expected: &[&str]) {
    let expected = expected.iter().map(|value| OsString::from(*value)).collect::<Vec<_>>();

    assert_eq!(actual, expected);
  }

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--inspect")],
        bun_options: vec![OsString::from("--inspect=127.0.0.1:9229")],
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_inspect_value_before_script_operand() -> Result<(), crate::error::BunodeError>
  {
    let options = parse_cli(&["node", "--inspect=127.0.0.1:9229", "script.js"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--inspect=127.0.0.1:9229")],
        bun_options: vec![OsString::from("--inspect=127.0.0.1:9229")],
        common_js_preloads: Vec::new(),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_double_dash_as_end_of_bunode_options()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("--script.js")),
        exec_argv: Vec::new(),
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_empty_script_operand_as_stdin_argument()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--", "", "arg"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Direct,
        exec_argv: Vec::new(),
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::new(), OsString::from("arg")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_eval_operands_as_arguments() -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "-p", "process.argv.slice(1)", "first", "--second"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Print(OsString::from("process.argv.slice(1)")),
        exec_argv: vec![OsString::from("-p"), OsString::from("process.argv.slice(1)")],
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("first"), OsString::from("--second")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_continue_options_after_print_expression() -> Result<(), crate::error::BunodeError>
  {
    let options =
      parse_cli(&["node", "-p", "process.argv.slice(1)", "--conditions=custom", "first"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Print(OsString::from("process.argv.slice(1)")),
        exec_argv: vec![
          OsString::from("-p"),
          OsString::from("process.argv.slice(1)"),
          OsString::from("--conditions=custom"),
        ],
        bun_options: vec![OsString::from("--conditions=custom")],
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("first")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_let_later_print_operand_replace_earlier_inline_command()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "-e", "\"eval\"", "-p", "\"print\""])?;

    assert_eq!(options.command, NodeCommand::Print(OsString::from("\"print\"")));

    let options = parse_cli(&["node", "-p", "\"a\"", "-p", "\"b\""])?;

    assert_eq!(options.command, NodeCommand::Print(OsString::from("\"b\"")));

    Ok(())
  }

  #[test]
  fn parse_should_prefer_version_over_help() -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--help", "--version"])?;

    assert_eq!(options.command, NodeCommand::Version);

    let options = parse_cli(&["node", "--version", "--help"])?;

    assert_eq!(options.command, NodeCommand::Version);

    Ok(())
  }

  #[test]
  fn parse_should_defer_print_without_expression_to_stdin() -> Result<(), crate::error::BunodeError>
  {
    let options = parse_cli(&["node", "-p"])?;

    assert_eq!(options.command, NodeCommand::PrintStdin);
    assert_eq!(options.exec_argv, vec![OsString::from("-p")]);

    Ok(())
  }

  #[test]
  fn parse_should_treat_dash_print_operand_as_stdin_argument()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "-p", "-", "arg"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::PrintStdin,
        exec_argv: vec![OsString::from("-p")],
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("-"), OsString::from("arg")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_print_after_double_dash_as_script() -> Result<(), crate::error::BunodeError>
  {
    let options = parse_cli(&["node", "-p", "--", "script.js", "--flag"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("-p")],
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_print_operand_after_option_as_script()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "-p", "--conditions", "custom", "script.js", "--flag"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![
          OsString::from("-p"),
          OsString::from("--conditions"),
          OsString::from("custom"),
        ],
        bun_options: vec![OsString::from("--conditions=custom")],
        common_js_preloads: Vec::new(),
        script_arguments: vec![OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_support_print_eval_shortcut() -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "-pe", "1 + 1"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Print(OsString::from("1 + 1")),
        exec_argv: vec![OsString::from("-pe"), OsString::from("1 + 1")],
        bun_options: Vec::new(),
        common_js_preloads: Vec::new(),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_translate_node_options_before_cli_options()
  -> Result<(), crate::error::BunodeError> {
    let options =
      parse_with_node_options(&["node", "--conditions", "cli", "script.js"], "--conditions env")?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--conditions"), OsString::from("cli")],
        bun_options: vec![OsString::from("--conditions=env"), OsString::from("--conditions=cli")],
        common_js_preloads: Vec::new(),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_reject_command_options_from_node_options() {
    let error = parse_with_node_options(&["node"], "--eval 1").unwrap_err();

    assert_eq!(error.to_string(), "`--eval` is not allowed in NODE_OPTIONS");
  }

  #[test]
  fn parse_should_reject_env_file_from_node_options() {
    let error = parse_with_node_options(&["node"], "--env-file .env").unwrap_err();

    assert_eq!(error.to_string(), "`--env-file` is not allowed in NODE_OPTIONS");
  }

  #[test]
  fn parse_should_reject_attached_short_values() {
    let error = parse_cli(&["node", "-econsole.log(1)"]).unwrap_err();

    assert_eq!(error.to_string(), "unsupported Node.js option `-econsole.log(1)`",);
  }

  #[test]
  fn parse_should_reject_short_equals_values() {
    let error = parse_cli(&["node", "-p=e"]).unwrap_err();

    assert_eq!(error.to_string(), "unsupported Node.js option `-p=e`");
  }

  #[test]
  fn parse_should_reject_missing_option_value_before_next_flag() {
    let error = parse_cli(&["node", "--require", "--eval", "0"]).unwrap_err();

    assert_eq!(error.to_string(), "option `--require` requires a value");
  }

  #[test]
  fn parse_should_reject_empty_inline_required_value() {
    let error = parse_cli(&["node", "--eval="]).unwrap_err();

    assert_eq!(error.to_string(), "option `--eval` requires a value");
  }

  #[test]
  fn parse_should_reject_empty_inline_optional_value() {
    let error = parse_cli(&["node", "--inspect="]).unwrap_err();

    assert_eq!(error.to_string(), "option `--inspect` requires a value");
  }

  #[test]
  fn parse_should_validate_env_file_before_early_exit() {
    let error =
      parse_cli(&["node", "--env-file", "missing-bunode-env-file.env", "--version"]).unwrap_err();

    assert_eq!(error.to_string(), "missing-bunode-env-file.env: not found");
  }

  #[test]
  fn parse_should_hide_bunode_options_from_exec_argv() -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--bun-smol", "--conditions", "cli", "-e", "0"])?;

    assert_eq!(
      options.exec_argv,
      vec![
        OsString::from("--conditions"),
        OsString::from("cli"),
        OsString::from("-e"),
        OsString::from("0"),
      ],
    );

    Ok(())
  }

  #[test]
  fn parse_should_run_common_js_preloads_before_es_module_imports()
  -> Result<(), crate::error::BunodeError> {
    let options =
      parse_cli(&["node", "--import", "./esm.mjs", "--require", "./cjs.cjs", "-e", "0"])?;

    assert_common_js_preloads(&options.common_js_preloads, &["./cjs.cjs"]);
    assert_eq!(options.bun_options, vec![OsString::from("--preload=./esm.mjs")]);

    Ok(())
  }

  #[test]
  fn parse_should_run_node_options_preloads_before_bun_preloads()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_with_node_options(
      &["node", "--bun-preload", "./cli.js", "-e", "0"],
      "--require ./env.cjs",
    )?;

    assert_common_js_preloads(&options.common_js_preloads, &["./env.cjs"]);
    assert_eq!(options.bun_options, vec![OsString::from("--preload=./cli.js")]);

    Ok(())
  }

  #[test]
  fn parse_should_reject_data_url_imports() {
    let error =
      parse_cli(&["node", "--import", "DATA:text/javascript,globalThis.loaded=1", "-e", "0"])
        .unwrap_err();

    assert_eq!(error.to_string(), "data URL imports passed to --import are not supported");
  }

  #[test]
  fn parse_should_skip_builtin_preloads() -> Result<(), crate::error::BunodeError> {
    let options = parse_cli(&["node", "--import", "node:fs", "--require", "fs", "-e", "0"])?;

    assert_eq!(
      options.exec_argv,
      vec![
        OsString::from("--import"),
        OsString::from("node:fs"),
        OsString::from("--require"),
        OsString::from("fs"),
        OsString::from("-e"),
        OsString::from("0"),
      ],
    );
    assert_eq!(options.bun_options, Vec::<OsString>::new());
    assert_eq!(options.common_js_preloads, Vec::<OsString>::new());

    Ok(())
  }

  #[test]
  fn parse_should_translate_node_options_from_env_file() -> Result<(), crate::error::BunodeError> {
    let path = std::env::temp_dir().join(format!(
      "bunode-node-options-{}-{}.env",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos(),
    ));
    std::fs::write(&path, "NODE_OPTIONS=\"--conditions from-env\"\n")
      .expect("test env file should be writable");
    let path = path.to_string_lossy().to_string();

    let options = parse_cli(&["node", "--env-file", &path, "--conditions", "cli", "-e", "0"])?;
    std::fs::remove_file(&path).expect("test env file should be removable");

    assert_eq!(
      options.bun_options,
      vec![
        OsString::from("--conditions=from-env"),
        super::actions::join_option_value("--env-file", OsString::from(&path)),
        OsString::from("--conditions=cli"),
      ],
    );

    Ok(())
  }

  #[test]
  fn parse_should_not_read_node_options_from_bun_env_file() -> Result<(), crate::error::BunodeError>
  {
    let path = std::env::temp_dir().join(format!(
      "bunode-bun-env-file-{}-{}.env",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos(),
    ));
    std::fs::write(&path, "NODE_OPTIONS=\"--eval 1\"\n").expect("test env file should be writable");
    let path = path.to_string_lossy().to_string();

    let options = parse_cli(&["node", "--bun-env-file", &path, "-e", "0"])?;
    std::fs::remove_file(&path).expect("test env file should be removable");

    assert_eq!(
      options.bun_options,
      vec![super::actions::join_option_value("--env-file", path.into())]
    );

    Ok(())
  }

  #[test]
  fn parse_should_not_read_env_file_node_options_when_real_node_options_exists()
  -> Result<(), crate::error::BunodeError> {
    let path = std::env::temp_dir().join(format!(
      "bunode-real-node-options-{}-{}.env",
      std::process::id(),
      std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos(),
    ));
    std::fs::write(&path, "NODE_OPTIONS=\"--bad\n").expect("test env file should be writable");
    let path = path.to_string_lossy().to_string();

    let options =
      parse_with_node_options(&["node", "--env-file", &path, "-e", "0"], "--conditions x")?;
    std::fs::remove_file(&path).expect("test env file should be removable");

    assert_eq!(
      options.bun_options,
      vec![
        OsString::from("--conditions=x"),
        super::actions::join_option_value("--env-file", path.into()),
      ],
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_double_quoted_node_options_value() -> Result<(), crate::error::BunodeError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require \"./with space.js\"")?;

    assert_eq!(options.bun_options, Vec::<OsString>::new());
    assert_common_js_preloads(&options.common_js_preloads, &["./with space.js"]);

    Ok(())
  }

  #[test]
  fn parse_should_keep_single_quotes_as_node_options_literal()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require './preload.js'")?;

    assert_eq!(options.bun_options, Vec::<OsString>::new());
    assert_common_js_preloads(&options.common_js_preloads, &["'./preload.js'"]);

    Ok(())
  }

  #[test]
  fn parse_should_preserve_node_options_backslashes() -> Result<(), crate::error::BunodeError> {
    let options = parse_with_node_options(&["node", "-e", "0"], r"--require C:\tmp\preload.js")?;

    assert_eq!(options.bun_options, Vec::<OsString>::new());
    assert_common_js_preloads(&options.common_js_preloads, &[r"C:\tmp\preload.js"]);

    Ok(())
  }

  #[test]
  fn parse_should_keep_escaped_double_quote_in_node_options()
  -> Result<(), crate::error::BunodeError> {
    let options = parse_with_node_options(&["node", "-e", "0"], r#"--require "./x\" y.js""#)?;

    assert_eq!(options.bun_options, Vec::<OsString>::new());
    assert_common_js_preloads(&options.common_js_preloads, &[r#"./x" y.js"#]);

    Ok(())
  }
}
