//! The MCP server of fairlead (R-ARCH-004, D-62): the `check`, `context` and `explain` verbs as
//! tools, and read-only resources, over the MCP stdio transport (newline-delimited JSON-RPC 2.0).
//!
//! The server runs the `fairlead` binary for every call, so a tool answers exactly what the CLI
//! answers; it writes nothing and offers no write verb. No network stack is involved: the
//! transport is the process's stdin and stdout.
#![forbid(unsafe_code)]

use serde_json::{Value, json};

/// The MCP protocol revision this server speaks.
pub const PROTOCOL: &str = "2025-06-18";

/// Runs `fairlead <args>` and returns (exit code, stdout, stderr).
pub trait Runner {
    fn run(&self, args: &[String]) -> (i32, String, String);
}

/// The tools: name, description, and the CLI arguments built from the call's arguments.
fn tools() -> Value {
    json!([
        {
            "name": "check",
            "description": "Run `fairlead check` and return the terse report: findings with code, path, line and fix hint. Read-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "paths": {"type": "array", "items": {"type": "string"}, "description": "Limit the report to these paths."},
                    "as_of": {"type": "string", "description": "Evaluate expiries as of this date (YYYY-MM-DD)."}
                }
            }
        },
        {
            "name": "context",
            "description": "Return the fairlead brief for a path, an ID or a task in plain words: the rules, items, drift and history that apply there, within a token budget. Run it before editing a path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": {"type": "string", "description": "A path, an ID (kind.name) or free text; empty for the session-start brief."},
                    "budget": {"type": "integer", "description": "Token budget; 0 is unlimited."},
                    "scope": {"type": "string", "description": "Pin a task query to this scope."}
                }
            }
        },
        {
            "name": "explain",
            "description": "Explain a diagnostic code (HN-...), an ID, a path or a topic (syntax, kinds, authority, ...).",
            "inputSchema": {
                "type": "object",
                "properties": {"target": {"type": "string"}},
                "required": ["target"]
            }
        }
    ])
}

/// The read-only resources and the CLI arguments that produce them.
const RESOURCES: [(&str, &str, &str, &[&str]); 3] = [
    (
        "fairlead://brief",
        "Session brief",
        "The session-start brief of the repository.",
        &["context", "--format", "terse"],
    ),
    (
        "fairlead://syntax",
        "Directive syntax",
        "The directive syntax card.",
        &["explain", "syntax"],
    ),
    (
        "fairlead://report",
        "Check report",
        "The terse check report.",
        &["check", "--format", "terse"],
    ),
];

fn str_arg(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::as_str).map(str::to_owned)
}

/// The CLI arguments of a tool call; `Err` is an invalid-params message.
pub fn tool_args(name: &str, a: &Value) -> Result<Vec<String>, String> {
    // a value that starts with `-` would be read as a flag: refused, so no call reaches a flag
    // the tool does not offer (`--yes`, `--root`, ...)
    let plain = |v: String| -> Result<String, String> {
        if v.starts_with('-') {
            Err(format!(
                "{v:?} starts with `-`; pass a path, an ID or plain words"
            ))
        } else {
            Ok(v)
        }
    };
    let mut args: Vec<String> = Vec::new();
    match name {
        "check" => {
            args.extend(["check".into(), "--format".into(), "terse".into()]);
            if let Some(d) = str_arg(a, "as_of") {
                args.extend(["--as-of".into(), plain(d)?]);
            }
            if let Some(ps) = a.get("paths").and_then(Value::as_array) {
                for p in ps.iter().filter_map(Value::as_str) {
                    args.push(plain(p.to_owned())?);
                }
            }
        }
        "context" => {
            args.extend(["context".into(), "--format".into(), "terse".into()]);
            if let Some(t) = str_arg(a, "target").filter(|t| !t.is_empty()) {
                args.push(plain(t)?);
            }
            if let Some(b) = a.get("budget").and_then(Value::as_u64) {
                args.extend(["--budget".into(), b.to_string()]);
            }
            if let Some(s) = str_arg(a, "scope") {
                args.extend(["--scope".into(), plain(s)?]);
            }
        }
        "explain" => {
            let t = str_arg(a, "target").ok_or("explain needs `target`")?;
            args.extend(["explain".into(), plain(t)?]);
        }
        other => return Err(format!("unknown tool {other:?}")),
    }
    Ok(args)
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn result(id: &Value, r: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": r})
}

/// Handles one JSON-RPC message; `None` for a notification (no reply).
pub fn handle(msg: &Value, runner: &dyn Runner) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => result(
            &id,
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {"tools": {"listChanged": false}, "resources": {"listChanged": false, "subscribe": false}},
                "serverInfo": {"name": "fairlead-mcp", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "fairlead declares, checks and briefs a repository's rules. Call `context` before editing a path and `check` after; explain any HN- code with `explain`. Every tool is read-only."
            }),
        ),
        "ping" => result(&id, json!({})),
        "tools/list" => result(&id, json!({"tools": tools()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match tool_args(name, &args) {
                Err(m) => error(&id, -32602, &m),
                Ok(a) => {
                    let (code, out, err) = runner.run(&a);
                    let mut text = out;
                    if !err.is_empty() {
                        text.push_str(&err);
                    }
                    // exit 1 is a check that found errors: a result, not a tool failure
                    result(
                        &id,
                        json!({"content": [{"type": "text", "text": text}], "isError": !(code == 0 || code == 1)}),
                    )
                }
            }
        }
        "resources/list" => result(
            &id,
            json!({"resources": RESOURCES.iter().map(|(uri, name, desc, _)| json!({"uri": uri, "name": name, "description": desc, "mimeType": "text/plain"})).collect::<Vec<_>>()}),
        ),
        "resources/read" => {
            let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
            match RESOURCES.iter().find(|(u, ..)| *u == uri) {
                None => error(&id, -32602, &format!("unknown resource {uri:?}")),
                Some((u, _, _, args)) => {
                    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
                    let (_, out, err) = runner.run(&args);
                    result(
                        &id,
                        json!({"contents": [{"uri": u, "mimeType": "text/plain", "text": format!("{out}{err}")}]}),
                    )
                }
            }
        }
        other => error(&id, -32601, &format!("method not found: {other}")),
    })
}

#[cfg(test)]
#[allow(non_snake_case, clippy::unwrap_used)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Fake(RefCell<Vec<Vec<String>>>);
    impl Runner for Fake {
        fn run(&self, args: &[String]) -> (i32, String, String) {
            self.0.borrow_mut().push(args.to_vec());
            (
                1,
                "HN-T001 error src/a.css:1:1 literal\n".into(),
                String::new(),
            )
        }
    }
    fn call(m: Value) -> (Option<Value>, Vec<Vec<String>>) {
        let f = Fake(RefCell::new(Vec::new()));
        let r = handle(&m, &f);
        (r, f.0.into_inner())
    }

    #[test]
    fn r_arch_004__initialize_and_list_tools() {
        let (r, _) = call(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));
        let r = r.unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL);
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let (t, _) = call(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
        let names: Vec<String> = t.unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(names, ["check", "context", "explain"]);
        assert!(
            call(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
                .0
                .is_none()
        );
    }

    #[test]
    fn r_arch_004__a_tool_call_runs_the_cli_read_only() {
        let (r, ran) = call(
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "context", "arguments": {"target": "add a progress bar", "budget": 0}}}),
        );
        assert_eq!(
            ran,
            vec![vec![
                "context",
                "--format",
                "terse",
                "add a progress bar",
                "--budget",
                "0"
            ]]
        );
        let r = r.unwrap();
        assert_eq!(
            r["result"]["isError"], false,
            "exit 1 is a report, not a failure"
        );
        assert!(
            r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("HN-T001")
        );
        let (bad, ran) = call(
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "seal", "arguments": {}}}),
        );
        assert!(ran.is_empty(), "no write verb is reachable");
        assert_eq!(bad.unwrap()["error"]["code"], -32602);
        assert!(tool_args("explain", &json!({"target": "--yes"})).is_err());
    }

    #[test]
    fn r_arch_004__resources_are_read_only_views() {
        let (r, _) = call(json!({"jsonrpc": "2.0", "id": 5, "method": "resources/list"}));
        assert_eq!(
            r.unwrap()["result"]["resources"].as_array().unwrap().len(),
            3
        );
        let (r, ran) = call(
            json!({"jsonrpc": "2.0", "id": 6, "method": "resources/read", "params": {"uri": "fairlead://syntax"}}),
        );
        assert_eq!(ran, vec![vec!["explain", "syntax"]]);
        assert!(r.unwrap()["result"]["contents"][0]["text"].is_string());
        let (r, _) = call(json!({"jsonrpc": "2.0", "id": 7, "method": "nope"}));
        assert_eq!(r.unwrap()["error"]["code"], -32601);
    }

    #[test]
    fn r_arch_004__the_fairlead_binary_carries_no_mcp_or_network_stack() {
        let cli = include_str!("../../fairlead/Cargo.toml");
        for banned in ["rmcp", "fairlead-mcp", "tokio", "reqwest", "hyper"] {
            assert!(!cli.contains(banned), "{banned} in the fairlead manifest");
        }
        let me = include_str!("../Cargo.toml");
        assert!(
            !me.contains("fairlead-core"),
            "the server runs the CLI; it links no core"
        );
    }
}
