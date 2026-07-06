//! Supported Node and Bunode option table.

use semver::Version;

#[derive(Clone, Copy)]
pub struct OptionShape {
  specs: &'static [OptionSpec],
}

#[derive(Clone, Copy)]
pub(super) struct OptionSpec {
  pub(super) long: &'static [&'static str],
  pub(super) short: Option<char>,
  pub(super) value: ValueMode,
  pub(super) node_options_allowed: bool,
  pub(super) help: Option<OptionHelp>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueMode {
  None,
  Required,
  OptionalEquals,
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

pub const fn option_shape_for_bun(_version: &Version) -> OptionShape {
  option_shape_for_bun_baseline()
}

const fn option_shape_for_bun_baseline() -> OptionShape {
  OptionShape { specs: OPTION_SPECS }
}

impl OptionShape {
  pub(super) const fn specs(&self) -> &'static [OptionSpec] {
    self.specs
  }
}

macro_rules! option_spec {
  ($source:ident, [$($long:literal),+], $short:expr, $value:expr, $description:literal $(, $value_name:literal)? $(,)?) => {
    OptionSpec {
      long: &[$($long),+],
      short: $short,
      value: $value,
      node_options_allowed: option_spec!(@node_options_allowed $source),
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
  option_spec!(command, ["--help"], Some('h'), ValueMode::None, "print Bunode supported options",),
  option_spec!(
    command,
    ["--version"],
    Some('v'),
    ValueMode::None,
    "print Node-compatible Bunode version",
  ),
  option_spec!(command, ["--eval"], Some('e'), ValueMode::Required, "evaluate script", "...",),
  option_spec!(
    command,
    ["--print"],
    Some('p'),
    ValueMode::Required,
    "evaluate script and print the result",
    "...",
  ),
  option_spec!(
    node,
    ["--require"],
    Some('r'),
    ValueMode::Required,
    "preload CommonJS module (translated to Bun preload)",
    "...",
  ),
  option_spec!(
    node,
    ["--import"],
    None,
    ValueMode::Required,
    "preload ES module (translated to Bun preload)",
    "...",
  ),
  option_spec!(
    node,
    ["--inspect"],
    None,
    ValueMode::OptionalEquals,
    "activate inspector",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--inspect-brk"],
    None,
    ValueMode::OptionalEquals,
    "activate inspector and break at start",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--inspect-wait"],
    None,
    ValueMode::OptionalEquals,
    "activate inspector and wait for debugger",
    "[host:]port",
  ),
  option_spec!(
    node,
    ["--conditions"],
    Some('C'),
    ValueMode::Required,
    "pass custom conditions to resolve",
    "...",
  ),
  option_spec!(node, ["--cpu-prof"], None, ValueMode::None, "start CPU profiler",),
  option_spec!(
    node,
    ["--cpu-prof-dir"],
    None,
    ValueMode::Required,
    "set CPU profile output directory",
    "...",
  ),
  option_spec!(
    node,
    ["--cpu-prof-interval"],
    None,
    ValueMode::Required,
    "set CPU profile sampling interval",
    "...",
  ),
  option_spec!(
    node,
    ["--cpu-prof-name"],
    None,
    ValueMode::Required,
    "set CPU profile output file name",
    "...",
  ),
  option_spec!(node, ["--heap-prof"], None, ValueMode::None, "write heap profile on exit",),
  option_spec!(
    node,
    ["--heap-prof-dir"],
    None,
    ValueMode::Required,
    "set heap profile output directory",
    "...",
  ),
  option_spec!(
    node,
    ["--heap-prof-name"],
    None,
    ValueMode::Required,
    "set heap profile output file name",
    "...",
  ),
  option_spec!(
    node,
    ["--dns-result-order"],
    None,
    ValueMode::Required,
    "set default dns.lookup result order",
    "...",
  ),
  option_spec!(
    node_cli,
    ["--env-file"],
    None,
    ValueMode::Required,
    "load environment variables from a file",
    "...",
  ),
  option_spec!(node, ["--expose-gc"], None, ValueMode::None, "expose gc on the global object",),
  option_spec!(node, ["--no-addons"], None, ValueMode::None, "disable native addons",),
  option_spec!(node, ["--no-deprecation"], None, ValueMode::None, "suppress deprecation warnings",),
  option_spec!(
    node,
    ["--throw-deprecation"],
    None,
    ValueMode::None,
    "throw deprecation warnings as exceptions",
  ),
  option_spec!(node, ["--title"], None, ValueMode::Required, "set process title", "...",),
  option_spec!(
    node,
    ["--unhandled-rejections"],
    None,
    ValueMode::Required,
    "set unhandled rejection mode",
    "...",
  ),
  option_spec!(node, ["--use-bundled-ca"], None, ValueMode::None, "use bundled CA store",),
  option_spec!(node, ["--use-openssl-ca"], None, ValueMode::None, "use OpenSSL CA store",),
  option_spec!(node, ["--use-system-ca"], None, ValueMode::None, "use system CA store",),
  option_spec!(
    node,
    ["--zero-fill-buffers"],
    None,
    ValueMode::None,
    "zero-fill Buffer.allocUnsafe",
  ),
  option_spec!(bun, ["--bun-config"], None, ValueMode::Required, "specify bunfig.toml path", "...",),
  option_spec!(
    bun,
    ["--bun-console-depth"],
    None,
    ValueMode::Required,
    "set console inspection depth",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-env-file"],
    None,
    ValueMode::Required,
    "load Bun environment file",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-fetch-preconnect"],
    None,
    ValueMode::Required,
    "preconnect while code is loading",
    "...",
  ),
  option_spec!(bun, ["--bun-hot"], None, ValueMode::None, "enable Bun hot reload",),
  option_spec!(
    bun,
    ["--bun-install"],
    None,
    ValueMode::Required,
    "configure Bun auto-install behavior",
    "...",
  ),
  option_spec!(
    bun,
    ["--bun-no-clear-screen"],
    None,
    ValueMode::None,
    "disable reload clear screen behavior",
  ),
  option_spec!(bun, ["--bun-no-env-file"], None, ValueMode::None, "disable automatic .env loading",),
  option_spec!(bun, ["--bun-port"], None, ValueMode::Required, "set default Bun.serve port", "...",),
  option_spec!(
    bun,
    ["--bun-prefer-latest"],
    None,
    ValueMode::None,
    "prefer latest packages in Bun runtime",
  ),
  option_spec!(
    bun,
    ["--bun-prefer-offline"],
    None,
    ValueMode::None,
    "prefer offline package resolution",
  ),
  option_spec!(
    bun,
    ["--bun-preload"],
    None,
    ValueMode::Required,
    "run an additional Bun preload",
    "...",
  ),
  option_spec!(bun, ["--bun-smol"], None, ValueMode::None, "enable Bun smol mode",),
  option_spec!(
    bun,
    ["--bun-user-agent"],
    None,
    ValueMode::Required,
    "set default HTTP User-Agent",
    "...",
  ),
  option_spec!(bun, ["--bun-watch"], None, ValueMode::None, "restart on file changes",),
];

pub(super) fn find_long_option(shape: &OptionShape, name: &str) -> Option<&'static OptionSpec> {
  shape.specs.iter().find(|spec| spec.long.contains(&name))
}

pub(super) fn find_short_option(shape: &OptionShape, short: char) -> Option<&'static OptionSpec> {
  shape.specs.iter().find(|spec| spec.short == Some(short))
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  #[test]
  fn option_specs_should_have_unique_names() {
    let shape = super::option_shape_for_bun_baseline();
    let mut long_names = HashSet::new();
    let mut short_names = HashSet::new();

    for spec in shape.specs() {
      for long in spec.long {
        assert!(long.starts_with("--"));
        assert!(long_names.insert(*long), "duplicate long option {long}");
      }

      if let Some(short) = spec.short {
        assert!(short_names.insert(short), "duplicate short option -{short}");
      }
    }
  }
}
