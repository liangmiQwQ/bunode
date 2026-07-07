//! Parser state and final command selection.
//!
//! The state stores facts that cannot be reconstructed from final Bun argv, then
//! folds them into an `ExecutionPlan`.

use std::ffi::{OsStr, OsString};

use crate::error::{BunodeError, CliUsageError};

use super::actions::join_option_value;
use super::{ExecutionPlan, NodeCommand};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
  CommandLine,
  NodeOptions,
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum CommandMode {
  #[default]
  Normal,
  Help,
  Version,
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum PrintMode {
  #[default]
  Disabled,
  Enabled,
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum PrintOperandMode {
  #[default]
  Expression,
  Script,
}

#[derive(Default, PartialEq, Eq)]
pub(super) enum OperandBoundary {
  #[default]
  ScriptPosition,
  DoubleDash,
}

#[derive(Default)]
pub(super) struct ParseState {
  pub(super) command_mode: CommandMode,
  pub(super) inline_command: Option<NodeCommand>,
  pub(super) print_mode: PrintMode,
  pub(super) print_operand_mode: PrintOperandMode,
  pub(super) operand_boundary: OperandBoundary,
  pub(super) env_file_node_options: Option<OsString>,
  pub(super) read_env_file_node_options: bool,
  pub(super) operands: Vec<OsString>,
}

#[derive(Default)]
pub(super) struct InvocationSegments {
  pub(super) exec_argv: Vec<OsString>,
  pub(super) bun_options: Vec<OsString>,
  pub(super) bun_preloads: Vec<OsString>,
  pub(super) common_js_preloads: Vec<OsString>,
  pub(super) es_module_preloads: Vec<OsString>,
}

impl ParseState {
  pub(super) fn should_capture_print_expression(&self, source: Source) -> bool {
    source == Source::CommandLine
      && self.print_mode == PrintMode::Enabled
      && self.print_operand_mode == PrintOperandMode::Expression
  }

  pub(super) fn finish(
    self,
    argv0: OsString,
    mut builder: InvocationSegments,
  ) -> Result<(ExecutionPlan, Option<OsString>), BunodeError> {
    let (command, script_operand_count) = self.command(&mut builder)?;
    let script_arguments =
      self.operands.iter().skip(script_operand_count).cloned().collect::<Vec<_>>();
    let mut bun_options = builder.bun_options;
    let env_file_node_options = self.env_file_node_options;

    bun_options.extend(resolve_es_module_preloads(builder.es_module_preloads));
    bun_options.extend(builder.bun_preloads);

    Ok((
      ExecutionPlan {
        argv0,
        command,
        exec_argv: builder.exec_argv,
        bun_options,
        common_js_preloads: builder.common_js_preloads,
        script_arguments,
      },
      env_file_node_options,
    ))
  }

  fn command(&self, builder: &mut InvocationSegments) -> Result<(NodeCommand, usize), BunodeError> {
    if self.command_mode == CommandMode::Help {
      return Ok((NodeCommand::Help, 0));
    }

    if self.command_mode == CommandMode::Version {
      return Ok((NodeCommand::Version, 0));
    }

    if let Some(command) = &self.inline_command {
      let command = if self.print_mode == PrintMode::Enabled {
        match command {
          NodeCommand::Eval(code) | NodeCommand::Print(code) => NodeCommand::Print(code.clone()),
          command => command.clone(),
        }
      } else {
        command.clone()
      };

      return Ok((command, 0));
    }

    if self.print_mode == PrintMode::Enabled
      && self.operands.first().is_some_and(|operand| operand == OsStr::new("-"))
    {
      return Ok((NodeCommand::PrintStdin, 0));
    }

    if self.print_mode == PrintMode::Enabled
      && (self.operand_boundary != OperandBoundary::DoubleDash || self.operands.is_empty())
      && (self.print_operand_mode == PrintOperandMode::Expression || self.operands.is_empty())
    {
      let expression =
        self.operands.first().cloned().unwrap_or_else(|| OsString::from("undefined"));

      if let Some(expression) = self.operands.first() {
        builder.exec_argv.push(expression.clone());
      }

      let command = if self.operands.is_empty() {
        NodeCommand::PrintStdin
      } else {
        NodeCommand::Print(expression)
      };

      return Ok((command, usize::from(!self.operands.is_empty())));
    }

    let Some(script) = self.operands.first() else {
      return Ok((NodeCommand::Direct, 0));
    };

    if script == OsStr::new("inspect") {
      return Err(CliUsageError::UnsupportedNodeInspect.into());
    }

    // Node treats an empty script operand like stdin/REPL while preserving it in process.argv.
    if script.is_empty() {
      return Ok((NodeCommand::Direct, 0));
    }

    Ok((NodeCommand::Script(script.clone()), 1))
  }
}

fn resolve_es_module_preloads(values: Vec<OsString>) -> Vec<OsString> {
  let mut preloads = Vec::with_capacity(values.len());

  for value in values {
    preloads.push(join_option_value("--preload", value));
  }

  preloads
}
