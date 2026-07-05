//! This module is used to call `bun` binary.
//! Only CLI arguments definition and its testing should be included in this modules.

use std::ffi::{OsStr, OsString};

use clap::{Arg, ArgAction, Command, builder::OsStringValueParser};

const HELP_DOCUMENT: &str = "Hello, World\n";

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
  pub help: bool,
  pub bunode_options: Vec<OsString>,
  pub script: Option<OsString>,
  pub script_arguments: Vec<OsString>,
}

#[derive(Debug, PartialEq, Eq)]
struct NodeArgSplit {
  bunode_options: Vec<OsString>,
  script: Option<OsString>,
  script_arguments: Vec<OsString>,
}

pub fn parse<I, T>(args: I) -> Result<Invocation, clap::Error>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString> + Clone,
{
  let mut args = args.into_iter();
  let program = args.next().map_or_else(|| OsString::from("node"), Into::into);
  let split = split_node_args(args.map(Into::into));

  let mut bunode_args = Vec::with_capacity(split.bunode_options.len() + 1);
  bunode_args.push(program);
  bunode_args.extend(split.bunode_options);

  let matches = command().try_get_matches_from(bunode_args)?;
  let bunode_options = matches
    .get_many::<OsString>("bunode-options")
    .map_or_else(Vec::new, |values| values.cloned().collect());

  Ok(Invocation {
    help: matches.get_flag("help"),
    bunode_options,
    script: split.script,
    script_arguments: split.script_arguments,
  })
}

pub fn print_help() {
  print!("{HELP_DOCUMENT}");
}

fn split_node_args(args: impl IntoIterator<Item = OsString>) -> NodeArgSplit {
  let mut bunode_options = Vec::new();
  let mut script = None;
  let mut script_arguments = Vec::new();
  let mut args = args.into_iter();

  while let Some(arg) = args.next() {
    if arg == OsStr::new("--") {
      script = args.next();
      script_arguments.extend(args);
      break;
    }

    if is_bunode_option(&arg) {
      bunode_options.push(arg);
      continue;
    }

    script = Some(arg);
    script_arguments.extend(args);
    break;
  }

  NodeArgSplit { bunode_options, script, script_arguments }
}

fn is_bunode_option(arg: &OsStr) -> bool {
  arg != OsStr::new("-") && arg.as_encoded_bytes().starts_with(b"-")
}

fn command() -> Command {
  Command::new("node")
    .disable_help_flag(true)
    .disable_help_subcommand(true)
    .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue))
    .arg(
      Arg::new("bunode-options")
        .num_args(0..)
        .allow_hyphen_values(true)
        .trailing_var_arg(true)
        .value_parser(OsStringValueParser::new()),
    )
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use super::{Invocation, parse};

  #[test]
  fn parse_should_keep_script_arguments_after_script_operand() -> Result<(), clap::Error> {
    let options = parse(["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      options,
      Invocation {
        help: false,
        bunode_options: vec![OsString::from("--inspect")],
        script: Some(OsString::from("script.js")),
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), clap::Error> {
    let options = parse(["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      options,
      Invocation {
        help: false,
        bunode_options: Vec::new(),
        script: Some(OsString::from("--script.js")),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_should_parse_help_before_script_operand() -> Result<(), clap::Error> {
    let options = parse(["node", "--help", "script.js"])?;

    assert_eq!(
      options,
      Invocation {
        help: true,
        bunode_options: Vec::new(),
        script: Some(OsString::from("script.js")),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }
}
