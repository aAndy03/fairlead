//! The backend process: one request per line on stdin, one reply per line on stdout.
use std::io::{BufRead, Write};

fn main() {
    let mut store = relay_backend::notes::Store::default();
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}", relay_backend::reply(&mut store, &line));
        let _ = out.flush();
    }
}
