//! Node option parsing and translation into Bun runtime flags.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use lexopt::Arg;

use crate::error::CliError;

use super::{
  builtins, data_url, env_file,
  options::{
    HelpSection, OptionAction, OptionShape, OptionSpec, PreloadKind, ValueMode, find_long_option,
    find_short_option,
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionPlan {
  pub argv0: OsString,
  pub command: NodeCommand,
  pub exec_argv: Vec<OsString>,
  pub bun_options: Vec<OsString>,
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
  bun_preloads: Vec<OsString>,
  common_js_preloads: Vec<OsString>,
  es_module_preloads: Vec<OsString>,
  env_file_node_options: Option<OsString>,
  read_env_file_node_options: bool,
  operands: Vec<OsString>,
}

pub fn parse<I, T>(
  args: I,
  node_options: Option<OsString>,
  shape: &OptionShape,
) -> Result<ExecutionPlan, CliError>
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
) -> Result<(ExecutionPlan, Option<OsString>), CliError> {
  // 1. Keep argv0 for process.argv0 correction in the generated preload.
  let argv0 = args.first().cloned().unwrap_or_else(|| OsString::from("node"));
  let mut state = ParseState { read_env_file_node_options, ..ParseState::default() };

  // 2. NODE_OPTIONS behaves as if it appears before CLI flags.
  if let Some(node_options) = node_options.filter(|value| !value.is_empty()) {
    let node_options = split_node_options(&node_options)?;
    parse_tokens(&node_options, Source::NodeOptions, &mut state, shape)?;
  }

  // 3. CLI operands stop option parsing once the script position is reached.
  parse_tokens(args.get(1..).unwrap_or_default(), Source::CommandLine, &mut state, shape)?;

  state.finish(argv0)
}

fn parse_tokens(
  tokens: &[OsString],
  source: Source,
  state: &mut ParseState,
  shape: &OptionShape,
) -> Result<(), CliError> {
  let mut parser = lexopt::Parser::from_args(tokens.iter().cloned());
  parser.set_short_equals(false);

  loop {
    if consume_double_dash(&mut parser, source, state)? {
      break;
    }

    let Some(argument) = parser.next().map_err(|error| CliError::new(error.to_string()))? else {
      break;
    };

    match argument {
      Arg::Long(name) => {
        let name = name.to_owned();
        parse_long_option(&name, &mut parser, source, state, shape)?;
      }
      Arg::Short(short) => parse_short_option(short, &mut parser, source, state, shape)?,
      Arg::Value(value) => {
        if parse_operand(value, &mut parser, source, state)? {
          break;
        }
      }
    }
  }

  Ok(())
}

fn parse_long_option(
  name: &str,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  shape: &OptionShape,
) -> Result<(), CliError> {
  let name = format!("--{name}");
  let Some(spec) = find_long_option(shape, &name) else {
    return Err(unsupported_option(&name));
  };
  let (value, original) = parse_long_value(spec, parser, &name)?;

  apply_option(spec, value, source, state)?;
  record_exec_argv(spec, source, original, state);

  Ok(())
}

fn parse_short_option(
  short: char,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  shape: &OptionShape,
) -> Result<(), CliError> {
  let attached_value = parser.optional_value();

  if short == 'p' && attached_value.as_ref().is_some_and(|value| value == OsStr::new("e")) {
    return parse_print_eval_shortcut(parser, source, state, shape);
  }

  if let Some(attached_value) = attached_value {
    return Err(unsupported_option(join_short_option_value(short, &attached_value)));
  }

  let Some(spec) = find_short_option(shape, short) else {
    return Err(unsupported_option(format!("-{short}")));
  };
  let option_name = format!("-{short}");

  let (value, original) = if matches!(spec.action, OptionAction::Print) {
    (None, vec![OsString::from(&option_name)])
  } else {
    match spec.value {
      ValueMode::None | ValueMode::OptionalEquals => (None, vec![OsString::from(&option_name)]),
      ValueMode::Required => {
        let value = required_next_value(parser, &option_name)?;
        (Some(value.clone()), vec![OsString::from(&option_name), value])
      }
    }
  };

  apply_option(spec, value, source, state)?;
  record_exec_argv(spec, source, original, state);

  Ok(())
}

fn parse_print_eval_shortcut(
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  shape: &OptionShape,
) -> Result<(), CliError> {
  let print_spec = find_short_option(shape, 'p').ok_or_else(|| unsupported_option("-p"))?;
  let eval_spec = find_short_option(shape, 'e').ok_or_else(|| unsupported_option("-e"))?;
  let value = required_next_value(parser, "-e")?;
  let original_value = value.clone();

  apply_option(print_spec, None, source, state)?;
  apply_option(eval_spec, Some(value), source, state)?;

  if source == Source::CommandLine {
    state.exec_argv.push(OsString::from("-pe"));
    state.exec_argv.push(original_value);
  }

  Ok(())
}

fn consume_double_dash(
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
) -> Result<bool, CliError> {
  let Some(mut raw_args) = parser.try_raw_args() else {
    return Ok(false);
  };

  if raw_args.peek().is_none_or(|argument| argument != OsStr::new("--")) {
    return Ok(false);
  }

  if source == Source::NodeOptions {
    return Err(CliError::new("`--` is not allowed in NODE_OPTIONS"));
  }

  let _ = raw_args.next();
  state.operand_boundary = OperandBoundary::DoubleDash;
  state.operands.extend(raw_args);

  Ok(true)
}

fn parse_operand(
  value: OsString,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
) -> Result<bool, CliError> {
  if source == Source::NodeOptions {
    return Err(CliError::new(format!(
      "`{}` is not allowed in NODE_OPTIONS",
      value.to_string_lossy(),
    )));
  }

  if value == OsStr::new("-") && state.should_capture_print_expression(source) {
    state.operands.push(value);
    state.operands.extend(parser.raw_args().map_err(|error| CliError::new(error.to_string()))?);
    return Ok(true);
  }

  if state.should_capture_print_expression(source) {
    state.inline_command = Some(NodeCommand::Print(value.clone()));
    state.print_operand_mode = PrintOperandMode::Script;
    state.exec_argv.push(value);
    return Ok(false);
  }

  state.operands.push(value);
  state.operands.extend(parser.raw_args().map_err(|error| CliError::new(error.to_string()))?);

  Ok(true)
}

fn parse_long_value(
  spec: &OptionSpec,
  parser: &mut lexopt::Parser,
  option: &str,
) -> Result<(Option<OsString>, Vec<OsString>), CliError> {
  let inline_value = parser.optional_value();
  let mut original = vec![format_long_original(option, inline_value.as_deref())];

  if matches!(spec.action, OptionAction::Print) {
    return Ok((inline_value, original));
  }

  let value = match spec.value {
    ValueMode::None => {
      if inline_value.is_some() {
        return Err(CliError::new(format!("option `{option}` does not take a value")));
      }

      None
    }
    ValueMode::Required => {
      let value = if let Some(value) = inline_value {
        if value.is_empty() {
          return Err(CliError::new(format!("option `{option}` requires a value")));
        }

        value
      } else {
        let value = required_next_value(parser, option)?;
        original.push(value.clone());
        value
      };

      Some(value)
    }
    ValueMode::OptionalEquals => match inline_value {
      Some(value) => {
        if value.is_empty() {
          return Err(CliError::new(format!("option `{option}` requires a value")));
        }

        Some(value)
      }
      None => None,
    },
  };

  Ok((value, original))
}

fn required_next_value(parser: &mut lexopt::Parser, option: &str) -> Result<OsString, CliError> {
  let mut raw_args = parser.raw_args().map_err(|error| CliError::new(error.to_string()))?;
  let Some(value) = raw_args.peek() else {
    return Err(CliError::new(format!("option `{option}` requires a value")));
  };

  if starts_with_dash(value) {
    return Err(CliError::new(format!("option `{option}` requires a value")));
  }

  raw_args.next().ok_or_else(|| CliError::new(format!("option `{option}` requires a value")))
}

fn format_long_original(option: &str, value: Option<&OsStr>) -> OsString {
  let mut original = OsString::from(option);

  if let Some(value) = value {
    original.push("=");
    original.push(value);
  }

  original
}

fn join_short_option_value(short: char, value: &OsStr) -> OsString {
  let mut option = OsString::from(format!("-{short}"));

  option.push(value);
  option
}

fn starts_with_dash(value: &OsStr) -> bool {
  value.to_string_lossy().starts_with('-')
}

fn record_exec_argv(
  spec: &OptionSpec,
  source: Source,
  original: Vec<OsString>,
  state: &mut ParseState,
) {
  if source == Source::CommandLine
    && spec.help.is_some_and(|help| help.section == HelpSection::Node)
  {
    state.exec_argv.extend(original);
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

  if state.print_mode == PrintMode::Enabled && !matches!(spec.action, OptionAction::Print) {
    state.print_operand_mode = PrintOperandMode::Script;
  }

  match spec.action {
    OptionAction::Help => {
      if state.command_mode != CommandMode::Version {
        state.command_mode = CommandMode::Help;
      }
    }
    OptionAction::Version => state.command_mode = CommandMode::Version,
    OptionAction::Eval => {
      state.inline_command = Some(NodeCommand::Eval(required_action_value(value, spec)?));
    }
    OptionAction::Print => {
      // Node accepts `--print=<value>` but still reads the expression from argv operands.
      state.print_mode = PrintMode::Enabled;
      state.print_operand_mode = PrintOperandMode::Expression;
    }
    OptionAction::Preload(kind) => {
      let value = required_action_value(value, spec)?;

      if builtins::is_builtin_module(&value.to_string_lossy()) {
        return Ok(());
      }

      match kind {
        PreloadKind::CommonJs => {
          state.common_js_preloads.push(join_option_value("--preload", value));
        }
        PreloadKind::EsModule => state.es_module_preloads.push(value),
      }
    }
    OptionAction::ForwardFlag(name) => state.bun_options.push(OsString::from(name)),
    OptionAction::ForwardValue(name) => {
      let value = required_action_value(value, spec)?;

      if name == "--env-file" {
        validate_env_file(&value)?;

        if source == Source::CommandLine
          && state.read_env_file_node_options
          && spec.help.is_some_and(|help| help.section == HelpSection::Node)
          && let Some(node_options) = env_file::read_node_options(&value)?
        {
          state.env_file_node_options = Some(node_options);
        }
      }

      if name == "--preload" {
        state.bun_preloads.push(join_option_value(name, value));
        return Ok(());
      }

      state.bun_options.push(join_option_value(name, value));
    }
    OptionAction::ForwardOptionalValue(name) => {
      let value = value.or_else(|| default_optional_value(name).map(OsString::from));

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

fn default_optional_value(name: &str) -> Option<&'static str> {
  match name {
    "--inspect" | "--inspect-brk" | "--inspect-wait" => Some("127.0.0.1:9229"),
    _ => None,
  }
}

fn validate_env_file(path: &OsStr) -> Result<(), CliError> {
  let path = Path::new(path);

  if path.is_file() {
    return Ok(());
  }

  Err(CliError::new(format!("{}: not found", path.display())))
}

fn unsupported_option(option: impl AsRef<OsStr>) -> CliError {
  CliError::new(format!("unsupported Node.js option `{}`", option.as_ref().to_string_lossy()))
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
  fn should_capture_print_expression(&self, source: Source) -> bool {
    source == Source::CommandLine
      && self.print_mode == PrintMode::Enabled
      && self.print_operand_mode == PrintOperandMode::Expression
  }

  fn finish(mut self, argv0: OsString) -> Result<(ExecutionPlan, Option<OsString>), CliError> {
    let (command, script_operand_count) = self.command()?;
    let script_arguments =
      self.operands.iter().skip(script_operand_count).cloned().collect::<Vec<_>>();
    let should_materialize_preloads = !matches!(command, NodeCommand::Help | NodeCommand::Version);
    let mut bun_options = self.bun_options;
    let env_file_node_options = self.env_file_node_options;

    bun_options.extend(self.common_js_preloads);
    bun_options
      .extend(resolve_es_module_preloads(self.es_module_preloads, should_materialize_preloads)?);
    bun_options.extend(self.bun_preloads);

    Ok((
      ExecutionPlan { argv0, command, exec_argv: self.exec_argv, bun_options, script_arguments },
      env_file_node_options,
    ))
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
      && self.operands.first().is_some_and(|operand| operand == OsStr::new("-"))
    {
      return Ok((NodeCommand::PrintStdin, 0));
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

      let command = if self.operands.is_empty() {
        NodeCommand::PrintStdin
      } else {
        NodeCommand::Print(expression)
      };

      return Ok((command, usize::from(!self.operands.is_empty())));
    }

    let Some(script) = self.operands.first() else {
      return Ok((NodeCommand::Direct, 0));
    };

    if script == OsStr::new("inspect") {
      return Err(CliError::new(
        "`node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.\nUse `node --inspect` / `node --inspect-brk` compatible flags instead.",
      ));
    }

    // Node treats an empty script operand like stdin/REPL while preserving it in process.argv.
    if script.is_empty() {
      return Ok((NodeCommand::Direct, 0));
    }

    Ok((NodeCommand::Script(script.clone()), 1))
  }
}

fn resolve_es_module_preloads(
  values: Vec<OsString>,
  should_materialize: bool,
) -> Result<Vec<OsString>, CliError> {
  let mut preloads = Vec::with_capacity(values.len());

  for value in values {
    if should_materialize
      && let Some(path) = data_url::materialize_javascript_module(&value.to_string_lossy())?
    {
      preloads.push(join_option_value("--preload", path.into_os_string()));
      continue;
    }

    preloads.push(join_option_value("--preload", value));
  }

  Ok(preloads)
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use semver::Version;

  use super::{ExecutionPlan, NodeCommand};

  use super::parse;

  fn parse_cli(args: &[&str]) -> Result<ExecutionPlan, crate::error::CliError> {
    let shape = super::super::options::option_shape_for_bun(&Version::new(1, 3, 14));

    parse(args, None, &shape)
  }

  fn parse_with_node_options(
    args: &[&str],
    node_options: &str,
  ) -> Result<ExecutionPlan, crate::error::CliError> {
    let shape = super::super::options::option_shape_for_bun(&Version::new(1, 3, 14));

    parse(args, Some(OsString::from(node_options)), &shape)
  }

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand() -> Result<(), crate::error::CliError>
  {
    let options = parse_cli(&["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Script(OsString::from("script.js")),
        exec_argv: vec![OsString::from("--inspect")],
        bun_options: vec![OsString::from("--inspect=127.0.0.1:9229")],
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_inspect_value_before_script_operand() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "--inspect=127.0.0.1:9229", "script.js"])?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), crate::error::CliError>
  {
    let options = parse_cli(&["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_treat_empty_script_operand_as_stdin_argument()
  -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "--", "", "arg"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::Direct,
        exec_argv: Vec::new(),
        bun_options: Vec::new(),
        script_arguments: vec![OsString::new(), OsString::from("arg")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_eval_operands_as_arguments() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "-p", "process.argv.slice(1)", "first", "--second"])?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_continue_options_after_print_expression() -> Result<(), crate::error::CliError> {
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
        script_arguments: vec![OsString::from("first")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_let_later_print_operand_replace_earlier_inline_command()
  -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "-e", "\"eval\"", "-p", "\"print\""])?;

    assert_eq!(options.command, NodeCommand::Print(OsString::from("\"print\"")));

    let options = parse_cli(&["node", "-p", "\"a\"", "-p", "\"b\""])?;

    assert_eq!(options.command, NodeCommand::Print(OsString::from("\"b\"")));

    Ok(())
  }

  #[test]
  fn parse_should_prefer_version_over_help() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "--help", "--version"])?;

    assert_eq!(options.command, NodeCommand::Version);

    let options = parse_cli(&["node", "--version", "--help"])?;

    assert_eq!(options.command, NodeCommand::Version);

    Ok(())
  }

  #[test]
  fn parse_should_defer_print_without_expression_to_stdin() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "-p"])?;

    assert_eq!(options.command, NodeCommand::PrintStdin);
    assert_eq!(options.exec_argv, vec![OsString::from("-p")]);

    Ok(())
  }

  #[test]
  fn parse_should_treat_dash_print_operand_as_stdin_argument() -> Result<(), crate::error::CliError>
  {
    let options = parse_cli(&["node", "-p", "-", "arg"])?;

    assert_eq!(
      options,
      ExecutionPlan {
        argv0: OsString::from("node"),
        command: NodeCommand::PrintStdin,
        exec_argv: vec![OsString::from("-p")],
        bun_options: Vec::new(),
        script_arguments: vec![OsString::from("-"), OsString::from("arg")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_print_after_double_dash_as_script() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "-p", "--", "script.js", "--flag"])?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_treat_print_operand_after_option_as_script() -> Result<(), crate::error::CliError>
  {
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
        script_arguments: vec![OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_support_print_eval_shortcut() -> Result<(), crate::error::CliError> {
    let options = parse_cli(&["node", "-pe", "1 + 1"])?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_translate_node_options_before_cli_options() -> Result<(), crate::error::CliError>
  {
    let options =
      parse_with_node_options(&["node", "--conditions", "cli", "script.js"], "--conditions env")?;

    assert_eq!(
      options,
      ExecutionPlan {
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
  fn parse_should_reject_short_equals_values() {
    let error = parse_cli(&["node", "-p=e"]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: unsupported Node.js option `-p=e`");
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
  fn parse_should_reject_empty_inline_optional_value() {
    let error = parse_cli(&["node", "--inspect="]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: option `--inspect` requires a value");
  }

  #[test]
  fn parse_should_validate_env_file_before_early_exit() {
    let error =
      parse_cli(&["node", "--env-file", "missing-bunode-env-file.env", "--version"]).unwrap_err();

    assert_eq!(error.to_string(), "bunode: missing-bunode-env-file.env: not found");
  }

  #[test]
  fn parse_should_hide_bunode_options_from_exec_argv() -> Result<(), crate::error::CliError> {
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
  -> Result<(), crate::error::CliError> {
    let options =
      parse_cli(&["node", "--import", "./esm.mjs", "--require", "./cjs.cjs", "-e", "0"])?;

    assert_eq!(
      options.bun_options,
      vec![OsString::from("--preload=./cjs.cjs"), OsString::from("--preload=./esm.mjs")],
    );

    Ok(())
  }

  #[test]
  fn parse_should_run_node_options_preloads_before_bun_preloads()
  -> Result<(), crate::error::CliError> {
    let options = parse_with_node_options(
      &["node", "--bun-preload", "./cli.js", "-e", "0"],
      "--require ./env.cjs",
    )?;

    assert_eq!(
      options.bun_options,
      vec![OsString::from("--preload=./env.cjs"), OsString::from("--preload=./cli.js")],
    );

    Ok(())
  }

  #[test]
  fn parse_should_materialize_data_url_imports() -> Result<(), crate::error::CliError> {
    let options =
      parse_cli(&["node", "--import", "data:text/javascript,globalThis.loaded%3D1", "-e", "0"])?;
    let Some(preload) = options.bun_options.first() else {
      panic!("data import should produce a Bun preload");
    };
    let preload = preload.to_string_lossy();
    let path = preload.strip_prefix("--preload=").expect("data import should become preload");

    assert_eq!(std::fs::read(path).unwrap(), b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn parse_should_defer_data_url_import_errors_for_version() -> Result<(), crate::error::CliError> {
    let options = parse_with_node_options(
      &["node", "--import", "data:text/javascript,%GG", "--version"],
      "--import data:text/javascript,%GG",
    )?;

    assert_eq!(options.command, NodeCommand::Version);

    Ok(())
  }

  #[test]
  fn parse_should_skip_builtin_preloads() -> Result<(), crate::error::CliError> {
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

    Ok(())
  }

  #[test]
  fn parse_should_translate_node_options_from_env_file() -> Result<(), crate::error::CliError> {
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
        super::join_option_value("--env-file", OsString::from(&path)),
        OsString::from("--conditions=cli"),
      ],
    );

    Ok(())
  }

  #[test]
  fn parse_should_not_read_node_options_from_bun_env_file() -> Result<(), crate::error::CliError> {
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

    assert_eq!(options.bun_options, vec![super::join_option_value("--env-file", path.into())]);

    Ok(())
  }

  #[test]
  fn parse_should_not_read_env_file_node_options_when_real_node_options_exists()
  -> Result<(), crate::error::CliError> {
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
      vec![OsString::from("--conditions=x"), super::join_option_value("--env-file", path.into()),],
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_double_quoted_node_options_value() -> Result<(), crate::error::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require \"./with space.js\"")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload=./with space.js")],);

    Ok(())
  }

  #[test]
  fn parse_should_keep_single_quotes_as_node_options_literal() -> Result<(), crate::error::CliError>
  {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require './preload.js'")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload='./preload.js'")],);

    Ok(())
  }

  #[test]
  fn parse_should_preserve_node_options_backslashes() -> Result<(), crate::error::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], r"--require C:\tmp\preload.js")?;

    assert_eq!(options.bun_options, vec![OsString::from(r"--preload=C:\tmp\preload.js")]);

    Ok(())
  }

  #[test]
  fn parse_should_keep_escaped_double_quote_in_node_options() -> Result<(), crate::error::CliError>
  {
    let options = parse_with_node_options(&["node", "-e", "0"], r#"--require "./x\" y.js""#)?;

    assert_eq!(options.bun_options, vec![OsString::from(r#"--preload=./x" y.js"#)]);

    Ok(())
  }
}
