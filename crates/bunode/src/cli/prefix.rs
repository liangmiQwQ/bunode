use std::{
  env, fs,
  io::{self, IsTerminal},
  path::{Path, PathBuf},
  process::{self, Command},
};

use super::{
  CliError, Result,
  args::{PatchOptions, RevertOptions},
  config::{Config, PrefixKind, PrefixRecord},
  download,
};

pub fn patch(options: &PatchOptions) -> Result<()> {
  let config = Config::discover()?;
  let mut state = config.state()?;
  let version = download::normalize_version(&options.version)?;
  let source_prefix = resolve_node_prefix(options.prefix.as_deref())?;
  let source_node = node_executable(&source_prefix);

  validate_original_node(&source_node)?;
  if state.prefixes.iter().any(|record| same_path(&record.path, &source_prefix)) {
    return Err(CliError::new(format!(
      "{} is already a managed Bunode prefix",
      source_prefix.display()
    )));
  }

  reject_installed_prefix(&source_prefix)?;
  let original_version = read_node_version(&source_node)?;
  let (mut target_prefix, kind) = select_target(&source_prefix, options.copy_to.as_deref())?;
  if state.prefixes.iter().any(|record| same_path(&record.path, &target_prefix)) {
    return Err(CliError::new(format!(
      "{} is already a managed Bunode prefix",
      target_prefix.display()
    )));
  }

  let template = config.wrapper_template();
  validate_executable_file(&template, "Bunode wrapper template")?;
  fs::create_dir_all(&config.root)?;
  let download_path =
    config.root.join(format!(".bun-download-{}{}", process::id(), executable_extension()));

  // 1. Download and validate Bun before changing the prefix.
  let result = (|| {
    download::download(&version, &download_path, &source_prefix)?;
    let downloaded_version = read_output(&download_path, &["--version"])?;
    if downloaded_version.trim_start_matches('v') != version {
      return Err(CliError::new(format!(
        "requested Bun {version}, but the package contains Bun {downloaded_version}"
      )));
    }
    let compatible_version = read_output(&download_path, &["-p", "process.version"])?;

    if compatible_version != original_version {
      confirm(
        &format!(
          "Bun {version} reports Node.js {compatible_version}, but the prefix uses {original_version}. Continue?"
        ),
        options.yes,
      )?;
    }
    if kind == PrefixKind::Modified && looks_version_managed(&source_prefix, &original_version) {
      confirm(
        &format!(
          "{} looks managed by a Node.js version manager. Modify it in place? (Use --copy <path> to keep it unchanged.)",
          source_prefix.display()
        ),
        options.yes,
      )?;
    } else {
      confirm(&format!("Install Bunode into {}?", target_prefix.display()), options.yes)?;
    }

    // 2. Copy the source prefix when requested, then install transactionally.
    if kind == PrefixKind::Copied {
      if let Err(error) = copy_tree(&source_prefix, &target_prefix) {
        let _ = fs::remove_dir_all(&target_prefix);
        return Err(error);
      }
      target_prefix = target_prefix.canonicalize().map_err(|error| {
        CliError::new(format!("failed to resolve {}: {error}", target_prefix.display()))
      })?;
    }
    let target_node = node_executable(&target_prefix);
    if let Err(error) = install_prefix(&target_prefix, &target_node, &template, &download_path) {
      if kind == PrefixKind::Copied {
        let _ = fs::remove_dir_all(&target_prefix);
      }
      return Err(error);
    }

    // 3. Record only completed installations; roll back if durable state fails.
    state.prefixes.push(PrefixRecord {
      path: target_prefix.clone(),
      original_version,
      bun_version: version.clone(),
      bunode_version: env!("CARGO_PKG_VERSION").to_owned(),
      kind,
    });
    if let Err(error) = config.save(&state) {
      let _ = revert_installed_prefix(&target_prefix, kind);
      return Err(error);
    }

    println!("Patched {} with Bun {version}.", target_prefix.display());
    Ok(())
  })();

  let _ = fs::remove_file(download_path);
  result
}

pub fn revert(options: &RevertOptions) -> Result<()> {
  let config = Config::discover()?;
  let mut state = config.state()?;
  let prefix = resolve_managed_prefix(options.prefix.as_deref())?;
  let index =
    state.prefixes.iter().position(|record| same_path(&record.path, &prefix)).ok_or_else(|| {
      CliError::new(format!("{} is not a recorded Bunode prefix", prefix.display()))
    })?;
  let record = state.prefixes[index].clone();
  let action = match record.kind {
    PrefixKind::Modified => "restore its original Node.js executable",
    PrefixKind::Copied => "delete the copied prefix",
  };

  confirm(&format!("Revert {} and {action}?", record.path.display()), options.yes)?;
  revert_installed_prefix(&record.path, record.kind)?;
  state.prefixes.remove(index);
  config.save(&state)?;
  println!("Reverted {}.", record.path.display());

  Ok(())
}

pub fn list() -> Result<()> {
  let state = Config::discover()?.state()?;

  if state.prefixes.is_empty() {
    println!("No managed Bunode prefixes.");
    return Ok(());
  }

  println!("NODE\tBUN\tBUNODE\tORIGINAL\tTYPE\tPREFIX");
  for record in state.prefixes {
    let node_version = read_output(&node_executable(&record.path), &["--version"])
      .unwrap_or_else(|_| "<broken>".to_owned());
    println!(
      "{}\t{}\t{}\t{}\t{}\t{}",
      node_version,
      record.bun_version,
      record.bunode_version,
      record.original_version,
      record.kind.label(),
      record.path.display()
    );
  }

  Ok(())
}

pub fn implode(yes: bool) -> Result<()> {
  let config = Config::discover()?;
  let mut state = config.state()?;

  if state.prefixes.is_empty() {
    println!("No managed Bunode prefixes.");
    return Ok(());
  }

  println!("Managed Bunode prefixes:");
  for record in &state.prefixes {
    println!("  {} ({})", record.path.display(), record.kind.label());
  }
  confirm("Revert every listed prefix?", yes)?;

  let mut failures = Vec::new();
  state.prefixes.retain(|record| match revert_installed_prefix(&record.path, record.kind) {
    Ok(()) => {
      println!("Reverted {}.", record.path.display());
      false
    }
    Err(error) => {
      failures.push(format!("{}: {error}", record.path.display()));
      true
    }
  });
  config.save(&state)?;

  if failures.is_empty() {
    Ok(())
  } else {
    Err(CliError::new(format!("some prefixes could not be reverted:\n{}", failures.join("\n"))))
  }
}

pub fn update(yes: bool) -> Result<()> {
  let config = Config::discover()?;
  let mut state = config.state()?;
  let current_version = env!("CARGO_PKG_VERSION");
  let indexes = state
    .prefixes
    .iter()
    .enumerate()
    .filter_map(|(index, record)| (record.bunode_version != current_version).then_some(index))
    .collect::<Vec<_>>();

  if indexes.is_empty() {
    println!("All managed prefixes already use Bunode {current_version}.");
    return Ok(());
  }

  validate_executable_file(&config.wrapper_template(), "Bunode wrapper template")?;
  confirm(
    &format!("Update {} managed prefix(es) to Bunode {current_version}?", indexes.len()),
    yes,
  )?;

  let mut failures = Vec::new();
  for index in indexes {
    let record = &mut state.prefixes[index];
    match update_wrapper(&record.path, &config.wrapper_template()) {
      Ok(()) => {
        current_version.clone_into(&mut record.bunode_version);
        println!("Updated {}.", record.path.display());
      }
      Err(error) => {
        eprintln!("warning: update failed for {}: {error}", record.path.display());
        failures.push(record.path.display().to_string());
      }
    }
  }
  config.save(&state)?;

  if failures.is_empty() {
    Ok(())
  } else {
    Err(CliError::new(format!("failed to update {} prefix(es)", failures.len())))
  }
}

fn select_target(source: &Path, copy_to: Option<&Path>) -> Result<(PathBuf, PrefixKind)> {
  let Some(copy_to) = copy_to else {
    return Ok((source.to_owned(), PrefixKind::Modified));
  };
  let target = absolute_path(copy_to)?;

  if target.exists() {
    return Err(CliError::new(format!("copy destination {} already exists", target.display())));
  }
  if target.starts_with(source) {
    return Err(CliError::new("copy destination cannot be inside the source prefix"));
  }

  Ok((target, PrefixKind::Copied))
}

fn install_prefix(prefix: &Path, node: &Path, template: &Path, bun_source: &Path) -> Result<()> {
  validate_original_node(node)?;
  let bun_directory = prefix.join("bun");
  let old_node = bun_directory.join(format!("node.old{}", executable_extension()));
  let bun = bun_directory.join(format!("bun{}", executable_extension()));

  if old_node.exists() {
    return Err(CliError::new(format!("{} already exists", old_node.display())));
  }
  fs::create_dir_all(&bun_directory)?;
  let staged_node = temporary_sibling(node, "new");
  let staged_bun = temporary_sibling(&bun, "new");
  copy_executable(template, &staged_node)?;
  copy_executable(bun_source, &staged_bun)?;

  // Keep the original executable recoverable until both replacements are ready.
  if let Err(error) = fs::rename(node, &old_node) {
    let _ = fs::remove_file(&staged_node);
    let _ = fs::remove_file(&staged_bun);
    return Err(error.into());
  }
  if let Err(error) = fs::rename(&staged_node, node) {
    let _ = fs::rename(&old_node, node);
    let _ = fs::remove_file(&staged_node);
    let _ = fs::remove_file(&staged_bun);
    return Err(error.into());
  }
  if let Err(error) = fs::rename(&staged_bun, &bun) {
    let _ = fs::remove_file(node);
    let _ = fs::rename(&old_node, node);
    let _ = fs::remove_file(&staged_bun);
    return Err(error.into());
  }

  Ok(())
}

fn revert_installed_prefix(prefix: &Path, kind: PrefixKind) -> Result<()> {
  if matches!(kind, PrefixKind::Copied) {
    if prefix.exists() {
      fs::remove_dir_all(prefix)?;
    }
    return Ok(());
  }

  let node = node_executable(prefix);
  let bun_directory = prefix.join("bun");
  let old_node = bun_directory.join(format!("node.old{}", executable_extension()));
  if !old_node.is_file() {
    return Err(CliError::new(format!(
      "original Node.js executable is missing at {}",
      old_node.display()
    )));
  }
  let staged_wrapper = temporary_sibling(&node, "revert");

  fs::rename(&node, &staged_wrapper)?;
  if let Err(error) = fs::rename(&old_node, &node) {
    let _ = fs::rename(&staged_wrapper, &node);
    return Err(error.into());
  }
  let _ = fs::remove_file(staged_wrapper);
  fs::remove_dir_all(bun_directory)?;

  Ok(())
}

fn update_wrapper(prefix: &Path, template: &Path) -> Result<()> {
  let node = node_executable(prefix);
  let staged = temporary_sibling(&node, "update");
  let backup = temporary_sibling(&node, "backup");
  copy_executable(template, &staged)?;
  fs::rename(&node, &backup)?;

  if let Err(error) = fs::rename(&staged, &node) {
    let _ = fs::rename(&backup, &node);
    let _ = fs::remove_file(&staged);
    return Err(error.into());
  }
  let validation = Command::new(&node).args(["-e", "if (1 == 1) { process.exit(0); }"]).status();
  if !validation.is_ok_and(|status| status.success()) {
    let _ = fs::remove_file(&node);
    let _ = fs::rename(&backup, &node);
    return Err(CliError::new("the updated wrapper failed its runtime check"));
  }
  fs::remove_file(backup)?;

  Ok(())
}

fn resolve_node_prefix(prefix: Option<&Path>) -> Result<PathBuf> {
  let path = match prefix {
    Some(path) => path.to_owned(),
    None => PathBuf::from(read_output(
      Path::new(if cfg!(windows) { "node.exe" } else { "node" }),
      &["-p", "process.execPath"],
    )?),
  };
  let path = absolute_path(&path)?;

  if path.is_file() {
    prefix_from_node(&path)
  } else {
    path
      .canonicalize()
      .map_err(|error| CliError::new(format!("invalid Node.js prefix {}: {error}", path.display())))
  }
}

fn resolve_managed_prefix(prefix: Option<&Path>) -> Result<PathBuf> {
  if let Some(path) = prefix {
    absolute_path(path)
  } else {
    let executable = read_output(
      Path::new(if cfg!(windows) { "node.exe" } else { "node" }),
      &["-p", "process.execPath"],
    )?;
    prefix_from_node(Path::new(&executable))
  }
}

fn prefix_from_node(node: &Path) -> Result<PathBuf> {
  let parent = node
    .parent()
    .ok_or_else(|| CliError::new(format!("invalid Node.js executable path {}", node.display())))?;

  if cfg!(windows) {
    Ok(parent.to_owned())
  } else if parent.file_name().is_some_and(|name| name == "bin") {
    parent
      .parent()
      .map(Path::to_owned)
      .ok_or_else(|| CliError::new(format!("invalid Node.js executable path {}", node.display())))
  } else {
    Err(CliError::new(format!(
      "expected the Node.js executable under a bin directory, got {}",
      node.display()
    )))
  }
}

fn node_executable(prefix: &Path) -> PathBuf {
  if cfg!(windows) { prefix.join("node.exe") } else { prefix.join("bin/node") }
}

fn validate_original_node(path: &Path) -> Result<()> {
  let metadata = fs::symlink_metadata(path).map_err(|error| {
    CliError::new(format!("invalid Node.js executable {}: {error}", path.display()))
  })?;

  if metadata.file_type().is_symlink() {
    return Err(CliError::new(format!("refusing symlinked Node.js executable {}", path.display())));
  }
  if !metadata.is_file() {
    return Err(CliError::new(format!("{} is not a file", path.display())));
  }
  read_node_version(path).map(|_| ())
}

fn reject_installed_prefix(prefix: &Path) -> Result<()> {
  let bun_directory = prefix.join("bun");
  let old_node = bun_directory.join(format!("node.old{}", executable_extension()));
  let bun = bun_directory.join(format!("bun{}", executable_extension()));

  if old_node.exists() || bun.exists() {
    Err(CliError::new(format!(
      "{} already contains a Bunode runtime; use `bunode revert` or repair it manually",
      prefix.display()
    )))
  } else {
    Ok(())
  }
}

fn validate_executable_file(path: &Path, description: &str) -> Result<()> {
  if path.is_file() {
    Ok(())
  } else {
    Err(CliError::new(format!("{description} is missing at {}", path.display())))
  }
}

fn read_node_version(node: &Path) -> Result<String> {
  let version = read_output(node, &["-p", "process.version"])?;
  if version.starts_with('v') {
    Ok(version)
  } else {
    Err(CliError::new(format!("{} did not report a Node.js version", node.display())))
  }
}

fn read_output(command: &Path, args: &[&str]) -> Result<String> {
  let output = Command::new(command)
    .args(args)
    .output()
    .map_err(|error| CliError::new(format!("failed to run {}: {error}", command.display())))?;

  if !output.status.success() {
    return Err(CliError::new(format!(
      "{} exited with {}: {}",
      command.display(),
      output.status,
      String::from_utf8_lossy(&output.stderr).trim()
    )));
  }

  String::from_utf8(output.stdout).map(|output| output.trim().to_owned()).map_err(|error| {
    CliError::new(format!("{} returned invalid UTF-8: {error}", command.display()))
  })
}

fn confirm(message: &str, yes: bool) -> Result<()> {
  if yes {
    return Ok(());
  }
  if !io::stdin().is_terminal() {
    return Err(CliError::new(format!("{message} Pass --yes to confirm.")));
  }

  if cliclack::confirm(message).initial_value(false).interact()? {
    Ok(())
  } else {
    Err(CliError::new("operation cancelled"))
  }
}

fn copy_executable(source: &Path, destination: &Path) -> Result<()> {
  fs::copy(source, destination)?;

  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(destination, fs::Permissions::from_mode(0o755))?;
  }

  Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
  fs::create_dir(destination)?;

  for entry in fs::read_dir(source)? {
    let entry = entry?;
    let source_path = entry.path();
    let destination_path = destination.join(entry.file_name());
    let file_type = entry.file_type()?;

    if file_type.is_dir() {
      copy_tree(&source_path, &destination_path)?;
    } else if file_type.is_symlink() {
      copy_symlink(&source_path, &destination_path)?;
    } else {
      fs::copy(&source_path, &destination_path)?;
    }
  }

  fs::set_permissions(destination, fs::metadata(source)?.permissions())?;
  Ok(())
}

#[cfg(unix)]
fn copy_symlink(source: &Path, destination: &Path) -> Result<()> {
  std::os::unix::fs::symlink(fs::read_link(source)?, destination)?;
  Ok(())
}

#[cfg(windows)]
fn copy_symlink(source: &Path, destination: &Path) -> Result<()> {
  let target = fs::read_link(source)?;
  let resolved = source.parent().unwrap_or_else(|| Path::new(".")).join(&target);

  if resolved.is_dir() {
    std::os::windows::fs::symlink_dir(target, destination)?;
  } else {
    std::os::windows::fs::symlink_file(target, destination)?;
  }
  Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
  use std::path::Component;

  let input = if path.is_absolute() { path.to_owned() } else { env::current_dir()?.join(path) };
  let mut absolute = PathBuf::new();
  for component in input.components() {
    match component {
      Component::CurDir => {}
      Component::ParentDir => {
        if absolute.parent().is_some() {
          let _ = absolute.pop();
        }
      }
      component => absolute.push(component.as_os_str()),
    }
  }

  if absolute.exists() {
    return absolute.canonicalize().map_err(|error| {
      CliError::new(format!("failed to resolve {}: {error}", absolute.display()))
    });
  }

  let mut ancestor = absolute.as_path();
  let mut suffix = Vec::new();
  while !ancestor.exists() {
    let name = ancestor.file_name().ok_or_else(|| {
      CliError::new(format!("could not resolve an existing parent of {}", absolute.display()))
    })?;
    suffix.push(name.to_owned());
    ancestor = ancestor.parent().ok_or_else(|| {
      CliError::new(format!("could not resolve an existing parent of {}", absolute.display()))
    })?;
  }
  let mut result = ancestor
    .canonicalize()
    .map_err(|error| CliError::new(format!("failed to resolve {}: {error}", ancestor.display())))?;
  result.extend(suffix.into_iter().rev());

  Ok(result)
}

fn temporary_sibling(path: &Path, purpose: &str) -> PathBuf {
  let name = path.file_name().unwrap_or_default().to_string_lossy();
  path.with_file_name(format!(".{name}.bunode.{purpose}.{}", process::id()))
}

fn looks_version_managed(prefix: &Path, original_version: &str) -> bool {
  let value = prefix.to_string_lossy().to_ascii_lowercase();
  ["/.nvm/", "/.fnm/", "/.volta/", "/.local/share/vp/", "/versions/node/", "\\fnm\\", "\\volta\\"]
    .iter()
    .any(|marker| value.contains(marker))
    || value.contains(&original_version.to_ascii_lowercase())
    || value.contains(original_version.trim_start_matches('v'))
}

fn same_path(left: &Path, right: &Path) -> bool {
  if cfg!(windows) {
    left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy())
  } else {
    left == right
  }
}

const fn executable_extension() -> &'static str {
  if cfg!(windows) { ".exe" } else { "" }
}
