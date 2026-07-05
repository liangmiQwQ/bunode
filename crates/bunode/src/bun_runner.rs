use std::{
  env,
  ffi::{OsStr, OsString},
  io,
  path::{Path, PathBuf},
  process::{Command, ExitCode, ExitStatus},
};

pub fn run_script(script: OsString, script_arguments: Vec<OsString>) -> io::Result<ExitCode> {
  let args = build_script_args(script, script_arguments);
  let status = Command::new(resolve_path()?).args(args).status()?;

  Ok(exit_code_from_status(status))
}

fn build_script_args(script: OsString, script_arguments: Vec<OsString>) -> Vec<OsString> {
  let mut args = Vec::with_capacity(script_arguments.len() + 4);
  args.push(OsString::from("run"));
  args.push(OsString::from("--no-install"));
  args.push(OsString::from("--no-env-file"));
  args.push(normalize_script_path(script));
  args.extend(script_arguments);
  args
}

fn normalize_script_path(script: OsString) -> OsString {
  if script == OsStr::new("-") || Path::new(&script).components().count() != 1 {
    return script;
  }

  PathBuf::from(".").join(script).into_os_string()
}

fn resolve_path() -> io::Result<PathBuf> {
  let executable = env::current_exe()?;
  let executable_dir = executable.parent().ok_or_else(|| {
    io::Error::new(io::ErrorKind::NotFound, "failed to resolve Bunode executable directory")
  })?;

  #[cfg(windows)]
  {
    Ok(executable_dir.join("bun").join("bun.exe"))
  }

  #[cfg(not(windows))]
  {
    Ok(executable_dir.join("..").join("bun").join("bun"))
  }
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
  status.code().and_then(|code| u8::try_from(code).ok()).map_or(ExitCode::FAILURE, ExitCode::from)
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use super::build_script_args;

  #[test]
  fn build_script_args_should_passthrough_script_arguments() {
    let args = build_script_args(
      OsString::from("script.js"),
      vec![OsString::from("--help"), OsString::from("--flag")],
    );

    assert_eq!(
      args,
      vec![
        OsString::from("run"),
        OsString::from("--no-install"),
        OsString::from("--no-env-file"),
        OsString::from("./script.js"),
        OsString::from("--help"),
        OsString::from("--flag"),
      ],
    );
  }
}
