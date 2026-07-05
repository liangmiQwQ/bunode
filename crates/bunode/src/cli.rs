//! Node-compatible CLI parsing and Bun argument translation.
//!
//! This module intentionally keeps option behavior table-driven. New supported
//! options should be added to `OPTION_SPECS` instead of branching in the parser.

use std::{
  ffi::{OsStr, OsString},
  process::ExitCode,
};

#[derive(Debug, PartialEq, Eq)]
pub struct BunodeCommandOption {
  pub argv0: OsString,
  pub command: NodeCommand,
  pub bun_options: Vec<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeCommand {
  Help,
  Version,
  Eval(OsString),
  Print(OsString),
  Script(OsString),
  Direct,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CliError {
  message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
  CommandLine,
  NodeOptions,
}

#[derive(Clone, Copy)]
struct OptionSpec {
  long: &'static [&'static str],
  short: Option<char>,
  value: ValueMode,
  node_options_allowed: bool,
  action: OptionAction,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueMode {
  None,
  Required,
  OptionalEquals,
}

#[derive(Clone, Copy)]
enum OptionAction {
  Help,
  Version,
  Eval,
  Print,
  ForwardFlag(&'static str),
  ForwardValue(&'static str),
  ForwardOptionalValue(&'static str),
}

#[derive(Default)]
struct ParseState {
  help: bool,
  version: bool,
  inline_command: Option<NodeCommand>,
  bun_options: Vec<OsString>,
  operands: Vec<OsString>,
}

const OPTION_SPECS: &[OptionSpec] = &[
  OptionSpec {
    long: &["--help"],
    short: Some('h'),
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::Help,
  },
  OptionSpec {
    long: &["--version"],
    short: Some('v'),
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::Version,
  },
  OptionSpec {
    long: &["--eval"],
    short: Some('e'),
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::Eval,
  },
  OptionSpec {
    long: &["--print"],
    short: Some('p'),
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::Print,
  },
  OptionSpec {
    long: &["--require"],
    short: Some('r'),
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--preload"),
  },
  OptionSpec {
    long: &["--import"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--preload"),
  },
  OptionSpec {
    long: &["--inspect"],
    short: None,
    value: ValueMode::OptionalEquals,
    node_options_allowed: true,
    action: OptionAction::ForwardOptionalValue("--inspect"),
  },
  OptionSpec {
    long: &["--inspect-brk"],
    short: None,
    value: ValueMode::OptionalEquals,
    node_options_allowed: true,
    action: OptionAction::ForwardOptionalValue("--inspect-brk"),
  },
  OptionSpec {
    long: &["--inspect-wait"],
    short: None,
    value: ValueMode::OptionalEquals,
    node_options_allowed: true,
    action: OptionAction::ForwardOptionalValue("--inspect-wait"),
  },
  OptionSpec {
    long: &["--conditions"],
    short: Some('C'),
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--conditions"),
  },
  OptionSpec {
    long: &["--cpu-prof"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--cpu-prof"),
  },
  OptionSpec {
    long: &["--cpu-prof-dir"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--cpu-prof-dir"),
  },
  OptionSpec {
    long: &["--cpu-prof-interval"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--cpu-prof-interval"),
  },
  OptionSpec {
    long: &["--cpu-prof-name"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--cpu-prof-name"),
  },
  OptionSpec {
    long: &["--heap-prof"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--heap-prof"),
  },
  OptionSpec {
    long: &["--heap-prof-dir"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--heap-prof-dir"),
  },
  OptionSpec {
    long: &["--heap-prof-name"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--heap-prof-name"),
  },
  OptionSpec {
    long: &["--dns-result-order"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--dns-result-order"),
  },
  OptionSpec {
    long: &["--env-file"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--env-file"),
  },
  OptionSpec {
    long: &["--expose-gc"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--expose-gc"),
  },
  OptionSpec {
    long: &["--no-addons"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--no-addons"),
  },
  OptionSpec {
    long: &["--no-deprecation"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--no-deprecation"),
  },
  OptionSpec {
    long: &["--throw-deprecation"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--throw-deprecation"),
  },
  OptionSpec {
    long: &["--title"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--title"),
  },
  OptionSpec {
    long: &["--unhandled-rejections"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: true,
    action: OptionAction::ForwardValue("--unhandled-rejections"),
  },
  OptionSpec {
    long: &["--use-bundled-ca"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--use-bundled-ca"),
  },
  OptionSpec {
    long: &["--use-openssl-ca"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--use-openssl-ca"),
  },
  OptionSpec {
    long: &["--use-system-ca"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--use-system-ca"),
  },
  OptionSpec {
    long: &["--zero-fill-buffers"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: true,
    action: OptionAction::ForwardFlag("--zero-fill-buffers"),
  },
  OptionSpec {
    long: &["--bun-config"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--config"),
  },
  OptionSpec {
    long: &["--bun-console-depth"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--console-depth"),
  },
  OptionSpec {
    long: &["--bun-env-file"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--env-file"),
  },
  OptionSpec {
    long: &["--bun-fetch-preconnect"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--fetch-preconnect"),
  },
  OptionSpec {
    long: &["--bun-hot"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--hot"),
  },
  OptionSpec {
    long: &["--bun-install"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--install"),
  },
  OptionSpec {
    long: &["--bun-no-clear-screen"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--no-clear-screen"),
  },
  OptionSpec {
    long: &["--bun-no-env-file"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--no-env-file"),
  },
  OptionSpec {
    long: &["--bun-port"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--port"),
  },
  OptionSpec {
    long: &["--bun-prefer-latest"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--prefer-latest"),
  },
  OptionSpec {
    long: &["--bun-prefer-offline"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--prefer-offline"),
  },
  OptionSpec {
    long: &["--bun-preload"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--preload"),
  },
  OptionSpec {
    long: &["--bun-smol"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--smol"),
  },
  OptionSpec {
    long: &["--bun-user-agent"],
    short: None,
    value: ValueMode::Required,
    node_options_allowed: false,
    action: OptionAction::ForwardValue("--user-agent"),
  },
  OptionSpec {
    long: &["--bun-watch"],
    short: None,
    value: ValueMode::None,
    node_options_allowed: false,
    action: OptionAction::ForwardFlag("--watch"),
  },
];

impl CliError {
  pub fn exit(&self) -> ExitCode {
    eprintln!("{}", self.message);
    ExitCode::from(1)
  }
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

pub fn print_help() {
  print!(
    "\
Usage: node [options] [ script.js ] [arguments]

Options:
  -                           script read from stdin (default if no file name is provided, interactive mode if a tty)
  --                          indicate the end of node options
  -h, --help                  print Bunode supported options
  -v, --version               print Node-compatible Bunode version
  -e, --eval=...              evaluate script
  -p, --print=...             evaluate script and print the result
  -r, --require=...           preload CommonJS module (translated to Bun preload)
  --import=...                preload ES module (translated to Bun preload)
  --inspect[=[host:]port]     activate inspector
  --inspect-brk[=[host:]port] activate inspector and break at start
  --inspect-wait[=[host:]port]
                              activate inspector and wait for debugger
  -C, --conditions=...        pass custom conditions to resolve
  --cpu-prof                  start CPU profiler
  --cpu-prof-dir=...          set CPU profile output directory
  --cpu-prof-interval=...     set CPU profile sampling interval
  --cpu-prof-name=...         set CPU profile output file name
  --dns-result-order=...      set default dns.lookup result order
  --env-file=...              load environment variables from a file
  --expose-gc                 expose gc on the global object
  --heap-prof                 write heap profile on exit
  --heap-prof-dir=...         set heap profile output directory
  --heap-prof-name=...        set heap profile output file name
  --no-addons                 disable native addons
  --no-deprecation            suppress deprecation warnings
  --throw-deprecation         throw deprecation warnings as exceptions
  --title=...                 set process title
  --unhandled-rejections=...  set unhandled rejection mode
  --use-bundled-ca            use bundled CA store
  --use-openssl-ca            use OpenSSL CA store
  --use-system-ca             use system CA store
  --zero-fill-buffers         zero-fill Buffer.allocUnsafe

Bun-specific options:
  --bun-config=...            specify bunfig.toml path
  --bun-console-depth=...     set console inspection depth
  --bun-env-file=...          load Bun environment file
  --bun-fetch-preconnect=...  preconnect while code is loading
  --bun-hot                   enable Bun hot reload
  --bun-install=...           configure Bun auto-install behavior
  --bun-no-clear-screen       disable reload clear screen behavior
  --bun-no-env-file           disable automatic .env loading
  --bun-port=...              set default Bun.serve port
  --bun-prefer-latest         prefer latest packages in Bun runtime
  --bun-prefer-offline        prefer offline package resolution
  --bun-preload=...           run an additional Bun preload
  --bun-smol                  enable Bun smol mode
  --bun-user-agent=...        set default HTTP User-Agent
  --bun-watch                 restart on file changes

Environment variables:
  NODE_OPTIONS                supported Node options are translated before CLI options
"
  );
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

impl CliError {
  fn new(message: impl Into<String>) -> Self {
    Self { message: format!("bunode: {}", message.into()) }
  }
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use super::{BunodeCommandOption, NodeCommand, parse};

  fn parse_cli(args: &[&str]) -> Result<BunodeCommandOption, super::CliError> {
    parse(args, None)
  }

  fn parse_with_node_options(
    args: &[&str],
    node_options: &str,
  ) -> Result<BunodeCommandOption, super::CliError> {
    parse(args, Some(OsString::from(node_options)))
  }

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand() -> Result<(), super::CliError> {
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
  fn parse_should_keep_inspect_value_before_script_operand() -> Result<(), super::CliError> {
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
  fn parse_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), super::CliError> {
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
  fn parse_should_treat_eval_operands_as_arguments() -> Result<(), super::CliError> {
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
  fn parse_should_translate_node_options_before_cli_options() -> Result<(), super::CliError> {
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

    assert_eq!(error.message, "bunode: `--eval` is not allowed in NODE_OPTIONS");
  }

  #[test]
  fn parse_should_keep_quoted_node_options_value() -> Result<(), super::CliError> {
    let options = parse_with_node_options(&["node", "-e", "0"], "--require './with space.js'")?;

    assert_eq!(options.bun_options, vec![OsString::from("--preload=./with space.js")],);

    Ok(())
  }
}
