use std::{ffi::OsString, path::PathBuf};

use super::{CliError, Result};

pub enum Command {
  Help,
  Version,
  Patch(PatchOptions),
  Revert(RevertOptions),
  List,
  Implode { yes: bool },
  Update { yes: bool },
}

pub struct PatchOptions {
  pub version: String,
  pub prefix: Option<PathBuf>,
  pub copy_to: Option<PathBuf>,
  pub yes: bool,
}

pub struct RevertOptions {
  pub prefix: Option<PathBuf>,
  pub yes: bool,
}

pub fn parse<I, T>(args: I) -> Result<Command>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString>,
{
  let mut values = args.into_iter().map(Into::into);
  let _ = values.next();
  let Some(command) = values.next() else {
    return Ok(Command::Help);
  };
  let command = command.to_str().ok_or_else(|| CliError::new("command name is not valid UTF-8"))?;
  let tail = values.collect::<Vec<_>>();

  match command {
    "-h" | "--help" | "help" => no_arguments(Command::Help, &tail),
    "-V" | "--version" | "version" => no_arguments(Command::Version, &tail),
    "patch" => parse_patch(&tail),
    "revert" => parse_revert(&tail),
    "list" | "ls" => no_arguments(Command::List, &tail),
    "implode" => parse_yes_only(&tail, |yes| Command::Implode { yes }),
    "update" => parse_yes_only(&tail, |yes| Command::Update { yes }),
    _ => Err(CliError::new(format!("unknown command `{command}`; run `bunode --help` for usage"))),
  }
}

fn parse_patch(values: &[OsString]) -> Result<Command> {
  let mut version = None;
  let mut prefix = None;
  let mut copy_to = None;
  let mut yes = false;
  let mut index = 0;

  while index < values.len() {
    match values[index].to_str() {
      Some("-y" | "--yes") => yes = true,
      Some("--copy") => {
        index += 1;
        copy_to = Some(PathBuf::from(
          values.get(index).ok_or_else(|| CliError::new("`--copy` requires a path"))?,
        ));
      }
      Some(value) if value.starts_with('-') => {
        return Err(CliError::new(format!("unknown patch option `{value}`")));
      }
      _ if version.is_none() => {
        version = Some(
          values[index]
            .to_str()
            .ok_or_else(|| CliError::new("Bun version is not valid UTF-8"))?
            .to_owned(),
        );
      }
      _ if prefix.is_none() => prefix = Some(PathBuf::from(&values[index])),
      _ => return Err(CliError::new("patch accepts at most one Node.js prefix")),
    }

    index += 1;
  }

  let version = version.ok_or_else(|| {
    CliError::new("missing Bun version; usage: bunode patch <version> [node-prefix]")
  })?;

  Ok(Command::Patch(PatchOptions { version, prefix, copy_to, yes }))
}

fn parse_revert(values: &[OsString]) -> Result<Command> {
  let mut prefix = None;
  let mut yes = false;

  for value in values {
    match value.to_str() {
      Some("-y" | "--yes") => yes = true,
      Some(value) if value.starts_with('-') => {
        return Err(CliError::new(format!("unknown revert option `{value}`")));
      }
      _ if prefix.is_none() => prefix = Some(PathBuf::from(value)),
      _ => return Err(CliError::new("revert accepts at most one Bunode prefix")),
    }
  }

  Ok(Command::Revert(RevertOptions { prefix, yes }))
}

fn parse_yes_only(values: &[OsString], command: impl FnOnce(bool) -> Command) -> Result<Command> {
  let mut yes = false;

  for value in values {
    match value.to_str() {
      Some("-y" | "--yes") => yes = true,
      Some(value) => return Err(CliError::new(format!("unknown option `{value}`"))),
      None => return Err(CliError::new("option is not valid UTF-8")),
    }
  }

  Ok(command(yes))
}

fn no_arguments(command: Command, values: &[OsString]) -> Result<Command> {
  if values.is_empty() {
    Ok(command)
  } else {
    Err(CliError::new("this command does not accept arguments"))
  }
}

pub fn print_help() {
  println!(
    "\
Bunode manages Node.js-compatible prefixes backed by Bun.

Usage: bunode <command> [options]

Commands:
  patch <version> [node-prefix]  Turn a Node.js prefix into a Bunode prefix
  revert [bunode-prefix]         Restore a managed prefix to Node.js
  list                           List managed Bunode prefixes
  update                         Install this Bunode wrapper into managed prefixes
  implode                        Revert every managed Bunode prefix

Patch options:
  --copy <new-prefix>            Patch a copy instead of the original prefix
  -y, --yes                      Accept confirmations

Revert, update, and implode options:
  -y, --yes                      Accept confirmations

Global options:
  -h, --help                     Print help
  -V, --version                  Print the Bunode CLI version"
  );
}

#[cfg(test)]
mod tests {
  use super::{Command, parse};

  #[test]
  fn patch_should_accept_options_around_operands() {
    let command =
      parse(["bunode", "patch", "--yes", "1.3.14", "/node", "--copy", "/bunode"]).unwrap();
    let Command::Patch(options) = command else {
      panic!("expected patch command");
    };

    assert_eq!(options.version, "1.3.14");
    assert_eq!(options.prefix.unwrap(), PathBuf::from("/node"));
    assert_eq!(options.copy_to.unwrap(), PathBuf::from("/bunode"));
    assert!(options.yes);
  }

  use std::path::PathBuf;
}
