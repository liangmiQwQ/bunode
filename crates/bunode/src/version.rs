//! Bunode version formatting.

use std::{io, process::Command};

use semver::{BuildMetadata, Version};

use crate::{bun, error::BunodeError};

pub fn bunode_version(bun_version: &Version, masqueraded_version: &Version) -> Version {
  let mut version =
    Version::new(masqueraded_version.major, masqueraded_version.minor, masqueraded_version.patch);

  version.build = BuildMetadata::new(&format!(
    "bun.{}.{}.{}",
    bun_version.major, bun_version.minor, bun_version.patch
  ))
  // SAFETY: bun_version is number, follow semver's rule.
  .unwrap();

  version
}

/// For performance, we should only call this once, and cache its result
pub fn bun_version() -> Result<Version, BunodeError> {
  let bun_version = read_bun_version_from_output(&["--version"])?;

  match Version::parse(&bun_version) {
    Ok(version) => check_bun_semver(version),
    Err(_) => Err(BunodeError::BadBunVersion(bun_version)),
  }
}

/// For performance, we should only call this once, and cache its result
pub fn masqueraded_version() -> Result<Version, BunodeError> {
  let bun_version = read_bun_version_from_output(&["-p", "process.version"])?;

  match Version::parse(&bun_version) {
    Ok(version) => Ok(version),
    Err(_) => Err(BunodeError::BadNodeCompatibleShimVersion(bun_version)),
  }
}

fn read_bun_version_from_output(args: &[&str]) -> Result<String, BunodeError> {
  let bun_path = bun::path()?;
  let bun_directory = bun_path
    .parent()
    .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bun directory"))?;
  // Version probes must not execute project bunfig preloads from the caller's cwd.
  let output = Command::new(&bun_path).current_dir(bun_directory).args(args).output()?;

  if !output.status.success() {
    return Err(BunodeError::CommandExecutionWithExitCode(output.status.code().unwrap_or(1)));
  }

  Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// We require both Bun itself's version and Node.js-compatibility shim is clear, means no prerelease and build metadata.
fn check_bun_semver(version: Version) -> Result<Version, BunodeError> {
  if version.pre.is_empty() && version.build.is_empty() {
    Ok(version)
  } else {
    Err(BunodeError::UnsupporttedBunVersion(version.to_string()))
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn version_metadata_should_keep_node_version_precedence() {
    let node_version = "v24.3.0";
    let bun_version = "1.3.14";

    assert_eq!(format!("{node_version}+bun.{bun_version}"), "v24.3.0+bun.1.3.14");
  }
}
