//! `fairlead-mcp`: the MCP stdio server. Reads one JSON-RPC message per line from stdin, writes
//! one reply per line to stdout. The `fairlead` binary it runs is `FAIRLEAD_BIN`, else the
//! `fairlead` beside this executable, else `fairlead` on the PATH.
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use fairlead_mcp::{Runner, handle};

struct Cli(PathBuf);

impl Runner for Cli {
    fn run(&self, args: &[String]) -> (i32, String, String) {
        match Command::new(&self.0).args(args).output() {
            Ok(o) => (
                o.status.code().unwrap_or(5),
                String::from_utf8_lossy(&o.stdout).into_owned(),
                String::from_utf8_lossy(&o.stderr).into_owned(),
            ),
            Err(e) => (
                5,
                String::new(),
                format!("cannot run {}: {e}\n", self.0.display()),
            ),
        }
    }
}

fn binary() -> PathBuf {
    #[allow(clippy::disallowed_methods)]
    // reason: the server binary reads its one setting, like the CLI binary
    if let Some(p) = std::env::var_os("FAIRLEAD_BIN") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) {
        "fairlead.exe"
    } else {
        "fairlead"
    };
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("fairlead"))
}

const USAGE: &str = "fairlead-mcp: the MCP server of fairlead, over stdio.

An AI agent's client starts this program and talks to it on stdin and stdout; it stops when the
client closes. It listens on no port and uses no network. Tools: check, context, explain (read-only).

Register it once with the client, for example:
  claude mcp add fairlead -- fairlead-mcp

The fairlead binary it runs is FAIRLEAD_BIN, else the one beside this program, else fairlead on PATH.

Usage: fairlead-mcp [--help | --version]
";

fn main() -> ExitCode {
    // Run by hand, the server would wait silently on stdin; these two flags answer instead.
    match std::env::args().nth(1).as_deref() {
        Some("-h" | "--help") => {
            let _ = std::io::stdout().write_all(USAGE.as_bytes());
            return ExitCode::SUCCESS;
        }
        Some("-V" | "--version") => {
            let _ = writeln!(
                std::io::stdout(),
                "fairlead-mcp {}",
                env!("CARGO_PKG_VERSION")
            );
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            let _ = write!(
                std::io::stderr(),
                "fairlead-mcp: unknown argument {other}\n\n{USAGE}"
            );
            return ExitCode::from(2);
        }
        None => {}
    }
    let runner = Cli(binary());
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(msg) => handle(&msg, &runner),
            Err(e) => Some(
                serde_json::json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}),
            ),
        };
        if let Some(r) = reply
            && (writeln!(stdout, "{r}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
    ExitCode::SUCCESS
}
