//! `lexopt` stream driver for CLI and `NODE_OPTIONS` tokens.
//!
//! This module owns token consumption rules that generic parsers cannot express,
//! such as `-p`, `--`, and script operand boundaries.

use std::ffi::{OsStr, OsString};

use lexopt::Arg;

use crate::error::{BunodeError, CliUsageError};

use super::super::options::{
  OptionShape, OptionSpec, ValueMode, find_long_option, find_short_option,
};
use super::NodeCommand;
use super::actions::{
  apply_option, ensure_source_allowed, option_requires_value, unsupported_option,
};
use super::state::{
  InvocationSegments, OperandBoundary, ParseState, PrintMode, PrintOperandMode, Source,
};

pub(super) fn parse_tokens(
  tokens: &[OsString],
  source: Source,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
  shape: &OptionShape,
) -> Result<(), BunodeError> {
  // `lexopt` owns token shape; Bunode owns what those tokens mean.
  let mut parser = lexopt::Parser::from_args(tokens.iter().cloned());
  parser.set_short_equals(false);

  loop {
    if consume_double_dash(&mut parser, source, state)? {
      break;
    }

    let Some(argument) =
      parser.next().map_err(|error| CliUsageError::ArgumentParse(error.to_string()))?
    else {
      break;
    };

    match argument {
      Arg::Long(name) => {
        let name = name.to_owned();
        parse_long_option(&name, &mut parser, source, state, builder, shape)?;
      }
      Arg::Short(short) => parse_short_option(short, &mut parser, source, state, builder, shape)?,
      Arg::Value(value) => {
        if parse_operand(value, &mut parser, source, state, builder)? {
          break;
        }
      }
    }
  }

  Ok(())
}

fn parse_long_option(
  name: &str,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
  shape: &OptionShape,
) -> Result<(), BunodeError> {
  let name = format!("--{name}");
  let Some(spec) = find_long_option(shape, &name) else {
    return Err(unsupported_option(&name));
  };
  let (value, original) = parse_long_value(spec, parser, &name)?;

  apply_option(&name, spec, value, source, original, state, builder)?;

  Ok(())
}

fn parse_short_option(
  short: char,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
  shape: &OptionShape,
) -> Result<(), BunodeError> {
  let attached_value = parser.optional_value();

  if short == 'p' && attached_value.as_ref().is_some_and(|value| value == OsStr::new("e")) {
    return parse_print_eval_shortcut(parser, source, state, builder, shape);
  }

  if let Some(attached_value) = attached_value {
    return Err(unsupported_option(join_short_option_value(short, &attached_value)));
  }

  let Some(spec) = find_short_option(shape, short) else {
    return Err(unsupported_option(format!("-{short}")));
  };
  let option_name = format!("-{short}");

  let (value, original) = if short == 'p' {
    (None, vec![OsString::from(&option_name)])
  } else {
    match spec.value {
      ValueMode::None | ValueMode::OptionalEquals => (None, vec![OsString::from(&option_name)]),
      ValueMode::Required => {
        let value = required_next_value(parser, &option_name)?;
        (Some(value.clone()), vec![OsString::from(&option_name), value])
      }
    }
  };

  apply_option(&option_name, spec, value, source, original, state, builder)?;

  Ok(())
}

fn parse_print_eval_shortcut(
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
  shape: &OptionShape,
) -> Result<(), BunodeError> {
  let print_spec = find_short_option(shape, 'p').ok_or_else(|| unsupported_option("-p"))?;
  let eval_spec = find_short_option(shape, 'e').ok_or_else(|| unsupported_option("-e"))?;
  ensure_source_allowed(print_spec, source)?;
  ensure_source_allowed(eval_spec, source)?;
  let value = required_next_value(parser, "-e")?;
  let original_value = value.clone();

  state.print_mode = PrintMode::Enabled;
  state.print_operand_mode = PrintOperandMode::Script;
  state.inline_command = Some(NodeCommand::Eval(value));

  if source == Source::CommandLine {
    builder.exec_argv.push(OsString::from("-pe"));
    builder.exec_argv.push(original_value);
  }

  Ok(())
}

fn consume_double_dash(
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
) -> Result<bool, BunodeError> {
  let Some(mut raw_args) = parser.try_raw_args() else {
    return Ok(false);
  };

  if raw_args.peek().is_none_or(|argument| argument != OsStr::new("--")) {
    return Ok(false);
  }

  if source == Source::NodeOptions {
    return Err(CliUsageError::NodeOptionsDisallowed("--".to_string()).into());
  }

  let _ = raw_args.next();
  state.operand_boundary = OperandBoundary::DoubleDash;
  state.operands.extend(raw_args);

  Ok(true)
}

fn parse_operand(
  value: OsString,
  parser: &mut lexopt::Parser,
  source: Source,
  state: &mut ParseState,
  builder: &mut InvocationSegments,
) -> Result<bool, BunodeError> {
  if source == Source::NodeOptions {
    return Err(CliUsageError::NodeOptionsDisallowed(value.to_string_lossy().into_owned()).into());
  }

  if value == OsStr::new("-") && state.should_capture_print_expression(source) {
    state.operands.push(value);
    state
      .operands
      .extend(parser.raw_args().map_err(|error| CliUsageError::ArgumentParse(error.to_string()))?);
    return Ok(true);
  }

  if state.should_capture_print_expression(source) {
    state.inline_command = Some(NodeCommand::Print(value.clone()));
    state.print_operand_mode = PrintOperandMode::Script;
    builder.exec_argv.push(value);
    return Ok(false);
  }

  state.operands.push(value);
  state
    .operands
    .extend(parser.raw_args().map_err(|error| CliUsageError::ArgumentParse(error.to_string()))?);

  Ok(true)
}

fn parse_long_value(
  spec: &OptionSpec,
  parser: &mut lexopt::Parser,
  option: &str,
) -> Result<(Option<OsString>, Vec<OsString>), BunodeError> {
  let inline_value = parser.optional_value();
  let mut original = vec![format_long_original(option, inline_value.as_deref())];

  if option == "--print" {
    return Ok((inline_value, original));
  }

  let value = match spec.value {
    ValueMode::None => {
      if inline_value.is_some() {
        return Err(CliUsageError::OptionDoesNotTakeValue(option.to_string()).into());
      }

      None
    }
    ValueMode::Required => {
      let value = if let Some(value) = inline_value {
        if value.is_empty() {
          return Err(option_requires_value(option));
        }

        value
      } else {
        let value = required_next_value(parser, option)?;
        original.push(value.clone());
        value
      };

      Some(value)
    }
    ValueMode::OptionalEquals => match inline_value {
      Some(value) => {
        if value.is_empty() {
          return Err(option_requires_value(option));
        }

        Some(value)
      }
      None => None,
    },
  };

  Ok((value, original))
}

fn required_next_value(parser: &mut lexopt::Parser, option: &str) -> Result<OsString, BunodeError> {
  let mut raw_args =
    parser.raw_args().map_err(|error| CliUsageError::ArgumentParse(error.to_string()))?;
  let Some(value) = raw_args.peek() else {
    return Err(option_requires_value(option));
  };

  if starts_with_dash(value) {
    return Err(option_requires_value(option));
  }

  raw_args.next().ok_or_else(|| option_requires_value(option))
}

fn format_long_original(option: &str, value: Option<&OsStr>) -> OsString {
  let mut original = OsString::from(option);

  if let Some(value) = value {
    original.push("=");
    original.push(value);
  }

  original
}

fn join_short_option_value(short: char, value: &OsStr) -> OsString {
  let mut option = OsString::from(format!("-{short}"));

  option.push(value);
  option
}

fn starts_with_dash(value: &OsStr) -> bool {
  value.to_string_lossy().starts_with('-')
}
