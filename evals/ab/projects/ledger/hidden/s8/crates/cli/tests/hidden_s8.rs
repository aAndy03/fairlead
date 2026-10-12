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
fn s8_export_prints_one_comma_separated_row_per_entry() {
    let (code, out, err) = ledger("open alice Alice\nopen bob Bob\ndeposit alice 5000\ntransfer alice bob 1200 rent\nexport alice\n");
    assert_eq!(code, 0, "{err}");
    let rows: Vec<&str> = out.lines().filter(|l| l.contains(',')).collect();
    // Two entries (the deposit and the transfer), perhaps after a header row.
    assert!(rows.len() == 2 || rows.len() == 3, "{out}");
    assert!(rows.iter().any(|r| r.contains("1200") || r.contains("12.00")), "{out}");
}
