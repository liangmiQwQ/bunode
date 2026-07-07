//! Node env-file parsing for early `NODE_OPTIONS` discovery.

use std::{ffi::OsString, fs, path::Path};

use deno_dotenv::parse_env_content_hook;

use crate::error::{BunodeError, CliFailureError, CliUsageError};

pub(super) fn read_node_options(path: &std::ffi::OsStr) -> Result<Option<OsString>, BunodeError> {
  let path = Path::new(path);

  if !path.is_file() {
    return Err(CliUsageError::FileNotFound(path.to_path_buf()).into());
  }

  let source = fs::read_to_string(path)
    .map_err(|source| CliFailureError::ReadEnvFile { path: path.to_path_buf(), source })?;
  let node_options = read_node_options_from_source(&source);

  Ok(node_options.map(OsString::from))
}

fn read_node_options_from_source(source: &str) -> Option<String> {
  let mut node_options = None;

  parse_env_content_hook(source, &mut |name, value| {
    if name != "NODE_OPTIONS" {
      return;
    }

    node_options = Some(value.to_string());
  });

  node_options
}

#[cfg(test)]
mod tests {
  use super::read_node_options_from_source;

  #[test]
  fn should_parse_node_options_assignment() {
    assert_eq!(
      read_node_options_from_source(r#"NODE_OPTIONS="--conditions from-env""#),
      Some("--conditions from-env".to_string()),
    );
    assert_eq!(
      read_node_options_from_source("export NODE_OPTIONS='--conditions from-env'"),
      Some("--conditions from-env".to_string()),
    );
    assert_eq!(
      read_node_options_from_source("NODE_OPTIONS=--conditions=from-env # comment"),
      Some("--conditions=from-env".to_string()),
    );
    assert_eq!(
      read_node_options_from_source("NODE_OPTIONS=--conditions=from-env#comment"),
      Some("--conditions=from-env".to_string()),
    );
    assert_eq!(
      read_node_options_from_source(r#"NODE_OPTIONS="--require C:\tmp\preload.js""#),
      Some(r"--require C:\tmp\preload.js".to_string()),
    );
    assert_eq!(
      read_node_options_from_source(r#"NODE_OPTIONS="--require ./x\" y.js""#),
      Some(r#"--require ./x\" y.js"#.to_string()),
    );
  }

  #[test]
  fn should_keep_last_node_options_assignment() {
    let source = "NODE_OPTIONS=--conditions first\nNODE_OPTIONS=--conditions second\n";

    assert_eq!(read_node_options_from_source(source), Some("--conditions second".to_string()));
  }

  #[test]
  fn should_follow_node_parser_for_malformed_values() {
    let source = "BAD=\"unterminated\nNODE_OPTIONS=\"--conditions from-env\"\n";

    assert_eq!(read_node_options_from_source(source), None);
  }

  #[test]
  fn should_parse_multiline_node_options_values() {
    let source = "NODE_OPTIONS=\"--conditions a\n--conditions b\"\n";

    assert_eq!(
      read_node_options_from_source(source),
      Some("--conditions a\n--conditions b".to_string())
    );
  }

  #[test]
  fn should_ignore_non_assignments() {
    assert_eq!(read_node_options_from_source("# NODE_OPTIONS=--bad"), None);
    assert_eq!(read_node_options_from_source(""), None);
  }
}
