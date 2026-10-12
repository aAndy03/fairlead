use std::io::Write;
use std::process::{Command, Stdio};

/// Runs the `ledger` binary with `input` on stdin; returns (exit code, stdout, stderr).
fn ledger(input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ledger"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the ledger binary runs");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn s4_the_command_line() {
    let (code, out, err) = ledger("open alice Alice\nopen bob Bob\ndeposit alice 5000\ntransfer alice bob 1200 rent\nwithdraw bob 200\nbalance alice\nbalance bob\n");
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("alice: 38.00 EUR"), "{out}");
    assert!(out.contains("bob: 10.00 EUR"), "{out}");
}

#[test]
fn s4_an_error_prints_its_code_and_exits_1() {
    let (code, out, err) = ledger("balance nobody\n");
    assert_eq!(code, 1);
    assert!(format!("{out}{err}").contains("E0"), "an error code: {out}{err}");
}
