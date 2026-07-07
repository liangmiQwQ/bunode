//! `CommonJS` `--require` preload materialization for Bun preload compatibility.

use std::{
  env,
  fmt::Write as _,
  fs::{self, OpenOptions},
  io::{self, Write as _},
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use crate::error::CliError;

pub(super) fn materialize_require_wrapper(specifier: &str) -> Result<PathBuf, CliError> {
  let source = build_require_wrapper(specifier);
  let path = wrapper_path(specifier);

  write_content_addressed_file(&path, source.as_bytes()).map_err(|error| {
    CliError::failure(format!("failed to prepare CommonJS preload wrapper: {error}"))
  })?;

  Ok(path)
}

fn build_require_wrapper(specifier: &str) -> String {
  let specifier = js_string_literal(specifier);

  format!(
    "const {{ createRequire }} = require(\"node:module\");\nconst {{ sep }} = require(\"node:path\");\nconst {{ pathToFileURL }} = require(\"node:url\");\nconst __bunodeCwd = process.cwd();\nconst __bunodeBase = __bunodeCwd.endsWith(sep) ? __bunodeCwd : __bunodeCwd + sep;\ncreateRequire(pathToFileURL(__bunodeBase))({specifier});\n",
  )
}

fn wrapper_path(specifier: &str) -> PathBuf {
  let mut path = env::temp_dir();

  path.push(format!("bunode-require-preload-{:016x}.cjs", fnv1a(specifier.as_bytes())));
  path
}

fn write_content_addressed_file(path: &Path, source: &[u8]) -> io::Result<()> {
  if fs::read(path).is_ok_and(|content| content == source) {
    return Ok(());
  }

  let temporary_path = write_private_temporary_file(path, source)?;

  match fs::rename(&temporary_path, path) {
    Ok(()) => Ok(()),
    Err(_) if fs::read(path).is_ok_and(|content| content == source) => {
      let _ = fs::remove_file(&temporary_path);
      Ok(())
    }
    Err(error) => {
      let _ = fs::remove_file(&temporary_path);
      Err(error)
    }
  }
}

fn write_private_temporary_file(path: &Path, source: &[u8]) -> io::Result<PathBuf> {
  let directory = path.parent().ok_or_else(|| {
    io::Error::new(io::ErrorKind::NotFound, "failed to resolve CommonJS preload directory")
  })?;
  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
  let prefix = path.file_name().and_then(|name| name.to_str()).unwrap_or("bunode-preload");

  for attempt in 0..32 {
    let temporary_path =
      directory.join(format!(".{prefix}.{}.{timestamp}.{attempt}.tmp", std::process::id()));
    let mut options = OpenOptions::new();

    options.write(true).create_new(true);

    #[cfg(unix)]
    {
      use std::os::unix::fs::OpenOptionsExt;

      options.mode(0o600);
    }

    match options.open(&temporary_path) {
      Ok(mut file) => {
        file.write_all(source)?;
        return Ok(temporary_path);
      }
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
      Err(error) => return Err(error),
    }
  }

  Err(io::Error::new(
    io::ErrorKind::AlreadyExists,
    "failed to create CommonJS preload wrapper file",
  ))
}

fn js_string_literal(value: &str) -> String {
  format!("\"{}\"", escape_json_string(value))
}

fn escape_json_string(value: &str) -> String {
  let mut result = String::new();

  for character in value.chars() {
    match character {
      '"' => result.push_str("\\\""),
      '\\' => result.push_str("\\\\"),
      '\n' => result.push_str("\\n"),
      '\r' => result.push_str("\\r"),
      '\t' => result.push_str("\\t"),
      character if character.is_control() => {
        let _ = write!(result, "\\u{:04x}", u32::from(character));
      }
      character => result.push(character),
    }
  }

  result
}

fn fnv1a(value: &[u8]) -> u64 {
  let mut result = 0xcbf2_9ce4_8422_2325;

  for byte in value {
    result ^= u64::from(*byte);
    result = result.wrapping_mul(0x0000_0100_0000_01b3);
  }

  result
}

#[cfg(test)]
mod tests {
  use super::materialize_require_wrapper;

  #[test]
  fn should_materialize_common_js_require_wrapper() -> Result<(), crate::error::CliError> {
    let path = materialize_require_wrapper("./preload.cjs")?;
    let wrapper = std::fs::read_to_string(path).unwrap();

    assert!(wrapper.contains("createRequire"));
    assert!(wrapper.contains("\"./preload.cjs\""));

    Ok(())
  }
}
