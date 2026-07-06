//! Supported Node and Bunode option table.

use clap::{Arg, ArgAction, Command};

use crate::cli::{self, CliOptionSchema};

#[derive(Clone, Copy)]
pub(super) struct OptionSpec {
  pub(super) long: &'static [&'static str],
  pub(super) short: Option<char>,
  pub(super) value: ValueMode,
  pub(super) node_options_allowed: bool,
  pub(super) action: OptionAction,
  pub(super) help: Option<OptionHelp>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueMode {
  None,
  Required,
  OptionalEquals,
}

#[derive(Clone, Copy)]
pub(super) enum OptionAction {
  Help,
  Version,
  Eval,
  Print,
  Preload(PreloadKind),
  ForwardFlag(&'static str),
  ForwardValue(&'static str),
  ForwardOptionalValue(&'static str),
}

#[derive(Clone, Copy)]
pub(super) enum PreloadKind {
  CommonJs,
  EsModule,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HelpSection {
  Node,
  Bun,
}

#[derive(Clone, Copy)]
pub(super) struct OptionHelp {
  pub(super) section: HelpSection,
  pub(super) value_name: Option<&'static str>,
  pub(super) description: &'static str,
}

pub(super) struct BaseOptionSchema;

impl CliOptionSchema for BaseOptionSchema {
  fn augment_command(command: Command) -> Command {
    OPTION_SPECS.iter().fold(command, |command, spec| command.arg(spec.clap_arg()))
  }
}

pub(super) fn clap_command() -> Command {
  cli::option_command::<BaseOptionSchema>()
    .override_usage("node [options] [ script.js ] [arguments]")
}

macro_rules! option_spec {
  ($source:ident, [$($long:literal),+], $short:expr, $value:expr, $action:expr, $description:literal $(, $value_name:literal)? $(,)?) => {
    OptionSpec {
      long: &[$($long),+],
      short: $short,
      value: $value,
      node_options_allowed: option_spec!(@node_options_allowed $source),
      action: $action,
      help: Some(OptionHelp {
        section: option_spec!(@help_section $source),
        value_name: option_spec!(@value_name $($value_name)?),
        description: $description,
      }),
    }
  };
  (@node_options_allowed command) => { false };
  (@node_options_allowed node_cli) => { false };
  (@node_options_allowed node) => { true };
  (@node_options_allowed bun) => { false };
  (@help_section command) => { HelpSection::Node };
  (@help_section node_cli) => { HelpSection::Node };
  (@help_section node) => { HelpSection::Node };
  (@help_section bun) => { HelpSection::Bun };
  (@value_name) => { None };
  (@value_name $value_name:literal) => { Some($value_name) };
}

pub(super) const OPTION_SPECS: &[OptionSpec] = &[
  option_spec!(
    command,
    ["--help"],
    Some('h'),
    ValueMode::None,
    OptionAction::Help,
    "print Bunode supported options",
  ),
  option_spec!(
    command,
    ["--version"],
    Some('v'),
    ValueMode::None,
    OptionAction::Version,
    "print Node-compatible Bunode version",
  ),
  option_spec!(
    command,
    ["--eval"],
    Some('e'),
    ValueMode::Required,
    OptionAction::Eval,
    "evaluate script",
    "...",
  ),
  option_spec!(
    command,
    ["--print"],
    Some('p'),
    ValueMode::Required,
    OptionAction::Print,
    "evaluate script and print the result",
    "...",
  ),
  option_spec!(
    node,
    ["--require"],
    Some('r'),
    ValueMode::Required,
    OptionAction::Preload(PreloadKind::CommonJs),
    "preload CommonJS module (translated to Bun preload)",
    "...",
  ),
  option_spec!(
    node,
    ["--import"],
    None,
    ValueMode::Required,
    OptionAction::Preload(PreloadKind::EsModule),
    "preload ES module (translated to Bun preload)",
    "...",
  ),
  option_spec!(
    node,
    ["--inspect"],
    None,
    ValueMode::OptionalEquals,
    OptionAction::ForwardOptionalValue("--inspect"),
    "activate inspector",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--inspect-brk"],
    None,
    ValueMode::OptionalEquals,
    OptionAction::ForwardOptionalValue("--inspect-brk"),
    "activate inspector and break at start",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--inspect-wait"],
    None,
    ValueMode::OptionalEquals,
    OptionAction::ForwardOptionalValue("--inspect-wait"),
    "activate inspector and wait for debugger",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--conditions"],
    Some('C'),
    ValueMode::Required,
    OptionAction::ForwardValue("--conditions"),
    "pass custom conditions to resolve",
    "...",
  ),
  option_spec!(
    node,
    ["--cpu-prof"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--cpu-prof"),
    "start CPU profiler",
  ),
  option_spec!(
    node,
    ["--cpu-prof-dir"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--cpu-prof-dir"),
    "set CPU profile output directory",
    "...",
  ),
  option_spec!(
    node,
    ["--cpu-prof-interval"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--cpu-prof-interval"),
    "set CPU profile sampling interval",
    "...",
  ),
  option_spec!(
    node,
    ["--cpu-prof-name"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--cpu-prof-name"),
    "set CPU profile output file name",
    "...",
  ),
  option_spec!(
    node,
    ["--heap-prof"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--heap-prof"),
    "write heap profile on exit",
  ),
  option_spec!(
    node,
    ["--heap-prof-dir"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--heap-prof-dir"),
    "set heap profile output directory",
    "...",
  ),
  option_spec!(
    node,
    ["--heap-prof-name"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--heap-prof-name"),
    "set heap profile output file name",
    "...",
  ),
  option_spec!(
    node,
    ["--dns-result-order"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--dns-result-order"),
    "set default dns.lookup result order",
    "...",
  ),
  option_spec!(
    node_cli,
    ["--env-file"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--env-file"),
    "load environment variables from a file",
    "...",
  ),
  option_spec!(
    node,
    ["--expose-gc"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--expose-gc"),
    "expose gc on the global object",
  ),
  option_spec!(
    node,
    ["--no-addons"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--no-addons"),
    "disable native addons",
  ),
  option_spec!(
    node,
    ["--no-deprecation"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--no-deprecation"),
    "suppress deprecation warnings",
  ),
  option_spec!(
    node,
    ["--throw-deprecation"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--throw-deprecation"),
    "throw deprecation warnings as exceptions",
  ),
  option_spec!(
    node,
    ["--title"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--title"),
    "set process title",
    "...",
  ),
  option_spec!(
    node,
    ["--unhandled-rejections"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--unhandled-rejections"),
    "set unhandled rejection mode",
    "...",
  ),
  option_spec!(
    node,
    ["--use-bundled-ca"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--use-bundled-ca"),
    "use bundled CA store",
  ),
  option_spec!(
    node,
    ["--use-openssl-ca"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--use-openssl-ca"),
    "use OpenSSL CA store",
  ),
  option_spec!(
    node,
    ["--use-system-ca"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--use-system-ca"),
    "use system CA store",
  ),
  option_spec!(
    node,
    ["--zero-fill-buffers"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--zero-fill-buffers"),
    "zero-fill Buffer.allocUnsafe",
  ),
  option_spec!(
    bun,
    ["--bun-config"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--config"),
    "specify bunfig.toml path",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-console-depth"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--console-depth"),
    "set console inspection depth",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-env-file"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--env-file"),
    "load Bun environment file",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-fetch-preconnect"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--fetch-preconnect"),
    "preconnect while code is loading",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-hot"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--hot"),
    "enable Bun hot reload",
  ),
  option_spec!(
    bun,
    ["--bun-install"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--install"),
    "configure Bun auto-install behavior",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-no-clear-screen"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--no-clear-screen"),
    "disable reload clear screen behavior",
  ),
  option_spec!(
    bun,
    ["--bun-no-env-file"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--no-env-file"),
    "disable automatic .env loading",
  ),
  option_spec!(
    bun,
    ["--bun-port"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--port"),
    "set default Bun.serve port",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-prefer-latest"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--prefer-latest"),
    "prefer latest packages in Bun runtime",
  ),
  option_spec!(
    bun,
    ["--bun-prefer-offline"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--prefer-offline"),
    "prefer offline package resolution",
  ),
  option_spec!(
    bun,
    ["--bun-preload"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--preload"),
    "run an additional Bun preload",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-smol"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--smol"),
    "enable Bun smol mode",
  ),
  option_spec!(
    bun,
    ["--bun-user-agent"],
    None,
    ValueMode::Required,
    OptionAction::ForwardValue("--user-agent"),
    "set default HTTP User-Agent",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-watch"],
    None,
    ValueMode::None,
    OptionAction::ForwardFlag("--watch"),
    "restart on file changes",
  ),
];

pub(super) fn find_long_option(name: &str) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.long.contains(&name))
}

pub(super) fn find_short_option(short: char) -> Option<&'static OptionSpec> {
  OPTION_SPECS.iter().find(|spec| spec.short == Some(short))
}

impl OptionSpec {
  fn clap_arg(&self) -> Arg {
    let help = self.help;
    let mut arg = Arg::new(strip_long_name(self.long[0]))
      .long(strip_long_name(self.long[0]))
      .help(help.map(|help| help.description).unwrap_or_default());

    for alias in &self.long[1..] {
      arg = arg.alias(strip_long_name(alias));
    }

    if let Some(short) = self.short {
      arg = arg.short(short);
    }

    match self.value {
      ValueMode::None => arg.action(ArgAction::SetTrue),
      ValueMode::Required => arg
        .action(ArgAction::Set)
        .value_name(help.and_then(|help| help.value_name).unwrap_or("..."))
        .num_args(1),
      ValueMode::OptionalEquals => arg
        .action(ArgAction::Set)
        .value_name(help.and_then(|help| help.value_name).unwrap_or("..."))
        .num_args(0..=1)
        .require_equals(true),
    }
  }
}

fn strip_long_name(name: &'static str) -> &'static str {
  name.strip_prefix("--").unwrap_or(name)
}

#[cfg(test)]
mod tests {
  use super::clap_command;

  #[test]
  fn option_specs_should_build_valid_clap_command() {
    clap_command().debug_assert();
  }
}
