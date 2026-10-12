#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]
//! The server binary run by hand: `--help` and `--version` answer instead of waiting on stdin.
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fairlead-mcp"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn r_arch_004__help_says_how_a_client_starts_it() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("claude mcp add fairlead -- fairlead-mcp"),
        "{text}"
    );
    assert!(text.contains("no port"), "{text}");
}

#[test]
fn r_arch_004__version_and_unknown_argument() {
    let v = run(&["--version"]);
    assert!(v.status.success());
    assert_eq!(
        String::from_utf8(v.stdout).unwrap(),
        format!("fairlead-mcp {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(run(&["--serve"]).status.code(), Some(2));
}
