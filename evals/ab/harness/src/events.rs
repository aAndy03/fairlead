//! An agent session's event log (`opencode run --format json`: one JSON event per line) reduced to
//! the cost measures C1 to C4 and C6 of PROTOCOL.md, plus the recent actions for the live view.
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Default, Debug, Clone)]
pub struct Summary {
    pub seconds: f64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub tokens_reasoning: u64,
    pub tokens_cache_read: u64,
    pub cost: f64,
    pub tool_calls: u64,
    pub turns: u64,
    pub fairlead_commands: u64,
    pub tools: std::collections::BTreeMap<String, u64>,
    pub final_text: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Action {
    pub t: f64,
    pub kind: String,
    pub text: String,
}

fn events(log: &str) -> impl Iterator<Item = Value> + '_ {
    log.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok())
}

fn tool_input(part: &Value) -> String {
    let input = &part["state"]["input"];
    for k in ["command", "filePath", "path", "pattern", "url"] {
        if let Some(s) = input[k].as_str() {
            return s.to_owned();
        }
    }
    input.to_string()
}

pub fn summarise(log: &str) -> Summary {
    let mut s = Summary::default();
    let (mut first, mut last) = (None::<f64>, 0.0_f64);
    let mut texts = Vec::new();
    for e in events(log) {
        if let Some(t) = e["timestamp"].as_f64() {
            first.get_or_insert(t);
            last = last.max(t);
        }
        let part = &e["part"];
        match e["type"].as_str() {
            Some("step_finish") => {
                s.turns += 1;
                s.cost += part["cost"].as_f64().unwrap_or(0.0);
                let tk = &part["tokens"];
                s.tokens_in += tk["input"].as_u64().unwrap_or(0);
                s.tokens_out += tk["output"].as_u64().unwrap_or(0);
                s.tokens_reasoning += tk["reasoning"].as_u64().unwrap_or(0);
                s.tokens_cache_read += tk["cache"]["read"].as_u64().unwrap_or(0);
            }
            Some("tool_use") => {
                s.tool_calls += 1;
                let tool = part["tool"].as_str().unwrap_or("?").to_owned();
                if tool_input(part).contains("fairlead ") || tool_input(part).starts_with("fairlead") {
                    s.fairlead_commands += 1;
                }
                *s.tools.entry(tool).or_default() += 1;
            }
            Some("text") => {
                if let Some(t) = part["text"].as_str() {
                    texts.push(t.to_owned());
                }
            }
            _ => {}
        }
    }
    s.seconds = first.map_or(0.0, |f| (last - f) / 1000.0);
    s.final_text = texts.last().cloned().unwrap_or_default();
    s
}

/// The last `n` actions, for the live view: tool calls with their input, and text replies.
pub fn recent(log: &str, n: usize) -> Vec<Action> {
    let mut out = Vec::new();
    for e in events(log) {
        let t = e["timestamp"].as_f64().unwrap_or(0.0);
        let part = &e["part"];
        match e["type"].as_str() {
            Some("tool_use") => out.push(Action {
                t,
                kind: part["tool"].as_str().unwrap_or("tool").to_owned(),
                text: tool_input(part).chars().take(240).collect(),
            }),
            Some("text") => out.push(Action {
                t,
                kind: "says".into(),
                text: part["text"].as_str().unwrap_or("").chars().take(400).collect(),
            }),
            _ => {}
        }
    }
    let skip = out.len().saturating_sub(n);
    out.into_iter().skip(skip).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_log() {
        let log = r#"{"type":"step_start","timestamp":1000,"part":{}}
{"type":"tool_use","timestamp":2000,"part":{"tool":"shell","state":{"input":{"command":"fairlead check --as-of 2026-10-12"}}}}
{"type":"step_finish","timestamp":3000,"part":{"cost":0.5,"tokens":{"input":10,"output":2,"reasoning":1,"cache":{"read":4}}}}
{"type":"text","timestamp":4000,"part":{"text":"done"}}"#;
        let s = summarise(log);
        assert_eq!((s.tool_calls, s.turns, s.fairlead_commands, s.tokens_in), (1, 1, 1, 10));
        assert_eq!(s.seconds, 3.0);
        assert_eq!(s.final_text, "done");
        assert_eq!(recent(log, 1)[0].kind, "says");
    }
}
