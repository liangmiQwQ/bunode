use std::{error::Error, process::Command};

#[test]
fn help_should_print_fake_document_when_requested() -> Result<(), Box<dyn Error>> {
  let output = Command::new(env!("CARGO_BIN_EXE_bunode")).arg("--help").output()?;

  assert!(output.status.success());
  assert!(String::from_utf8(output.stdout)?.contains("Hello, World"));

  Ok(())
}
