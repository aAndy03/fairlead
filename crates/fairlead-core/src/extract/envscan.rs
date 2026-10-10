//! Env gate tokens (R-ENV-030 to R-ENV-036): textual only, never evaluated. TS/JS files yield
//! `import.meta.env.X` and `process.env.NODE_ENV`; Rust files `cfg(...)` (spaces removed) and
//! `feature:X`; a `Cargo.toml` yields `package:X`, `feature:X`, `default:X` and `dep:X` (normal
//! and target `[dependencies]` only). No AST, no `cargo metadata` (D-27, D-16).
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct EnvToken {
    pub token: String,
    pub line: u32,
}

const TS_EXT: [&str; 9] = [
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte", "astro",
];

fn line_no(i: usize) -> u32 {
    u32::try_from(i + 1).unwrap_or(u32::MAX)
}

fn ident_end(s: &str) -> usize {
    s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .unwrap_or(s.len())
}

fn ts_tokens(text: &str, out: &mut Vec<EnvToken>) {
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(at) = rest.find("import.meta.env.") {
            let tail = &rest[at + 16..];
            let n = ident_end(tail);
            if n > 0 {
                out.push(EnvToken {
                    token: format!("import.meta.env.{}", &tail[..n]),
                    line: line_no(i),
                });
            }
            rest = &tail[n..];
        }
        if line.contains("process.env.NODE_ENV") {
            out.push(EnvToken {
                token: "process.env.NODE_ENV".into(),
                line: line_no(i),
            });
        }
    }
}

/// `feature = "x"` inside a cfg expression, as `feature:x`.
fn features_in(expr: &str, line: u32, out: &mut Vec<EnvToken>) {
    let mut rest = expr;
    while let Some(at) = rest.find("feature=\"") {
        let tail = &rest[at + 9..];
        let Some(end) = tail.find('"') else { break };
        out.push(EnvToken {
            token: format!("feature:{}", &tail[..end]),
            line,
        });
        rest = &tail[end..];
    }
}

fn rust_tokens(text: &str, out: &mut Vec<EnvToken>) {
    for (i, line) in text.lines().enumerate() {
        let squashed: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        let mut rest = squashed.as_str();
        while let Some(at) = rest.find("cfg(") {
            let tail = &rest[at..];
            let mut depth = 0usize;
            let mut end = None;
            for (j, c) in tail.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            end = Some(j + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(end) = end else { break };
            let expr = &tail[..end];
            out.push(EnvToken {
                token: expr.to_string(),
                line: line_no(i),
            });
            features_in(expr, line_no(i), out);
            rest = &tail[end..];
        }
    }
}

fn quoted_words(s: &str) -> Vec<String> {
    s.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn key_of(line: &str) -> Option<&str> {
    let k = line.split(['=', '.']).next()?.trim().trim_matches('"');
    (!k.is_empty() && !k.starts_with('#') && !k.starts_with('[')).then_some(k)
}

fn is_dep_table(header: &str) -> bool {
    header == "dependencies"
        || (header.ends_with(".dependencies") && !header.starts_with("workspace"))
}

fn cargo_tokens(text: &str, out: &mut Vec<EnvToken>) {
    let mut header = String::new();
    // An open multi-line feature array: (line of its key, is it `default`).
    let mut open: Option<(u32, bool)> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let n = line_no(i);
        if let Some((l, is_default)) = open {
            if is_default {
                for w in quoted_words(line.split('#').next().unwrap_or("")) {
                    out.push(EnvToken {
                        token: format!("default:{w}"),
                        line: l,
                    });
                }
            }
            if line.contains(']') {
                open = None;
            }
            continue;
        }
        if let Some(h) = line.strip_prefix('[') {
            header = h.trim_end_matches(']').trim().to_string();
            if let Some(dep) = header.strip_prefix("dependencies.") {
                out.push(EnvToken {
                    token: format!("dep:{dep}"),
                    line: n,
                });
            }
            continue;
        }
        let Some(key) = key_of(line) else { continue };
        match header.as_str() {
            "package" if key == "name" => {
                if let Some(name) = quoted_words(line).first() {
                    out.push(EnvToken {
                        token: format!("package:{name}"),
                        line: n,
                    });
                }
            }
            "features" => {
                out.push(EnvToken {
                    token: format!("feature:{key}"),
                    line: n,
                });
                let value = line.split_once('=').map_or("", |(_, v)| v);
                if key == "default" {
                    for w in quoted_words(value.split('#').next().unwrap_or("")) {
                        out.push(EnvToken {
                            token: format!("default:{w}"),
                            line: n,
                        });
                    }
                }
                if value.contains('[') && !value.contains(']') {
                    open = Some((n, key == "default"));
                }
            }
            h if is_dep_table(h) => out.push(EnvToken {
                token: format!("dep:{key}"),
                line: n,
            }),
            _ => {}
        }
    }
}

/// Env tokens of one file; `ext` is lowercase without the dot.
pub fn scan_env_tokens(text: &str, ext: &str, file_name: &str) -> Vec<EnvToken> {
    let mut out = Vec::new();
    if file_name == "Cargo.toml" {
        cargo_tokens(text, &mut out);
    } else if ext == "rs" {
        rust_tokens(text, &mut out);
    } else if TS_EXT.contains(&ext) {
        ts_tokens(text, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(text: &str, ext: &str, name: &str) -> Vec<String> {
        scan_env_tokens(text, ext, name)
            .into_iter()
            .map(|t| format!("{}@{}", t.token, t.line))
            .collect()
    }

    #[test]
    fn r_env_033__ts_env_tokens() {
        let t = toks(
            "if (import.meta.env.DEV && import.meta.env.VITE_X) {}\nconst p = process.env.NODE_ENV;\n",
            "tsx",
            "a.tsx",
        );
        assert_eq!(
            t,
            [
                "import.meta.env.DEV@1",
                "import.meta.env.VITE_X@1",
                "process.env.NODE_ENV@2"
            ]
        );
    }

    #[test]
    fn r_env_032__rust_cfg_tokens_and_features() {
        let t = toks(
            "#[cfg(test)]\nmod t {}\n#[cfg(all(debug_assertions, feature = \"devtools\"))]\nfn f() {}\n",
            "rs",
            "lib.rs",
        );
        assert_eq!(
            t,
            [
                "cfg(test)@1",
                "cfg(all(debug_assertions,feature=\"devtools\"))@3",
                "feature:devtools@3"
            ]
        );
    }

    #[test]
    fn r_env_032__cargo_manifest_tokens() {
        let text = "[package]\nname = \"app\"\n\n[features]\ndefault = [\"std\"]\ndevtools = []\nextra = [\n]\n\n[dependencies]\nserde = \"1\"\nprobe.workspace = true\n\n[dev-dependencies]\nproptest = \"1\"\n\n[target.'cfg(unix)'.dependencies]\nlibc = \"0.2\"\n\n[dependencies.tracing]\nversion = \"0.1\"\n";
        let t = toks(text, "toml", "Cargo.toml");
        assert_eq!(
            t,
            [
                "package:app@2",
                "feature:default@5",
                "default:std@5",
                "feature:devtools@6",
                "feature:extra@7",
                "dep:serde@11",
                "dep:probe@12",
                "dep:libc@18",
                "dep:tracing@20"
            ]
        );
    }
}
