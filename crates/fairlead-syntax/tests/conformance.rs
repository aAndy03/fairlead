#![allow(non_snake_case)] // reason: test names start with the rule id (r_syn_016__case)
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
//! Golden conformance suite (R-SYN-046). Reads `fixtures/syntax/<host>/<case>.<ext>` + `<case>.expect`.
use fairlead_syntax::{Extent, ExtractConfig, Extraction, ScanStatus, extract};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_HOSTS: [&str; 11] = [
    "css",
    "html",
    "javascript",
    "json",
    "markdown",
    "rust",
    "shell",
    "sql",
    "toml",
    "typescript",
    "yaml",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/syntax")
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    v.sort();
    v
}

fn render(ex: &Extraction) -> String {
    let mut out = String::new();
    if ex.status != ScanStatus::Scanned {
        out.push_str(&format!("status {:?}\n", ex.status));
    }
    for d in &ex.directives {
        let mut names: Vec<String> = d.ids.iter().map(|i| i.to_string()).collect();
        names.extend(d.bare.iter().cloned());
        let names = if names.is_empty() {
            "-".to_owned()
        } else {
            names.join(",")
        };
        let extent = match d.extent {
            Extent::File => "file".to_owned(),
            Extent::Doc => "doc".to_owned(),
            Extent::Lines { first, last } => format!("L{first}-{last}"),
        };
        out.push_str(&format!("{} {} {}\n", serde_verb(d.verb), names, extent));
    }
    for d in &ex.diags {
        out.push_str(&format!("{} {}:{}\n", d.code.0, d.span.line, d.span.col));
    }
    out
}

fn serde_verb(v: fairlead_syntax::Verb) -> &'static str {
    v.as_str()
}

fn expected(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| format!("{l}\n"))
        .collect()
}

#[test]
fn r_syn_046__every_required_host_has_fixtures() {
    for h in REQUIRED_HOSTS {
        let dir = root().join(h);
        assert!(dir.is_dir(), "missing fixture directory {}", dir.display());
        let cases = sorted_entries(&dir)
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e != "expect"))
            .count();
        assert!(cases > 0, "no fixtures for host {h}");
    }
}

#[test]
fn r_syn_046__conformance_fixtures_match_expectations() {
    let cfg = ExtractConfig::default();
    let mut failures = Vec::new();
    let mut hosts = BTreeSet::new();
    let mut count = 0;
    for host_dir in sorted_entries(&root()).into_iter().filter(|p| p.is_dir()) {
        let host = host_dir.file_name().unwrap().to_string_lossy().into_owned();
        for case in sorted_entries(&host_dir) {
            let ext = case.extension().unwrap().to_string_lossy().into_owned();
            if ext == "expect" {
                continue;
            }
            let want = expected(&case.with_extension("expect"));
            let bytes = fs::read(&case).unwrap();
            let got = render(&extract(&bytes, &ext, &cfg));
            let crlf = String::from_utf8(bytes.clone())
                .unwrap()
                .replace('\n', "\r\n");
            let got_crlf = render(&extract(crlf.as_bytes(), &ext, &cfg));
            if got != want {
                failures.push(format!(
                    "{host}/{}: LF mismatch\n--- expected\n{want}--- actual\n{got}",
                    case.file_name().unwrap().to_string_lossy()
                ));
            }
            if got_crlf != want {
                failures.push(format!(
                    "{host}/{}: CRLF checkout differs\n--- actual\n{got_crlf}",
                    case.file_name().unwrap().to_string_lossy()
                ));
            }
            hosts.insert(host.clone());
            count += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {count} fixtures failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(count >= 20, "suite shrank: only {count} fixtures");
    assert!(REQUIRED_HOSTS.iter().all(|h| hosts.contains(*h)));
}
