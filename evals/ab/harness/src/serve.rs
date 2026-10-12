//! The dashboard on localhost: the page, and JSON read from a results folder laid out as
//! `<results>/<run>/plan.json`, `sessions.jsonl` (one scored session per line) and
//! `live/<session>.json` (the agent's event log; `.json.part` while the session runs).
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::events;
use crate::Args;

const PAGE: &str = include_str!("../../dashboard/index.html");

pub fn run(args: &Args) -> Result<(), String> {
    let results = args.path("results")?;
    let port = args.get("port").unwrap_or("7878");
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).map_err(|e| format!("port {port}: {e}"))?;
    println!("dashboard: http://127.0.0.1:{port}/  (results: {})", results.display());
    for stream in listener.incoming().flatten() {
        if let Err(e) = handle(stream, &results) {
            eprintln!("ab serve: {e}");
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream, results: &Path) -> Result<(), String> {
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).map_err(|e| e.to_string())?;
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
    let (status, kind, body) = route(&path, results);
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body.as_bytes())).map_err(|e| e.to_string())
}

/// Only plain names: a path segment never climbs out of the results folder.
fn safe(seg: &str) -> Option<&str> {
    (!seg.is_empty() && seg.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && !seg.contains(".."))
        .then_some(seg)
}

fn route(path: &str, results: &Path) -> (&'static str, &'static str, String) {
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let ok = |v: Value| ("200 OK", "application/json", v.to_string());
    match parts.as_slice() {
        [""] | ["index.html"] => ("200 OK", "text/html; charset=utf-8", PAGE.to_owned()),
        ["api", "runs"] => ok(runs(results)),
        ["api", "run", name] => match safe(name) {
            Some(n) => ok(run_view(&results.join(n))),
            None => ("400 Bad Request", "text/plain", "bad run".into()),
        },
        ["api", "session", name, id] => match (safe(name), safe(id)) {
            (Some(n), Some(i)) => ok(session_view(&results.join(n), i)),
            _ => ("400 Bad Request", "text/plain", "bad session".into()),
        },
        _ => ("404 Not Found", "text/plain", "not found".into()),
    }
}

fn runs(results: &Path) -> Value {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(results) {
        for e in rd.flatten() {
            if e.path().join("plan.json").exists() {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out.reverse();
    json!(out)
}

fn live_file(run: &Path, id: &str) -> Option<(PathBuf, bool)> {
    let part = run.join("live").join(format!("{id}.json.part"));
    let done = run.join("live").join(format!("{id}.json"));
    if part.exists() {
        Some((part, true))
    } else if done.exists() {
        Some((done, false))
    } else {
        None
    }
}

fn run_view(run: &Path) -> Value {
    let plan: Value = std::fs::read_to_string(run.join("plan.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(json!([]));
    let sessions: Vec<Value> = std::fs::read_to_string(run.join("sessions.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let mut live = Vec::new();
    if let Ok(rd) = std::fs::read_dir(run.join("live")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".json.part") {
                let log = read_shared(&e.path());
                live.push(json!({
                    "id": id,
                    "summary": events::summarise(&log),
                    "actions": events::recent(&log, 40),
                }));
            }
        }
    }
    json!({ "plan": plan, "sessions": sessions, "live": live })
}

fn session_view(run: &Path, id: &str) -> Value {
    match live_file(run, id) {
        Some((p, running)) => {
            let log = read_shared(&p);
            json!({ "id": id, "running": running, "summary": events::summarise(&log), "actions": events::recent(&log, 400) })
        }
        None => json!({ "id": id, "missing": true }),
    }
}

/// The agent process still holds a running log open; read what is there.
fn read_shared(p: &Path) -> String {
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}
