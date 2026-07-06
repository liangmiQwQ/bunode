//! Minimal Node env-file parsing for early `NODE_OPTIONS` discovery.

use std::{ffi::OsString, fs, path::Path};

use crate::error::CliError;

pub(super) fn read_node_options(path: &std::ffi::OsStr) -> Result<Option<OsString>, CliError> {
  let path = Path::new(path);

  if !path.is_file() {
    return Err(CliError::new(format!("{}: not found", path.display())));
  }

  let source = fs::read_to_string(path)
    .map_err(|error| CliError::failure(format!("failed to read {}: {error}", path.display())))?;
  let mut node_options = None;

  for line in source.lines() {
    let Some((name, value)) = parse_assignment(line)? else {
      continue;
    };

    if name == "NODE_OPTIONS" {
      node_options = Some(OsString::from(value));
    }
  }

  Ok(node_options)
}

fn parse_assignment(line: &str) -> Result<Option<(&str, String)>, CliError> {
  let line = line.trim_start();

  if line.is_empty() || line.starts_with('#') {
    return Ok(None);
  }

  let line = line.strip_prefix("export ").unwrap_or(line);
  let Some((name, value)) = line.split_once('=') else {
    return Ok(None);
  };
  let name = name.trim();

  if name.is_empty() {
    return Ok(None);
  }

  Ok(Some((name, parse_value(value)?)))
}

fn parse_value(value: &str) -> Result<String, CliError> {
  let value = value.trim();

  let value = match value.as_bytes().first() {
    Some(b'"') => parse_quoted_value(value, '"'),
    Some(b'\'') => parse_quoted_value(value, '\''),
    _ => Ok(parse_unquoted_value(value)),
  }?;

  Ok(value)
}

fn parse_quoted_value(value: &str, quote: char) -> Result<String, CliError> {
  let mut result = String::new();
  let mut characters = value[quote.len_utf8()..].chars().peekable();

  while let Some(character) = characters.next() {
    if quote == '"' && character == '\\' && characters.peek() == Some(&quote) {
      result.push(quote);
      let _ = characters.next();
      continue;
    }

    if character == quote {
      return Ok(result);
    }

    result.push(character);
  }

  Err(CliError::new("unterminated quoted value in env file"))
}

fn parse_unquoted_value(value: &str) -> String {
  let mut result = String::new();

  for character in value.chars() {
    if character == '#' {
      break;
    }

    result.push(character);
  }

  result.trim_end().to_string()
}

#[cfg(test)]
mod tests {
  use super::parse_assignment;

  #[test]
  fn should_parse_node_options_assignment() {
    assert_eq!(
      parse_assignment(r#"NODE_OPTIONS="--conditions from-env""#).unwrap(),
      Some(("NODE_OPTIONS", "--conditions from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("export NODE_OPTIONS='--conditions from-env'").unwrap(),
      Some(("NODE_OPTIONS", "--conditions from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("NODE_OPTIONS=--conditions=from-env # comment").unwrap(),
      Some(("NODE_OPTIONS", "--conditions=from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("NODE_OPTIONS=--conditions=from-env#comment").unwrap(),
      Some(("NODE_OPTIONS", "--conditions=from-env".to_string())),
    );
    assert_eq!(
      parse_assignment(r#"NODE_OPTIONS="--require C:\tmp\preload.js""#).unwrap(),
      Some(("NODE_OPTIONS", r"--require C:\tmp\preload.js".to_string())),
    );
    assert_eq!(
      parse_assignment(r#"NODE_OPTIONS="--require ./x\" y.js""#).unwrap(),
      Some(("NODE_OPTIONS", r#"--require ./x" y.js"#.to_string())),
    );
  }

  #[test]
  fn should_reject_unterminated_quoted_values() {
    assert!(parse_assignment(r#"NODE_OPTIONS="--conditions from-env"#).is_err());
  }

  #[test]
  fn should_ignore_non_assignments() {
    assert_eq!(parse_assignment("# NODE_OPTIONS=--bad").unwrap(), None);
    assert_eq!(parse_assignment("").unwrap(), None);
  }
}
