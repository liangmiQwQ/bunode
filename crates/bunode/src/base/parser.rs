//! Node option parsing and translation into Bun runtime flags.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::cli::{BunodeCommandOption, CliError, NodeCommand};

use super::options::{HelpSection, OPTION_SPECS, OptionAction, OptionSpec, ValueMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
  CommandLine,
  NodeOptions,
}

#[derive(Default, PartialEq, Eq)]
enum CommandMode {
  #[default]
  Normal,
  Help,
  Version,
}

#[derive(Default, PartialEq, Eq)]
enum PrintMode {
  #[default]
  Disabled,
  Enabled,
}

#[derive(Default, PartialEq, Eq)]
enum PrintOperandMode {
  #[default]
  Expression,
  Script,
}

#[derive(Default, PartialEq, Eq)]
enum OperandBoundary {
  #[default]
  ScriptPosition,
  DoubleDash,
}

#[derive(Default)]
struct ParseState {
  command_mode: CommandMode,
  inline_command: Option<NodeCommand>,
  print_mode: PrintMode,
  print_operand_mode: PrintOperandMode,
  operand_boundary: OperandBoundary,
  exec_argv: Vec<OsString>,
  bun_options: Vec<OsString>,
  operands: Vec<OsString>,
}

pub fn parse<I, T>(args: I, node_options: Option<OsString>) -> Result<BunodeCommandOption, CliError>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString>,
{
  // 1. Keep argv0 for process.argv0 correction in the generated preload.
  let mut args = args.into_iter().map(Into::into);
  let argv0 = args.next().unwrap_or_else(|| OsString::from("node"));
  let mut state = ParseState::default();

  // 2. NODE_OPTIONS behaves as if it appears before CLI flags.
  if let Some(node_options) = node_options.filter(|value| !value.is_empty()) {
    let node_options = split_node_options(&node_options)?;
    parse_tokens(&node_options, Source::NodeOptions, &mut state)?;
  }

  // 3. CLI operands stop option parsing once the script position is reached.
  let args = args.collect::<Vec<_>>();
  parse_tokens(&args, Source::CommandLine, &mut state)?;

  let parsed = state.finish()?;

  Ok(BunodeCommandOption {
    argv0,
    command: parsed.command,
    exec_argv: parsed.exec_argv,
    bun_options: parsed.bun_options,
    script_arguments: parsed.script_arguments,
  })
}

struct ParsedState {
  command: NodeCommand,
  exec_argv: Vec<OsString>,
  bun_options: Vec<OsString>,
  script_arguments: Vec<OsString>,
}

fn parse_tokens(
  tokens: &[OsString],
  source: Source,
  state: &mut ParseState,
) -> Result<(), CliError> {
  let mut index = 0;

  while index < tokens.len() {
    let token = tokens[index].clone();
    let token_text = token.to_string_lossy();

    if token_text == "--" {
      if source == Source::NodeOptions {
        return Err(CliError::new("`--` is not allowed in NODE_OPTIONS"));
      }

      state.operand_boundary = OperandBoundary::DoubleDash;
      state.operands.extend(tokens[(index + 1)..].iter().cloned());
      break;
    }

    if token_text == "-" || !token_text.starts_with('-') {
      if source == Source::NodeOptions {
        return Err(CliError::new(format!("`{token_text}` is not allowed in NODE_OPTIONS")));
      }

      state.operands.push(token);
      state.operands.extend(tokens[(index + 1)..].iter().cloned());
      break;
    }

    if token_text.starts_with("--") {
      index = parse_long_option(tokens, index, source, state)?;
    } else {
      index = parse_short_option(tokens, index, source, state)?;
    }
  }

  Ok(())
}

fn parse_long_option(
  tokens: &[OsString],
  index: usize,
  source: Source,
  state: &mut ParseState,
) -> Result<usize, CliError> {
  let token = tokens[index].to_string_lossy();
  let (name, inline_value) = split_long_option(&token);
  let Some(spec) = find_long_option(name) else {
    return Err(unsupported_option(name));
  };

  let (value, next_index) = if matches!(spec.action, OptionAction::Print) {
    (inline_value.map(OsString::from), index + 1)
  } else {
    match spec.value {
      ValueMode::None => {
        if inline_value.is_some() {
          return Err(CliError::new(format!("option `{name}` does not take a value")));
        }

        (None, index + 1)
      }
      ValueMode::Required => {
        if let Some(value) = inline_value {
          if value.is_empty() {
            return Err(CliError::new(format!("option `{name}` requires a value")));
          }

          (Some(OsString::from(value)), index + 1)
        } else {
          let value = required_next_token(tokens, index + 1, name)?;
          (Some(value), index + 2)
        }
      }
      ValueMode::OptionalEquals => (inline_value.map(OsString::from), index + 1),
    }
  };

  apply_option(spec, value, source, state)?;
  record_exec_argv(spec, source, tokens, index, next_index, state);

  Ok(next_index)
}

fn parse_short_option(
  tokens: &[OsString],
  index: usize,
  source: Source,
  state: &mut ParseState,
) -> Result<usize, CliError> {
  let token = tokens[index].to_string_lossy();

  if token == "-pe" {
    return parse_print_eval_shortcut(tokens, index, source, state);
  }

  let Some(short) = token[1..].chars().next() else {
    return Err(unsupported_option(&token));
  };
  let Some(spec) = find_short_option(short) else {
    return Err(unsupported_option(&token));
  };
  let rest = &token[(1 + short.len_utf8())..];
  let option_name = format!("-{short}");

  if !rest.is_empty() {
    return Err(unsupported_option(&token));
  }

  let (value, next_index) = if matches!(spec.action, OptionAction::Print) {
    (None, index + 1)
  } else {
    match spec.value {
      ValueMode::None | ValueMode::OptionalEquals => (None, index + 1),
      ValueMode::Required => {
        (Some(required_next_token(tokens, index + 1, &option_name)?), index + 2)
      }
    }
  };

  apply_option(spec, value, source, state)?;
  record_exec_argv(spec, source, tokens, index, next_index, state);

  Ok(next_index)
}

fn parse_print_eval_shortcut(
  tokens: &[OsString],
  index: usize,
  source: Source,
  state: &mut ParseState,
) -> Result<usize, CliError> {
  let print_spec = find_short_option('p').ok_or_else(|| unsupported_option("-p"))?;
  let eval_spec = find_short_option('e').ok_or_else(|| unsupported_option("-e"))?;
  let value = required_next_token(tokens, index + 1, "-e")?;

  apply_option(print_spec, None, source, state)?;
  apply_option(eval_spec, Some(value), source, state)?;

  if source == Source::CommandLine {
    state.exec_argv.extend(tokens[index..(index + 2)].iter().cloned());
  }

  Ok(index + 2)
}

fn split_long_option(token: &str) -> (&str, Option<&str>) {
  token.split_once('=').map_or((token, None), |(name, value)| (name, Some(value)))
}

fn required_next_token(
  tokens: &[OsString],
  index: usize,
  option: &str,
) -> Result<OsString, CliError> {
  let Some(value) = tokens.get(index) else {
    return Err(CliError::new(format!("option `{option}` requires a value")));
  };

  if value.to_string_lossy().starts_with('-') {
    return Err(CliError::new(format!("option `{option}` requires a value")));
  }

  Ok(value.clone())
}

fn find_long_option(name: &str) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.long.contains(&name))
}

fn find_short_option(short: char) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.short == Some(short))
}

fn record_exec_argv(
  spec: &OptionSpec,
  source: Source,
  tokens: &[OsString],
  start: usize,
  end: usize,
  state: &mut ParseState,
) {
  if source == Source::CommandLine
    && spec.help.is_some_and(|help| help.section == HelpSection::Node)
  {
    state.exec_argv.extend(tokens[start..end].iter().cloned());
  }
}

fn apply_option(
  spec: &OptionSpec,
  value: Option<OsString>,
  source: Source,
  state: &mut ParseState,
) -> Result<(), CliError> {
  if source == Source::NodeOptions && !spec.node_options_allowed {
    let name = spec.long.first().copied().unwrap_or("option");
    return Err(CliError::new(format!("`{name}` is not allowed in NODE_OPTIONS")));
  }

  if state.print_mode == PrintMode::Enabled
    && state.inline_command.is_none()
    && !matches!(spec.action, OptionAction::Print)
  {
    state.print_operand_mode = PrintOperandMode::Script;
  }

  match spec.action {
    OptionAction::Help => state.command_mode = CommandMode::Help,
    OptionAction::Version => state.command_mode = CommandMode::Version,
    OptionAction::Eval => {
      state.inline_command = Some(NodeCommand::Eval(required_action_value(value, spec)?));
    }
    OptionAction::Print => {
      // Node accepts `--print=<value>` but still reads the expression from argv operands.
      state.print_mode = PrintMode::Enabled;
    }
    OptionAction::ForwardFlag(name) => state.bun_options.push(OsString::from(name)),
    OptionAction::ForwardValue(name) => {
      let value = required_action_value(value, spec)?;

      if name == "--env-file" {
        validate_env_file(&value)?;
      }

      state.bun_options.push(join_option_value(name, value));
    }
    OptionAction::ForwardOptionalValue(name) => {
      state
        .bun_options
        .push(value.map_or_else(|| OsString::from(name), |value| join_option_value(name, value)));
    }
  }

  Ok(())
}

fn required_action_value(value: Option<OsString>, spec: &OptionSpec) -> Result<OsString, CliError> {
  value.ok_or_else(|| {
    let name = spec.long.first().copied().unwrap_or("option");
    CliError::new(format!("option `{name}` requires a value"))
  })
}

fn join_option_value(name: &str, value: OsString) -> OsString {
  let mut option = OsString::from(name);
  option.push("=");
  option.push(value);
  option
}

fn validate_env_file(path: &OsStr) -> Result<(), CliError> {
  let path = Path::new(path);

  if path.is_file() {
    return Ok(());
  }

  Err(CliError::new(format!("{}: not found", path.display())))
}

fn unsupported_option(option: &str) -> CliError {
  CliError::new(format!("unsupported Node.js option `{option}`"))
}

fn split_node_options(value: &OsStr) -> Result<Vec<OsString>, CliError> {
  let value = value.to_string_lossy();
  let mut result = Vec::new();
  let mut current = String::new();
  let mut quote = None;

  // NODE_OPTIONS follows shell-like quoting, but it is parsed without a shell.
  let mut characters = value.chars().peekable();

  while let Some(character) = characters.next() {
    if quote.is_some() && character == '\\' && characters.peek() == Some(&'"') {
      characters.next();
      current.push('"');
      continue;
    }

    if Some(character) == quote {
      quote = None;
      continue;
    }

    if quote.is_none() && character == '"' {
      quote = Some(character);
      continue;
    }

    if quote.is_none() && character.is_whitespace() {
      if !current.is_empty() {
        result.push(OsString::from(std::mem::take(&mut current)));
      }

      continue;
    }

    current.push(character);
  }

  if quote.is_some() {
    return Err(CliError::new("unterminated quote in NODE_OPTIONS"));
  }

  if !current.is_empty() {
    result.push(OsString::from(current));
  }

  Ok(result)
}

impl ParseState {
  fn finish(mut self) -> Result<ParsedState, CliError> {
    let (command, script_operand_count) = self.command()?;
    let script_arguments =
      self.operands.iter().skip(script_operand_count).cloned().collect::<Vec<_>>();

    Ok(ParsedState {
      command,
      exec_argv: self.exec_argv,
      bun_options: self.bun_options,
      script_arguments,
    })
  }

  fn command(&mut self) -> Result<(NodeCommand, usize), CliError> {
    if self.command_mode == CommandMode::Help {
      return Ok((NodeCommand::Help, 0));
    }

    if self.command_mode == CommandMode::Version {
      return Ok((NodeCommand::Version, 0));
    }

    if let Some(command) = &self.inline_command {
      let command = if self.print_mode == PrintMode::Enabled {
        match command {
          NodeCommand::Eval(code) | NodeCommand::Print(code) => NodeCommand::Print(code.clone()),
          command => command.clone(),
        }
      } else {
        command.clone()
      };

      return Ok((command, 0));
    }

    if self.print_mode == PrintMode::Enabled
      && (self.operand_boundary != OperandBoundary::DoubleDash || self.operands.is_empty())
      && (self.print_operand_mode == PrintOperandMode::Expression || self.operands.is_empty())
    {
      let expression =
        self.operands.first().cloned().unwrap_or_else(|| OsString::from("undefined"));

      if let Some(expression) = self.operands.first() {
        self.exec_argv.push(expression.clone());
      }

      return Ok((NodeCommand::Print(expression), usize::from(!self.operands.is_empty())));
    }

    let Some(script) = self.operands.first() else {
      return Ok((NodeCommand::Direct, 0));
    };

    if script == OsStr::new("inspect") {
      return Err(CliError::new(
        "`node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.\nUse `node --inspect` / `node --inspect-brk` compatible flags instead.",
      ));
    }

    Ok((NodeCommand::Script(script.clone()), 1))
  }
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use crate::cli::{BunodeCommandOption, NodeCommand};

  use super::parse;

  fn parse_cli(args: &[&str]) -> Result<BunodeCommandOption, crate::cli::CliError> {
    parse(args, None)
  }

  fn parse_with_node_options(
    args: &[&str],
    node_options: &str,
  ) -> Result<BunodeCommandOption, crate::cli::CliError> {
    parse(args, Some(OsString::from(node_options)))
  }

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--inspect")],
        bun_options: vec![OsString::from("--inspect")],
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_inspect_value_before_script_operand() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "--inspect=127.0.0.1:9229", "script.js"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--inspect=127.0.0.1:9229")],
        bun_options: vec![OsString::from("--inspect=127.0.0.1:9229")],
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("--script.js")),
        exec_argv: Vec::new(),
        bun_options: Vec::new(),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_eval_operands_as_arguments() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "-p", "process.argv.slice(1)", "first", "--second"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Print(OsString::from("process.argv.slice(1)")),
        exec_argv: vec![OsString::from("-p"), OsString::from("process.argv.slice(1)")],
        bun_options: Vec::new(),
        script_arguments: vec![OsString::from("first"), OsString::from("--second")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_default_print_expression_to_undefined() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "-p"])?;

    assert_eq!(options.command, NodeCommand::Print(OsString::from("undefined")),);
    assert_eq!(options.exec_argv, vec![OsString::from("-p")]);

    Ok(())
  }

  #[test]
  fn parse_should_treat_print_after_double_dash_as_script() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "-p", "--", "script.js", "--flag"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("-p")],
        bun_options: Vec::new(),
        script_arguments: vec![OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_print_operand_after_option_as_script() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "-p", "--conditions", "custom", "script.js", "--flag"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![
          OsString::from("-p"),
          OsString::from("--conditions"),
          OsString::from("custom"),
        ],
        bun_options: vec![OsString::from("--conditions=custom")],
        script_arguments: vec![OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_support_print_eval_shortcut() -> Result<(), crate::cli::CliError> {
    let options = parse_cli(&["node", "-pe", "1 + 1"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Print(OsString::from("1 + 1")),
        exec_argv: vec![OsString::from("-pe"), OsString::from("1 + 1")],
        bun_options: Vec::new(),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_translate_node_options_before_cli_options() -> Result<(), crate::cli::CliError> {
    let options =
      parse_with_node_options(&["node", "--conditions", "cli", "script.js"], "--conditions env")?;

    assert_eq!(
      options,
      BunodeCommandOption {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--conditions"), OsString::from("cli")],
        bun_options: vec![OsString::from("--conditions=env"), OsString::from("--conditions=cli")],
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_reject_command_options_from_node_options() {
    let error = parse_with_node_options(&["node"], "--eval 1").unwrap_err();

    assert_eq!(error.to_string(), "bunode: `--eval` is not allowed in NODE_OPTIONS");
  }

  #[test]
  fn parse_should_reject_env_file_from_node_options() {
    let error = parse_with_node_options(&["node"], "--env-file .env").unwrap_err();

    assert_eq!(error.to_string(), "bunode: `--env-file` is not allowed in NODE_OPTIONS");
  }

  #[test]
  fn parse_should_reject_attached_short_values() {
    let error = parse_cli(&["node", "-econsole.log(1)"]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: unsupported Node.js option `-econsole.log(1)`",);
  }

  #[test]
  fn parse_should_reject_missing_option_value_before_next_flag() {
    let error = parse_cli(&["node", "--require", "--eval", "0"]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: option `--require` requires a value");
  }

  #[test]
  fn parse_should_reject_empty_inline_required_value() {
    let error = parse_cli(&["node", "--eval="]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: option `--eval` requires a value");
  }

  #[test]
  fn parse_should_validate_env_file_before_early_exit() {
    let error =
      parse_cli(&["node", "--env-file", "missing-bunode-env-file.env", "--version"]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: missing-bunode-env-file.env: not found");
  }

  #[test]
  fn parse_should_hide_bunode_options_from_exec_argv() -> Result<(), crate::cli::CliError> {
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
  fn parse_should_keep_double_quoted_node_options_value() -> Result<(), crate::cli::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require \"./with space.js\"")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload=./with space.js")],);

    Ok(())
  }

  #[test]
  fn parse_should_keep_single_quotes_as_node_options_literal() -> Result<(), crate::cli::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require './preload.js'")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload='./preload.js'")],);

    Ok(())
  }

  #[test]
  fn parse_should_preserve_node_options_backslashes() -> Result<(), crate::cli::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], r"--require C:\tmp\preload.js")?;

    assert_eq!(options.bun_options, vec![OsString::from(r"--preload=C:\tmp\preload.js")]);

    Ok(())
  }

  #[test]
  fn parse_should_keep_escaped_double_quote_in_node_options() -> Result<(), crate::cli::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], r#"--require "./x\" y.js""#)?;

    assert_eq!(options.bun_options, vec![OsString::from(r#"--preload=./x" y.js"#)]);

    Ok(())
  }
}
