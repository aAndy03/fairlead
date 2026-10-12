//! Parses every `.ts`/`.tsx` file under the given directories with the pinned grammar and
//! reports `pass` (no syntax errors) or `fail` with the files that did not parse cleanly.
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if matches!(p.extension().and_then(|x| x.to_str()), Some("ts" | "tsx")) {
            out.push(p);
        }
    }
}

fn main() -> std::process::ExitCode {
    let mut all = Vec::new();
    for d in std::env::args().skip(1) {
        files(Path::new(&d), &mut all);
    }
    all.sort();
    let (mut ok, mut bad) = (0usize, Vec::new());
    for f in &all {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let lang = if f.extension().and_then(|x| x.to_str()) == Some("tsx") {
            tree_sitter_typescript::LANGUAGE_TSX
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        };
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&lang.into()).is_err() {
            eprintln!("fail: the grammar does not load into this tree-sitter core");
            return std::process::ExitCode::from(1);
        }
        match parser.parse(&src, None) {
            Some(t) if !t.root_node().has_error() => ok += 1,
            _ => bad.push(f.display().to_string()),
        }
    }
    println!("tree-sitter gate: files={} clean={ok} errors={}", all.len(), bad.len());
    for b in &bad {
        println!("  error: {b}");
    }
    if bad.is_empty() && !all.is_empty() {
        println!("pass");
        std::process::ExitCode::SUCCESS
    } else {
        println!("fail");
        std::process::ExitCode::from(1)
    }
}
