use std::{
  env,
  ffi::{OsStr, OsString},
  io,
  path::{Path, PathBuf},
  process::{Command as ProcessCommand, ExitCode, ExitStatus},
};

use clap::{Arg, ArgAction, Command as ClapCommand, builder::OsStringValueParser};

const HELP_DOCUMENT: &str = "Hello, World\n";

#[derive(Debug, PartialEq, Eq)]
struct Cli {
  help: bool,
  bunode_options: Vec<OsString>,
  script: Option<OsString>,
  script_arguments: Vec<OsString>,
}

#[derive(Debug, PartialEq, Eq)]
struct NodeArgSplit {
  bunode_options: Vec<OsString>,
  script: Option<OsString>,
  script_arguments: Vec<OsString>,
}

fn main() -> ExitCode {
  match parse_cli(env::args_os()) {
    Ok(cli) => run(cli),
    Err(error) => error.exit(),
  }
}

fn run(cli: Cli) -> ExitCode {
  let Cli { help, bunode_options: _, script, script_arguments } = cli;

  if help {
    print!("{HELP_DOCUMENT}");
    return ExitCode::SUCCESS;
  }

  let Some(script) = script else {
    return ExitCode::SUCCESS;
  };

  let bun_args = build_bun_script_args(script, script_arguments);

  match run_bun(&bun_args) {
    Ok(code) => code,
    Err(error) => {
      eprintln!("bunode: failed to run Bun: {error}");
      ExitCode::FAILURE
    }
  }
}

fn parse_cli<I, T>(args: I) -> Result<Cli, clap::Error>
where
  I: IntoIterator<Item = T>,
  T: Into<OsString> + Clone,
{
  let mut args = args.into_iter();
  let program = args.next().map_or_else(|| OsString::from("node"), Into::into);
  let split = split_node_args(args.map(Into::into));

  let mut bunode_args = Vec::with_capacity(split.bunode_options.len() + 1);
  bunode_args.push(program);
  bunode_args.extend(split.bunode_options);

  let matches = clap_command().try_get_matches_from(bunode_args)?;
  let bunode_options = matches
    .get_many::<OsString>("bunode-options")
    .map_or_else(Vec::new, |values| values.cloned().collect());

  Ok(Cli {
    help: matches.get_flag("help"),
    bunode_options,
    script: split.script,
    script_arguments: split.script_arguments,
  })
}

fn split_node_args(args: impl IntoIterator<Item = OsString>) -> NodeArgSplit {
  let mut bunode_options = Vec::new();
  let mut script = None;
  let mut script_arguments = Vec::new();
  let mut args = args.into_iter();

  while let Some(arg) = args.next() {
    if arg == OsStr::new("--") {
      script = args.next();
      script_arguments.extend(args);
      break;
    }

    if is_bunode_option(&arg) {
      bunode_options.push(arg);
      continue;
    }

    script = Some(arg);
    script_arguments.extend(args);
    break;
  }

  NodeArgSplit { bunode_options, script, script_arguments }
}

fn is_bunode_option(arg: &OsStr) -> bool {
  arg != OsStr::new("-") && arg.as_encoded_bytes().starts_with(b"-")
}

fn build_bun_script_args(script: OsString, script_arguments: Vec<OsString>) -> Vec<OsString> {
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

fn run_bun(args: &[OsString]) -> io::Result<ExitCode> {
  let status = ProcessCommand::new(resolve_bun_path()?).args(args).status()?;

  Ok(exit_code_from_status(status))
}

fn resolve_bun_path() -> io::Result<PathBuf> {
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

fn clap_command() -> ClapCommand {
  ClapCommand::new("node")
    .disable_help_flag(true)
    .disable_help_subcommand(true)
    .arg(Arg::new("help").short('h').long("help").action(ArgAction::SetTrue))
    .arg(
      Arg::new("bunode-options")
        .num_args(0..)
        .allow_hyphen_values(true)
        .trailing_var_arg(true)
        .value_parser(OsStringValueParser::new()),
    )
}

#[cfg(test)]
mod tests {
  use std::ffi::OsString;

  use super::{Cli, build_bun_script_args, parse_cli};

  #[test]
  fn parse_cli_should_keep_script_arguments_after_script_operand() -> Result<(), clap::Error> {
    let cli = parse_cli(["node", "--inspect", "script.js", "--help", "--flag"])?;

    assert_eq!(
      cli,
      Cli {
        help: false,
        bunode_options: vec![OsString::from("--inspect")],
        script: Some(OsString::from("script.js")),
        script_arguments: vec![OsString::from("--help"), OsString::from("--flag")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_cli_should_treat_double_dash_as_end_of_bunode_options() -> Result<(), clap::Error> {
    let cli = parse_cli(["node", "--", "--script.js", "--help"])?;

    assert_eq!(
      cli,
      Cli {
        help: false,
        bunode_options: Vec::new(),
        script: Some(OsString::from("--script.js")),
        script_arguments: vec![OsString::from("--help")],
      },
    );

    Ok(())
  }

  #[test]
  fn parse_cli_should_parse_help_before_script_operand() -> Result<(), clap::Error> {
    let cli = parse_cli(["node", "--help", "script.js"])?;

    assert_eq!(
      cli,
      Cli {
        help: true,
        bunode_options: Vec::new(),
        script: Some(OsString::from("script.js")),
        script_arguments: Vec::new(),
      },
    );

    Ok(())
  }

  #[test]
  fn build_bun_script_args_should_passthrough_script_arguments() {
    let args = build_bun_script_args(
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
