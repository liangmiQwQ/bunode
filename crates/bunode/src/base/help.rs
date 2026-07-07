//! Supported Bunode help generation.

use textwrap::{Options, WordSplitter, wrap};

use super::options::{HelpRow, HelpSection, OptionShape};

const HELP_WIDTH: usize = 80;
const LEFT_WIDTH: usize = 34;
const ROW_INDENT: &str = "  ";
const NODE_SPECIAL_ROWS: &[HelpRow] = &[
  HelpRow {
    left: "-",
    description: "script read from stdin (default if no file name is provided, interactive mode if a tty)",
  },
  HelpRow { left: "--", description: "indicate the end of node options" },
];
const ENVIRONMENT_ROWS: &[HelpRow] = &[
  HelpRow {
    left: "FORCE_COLOR",
    description: "when set to 'true', 1, 2, 3, or an empty string causes NO_COLOR and NODE_DISABLE_COLORS to be ignored.",
  },
  HelpRow { left: "NO_COLOR", description: "Alias for NODE_DISABLE_COLORS" },
  HelpRow { left: "NODE_DISABLE_COLORS", description: "set to 1 to disable colors in the REPL" },
  HelpRow {
    left: "NODE_OPTIONS",
    description: "environment-allowed Node options are translated before CLI options",
  },
];

pub fn print(shape: &OptionShape) {
  println!("Bunode is a Node.js-compatible wrapper that runs programs on Bun.");
  println!();
  println!("Usage: node [options] [ script.js ] [arguments]");
  println!();
  println!("Options:");
  print_rows(NODE_SPECIAL_ROWS);
  print_option_section(shape, HelpSection::Node);
  println!();
  println!("Bun-specific options:");
  print_option_section(shape, HelpSection::Bun);
  println!();
  println!("Environment variables:");
  print_rows(ENVIRONMENT_ROWS);
}

fn print_option_section(shape: &OptionShape, section: HelpSection) {
  for spec in shape.specs() {
    let Some(help) = spec.help else {
      continue;
    };

    if help.section == section {
      print_row(help.row);
    }
  }
}

fn print_rows(rows: &[HelpRow]) {
  for row in rows {
    print_row(*row);
  }
}

fn print_row(row: HelpRow) {
  let description_width = HELP_WIDTH.saturating_sub(ROW_INDENT.len() + LEFT_WIDTH + 1).max(1);
  let wrap_options =
    Options::new(description_width).break_words(false).word_splitter(WordSplitter::NoHyphenation);
  let lines = wrap(row.description, wrap_options);

  if row.left.len() >= LEFT_WIDTH {
    println!("{ROW_INDENT}{}", row.left);

    for line in lines {
      println!("{ROW_INDENT}{:<LEFT_WIDTH$} {line}", "");
    }

    return;
  }

  let Some((first, rest)) = lines.split_first() else {
    println!("{ROW_INDENT}{}", row.left);
    return;
  };

  println!("{ROW_INDENT}{:<LEFT_WIDTH$} {first}", row.left);

  for line in rest {
    println!("{ROW_INDENT}{:<LEFT_WIDTH$} {line}", "");
  }
}
