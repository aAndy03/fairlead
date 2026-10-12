//! `ab`: the A/B/C evaluation harness (evals/ab/PROTOCOL.md). It prepares one arm's starting
//! repository, scores a finished session from outside (never calling fairlead or linking its
//! crates), summarises an agent's event log, and serves the dashboard.
//!
//! ```text
//! ab prepare --project <dir> --track maint|scratch --arm A|B|C --out <dir> [--fairlead <bin>] [--as-of <date>]
//! ab score   --project <dir> --dir <repo> --team <team> --task <id> [--earlier <ids>] [--pristine <rev>] [--events <file>]
//! ab events  <file>
//! ab serve   --results <dir> [--port 7878]
//! ab diff    --dir <repo>          (the blind reviewer's view of a session)
//! ab tasks   <file.toml>           (a task list as JSON, for the runner)
//! ```
mod blind;
mod events;
mod glob;
mod prepare;
mod score;
mod serve;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

/// `--key value` pairs after the subcommand; a bare word is kept under `""`.
pub struct Args(BTreeMap<String, String>);

impl Args {
    fn parse(words: &[String]) -> Result<Args, String> {
        let mut map = BTreeMap::new();
        let mut it = words.iter();
        while let Some(w) = it.next() {
            if let Some(key) = w.strip_prefix("--") {
                let value = it.next().ok_or_else(|| format!("--{key} needs a value"))?;
                map.insert(key.to_owned(), value.clone());
            } else {
                map.insert(String::new(), w.clone());
            }
        }
        Ok(Args(map))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn need(&self, key: &str) -> Result<&str, String> {
        self.get(key).ok_or_else(|| format!("missing --{key}"))
    }

    pub fn path(&self, key: &str) -> Result<PathBuf, String> {
        self.need(key).map(PathBuf::from)
    }
}

fn run() -> Result<(), String> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Some((cmd, rest)) = words.split_first() else {
        return Err("usage: ab prepare|score|events|serve ... (see src/main.rs)".into());
    };
    let args = Args::parse(rest)?;
    match cmd.as_str() {
        "prepare" => prepare::run(&args),
        "score" => score::run(&args),
        "events" => {
            let file = args.get("").ok_or("ab events <file>")?;
            let summary = events::summarise(&std::fs::read_to_string(file).map_err(|e| e.to_string())?);
            println!("{}", serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?);
            Ok(())
        }
        "serve" => serve::run(&args),
        "diff" => {
            print!("{}", blind::diff(&args.path("dir")?)?);
            Ok(())
        }
        "tasks" => {
            let file = args.get("").ok_or("ab tasks <file.toml>")?;
            let v: toml::Value = toml::from_str(&std::fs::read_to_string(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string(&v).map_err(|e| e.to_string())?);
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn main() -> ExitCode {
    // Arms A and B run with a copy of this binary named `fairlead` first on the PATH, so a
    // globally installed fairlead is not found, as on a machine without it.
    let stem = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()));
    if stem.as_deref() == Some("fairlead") {
        eprintln!("fairlead: command not found");
        return ExitCode::from(127);
    }
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ab: {e}");
            ExitCode::from(2)
        }
    }
}
