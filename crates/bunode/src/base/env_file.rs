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
    let Some((name, value)) = parse_assignment(line) else {
      continue;
    };

    if name == "NODE_OPTIONS" {
      node_options = Some(OsString::from(value));
    }
  }

  Ok(node_options)
}

fn parse_assignment(line: &str) -> Option<(&str, String)> {
  let line = line.trim_start();

  if line.is_empty() || line.starts_with('#') {
    return None;
  }

  let line = line.strip_prefix("export ").unwrap_or(line);
  let (name, value) = line.split_once('=')?;
  let name = name.trim();

  if name.is_empty() {
    return None;
  }

  Some((name, parse_value(value)))
}

fn parse_value(value: &str) -> String {
  let value = value.trim();

  match value.as_bytes().first() {
    Some(b'"') => parse_quoted_value(value, '"'),
    Some(b'\'') => parse_quoted_value(value, '\''),
    _ => parse_unquoted_value(value),
  }
}

fn parse_quoted_value(value: &str, quote: char) -> String {
  let mut result = String::new();
  let mut characters = value[quote.len_utf8()..].chars();
  let mut escaped = false;

  for character in &mut characters {
    if escaped {
      result.push(character);
      escaped = false;
      continue;
    }

    if quote == '"' && character == '\\' {
      escaped = true;
      continue;
    }

    if character == quote {
      break;
    }

    result.push(character);
  }

  result
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
      parse_assignment(r#"NODE_OPTIONS="--conditions from-env""#),
      Some(("NODE_OPTIONS", "--conditions from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("export NODE_OPTIONS='--conditions from-env'"),
      Some(("NODE_OPTIONS", "--conditions from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("NODE_OPTIONS=--conditions=from-env # comment"),
      Some(("NODE_OPTIONS", "--conditions=from-env".to_string())),
    );
    assert_eq!(
      parse_assignment("NODE_OPTIONS=--conditions=from-env#comment"),
      Some(("NODE_OPTIONS", "--conditions=from-env".to_string())),
    );
  }

  #[test]
  fn should_ignore_non_assignments() {
    assert_eq!(parse_assignment("# NODE_OPTIONS=--bad"), None);
    assert_eq!(parse_assignment(""), None);
  }
}
