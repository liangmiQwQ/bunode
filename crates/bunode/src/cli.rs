//! This module is used to call `bun` binary.
//! Only CLI arguments definition and its testing should be included in this modules.

use std::ffi::OsString;

use clap::{Args, Parser, builder::OsStringValueParser};

#[derive(Debug, PartialEq, Eq)]
pub struct BunodeCommandOption {
  pub node_options: NodeOptions,
  pub script: Option<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Debug, Parser)]
#[command(
  name = "node",
  disable_help_flag = true,
  disable_help_subcommand = true,
  disable_version_flag = true,
  trailing_var_arg = true
)]
struct NodeCommand {
  #[clap(flatten)]
  options: NodeOptions,

  #[arg(num_args = 0.., value_parser = OsStringValueParser::new())]
  script_and_arguments: Vec<OsString>,
}

#[derive(Debug, PartialEq, Eq, Args)]
pub struct NodeOptions {
  #[clap(short = 'h', long)]
  pub help: bool,

  #[arg(short = 'v', long)]
  pub version: bool,

  #[arg(long, value_name = "host:port", num_args = 0..=1, require_equals = true, default_missing_value = "", value_parser = OsStringValueParser::new())]
  pub inspect: Option<OsString>,

  #[arg(long = "test-name-pattern", value_name = "pattern", allow_hyphen_values = true, value_parser = OsStringValueParser::new())]
  pub test_name_pattern: Option<OsString>,
}

pub fn parse<I, T>(args: I) -> Result<BunodeCommandOption, clap::Error>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString> + Clone,
{
  // 1. Parse Node options and collect the script tail.
  let command = NodeCommand::try_parse_from(args)?;
  let NodeCommand { options, script_and_arguments } = command;

  // 2. Split the first trailing operand as the script name.
  let mut script_and_arguments = script_and_arguments.into_iter();
  let script = script_and_arguments.next();
  let script_arguments = script_and_arguments.collect();

  Ok(BunodeCommandOption { node_options: options, script, script_arguments })
}

pub fn print_help() {
  print!("Hello, World");
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use super::{BunodeCommandOption, NodeOptions, parse};

  fn empty_options() -> NodeOptions {
    NodeOptions { help: false, version: false, inspect: None, test_name_pattern: None }
  }

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand() -> Result<(), clap::Error> {
    let options = parse(["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        node_options: NodeOptions { inspect: Some(OsString::new()), ..empty_options() },
        script: Some(OsString::from("script.js")),
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_keep_inspect_value_before_script_operand() -> Result<(), clap::Error> {
    let options = parse(["node", "--inspect=127.0.0.1:9229", "script.js"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        node_options: NodeOptions {
          inspect: Some(OsString::from("127.0.0.1:9229")),
          ..empty_options()
        },
        script: Some(OsString::from("script.js")),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), clap::Error> {
    let options = parse(["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        node_options: empty_options(),
        script: Some(OsString::from("--script.js")),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_allow_help_with_script_operand() {
    parse(["node", "--help", "script.js"]).unwrap();
  }

  #[test]
  fn parse_should_keep_option_pattern_before_script_operand() -> Result<(), clap::Error> {
    let options = parse(["node", "--test-name-pattern", "--unit", "script.js", "--help"])?;

    assert_eq!(
      options,
      BunodeCommandOption {
        node_options: NodeOptions {
          test_name_pattern: Some(OsString::from("--unit")),
          ..empty_options()
        },
        script: Some(OsString::from("script.js")),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }
}
