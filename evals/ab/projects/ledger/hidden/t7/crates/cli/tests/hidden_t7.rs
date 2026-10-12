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
fn t7_blank_lines_and_extra_spacing_are_tolerated() {
    let tidy = "open alice Alice\nopen bob Bob\ndeposit alice 5000\ntransfer alice bob 1200 rent\nbalance alice\nbalance bob\n";
    let messy = "\n  open   alice  Alice\n\n\topen bob    Bob  \n   \ndeposit  alice   5000\ntransfer alice   bob 1200   rent\n\nbalance    alice\n  balance bob\n\n";
    let (c1, o1, _) = ledger(tidy);
    let (c2, o2, e2) = ledger(messy);
    assert_eq!((c1, c2), (0, 0), "{e2}");
    assert_eq!(o1, o2);
}
