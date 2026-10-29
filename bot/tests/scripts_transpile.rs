//! Every script in the repo's scripts/ folder transpiles with the engine's loader.

use std::path::Path;

fn visit(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "node_modules") {
                visit(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "ts") {
            out.push(path);
        }
    }
}

#[test]
fn repo_scripts_transpile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts");
    let mut files = Vec::new();
    visit(&root, &mut files);
    assert!(!files.is_empty());
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        turu_bot::script::transpile::transpile(&source, &file).unwrap_or_else(|e| panic!("{e}"));
    }
}
