#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! The v1 API surface snapshot (D-53, R-ARCH-003; 0.3 P7-T7). The surface freezes at the end of
//! 0.4 (D-64); until then a change here updates the snapshot together with an errata entry.
//!
//! A missing snapshot file is written and the test passes (its creation is the review point);
//! a differing one fails and leaves `<file>.new` beside it to review and rename.
#[path = "testkit/mod.rs"]
mod testkit;

use std::path::{Path, PathBuf};

use testkit::RepoBuilder;

const VERBS: [&str; 10] = [
    "init", "check", "sync", "new", "context", "explain", "approve", "move", "seal", "upgrade",
];

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/v1_api")
}

fn snapshot(name: &str, now: &str) {
    let p = dir().join(name);
    // one snapshot for every platform: the Windows binary name is normalised
    let now = now
        .replace("\r\n", "\n")
        .replace("fairlead.exe", "fairlead");
    match std::fs::read_to_string(&p) {
        Err(_) => {
            std::fs::create_dir_all(dir()).unwrap();
            std::fs::write(&p, &now).unwrap();
        }
        Ok(was) => {
            if was.replace("\r\n", "\n") != now {
                std::fs::write(p.with_extension("new"), &now).unwrap();
                panic!(
                    "the v1 API surface changed: {name} (review {name}.new; keep it only with an errata entry, D-53)"
                );
            }
        }
    }
}

#[test]
fn d_53__every_verb_help_is_the_snapshot() {
    let repo = RepoBuilder::new().build();
    let top = repo.run(&["--help"]);
    snapshot("help.txt", &top.stdout);
    for v in VERBS {
        let h = repo.run(&[v, "--help"]);
        assert_eq!(h.code, 0, "{v}: {}", h.stderr);
        snapshot(&format!("help-{v}.txt"), &h.stdout);
    }
}

#[test]
fn d_53__the_code_table_is_the_snapshot() {
    let mut out = String::new();
    for s in fairlead_core::check::all_specs() {
        out.push_str(&format!(
            "{} {} {:?} {:?} {:?} floor={} waivable={}\n",
            s.code.0,
            s.mnemonic,
            s.default_severity,
            s.default_stage,
            s.milestone,
            s.floor,
            s.waivable
        ));
    }
    snapshot("codes.txt", &out);
}

/// The shape of a JSON value: one `path: type` line per key path, arrays as `[]`.
fn shape(v: &serde_json::Value, path: &str, out: &mut std::collections::BTreeSet<String>) {
    use serde_json::Value;
    let kind = match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    out.insert(format!("{path}: {kind}"));
    match v {
        Value::Array(a) => {
            for x in a {
                shape(x, &format!("{path}[]"), out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                shape(x, &format!("{path}.{k}"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn d_53__check_and_context_json_shapes_are_the_snapshot() {
    let repo = RepoBuilder::new()
        .file(
            "src/a.ts",
            "// fairlead: use cmp.missing\nexport const a = 1;\n",
        )
        .git_init(true)
        .build();
    assert_eq!(
        repo.run(&["init", "--yes", "--no-baseline", "--as-of", "2026-10-12"])
            .code,
        0
    );
    for (name, args) in [
        (
            "check.shape.txt",
            vec!["check", "--format", "json", "--as-of", "2026-10-12"],
        ),
        (
            "context.shape.txt",
            vec![
                "context",
                "src/a.ts",
                "--format",
                "json",
                "--as-of",
                "2026-10-12",
            ],
        ),
    ] {
        let run = repo.run(&args);
        let v: serde_json::Value = serde_json::from_str(&run.stdout)
            .unwrap_or_else(|e| panic!("{name}: {e}\n{}", run.stdout));
        let mut s = std::collections::BTreeSet::new();
        shape(&v, "$", &mut s);
        snapshot(
            name,
            &s.into_iter().map(|l| format!("{l}\n")).collect::<String>(),
        );
    }
}
