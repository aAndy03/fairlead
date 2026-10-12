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
fn s9_statements_show_the_running_balance() {
    let (code, out, err) = ledger("open alice Alice\nopen bob Bob\ndeposit alice 5000\ntransfer alice bob 1200 rent\ndeposit alice 300\nstatement alice\n");
    assert_eq!(code, 0, "{err}");
    let lines: Vec<&str> = out.lines().filter(|l| !l.starts_with("opened") && !l.contains(": ")).collect();
    let has = |a: &str, b: &str| lines.iter().any(|l| l.contains(a) || l.contains(b));
    // Balances after each entry: 50.00, then 38.00, then 41.00 (in cents or as money).
    assert!(has("5000", "50.00") && has("3800", "38.00") && has("4100", "41.00"), "{out}");
}
