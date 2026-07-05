use std::{env, error::Error, fs, path::PathBuf};

#[path = "src/preload/minify.rs"]
mod minify;

fn main() -> Result<(), Box<dyn Error>> {
  let source_path = PathBuf::from("src/preload/preload.js");
  let output_path = PathBuf::from(env::var("OUT_DIR")?).join("preload.min.js");
  let source = fs::read_to_string(&source_path)?;
  let minified = minify::minify(&source).map_err(std::io::Error::other)?;

  println!("cargo:rerun-if-changed={}", source_path.display());
  println!("cargo:rerun-if-changed=src/preload/minify.rs");
  fs::write(output_path, minified)?;

  Ok(())
}
