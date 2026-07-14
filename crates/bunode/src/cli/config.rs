use std::{
  env, fs,
  path::{Path, PathBuf},
  process,
};

use serde::{Deserialize, Serialize};

use super::{CliError, Result};

#[derive(Clone, Deserialize, Serialize)]
pub struct PrefixRecord {
  pub path: PathBuf,
  pub original_version: String,
  pub bun_version: String,
  pub bunode_version: String,
  pub kind: PrefixKind,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrefixKind {
  Modified,
  Copied,
}

impl PrefixKind {
  pub const fn label(self) -> &'static str {
    match self {
      Self::Modified => "modified",
      Self::Copied => "copied",
    }
  }
}

#[derive(Default, Deserialize, Serialize)]
pub struct State {
  pub prefixes: Vec<PrefixRecord>,
}

pub struct Config {
  pub root: PathBuf,
}

impl Config {
  pub fn discover() -> Result<Self> {
    if let Some(root) = env::var_os("BUNODE_HOME") {
      return Ok(Self { root: PathBuf::from(root) });
    }

    let home = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
      .ok_or_else(|| CliError::new("could not determine the user home directory"))?;

    Ok(Self { root: PathBuf::from(home).join(".bunode") })
  }

  pub fn state(&self) -> Result<State> {
    let path = self.state_path();

    match fs::read(&path) {
      Ok(content) => serde_json::from_slice(&content)
        .map_err(|error| CliError::new(format!("failed to read {}: {error}", path.display()))),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
      Err(error) => Err(CliError::new(format!("failed to read {}: {error}", path.display()))),
    }
  }

  pub fn save(&self, state: &State) -> Result<()> {
    fs::create_dir_all(&self.root)?;
    let destination = self.state_path();
    let temporary = self.root.join(format!(".prefixes.{}.tmp", process::id()));
    let content = serde_json::to_vec_pretty(state)
      .map_err(|error| CliError::new(format!("failed to serialize Bunode state: {error}")))?;

    fs::write(&temporary, content)?;
    replace_file(&temporary, &destination).map_err(|error| {
      let _ = fs::remove_file(&temporary);
      CliError::new(format!("failed to save {}: {error}", destination.display()))
    })
  }

  pub fn wrapper_template(&self) -> PathBuf {
    self.root.join(if cfg!(windows) { "node.exe" } else { "node" })
  }

  fn state_path(&self) -> PathBuf {
    self.root.join("prefixes.json")
  }
}

fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
  match fs::rename(source, destination) {
    Ok(()) => Ok(()),
    Err(_error) if cfg!(windows) && destination.exists() => {
      fs::remove_file(destination)?;
      fs::rename(source, destination)
    }
    Err(error) => Err(error),
  }
}
