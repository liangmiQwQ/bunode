use std::{
  env, fs,
  io::Cursor,
  path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha512};

use super::{CliError, Result};

const MAX_BUN_PACKAGE_SIZE: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
struct PackageMetadata {
  dist: PackageDistribution,
}

#[derive(Deserialize)]
struct PackageDistribution {
  integrity: String,
  tarball: String,
}

pub fn normalize_version(version: &str) -> Result<String> {
  let version = version
    .trim()
    .strip_prefix("bun-v")
    .or_else(|| version.trim().strip_prefix('v'))
    .unwrap_or_else(|| version.trim());

  if version.is_empty()
    || !version
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
  {
    return Err(CliError::new(format!("invalid Bun version `{version}`")));
  }

  Ok(version.to_owned())
}

pub fn download(version: &str, destination: &Path) -> Result<()> {
  let package = platform_package()?;
  let registry = env::var("BUNODE_REGISTRY")
    .or_else(|_| env::var("npm_config_registry"))
    .unwrap_or_else(|_| "https://registry.npmjs.org".to_owned());
  let metadata_url =
    format!("{}/{}/{version}", registry.trim_end_matches('/'), package.replace('/', "%2F"));
  let metadata = get_json::<PackageMetadata>(&metadata_url)?;
  let archive = get_bytes(&metadata.dist.tarball)?;

  verify_integrity(&archive, &metadata.dist.integrity)?;
  extract_binary(&archive, destination)
}

fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
  let mut response = ureq::get(url)
    .header("User-Agent", concat!("bunode/", env!("CARGO_PKG_VERSION")))
    .call()
    .map_err(|error| CliError::new(format!("failed to download {url}: {error}")))?;

  response
    .body_mut()
    .read_json()
    .map_err(|error| CliError::new(format!("invalid registry response from {url}: {error}")))
}

fn get_bytes(url: &str) -> Result<Vec<u8>> {
  let mut response = ureq::get(url)
    .header("User-Agent", concat!("bunode/", env!("CARGO_PKG_VERSION")))
    .call()
    .map_err(|error| CliError::new(format!("failed to download {url}: {error}")))?;

  response
    .body_mut()
    .with_config()
    .limit(MAX_BUN_PACKAGE_SIZE)
    .read_to_vec()
    .map_err(|error| CliError::new(format!("failed to read {url}: {error}")))
}

fn verify_integrity(archive: &[u8], integrity: &str) -> Result<()> {
  let encoded = integrity
    .strip_prefix("sha512-")
    .ok_or_else(|| CliError::new("Bun package does not provide sha512 integrity"))?;
  let expected = STANDARD
    .decode(encoded)
    .map_err(|error| CliError::new(format!("invalid Bun package integrity: {error}")))?;
  let actual = Sha512::digest(archive);

  if actual.as_slice() == expected {
    Ok(())
  } else {
    Err(CliError::new("downloaded Bun package failed its integrity check"))
  }
}

fn extract_binary(archive: &[u8], destination: &Path) -> Result<()> {
  let decoder = GzDecoder::new(Cursor::new(archive));
  let mut archive = tar::Archive::new(decoder);
  let expected =
    PathBuf::from(if cfg!(windows) { "package/bin/bun.exe" } else { "package/bin/bun" });
  let entries = archive
    .entries()
    .map_err(|error| CliError::new(format!("failed to open Bun package: {error}")))?;

  for entry in entries {
    let mut entry =
      entry.map_err(|error| CliError::new(format!("failed to read Bun package: {error}")))?;
    let path = entry
      .path()
      .map_err(|error| CliError::new(format!("invalid path in Bun package: {error}")))?;

    if path == expected {
      if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
      }
      entry
        .unpack(destination)
        .map_err(|error| CliError::new(format!("failed to extract Bun: {error}")))?;
      return Ok(());
    }
  }

  Err(CliError::new(format!("Bun package does not contain {}", expected.display())))
}

fn platform_package() -> Result<&'static str> {
  match (env::consts::OS, env::consts::ARCH) {
    ("macos", "aarch64") => Ok("@oven/bun-darwin-aarch64"),
    ("macos", "x86_64") => Ok("@oven/bun-darwin-x64"),
    ("linux", "aarch64") if cfg!(target_env = "musl") => Ok("@oven/bun-linux-aarch64-musl"),
    ("linux", "aarch64") => Ok("@oven/bun-linux-aarch64"),
    ("linux", "x86_64") if cfg!(target_env = "musl") => Ok("@oven/bun-linux-x64-musl"),
    ("linux", "x86_64") => Ok("@oven/bun-linux-x64"),
    ("windows", "aarch64") => Ok("@oven/bun-windows-aarch64"),
    ("windows", "x86_64") => Ok("@oven/bun-windows-x64"),
    (os, arch) => Err(CliError::new(format!("Bun does not publish an npm binary for {os}-{arch}"))),
  }
}
