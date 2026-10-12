//! `fairlead-mcp`: the MCP stdio server. Reads one JSON-RPC message per line from stdin, writes
//! one reply per line to stdout. The `fairlead` binary it runs is `FAIRLEAD_BIN`, else the
//! `fairlead` beside this executable, else `fairlead` on the PATH.
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;

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

fn main() {
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
}
