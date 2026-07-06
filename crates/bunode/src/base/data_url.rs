//! `data:` JavaScript module materialization for Bun preload compatibility.

use std::{
  env, fs, io,
  path::{Path, PathBuf},
};

use crate::error::CliError;

pub(super) fn materialize_javascript_module(specifier: &str) -> Result<Option<PathBuf>, CliError> {
  let Some(rest) = specifier.strip_prefix("data:") else {
    return Ok(None);
  };
  let Some((metadata, payload)) = rest.split_once(',') else {
    return Err(CliError::new("invalid data URL passed to --import"));
  };

  if !is_javascript_metadata(metadata) {
    return Ok(None);
  }

  let source = decode_payload(metadata, payload)?;
  let path = module_path(specifier);
  write_content_addressed_file(&path, &source)
    .map_err(|error| CliError::failure(format!("failed to prepare data URL import: {error}")))?;

  Ok(Some(path))
}

fn is_javascript_metadata(metadata: &str) -> bool {
  let media_type = metadata.split(';').next().unwrap_or_default();

  matches!(media_type, "text/javascript" | "application/javascript")
}

fn decode_payload(metadata: &str, payload: &str) -> Result<Vec<u8>, CliError> {
  if metadata.split(';').any(|part| part.eq_ignore_ascii_case("base64")) {
    return decode_base64(payload);
  }

  decode_percent(payload)
}

fn decode_percent(value: &str) -> Result<Vec<u8>, CliError> {
  let mut result = Vec::new();
  let bytes = value.as_bytes();
  let mut index = 0;

  while index < bytes.len() {
    if bytes[index] != b'%' {
      result.push(bytes[index]);
      index += 1;
      continue;
    }

    let Some(hex) = bytes.get((index + 1)..(index + 3)) else {
      return Err(CliError::new("invalid percent escape in data URL import"));
    };
    let hex = std::str::from_utf8(hex)
      .map_err(|_| CliError::new("invalid percent escape in data URL import"))?;
    let byte = u8::from_str_radix(hex, 16)
      .map_err(|_| CliError::new("invalid percent escape in data URL import"))?;

    result.push(byte);
    index += 3;
  }

  Ok(result)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, CliError> {
  let mut result = Vec::new();
  let mut buffer = 0u32;
  let mut bits = 0u8;

  for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
    if byte == b'=' {
      break;
    }

    let Some(value) = base64_value(byte) else {
      return Err(CliError::new("invalid base64 payload in data URL import"));
    };

    buffer = (buffer << 6) | u32::from(value);
    bits += 6;

    while bits >= 8 {
      bits -= 8;
      result.push(((buffer >> bits) & 0xff) as u8);
    }
  }

  Ok(result)
}

const fn base64_value(byte: u8) -> Option<u8> {
  match byte {
    b'A'..=b'Z' => Some(byte - b'A'),
    b'a'..=b'z' => Some(byte - b'a' + 26),
    b'0'..=b'9' => Some(byte - b'0' + 52),
    b'+' => Some(62),
    b'/' => Some(63),
    _ => None,
  }
}

fn module_path(specifier: &str) -> PathBuf {
  let mut path = env::temp_dir();

  path.push(format!("bunode-data-import-{:016x}.mjs", fnv1a(specifier.as_bytes())));
  path
}

fn write_content_addressed_file(path: &Path, source: &[u8]) -> io::Result<()> {
  if fs::read(path).is_ok_and(|content| content == source) {
    return Ok(());
  }

  let temporary_path = path.with_extension(format!("{}.tmp", std::process::id()));
  fs::write(&temporary_path, source)?;

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
  use super::{decode_base64, decode_percent, materialize_javascript_module};

  #[test]
  fn should_decode_percent_data_payload() -> Result<(), crate::error::CliError> {
    let decoded = decode_percent("globalThis.loaded%3D1")?;

    assert_eq!(decoded, b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn should_decode_base64_data_payload() -> Result<(), crate::error::CliError> {
    let decoded = decode_base64("Z2xvYmFsVGhpcy5sb2FkZWQ9MQ==")?;

    assert_eq!(decoded, b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn should_materialize_javascript_data_import() -> Result<(), crate::error::CliError> {
    let path =
      materialize_javascript_module("data:text/javascript,globalThis.loaded%3D1")?.unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn should_ignore_non_data_imports() -> Result<(), crate::error::CliError> {
    assert!(materialize_javascript_module("./preload.mjs")?.is_none());

    Ok(())
  }
}
