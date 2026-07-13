use std::{
  env, fs,
  fs::File,
  path::{Component, Path, PathBuf},
  process::Command,
};

use flate2::read::GzDecoder;
use serde::Deserialize;

use super::{CliError, Result};

#[derive(Deserialize)]
struct PackedPackage {
  filename: PathBuf,
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

pub fn download(version: &str, destination: &Path, source_prefix: &Path) -> Result<()> {
  let package = platform_package()?;
  let archive_directory = destination.with_extension("package");
  fs::create_dir(&archive_directory)?;
  let result = (|| {
    let archive = npm_pack(package, version, source_prefix, &archive_directory)?;
    extract_binary(&archive, destination)
  })();
  let _ = fs::remove_dir_all(archive_directory);
  result
}

fn npm_pack(
  package: &str,
  version: &str,
  source_prefix: &Path,
  destination: &Path,
) -> Result<PathBuf> {
  // 1. Resolve npm from the original prefix and bind its env-node shebang to the same prefix.
  let npm = npm_executable(source_prefix);
  if !npm.is_file() {
    return Err(CliError::new(format!(
      "npm is missing from the original Node.js prefix at {}",
      npm.display()
    )));
  }

  let mut paths = vec![node_bin_directory(source_prefix)];
  if let Some(path) = env::var_os("PATH") {
    paths.extend(env::split_paths(&path));
  }
  let path = env::join_paths(paths)
    .map_err(|error| CliError::new(format!("failed to prepare npm PATH: {error}")))?;

  // 2. Keep npm isolated from project-level configuration while preserving user registry settings.
  let mut command = Command::new(&npm);
  command
    .arg("pack")
    .arg(format!("{package}@{version}"))
    .args(["--json", "--ignore-scripts", "--pack-destination"])
    .arg(destination)
    .current_dir(destination)
    .env("PATH", path)
    .env("npm_config_update_notifier", "false");
  if let Some(registry) = env::var_os("BUNODE_REGISTRY") {
    command.env("npm_config_registry", registry);
  }

  let output = command
    .output()
    .map_err(|error| CliError::new(format!("failed to run {}: {error}", npm.display())))?;
  if !output.status.success() {
    return Err(CliError::new(format!(
      "npm pack exited with {}: {}",
      output.status,
      String::from_utf8_lossy(&output.stderr).trim()
    )));
  }

  // 3. Resolve the archive npm wrote without trusting a path outside the temporary directory.
  let mut packages = serde_json::from_slice::<Vec<PackedPackage>>(&output.stdout)
    .map_err(|error| CliError::new(format!("invalid npm pack response: {error}")))?;
  if packages.len() != 1 {
    return Err(CliError::new(format!(
      "npm pack returned {} packages instead of one",
      packages.len()
    )));
  }
  let filename =
    packages.pop().ok_or_else(|| CliError::new("npm pack returned no package"))?.filename;
  let mut components = filename.components();
  if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
    return Err(CliError::new("npm pack returned an invalid archive filename"));
  }
  let archive = destination.join(filename);
  if !archive.is_file() {
    return Err(CliError::new(format!("npm pack did not create {}", archive.display())));
  }

  Ok(archive)
}

fn extract_binary(archive: &Path, destination: &Path) -> Result<()> {
  let decoder = GzDecoder::new(File::open(archive)?);
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

fn npm_executable(prefix: &Path) -> PathBuf {
  if cfg!(windows) { prefix.join("npm.cmd") } else { prefix.join("bin/npm") }
}

fn node_bin_directory(prefix: &Path) -> PathBuf {
  if cfg!(windows) { prefix.to_owned() } else { prefix.join("bin") }
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
