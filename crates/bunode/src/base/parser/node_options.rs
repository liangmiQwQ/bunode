//! `NODE_OPTIONS` splitting before token parsing.
//!
//! Node parses this environment variable without a shell, so these quote rules stay
//! local instead of using a POSIX shell splitter.

use std::ffi::{OsStr, OsString};

use crate::error::{BunodeError, CliUsageError};

pub(super) fn split_node_options(value: &OsStr) -> Result<Vec<OsString>, BunodeError> {
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
    return Err(CliUsageError::UnterminatedNodeOptionsQuote.into());
  }

  if !current.is_empty() {
    result.push(OsString::from(current));
  }

  Ok(result)
}

#[cfg(test)]
mod tests {
  use std::ffi::{OsStr, OsString};

  use super::split_node_options;

  #[test]
  fn split_should_keep_double_quoted_value() -> Result<(), crate::error::BunodeError> {
    let values = split_node_options(OsStr::new("--require \"./with space.js\""))?;

    assert_eq!(values, vec![OsString::from("--require"), OsString::from("./with space.js")]);

    Ok(())
  }

  #[test]
  fn split_should_keep_single_quotes_as_literal() -> Result<(), crate::error::BunodeError> {
    let values = split_node_options(OsStr::new("--require './preload.js'"))?;

    assert_eq!(values, vec![OsString::from("--require"), OsString::from("'./preload.js'")]);

    Ok(())
  }

  #[test]
  fn split_should_preserve_backslashes() -> Result<(), crate::error::BunodeError> {
    let values = split_node_options(OsStr::new(r"--require C:\tmp\preload.js"))?;

    assert_eq!(values, vec![OsString::from("--require"), OsString::from(r"C:\tmp\preload.js")]);

    Ok(())
  }

  #[test]
  fn split_should_keep_escaped_double_quote() -> Result<(), crate::error::BunodeError> {
    let values = split_node_options(OsStr::new(r#"--require "./x\" y.js""#))?;

    assert_eq!(values, vec![OsString::from("--require"), OsString::from(r#"./x" y.js"#)]);

    Ok(())
  }
}
