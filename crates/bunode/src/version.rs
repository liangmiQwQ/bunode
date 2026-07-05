//! Bunode version formatting.

use std::{io, process::Command};

use crate::bun;

pub(crate) fn bunode_version() -> io::Result<String> {
  let node_version = read_bun_output(&["-p", "process.version"])?;
  let bun_version = read_bun_output(&["--version"])?;

  Ok(format!("{}+bun.{}", node_version.trim(), bun_version.trim().trim_start_matches('v')))
}

fn read_bun_output(args: &[&str]) -> io::Result<String> {
  let output = Command::new(bun::path()?).args(args).output()?;

  if !output.status.success() {
    return Err(io::Error::other(format!(
      "Bun exited with code {} while reading version",
      output.status.code().unwrap_or(1)
    )));
  }

  Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
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
