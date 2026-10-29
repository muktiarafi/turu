//! TypeScript -> JavaScript: types are stripped with oxc; nothing is type checked here
//! (editors and `npx tsc --noEmit` in scripts/ do that).

use oxc::allocator::Allocator;
use oxc::codegen::Codegen;
use oxc::parser::Parser;
use oxc::semantic::SemanticBuilder;
use oxc::span::SourceType;
use oxc::transformer::{TransformOptions, Transformer};
use std::path::Path;

pub fn transpile(source: &str, path: &Path) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .unwrap_or_else(|_| SourceType::ts())
        .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if let Some(error) = parsed.diagnostics.first() {
        return Err(describe(source, path, error));
    }
    let mut program = parsed.program;
    let scoping = SemanticBuilder::new()
        .with_enum_eval(true)
        .build(&program)
        .semantic
        .into_scoping();
    let options = TransformOptions::default();
    let transformed =
        Transformer::new(&allocator, path, &options).build_with_scoping(scoping, &mut program);
    if let Some(error) = transformed.diagnostics.first() {
        return Err(describe(source, path, error));
    }
    Ok(Codegen::new().build(&program).code)
}

/// "file.ts:12:5: message".
fn describe(source: &str, path: &Path, error: &oxc::diagnostics::OxcDiagnostic) -> String {
    let offset = error.labels.first().map(|label| label.offset() as usize);
    match offset {
        Some(offset) => {
            let before = &source[..offset.min(source.len())];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            format!("{}:{line}:{column}: {error}", path.display())
        }
        None => format!("{}: {error}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_types() {
        let js = transpile(
            "interface A { x: number }\nexport const a: A = { x: 1 };\nenum E { One = 1 }\nexport const e = E.One;",
            Path::new("a.ts"),
        )
        .unwrap();
        assert!(js.contains("export const a = { x: 1 }"), "{js}");
        assert!(!js.contains("interface"), "{js}");
    }

    #[test]
    fn reports_line() {
        let err = transpile("const a = ;", Path::new("bad.ts")).unwrap_err();
        assert!(err.starts_with("bad.ts:1:"), "{err}");
    }
}
