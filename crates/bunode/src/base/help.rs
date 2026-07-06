//! Supported Bunode help generation.

use super::options::{HelpSection, OptionShape, OptionSpec, ValueMode};

const OPTION_COLUMN_WIDTH: usize = 31;
const NODE_SPECIAL_ROWS: &[(&str, &str)] = &[
  ("-", "script read from stdin (default if no file name is provided, interactive mode if a tty)"),
  ("--", "indicate the end of node options"),
];

pub fn print(shape: &OptionShape) {
  // Keep the custom help text tied to the same Clap schema future parsers can reuse.
  super::options::clap_command_for(shape).debug_assert();

  println!("Usage: node [options] [ script.js ] [arguments]");
  println!();
  println!("Options:");
  print_rows(NODE_SPECIAL_ROWS.iter().copied());
  print_option_section(shape, HelpSection::Node);
  println!();
  println!("Bun-specific options:");
  print_option_section(shape, HelpSection::Bun);
  println!();
  println!("Environment variables:");
  print_row("NODE_OPTIONS", "environment-allowed Node options are translated before CLI options");
}

fn print_option_section(shape: &OptionShape, section: HelpSection) {
  let rows = shape.specs().iter().filter_map(|spec| {
    let help = spec.help?;

    if help.section == section {
      return Some((format_option(spec), help.description));
    }

    None
  });

  print_rows(rows);
}

fn print_rows(rows: impl IntoIterator<Item = (impl AsRef<str>, &'static str)>) {
  for (option, description) in rows {
    print_row(option.as_ref(), description);
  }
}

fn print_row(option: &str, description: &str) {
  println!("  {option:<OPTION_COLUMN_WIDTH$} {description}");
}

fn format_option(spec: &OptionSpec) -> String {
  let long = format_long_option(spec);

  spec.short.map_or_else(|| long.clone(), |short| format!("-{short}, {long}"))
}

fn format_long_option(spec: &OptionSpec) -> String {
  let long = spec.long[0];
  let value_name = spec.help.and_then(|help| help.value_name).unwrap_or("...");

  match spec.value {
    ValueMode::None => long.to_owned(),
    ValueMode::Required => format!("{long}={value_name}"),
    ValueMode::OptionalEquals => format!("{long}[={value_name}]"),
  }
}
