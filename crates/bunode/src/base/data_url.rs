//! `data:` JavaScript module materialization for Bun preload compatibility.

use std::{
  env,
  fs::{self, OpenOptions},
  io::{self, Write},
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use crate::error::{BunodeError, CliFailureError, CliUsageError};

pub(super) fn materialize_javascript_module(
  specifier: &str,
) -> Result<Option<PathBuf>, BunodeError> {
  let Some((scheme, rest)) = specifier.split_once(':') else {
    return Ok(None);
  };

  if !scheme.eq_ignore_ascii_case("data") {
    return Ok(None);
  }

  let Some((metadata, payload)) = rest.split_once(',') else {
    return Err(CliUsageError::InvalidDataUrlImport.into());
  };
  let payload = strip_fragment(payload);

  if !is_javascript_metadata(metadata) {
    return Ok(None);
  }

  let source = build_blob_import_wrapper(&decode_payload(metadata, payload)?);
  let path = module_path(specifier);
  write_content_addressed_file(&path, &source).map_err(CliFailureError::PrepareDataUrlImport)?;

  Ok(Some(path))
}

fn is_javascript_metadata(metadata: &str) -> bool {
  let media_type = metadata.split(';').next().unwrap_or_default();

  media_type.eq_ignore_ascii_case("text/javascript")
    || media_type.eq_ignore_ascii_case("application/javascript")
}

fn decode_payload(metadata: &str, payload: &str) -> Result<Vec<u8>, BunodeError> {
  if metadata.split(';').any(|part| part.eq_ignore_ascii_case("base64")) {
    let payload = decode_percent(payload)?;
    let payload =
      std::str::from_utf8(&payload).map_err(|_| CliUsageError::InvalidDataUrlBase64Payload)?;

    return decode_base64(payload);
  }

  decode_percent(payload)
}

fn strip_fragment(payload: &str) -> &str {
  payload.split_once('#').map_or(payload, |(payload, _)| payload)
}

fn decode_percent(value: &str) -> Result<Vec<u8>, BunodeError> {
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
      return Err(CliUsageError::InvalidDataUrlPercentEscape.into());
    };
    let hex = std::str::from_utf8(hex).map_err(|_| CliUsageError::InvalidDataUrlPercentEscape)?;
    let byte =
      u8::from_str_radix(hex, 16).map_err(|_| CliUsageError::InvalidDataUrlPercentEscape)?;

    result.push(byte);
    index += 3;
  }

  Ok(result)
}

fn decode_base64(value: &str) -> Result<Vec<u8>, BunodeError> {
  validate_base64(value)?;

  let mut result = Vec::new();
  let mut buffer = 0u32;
  let mut bits = 0u8;

  for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
    if byte == b'=' {
      break;
    }

    let Some(value) = base64_value(byte) else {
      return Err(CliUsageError::InvalidDataUrlBase64Payload.into());
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

fn build_blob_import_wrapper(source: &[u8]) -> Vec<u8> {
  let encoded = encode_base64(source);

  format!(
    "const __bunodeDataImportBytes=Uint8Array.from(atob(\"{encoded}\"),character=>character.charCodeAt(0));\nconst __bunodeDataImportUrl=URL.createObjectURL(new Blob([__bunodeDataImportBytes],{{type:\"text/javascript\"}}));\ntry{{await import(__bunodeDataImportUrl)}}finally{{URL.revokeObjectURL(__bunodeDataImportUrl)}}\n",
  )
  .into_bytes()
}

fn encode_base64(value: &[u8]) -> String {
  const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  let mut result = String::new();
  let mut index = 0;

  while index < value.len() {
    let first = value[index];
    let second = value.get(index + 1).copied();
    let third = value.get(index + 2).copied();

    result.push(ALPHABET[(first >> 2) as usize] as char);
    result.push(
      ALPHABET[(((first & 0b0000_0011) << 4) | (second.unwrap_or_default() >> 4)) as usize] as char,
    );

    match (second, third) {
      (Some(second), Some(third)) => {
        result.push(ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        result.push(ALPHABET[(third & 0b0011_1111) as usize] as char);
      }
      (Some(second), None) => {
        result.push(ALPHABET[((second & 0b0000_1111) << 2) as usize] as char);
        result.push('=');
      }
      (None, _) => {
        result.push('=');
        result.push('=');
      }
    }

    index += 3;
  }

  result
}

fn validate_base64(value: &str) -> Result<(), BunodeError> {
  let bytes = value.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect::<Vec<_>>();
  let padding_start = bytes.iter().position(|byte| *byte == b'=').unwrap_or(bytes.len());
  let padding_count = bytes.len() - padding_start;
  let data_len = padding_start;

  if padding_count > 2 || bytes.iter().skip(padding_start).any(|byte| *byte != b'=') {
    return Err(CliUsageError::InvalidDataUrlBase64Payload.into());
  }

  if data_len % 4 == 1 {
    return Err(CliUsageError::InvalidDataUrlBase64Payload.into());
  }

  if padding_count > 0 {
    let expected_padding = match data_len % 4 {
      2 => 2,
      3 => 1,
      _ => return Err(CliUsageError::InvalidDataUrlBase64Payload.into()),
    };

    if padding_count != expected_padding {
      return Err(CliUsageError::InvalidDataUrlBase64Payload.into());
    }
  }

  Ok(())
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
  let directory = path.parent().unwrap_or_else(|| Path::new("."));
  let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("bunode-data-import");
  let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();

  for attempt in 0..32 {
    let temporary_path =
      directory.join(format!(".{file_name}.{}.{timestamp}.{attempt}.tmp", std::process::id()));
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

  Err(io::Error::new(io::ErrorKind::AlreadyExists, "failed to create data URL import file"))
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
  use super::{
    build_blob_import_wrapper, decode_base64, decode_percent, materialize_javascript_module,
  };

  #[test]
  fn should_decode_percent_data_payload() -> Result<(), crate::error::BunodeError> {
    let decoded = decode_percent("globalThis.loaded%3D1")?;

    assert_eq!(decoded, b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn should_decode_base64_data_payload() -> Result<(), crate::error::BunodeError> {
    let decoded = decode_base64("Z2xvYmFsVGhpcy5sb2FkZWQ9MQ==")?;

    assert_eq!(decoded, b"globalThis.loaded=1");

    Ok(())
  }

  #[test]
  fn should_reject_invalid_base64_payload() {
    assert!(decode_base64("Z").is_err());
    assert!(decode_base64("Zg==bad").is_err());
  }

  #[test]
  fn should_percent_decode_base64_data_payload() -> Result<(), crate::error::BunodeError> {
    let path = materialize_javascript_module(
      "data:text/javascript;base64,Z2xvYmFsVGhpcy5sb2FkZWQ9MQ%3D%3D",
    )?
    .unwrap();

    let wrapper = std::fs::read_to_string(&path).unwrap();

    assert!(wrapper.contains("URL.createObjectURL"));
    assert!(wrapper.contains("Z2xvYmFsVGhpcy5sb2FkZWQ9MQ=="));

    Ok(())
  }

  #[test]
  fn should_materialize_javascript_data_import() -> Result<(), crate::error::BunodeError> {
    let path =
      materialize_javascript_module("data:text/javascript,globalThis.loaded%3D1")?.unwrap();

    assert!(std::fs::read_to_string(&path).unwrap().contains("URL.createObjectURL"));

    Ok(())
  }

  #[test]
  fn should_materialize_case_insensitive_javascript_data_import()
  -> Result<(), crate::error::BunodeError> {
    let path =
      materialize_javascript_module("data:Text/JavaScript,globalThis.loaded%3D1")?.unwrap();

    assert!(std::fs::read_to_string(&path).unwrap().contains("URL.createObjectURL"));

    Ok(())
  }

  #[test]
  fn should_materialize_case_insensitive_data_scheme() -> Result<(), crate::error::BunodeError> {
    let path =
      materialize_javascript_module("DATA:text/javascript,globalThis.loaded%3D1")?.unwrap();

    assert!(std::fs::read_to_string(&path).unwrap().contains("URL.createObjectURL"));

    Ok(())
  }

  #[test]
  fn should_wrap_data_payload_as_blob_import() {
    let wrapper = String::from_utf8(build_blob_import_wrapper(b"import './x.js'")).unwrap();

    assert!(wrapper.contains("await import(__bunodeDataImportUrl)"));
    assert!(!wrapper.contains("import './x.js'"));
  }

  #[test]
  fn should_strip_data_import_fragment() -> Result<(), crate::error::BunodeError> {
    let path =
      materialize_javascript_module("data:text/javascript,globalThis.loaded%3D1#cache")?.unwrap();

    assert!(std::fs::read_to_string(&path).unwrap().contains("URL.createObjectURL"));

    Ok(())
  }

  #[test]
  fn should_ignore_non_data_imports() -> Result<(), crate::error::BunodeError> {
    assert!(materialize_javascript_module("./preload.mjs")?.is_none());

    Ok(())
  }
}
