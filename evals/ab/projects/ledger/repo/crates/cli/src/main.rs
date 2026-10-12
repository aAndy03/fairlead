//! `ledger`: runs ledger commands, one per line, from a file or standard input.
// fairlead: def ctr.cli path="crates/cli/**"
// fairlead: def un.cli layer=core.shell container=ctr.cli etype=user-action sum="the command line"
// fairlead: use un.service un.core
mod commands;

use std::io::{BufRead, Write};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input: Box<dyn BufRead> = match args.first().map(String::as_str) {
        None | Some("-") => Box::new(std::io::stdin().lock()),
        Some(path) => match std::fs::File::open(path) {
            Ok(f) => Box::new(std::io::BufReader::new(f)),
            Err(e) => {
                eprintln!("ledger: cannot open {path}: {e}");
                return std::process::ExitCode::from(2);
            }
        },
    };
    let mut ledger = ledger_service::Ledger::new();
    let mut out = std::io::stdout().lock();
    let mut failed = false;
    for (n, line) in input.lines().enumerate() {
        let Ok(line) = line else { break };
        match commands::run_line(&mut ledger, &line) {
            Ok(Some(text)) => {
                let _ = writeln!(out, "{text}");
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("line {}: {e}", n + 1);
                failed = true;
            }
        }
    }
    if failed {
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::SUCCESS
    }
}
