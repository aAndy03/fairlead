//! Floor secret scan (R-AGT-022..024, R-AGT-032). Pure: text in, hits out.
//! A hit never carries the matched value, only its class, span and a fingerprint prefix.

use std::sync::OnceLock;

use fairlead_syntax::diag::Span;
use regex::Regex;

use crate::extract::SecretHit;
use crate::hash::{base32, hash_raw};

#[derive(Clone, Copy)]
enum Gate {
    Always,
    NeedsDigit,
    HighEntropy,
    Credential,
    UrlPassword,
}

struct Spec {
    class: &'static str,
    pattern: &'static str,
    /// Capture group holding the secret (0 = whole match).
    group: usize,
    gate: Gate,
}

/// Order matters: on equal span the earlier (more specific) class wins.
const SPECS: &[Spec] = &[
    Spec {
        class: "private-key",
        pattern: r"(?s)-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----.*?(?:-----END (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----|\z)",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "aws-access-key",
        pattern: r"\b(?:AKIA|ASIA|AGPA|AIDA|AROA|ANPA|ANVA)[0-9A-Z]{16}\b",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "github-token",
        pattern: r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{22,})",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "slack-token",
        pattern: r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "stripe-key",
        pattern: r"\b[rs]k_live_[0-9A-Za-z]{16,}\b",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "google-api-key",
        pattern: r"\bAIza[0-9A-Za-z_-]{35}",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "llm-api-key",
        pattern: r"\bsk-(?:ant-|proj-|svcacct-)?[A-Za-z0-9_-]{20,}",
        group: 0,
        gate: Gate::NeedsDigit,
    },
    Spec {
        class: "npm-token",
        pattern: r"\bnpm_[A-Za-z0-9]{36}\b",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "jwt",
        pattern: r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        group: 0,
        gate: Gate::Always,
    },
    Spec {
        class: "url-credentials",
        pattern: r#"\b[a-zA-Z][a-zA-Z0-9+.-]*://[^\s/:@"']+:([^\s/@"']+)@"#,
        group: 1,
        gate: Gate::UrlPassword,
    },
    Spec {
        class: "credential-assignment",
        pattern: r#"(?i)\b(?:api[_-]?key|secret|token|passwd|password|private[_-]?key|client[_-]?secret|access[_-]?key)["']?\s*[:=]\s*["']([A-Za-z0-9/+_=.-]{16,})["']"#,
        group: 1,
        gate: Gate::Credential,
    },
    Spec {
        class: "high-entropy",
        pattern: r"[A-Za-z0-9+/_-]{32,}={0,2}",
        group: 0,
        gate: Gate::HighEntropy,
    },
];

struct Compiled {
    class: &'static str,
    re: Regex,
    group: usize,
    gate: Gate,
}

fn compiled() -> &'static [Compiled] {
    static CELL: OnceLock<Vec<Compiled>> = OnceLock::new();
    CELL.get_or_init(|| {
        SPECS
            .iter()
            .filter_map(|s| {
                Regex::new(s.pattern).ok().map(|re| Compiled {
                    class: s.class,
                    re,
                    group: s.group,
                    gate: s.gate,
                })
            })
            .collect()
    })
}

/// Shannon entropy in bits per character of an ASCII string.
fn shannon(s: &str) -> f64 {
    let mut counts = [0u32; 128];
    let mut n = 0u32;
    for b in s.bytes() {
        if let Some(c) = counts.get_mut(usize::from(b)) {
            *c += 1;
            n += 1;
        }
    }
    if n == 0 {
        return 0.0;
    }
    let n = f64::from(n);
    counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = f64::from(*c) / n;
            -p * p.log2()
        })
        .sum()
}

fn is_placeholder(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    let same = v.chars().next().is_some_and(|f| v.chars().all(|c| c == f));
    same || ["example", "changeme", "placeholder", "your", "xxxxxxxx"]
        .iter()
        .any(|p| lower.contains(p))
}

fn high_entropy(v: &str) -> bool {
    v.len() >= 32
        && v.chars().any(|c| c.is_ascii_lowercase())
        && v.chars().any(|c| c.is_ascii_uppercase())
        && v.chars().any(|c| c.is_ascii_digit())
        && !v.chars().all(|c| c.is_ascii_hexdigit())
        && shannon(v) >= 4.2
}

fn passes(gate: Gate, v: &str) -> bool {
    match gate {
        Gate::Always => true,
        Gate::NeedsDigit => v.chars().any(|c| c.is_ascii_digit()),
        Gate::HighEntropy => high_entropy(v),
        Gate::Credential => v.len() >= 16 && shannon(v) >= 3.0 && !is_placeholder(v),
        Gate::UrlPassword => {
            !matches!(v.chars().next(), Some('$' | '{' | '<' | '%' | '*' | '['))
                && v.len() >= 8
                && shannon(v) >= 2.5
                && !is_placeholder(v)
        }
    }
}

struct Raw {
    start: usize,
    end: usize,
    class: &'static str,
}

/// All non-overlapping secret ranges, sorted by start. Overlaps merge; on equal start the
/// longer range, then the earlier (more specific) class, wins (the sort is stable).
fn find_all(text: &str) -> Vec<Raw> {
    let mut raws = Vec::new();
    for c in compiled() {
        for caps in c.re.captures_iter(text) {
            let Some(m) = caps.get(c.group) else { continue };
            if passes(c.gate, m.as_str()) {
                raws.push(Raw {
                    start: m.start(),
                    end: m.end(),
                    class: c.class,
                });
            }
        }
    }
    raws.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut merged: Vec<Raw> = Vec::new();
    for r in raws {
        match merged.last_mut() {
            Some(last) if r.start < last.end => {
                if r.end > last.end {
                    last.end = r.end;
                }
            }
            _ => merged.push(r),
        }
    }
    merged
}

fn line_col(text: &str, at: usize) -> (u32, u32) {
    let head = text.get(..at).unwrap_or("");
    let line = head.matches('\n').count() + 1;
    let col = head.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(col).unwrap_or(u32::MAX),
    )
}

/// Scan `text` for secret-like values. Offsets are byte offsets into `text`; line and column
/// are 1-based, columns count Unicode scalar values (C-16).
pub fn scan_secrets(text: &str) -> Vec<SecretHit> {
    find_all(text)
        .into_iter()
        .map(|r| {
            let value = text.get(r.start..r.end).unwrap_or("");
            let (line, col) = line_col(text, r.start);
            SecretHit {
                class: r.class.to_string(),
                span: Span {
                    start: u32::try_from(r.start).unwrap_or(u32::MAX),
                    end: u32::try_from(r.end).unwrap_or(u32::MAX),
                    line,
                    col,
                },
                fp_prefix: base32(&hash_raw(value.as_bytes()).0, 8),
            }
        })
        .collect()
}

/// True when [`scan_secrets`] would report anything (the write guard of P7 uses this).
pub fn has_secret(text: &str) -> bool {
    !find_all(text).is_empty()
}

/// `[secrets] allow` clears a hit by fingerprint (R-AGT-024). An allow entry matches when it is
/// the 8-character prefix the diagnostic printed, or a longer fingerprint that starts with it.
pub fn is_allowed(hit: &SecretHit, allow: &[String]) -> bool {
    allow
        .iter()
        .any(|a| a.len() >= hit.fp_prefix.len() && a.starts_with(&hit.fp_prefix))
}

/// Replace every secret range by `[redacted:<class>]` (R-AGT-023). Idempotent.
pub fn redact(text: &str) -> String {
    let hits = find_all(text);
    if hits.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for h in &hits {
        out.push_str(text.get(cursor..h.start).unwrap_or(""));
        out.push_str("[redacted:");
        out.push_str(h.class);
        out.push(']');
        cursor = h.end;
    }
    out.push_str(text.get(cursor..).unwrap_or(""));
    out
}

#[cfg(test)]
mod tests {
    #![allow(
        non_snake_case,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    const ALNUM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const ALNUM_US: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";
    const UPNUM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    const DIGITS: &str = "0123456789";
    const B64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const B64URL: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    /// Deterministic pseudo-random text from a fixed seed (no rand dependency).
    fn body(seed: u64, len: usize, alphabet: &str) -> String {
        let chars: Vec<char> = alphabet.chars().collect();
        let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                chars[((state >> 33) as usize) % chars.len()]
            })
            .collect()
    }

    struct Case {
        class: &'static str,
        text: String,
        /// A 12-character piece of the secret that must never survive `redact`.
        needle: String,
    }

    fn take(s: &str, skip: usize, n: usize) -> String {
        s.chars().skip(skip).take(n).collect()
    }

    /// 50 seeded secret-like values: 10 private keys, 10 AWS, 10 GitHub, 10 LLM keys, 10 mixed.
    fn corpus() -> Vec<Case> {
        let mut cases = Vec::new();
        let heads = [
            "RSA ",
            "EC ",
            "OPENSSH ",
            "DSA ",
            "",
            "ENCRYPTED ",
            "PGP ",
            "RSA ",
            "EC ",
            "",
        ];
        for (i, head) in heads.iter().enumerate() {
            let payload = body(100 + i as u64, 64, B64);
            let block = if *head == "PGP " { " BLOCK" } else { "" };
            // The last one has no END line: an unterminated block is still a private key.
            let end = if i == 9 {
                String::new()
            } else {
                format!("-----END {head}PRIVATE KEY{block}-----\n")
            };
            cases.push(Case {
                class: "private-key",
                needle: take(&payload, 0, 12),
                text: format!("-----BEGIN {head}PRIVATE KEY{block}-----\n{payload}\n{end}"),
            });
        }
        for i in 0..10u64 {
            let key = format!("AKIA{}", body(200 + i, 16, UPNUM));
            cases.push(Case {
                class: "aws-access-key",
                needle: take(&key, 4, 12),
                text: format!("aws_access_key_id = {key}\n"),
            });
        }
        let gh = [
            "ghp_",
            "gho_",
            "ghu_",
            "ghs_",
            "ghr_",
            "ghp_",
            "ghp_",
            "gho_",
            "github_pat_",
            "github_pat_",
        ];
        for (i, prefix) in gh.iter().enumerate() {
            let (len, alphabet) = if prefix.starts_with("github_pat_") {
                (40, ALNUM_US)
            } else {
                (36, ALNUM)
            };
            let payload = body(300 + i as u64, len, alphabet);
            cases.push(Case {
                class: "github-token",
                needle: take(&payload, 5, 12),
                text: format!("GITHUB_TOKEN={prefix}{payload}\n"),
            });
        }
        let llm = [
            "sk-",
            "sk-ant-api03-",
            "sk-proj-",
            "sk-",
            "sk-ant-api03-",
            "sk-proj-",
            "sk-",
            "sk-svcacct-",
            "sk-proj-",
            "sk-ant-api03-",
        ];
        for (i, prefix) in llm.iter().enumerate() {
            let payload = format!("7{}", body(400 + i as u64, 40, B64URL));
            cases.push(Case {
                class: "llm-api-key",
                needle: take(&payload, 1, 12),
                text: format!("OPENAI_API_KEY={prefix}{payload}\n"),
            });
        }
        for i in 0..2u64 {
            let tail = body(500 + i, 24, ALNUM);
            let tok = format!(
                "xoxb-{}-{}-{tail}",
                body(510 + i, 10, DIGITS),
                body(520 + i, 12, DIGITS)
            );
            cases.push(Case {
                class: "slack-token",
                needle: take(&tail, 6, 12),
                text: format!("SLACK_BOT={tok}\n"),
            });
        }
        for (i, prefix) in ["sk_live_", "rk_live_"].iter().enumerate() {
            let tail = body(600 + i as u64, 24, ALNUM);
            cases.push(Case {
                class: "stripe-key",
                needle: take(&tail, 4, 12),
                text: format!("STRIPE={prefix}{tail}\n"),
            });
        }
        for i in 0..2u64 {
            let tail = body(700 + i, 35, B64URL);
            cases.push(Case {
                class: "google-api-key",
                needle: take(&tail, 5, 12),
                text: format!("MAPS=AIza{tail}\n"),
            });
        }
        let npm = body(800, 36, ALNUM);
        cases.push(Case {
            class: "npm-token",
            needle: take(&npm, 5, 12),
            text: format!("//registry:_authToken=npm_{npm}\n"),
        });
        let (a, b, c) = (
            body(900, 20, B64URL),
            body(901, 20, B64URL),
            body(902, 20, B64URL),
        );
        cases.push(Case {
            class: "jwt",
            needle: take(&c, 3, 12),
            text: format!("Authorization: Bearer eyJ{a}.eyJ{b}.{c}\n"),
        });
        let pw = body(910, 16, ALNUM);
        cases.push(Case {
            class: "url-credentials",
            needle: take(&pw, 0, 12),
            text: format!("DATABASE_URL=postgres://app:{pw}@db.internal:5432/main\n"),
        });
        let v = body(920, 24, ALNUM);
        cases.push(Case {
            class: "credential-assignment",
            needle: take(&v, 4, 12),
            text: format!("client_secret = \"{v}\"\n"),
        });
        cases
    }

    #[test]
    fn r_agt_022__all_patterns_compile() {
        assert_eq!(
            compiled().len(),
            SPECS.len(),
            "a pattern failed to compile: the scan would fail open"
        );
    }

    #[test]
    fn r_agt_022__seeded_corpus_50_of_50_detected() {
        let cases = corpus();
        assert_eq!(cases.len(), 50);
        let missed: Vec<usize> = cases
            .iter()
            .enumerate()
            .filter(|(_, c)| scan_secrets(&c.text).is_empty())
            .map(|(i, _)| i)
            .collect();
        assert!(
            missed.is_empty(),
            "undetected seeded secrets (case indexes): {missed:?}"
        );
    }

    #[test]
    fn r_agt_022__long_high_entropy_string_is_flagged() {
        let token = format!("{}9aZ", body(1000, 41, B64));
        let hits = scan_secrets(&format!("blob: {token}\n"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].class, "high-entropy");
    }

    #[test]
    fn r_agt_022__benign_corpus_has_no_hits() {
        let benign = [
            "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678", // git sha (hex)
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", // blake3 hex
            "550e8400-e29b-41d4-a716-446655440000",     // uuid
            "k3m9q2x7a1bz4n8r5t6w",                     // body hash
            "apps/web/src/components/organisms/sidebar/sidebar.css", // long path
            "ThisIsAVeryLongPascalCaseIdentifierNameThatKeepsGoingAndGoingOn", // no digit
            "value=env:API_KEY",                        // env-var name only
            "api_key = env:STRIPE_API_KEY",
            "// fairlead: def tok.primary role=brand policy=sealed css=--primary",
            "task-force-management-committee-notes-2026-q3-roadmap-review", // contains 'sk-' inside a word
            "https://example.com/path?x=1&y=2",
            "git@github.com:org/repo.git",
            "token_budget = 4000",
            "password = \"changeme-please-change-it\"", // placeholder
            "-----BEGIN CERTIFICATE-----",
            "ghp_short",
            "postgres://postgres:${DB_PASSWORD}@localhost:5432/app",
        ];
        for text in benign {
            assert!(scan_secrets(text).is_empty(), "false positive on: {text}");
        }
    }

    #[test]
    fn r_agt_023__hit_never_carries_the_value() {
        for case in corpus() {
            let hits = scan_secrets(&case.text);
            let shown = format!("{hits:?}");
            assert!(
                !shown.contains(&case.needle),
                "debug output leaks the value for class {}",
                case.class
            );
            for h in &hits {
                assert_eq!(h.fp_prefix.chars().count(), 8);
                assert!(
                    h.fp_prefix
                        .chars()
                        .all(|c| "0123456789abcdefghjkmnpqrstvwxyz".contains(c))
                );
            }
        }
    }

    #[test]
    fn r_agt_024__allow_list_clears_by_fingerprint_prefix_only() {
        let key = format!("AKIA{}", body(1, 16, UPNUM));
        let hit = scan_secrets(&format!("id = {key}\n")).remove(0);
        assert!(is_allowed(&hit, &[hit.fp_prefix.clone()]));
        assert!(
            is_allowed(&hit, &[format!("{}abcd", hit.fp_prefix)]),
            "a 12-character fingerprint starting with the prefix also clears it"
        );
        assert!(!is_allowed(&hit, &["zzzzzzzz".to_string()]));
        assert!(!is_allowed(&hit, &[]));
        assert!(
            !is_allowed(&hit, &[hit.fp_prefix[..4].to_string()]),
            "a shorter string is not a fingerprint"
        );
    }

    #[test]
    fn r_agt_022__env_var_name_declaration_is_not_a_secret() {
        assert!(!has_secret("def cfg.stripe value=env:STRIPE_SECRET_KEY"));
    }

    #[test]
    fn r_agt_032__span_is_one_based_with_char_columns_and_byte_offsets() {
        let key = format!("AKIA{}", body(2, 16, UPNUM));
        let text = format!("first line\nkey {key}\n");
        let h = &scan_secrets(&text)[0];
        assert_eq!((h.span.line, h.span.col), (2, 5));
        assert_eq!(h.span.start as usize, "first line\nkey ".len());
        assert_eq!((h.span.end - h.span.start) as usize, key.len());

        let text = format!("\u{e9} key {key}");
        let h = &scan_secrets(&text)[0];
        assert_eq!(
            (h.span.line, h.span.col),
            (1, 7),
            "column counts scalar values"
        );
        assert_eq!(h.span.start, 7, "offset counts bytes");
    }

    #[test]
    fn r_agt_022__overlapping_patterns_yield_one_hit() {
        let tok = format!("ghp_{}", body(3, 36, ALNUM));
        assert_eq!(scan_secrets(&format!("x {tok} y")).len(), 1);
    }

    #[test]
    fn r_agt_023__redact_removes_every_seeded_value() {
        for (i, case) in corpus().iter().enumerate() {
            let out = redact(&case.text);
            assert!(
                !out.contains(&case.needle),
                "case {i} ({}) survived redaction",
                case.class
            );
            assert!(
                out.contains("[redacted:"),
                "case {i} ({}) left no marker",
                case.class
            );
        }
    }

    #[test]
    fn r_agt_023__redact_is_idempotent_and_keeps_surrounding_text() {
        for case in corpus() {
            let once = redact(&case.text);
            assert_eq!(redact(&once), once, "class {}", case.class);
        }
        let tok = format!("ghp_{}", body(4, 36, ALNUM));
        assert_eq!(redact(&format!("a {tok} b")), "a [redacted:github-token] b");
        assert_eq!(redact("nothing secret here"), "nothing secret here");
    }

    #[test]
    fn r_agt_023__only_the_url_password_is_replaced() {
        let out = redact("postgres://app:Zq8vKd3LmP0xTn5R@db:5432/main");
        assert_eq!(
            out,
            "postgres://app:[redacted:url-credentials]@db:5432/main"
        );
    }
}
