//! Node option parsing and translation into Bun runtime flags.

use std::ffi::{OsStr, OsString};

use crate::cli::{BunodeCommandOption, CliError, NodeCommand};

use super::options::{OPTION_SPECS, OptionAction, OptionSpec, ValueMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
  CommandLine,
  NodeOptions,
}

#[derive(Default)]
struct ParseState {
  help: bool,
  version: bool,
  inline_command: Option<NodeCommand>,
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

  let command = state.command()?;
  let script_arguments = state.script_arguments();

  Ok(BunodeCommandOption { argv0, command, bun_options: state.bun_options, script_arguments })
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

  let value = match spec.value {
    ValueMode::None => {
      if inline_value.is_some() {
        return Err(CliError::new(format!("option `{name}` does not take a value")));
      }

      None
    }
    ValueMode::Required => Some(match inline_value {
      Some(value) => OsString::from(value),
      None => tokens
        .get(index + 1)
        .cloned()
        .ok_or_else(|| CliError::new(format!("option `{name}` requires a value")))?,
    }),
    ValueMode::OptionalEquals => inline_value.map(OsString::from),
  };

  apply_option(spec, value, source, state)?;

  if spec.value == ValueMode::Required && inline_value.is_none() {
    Ok(index + 2)
  } else {
    Ok(index + 1)
  }
}

fn parse_short_option(
  tokens: &[OsString],
  index: usize,
  source: Source,
  state: &mut ParseState,
) -> Result<usize, CliError> {
  let token = tokens[index].to_string_lossy();
  let Some(short) = token[1..].chars().next() else {
    return Err(unsupported_option(&token));
  };
  let Some(spec) = find_short_option(short) else {
    return Err(unsupported_option(&token));
  };
  let rest = &token[(1 + short.len_utf8())..];
  let option_name = format!("-{short}");

  let value = match spec.value {
    ValueMode::None => {
      if !rest.is_empty() {
        return Err(CliError::new(format!("option `{option_name}` does not take a value")));
      }

      None
    }
    ValueMode::Required => Some(if rest.is_empty() {
      tokens
        .get(index + 1)
        .cloned()
        .ok_or_else(|| CliError::new(format!("option `{option_name}` requires a value")))?
    } else {
      OsString::from(rest)
    }),
    ValueMode::OptionalEquals => None,
  };

  apply_option(spec, value, source, state)?;

  if spec.value == ValueMode::Required && rest.is_empty() { Ok(index + 2) } else { Ok(index + 1) }
}

fn split_long_option(token: &str) -> (&str, Option<&str>) {
  token.split_once('=').map_or((token, None), |(name, value)| (name, Some(value)))
}

fn find_long_option(name: &str) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.long.contains(&name))
}

fn find_short_option(short: char) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.short == Some(short))
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

  match spec.action {
    OptionAction::Help => state.help = true,
    OptionAction::Version => state.version = true,
    OptionAction::Eval => {
      state.inline_command = Some(NodeCommand::Eval(required_action_value(value, spec)?));
    }
    OptionAction::Print => {
      state.inline_command = Some(NodeCommand::Print(required_action_value(value, spec)?));
    }
    OptionAction::ForwardFlag(name) => state.bun_options.push(OsString::from(name)),
    OptionAction::ForwardValue(name) => {
      state.bun_options.push(join_option_value(name, required_action_value(value, spec)?));
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

fn unsupported_option(option: &str) -> CliError {
  CliError::new(format!("unsupported Node.js option `{option}`"))
}

fn split_node_options(value: &OsStr) -> Result<Vec<OsString>, CliError> {
  let value = value.to_string_lossy();
  let mut result = Vec::new();
  let mut current = String::new();
  let mut quote = None;
  let mut escaped = false;

  // NODE_OPTIONS follows shell-like quoting, but it is parsed without a shell.
  for character in value.chars() {
    if escaped {
      current.push(character);
      escaped = false;
      continue;
    }

    if character == '\\' {
      escaped = true;
      continue;
    }

    if Some(character) == quote {
      quote = None;
      continue;
    }

    if quote.is_none() && (character == '\'' || character == '"') {
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

  if escaped {
    current.push('\\');
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
  fn command(&self) -> Result<NodeCommand, CliError> {
    if self.help {
      return Ok(NodeCommand::Help);
    }

    if self.version {
      return Ok(NodeCommand::Version);
    }

    if let Some(command) = &self.inline_command {
      return Ok(command.clone());
    }

    let Some(script) = self.operands.first() else {
      return Ok(NodeCommand::Direct);
    };

    if script == OsStr::new("inspect") {
      return Err(CliError::new(
        "`node inspect` is not supported because Bun does not provide Node's built-in CLI debugger.\nUse `node --inspect` / `node --inspect-brk` compatible flags instead.",
      ));
    }

    Ok(NodeCommand::Script(script.clone()))
  }

  fn script_arguments(&self) -> Vec<OsString> {
    let skip_script = usize::from(!(self.inline_command.is_some() || self.help || self.version));

    self.operands.iter().skip(skip_script).cloned().collect()
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
        bun_options: Vec::new(),
        script_arguments: vec![OsString::from("first"), OsString::from("--second")],
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
  fn parse_should_keep_quoted_node_options_value() -> Result<(), crate::cli::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require './with space.js'")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload=./with space.js")],);

    Ok(())
  }
}
