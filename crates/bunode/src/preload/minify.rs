use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc_minifier::{CompressOptions, MangleOptions, Minifier, MinifierOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

pub fn minify(source: &str) -> Result<String, String> {
  let allocator = Allocator::default();
  let source_type = SourceType::mjs();
  let parsed = Parser::new(&allocator, source, source_type).parse();

  if !parsed.diagnostics.is_empty() {
    return Err(format!("{:?}", parsed.diagnostics));
  }

  let mut program = parsed.program;
  let minifier = Minifier::new(MinifierOptions {
    mangle: Some(MangleOptions::default()),
    compress: Some(CompressOptions::smallest()),
  });
  let result = minifier.minify(&allocator, &mut program);
  let generated = Codegen::new()
    .with_options(CodegenOptions {
      minify: true,
      comments: CommentOptions::disabled(),
      ..CodegenOptions::default()
    })
    .with_scoping(result.scoping)
    .build(&program);

  Ok(generated.code)
}
