//! TOML frontmatter delimited by `+++` (R-SYN-036, R-SYN-037, D-05).
use crate::diag::Span;
use crate::normalize::{Normalized, slice};
use core::ops::Range;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Frontmatter {
    pub span: Span,
    pub table: toml::Table,
    pub directive_bodies: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ParsedFrontmatter {
    pub fm: Frontmatter,
    /// Normalized offset of each body (start of its string in the source; the `directives` key as fallback).
    pub offsets: Vec<usize>,
    /// Normalized offset just after the closing `+++`.
    pub end: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FmError {
    pub at: usize,
    pub len: usize,
    pub message: String,
}

#[derive(Clone, PartialEq, Debug)]
pub enum FmOutcome {
    Absent,
    Parsed(ParsedFrontmatter),
    Bad(FmError),
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches(['\r', ' ', '\t']) == "+++"
}

/// `(outer, inner)`: outer runs from the opening `+++` to the end of the closing `+++`, inner is the TOML text.
pub fn split_frontmatter(text: &str) -> Option<(Range<usize>, Range<usize>)> {
    let bom = if text.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let t = text.get(bom..)?;
    let mut lines = t.split_inclusive('\n');
    let first = lines.next()?;
    if !is_fence(first.trim_end_matches('\n')) {
        return None;
    }
    let inner_start = bom + first.len();
    let mut off = inner_start;
    for line in lines {
        if is_fence(line.trim_end_matches('\n')) {
            return Some((bom..off + 3, inner_start..off));
        }
        off += line.len();
    }
    None
}

fn bad(at: usize, len: usize, message: impl Into<String>) -> FmOutcome {
    FmOutcome::Bad(FmError {
        at,
        len: len.max(1),
        message: message.into(),
    })
}

/// Parses the frontmatter of a normalized document. Pure and total.
pub fn parse_frontmatter(n: &Normalized) -> FmOutcome {
    let text = n.text.as_str();
    if !text.lines().next().is_some_and(is_fence) {
        return FmOutcome::Absent;
    }
    let Some((outer, inner)) = split_frontmatter(text) else {
        return bad(0, 3, "unterminated frontmatter: no closing `+++` line");
    };
    let body = slice(text, inner.start, inner.end);
    let table = match toml::from_str::<toml::Table>(body) {
        Ok(t) => t,
        Err(e) => {
            let r = e.span().unwrap_or(0..0);
            let at = (inner.start + r.start).min(inner.end);
            return bad(
                at,
                r.len(),
                format!("frontmatter is not valid TOML: {}", e.message()),
            );
        }
    };
    let key_at = inner.start + body.find("directives").unwrap_or(0);
    let mut bodies = Vec::new();
    let mut offsets = Vec::new();
    match table.get("directives") {
        None => {}
        Some(toml::Value::Array(items)) => {
            let mut cursor = 0usize;
            for item in items {
                let toml::Value::String(raw) = item else {
                    return bad(key_at, 10, "`directives` must be an array of strings");
                };
                let at = match body.get(cursor..).and_then(|r| r.find(raw.as_str())) {
                    Some(p) => {
                        cursor += p + raw.len();
                        inner.start + cursor - raw.len()
                    }
                    None => key_at,
                };
                let joined = raw
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                bodies.push(joined);
                offsets.push(at);
            }
        }
        Some(_) => return bad(key_at, 10, "`directives` must be an array of strings"),
    }
    FmOutcome::Parsed(ParsedFrontmatter {
        fm: Frontmatter {
            span: n.span(outer.start, outer.end),
            table,
            directive_bodies: bodies,
        },
        offsets,
        end: outer.end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_syn_036__splitter_finds_both_fences() {
        let t = "+++\ntitle = \"x\"\n+++\n# H\n";
        let (outer, inner) = split_frontmatter(t).unwrap();
        assert_eq!(&t[inner.clone()], "title = \"x\"\n");
        assert_eq!(&t[outer], "+++\ntitle = \"x\"\n+++");
    }

    #[test]
    fn r_syn_036__only_a_leading_fence_line_counts() {
        for t in [
            "# H\n+++\na=1\n+++\n",
            " +++\na=1\n+++\n",
            "++++\na=1\n+++\n",
            "+++\na=1\n",
            "+++",
            "",
        ] {
            assert_eq!(split_frontmatter(t), None, "{t:?}");
        }
        assert!(
            split_frontmatter("+++  \na=1\n+++ \nbody").is_some(),
            "trailing blanks on fence lines are fine"
        );
        assert!(
            split_frontmatter("+++\n+++\nbody").is_some(),
            "empty frontmatter"
        );
    }

    fn parsed(src: &str) -> ParsedFrontmatter {
        match parse_frontmatter(&Normalized::new(src)) {
            FmOutcome::Parsed(p) => p,
            other => panic!("expected Parsed, got {other:?}"),
        }
    }

    #[test]
    fn r_syn_036__directives_array_of_strings() {
        let src = "+++\ntitle = \"Design tokens\"\ndirectives = [\n  'def tok.primary applies=color,background-color min-len=4',\n  'use rule.no-raw-hex',\n]\n+++\n\n# Colour\n";
        let p = parsed(src);
        assert_eq!(
            p.fm.directive_bodies,
            vec![
                "def tok.primary applies=color,background-color min-len=4",
                "use rule.no-raw-hex"
            ]
        );
        assert!(
            p.fm.table.contains_key("title"),
            "other keys are kept and ignored"
        );
        let n = Normalized::new(src);
        assert!(n.text[p.offsets[0]..].starts_with("def tok.primary"));
        assert!(n.text[p.offsets[1]..].starts_with("use rule.no-raw-hex"));
        assert_eq!(p.fm.span.line, 1);
        assert_eq!(
            &n.text[..p.end],
            "+++\ntitle = \"Design tokens\"\ndirectives = [\n  'def tok.primary applies=color,background-color min-len=4',\n  'use rule.no-raw-hex',\n]\n+++"
        );
    }

    #[test]
    fn r_syn_036__multi_line_bodies_join_with_one_space() {
        let p = parsed("+++\ndirectives = ['''\novr rule.x\n   why=\"legacy\"\n''']\n+++\n");
        assert_eq!(p.fm.directive_bodies, vec!["ovr rule.x why=\"legacy\""]);
    }

    #[test]
    fn r_syn_036__no_directives_key_is_fine_and_no_fence_is_absent() {
        assert!(
            parsed("+++\ntitle = \"x\"\n+++\n")
                .fm
                .directive_bodies
                .is_empty()
        );
        assert!(matches!(
            parse_frontmatter(&Normalized::new("# Plain\n")),
            FmOutcome::Absent
        ));
        assert!(matches!(
            parse_frontmatter(&Normalized::new("+++x\n")),
            FmOutcome::Absent
        ));
    }

    #[test]
    fn r_syn_036__crlf_files_split_after_normalisation() {
        let p = parsed("+++\r\ndirectives = ['use tt.b']\r\n+++\r\nbody\r\n");
        assert_eq!(p.fm.directive_bodies, vec!["use tt.b"]);
    }

    fn bad(src: &str) -> FmError {
        match parse_frontmatter(&Normalized::new(src)) {
            FmOutcome::Bad(e) => e,
            other => panic!("expected Bad, got {other:?}"),
        }
    }

    #[test]
    fn r_syn_037__invalid_toml_is_bad_with_a_position_inside_the_block() {
        let e = bad("+++\ntitle = \n+++\n");
        assert!(
            e.message.starts_with("frontmatter is not valid TOML"),
            "{}",
            e.message
        );
        assert!((4..=13).contains(&e.at), "{}", e.at);
    }

    #[test]
    fn r_syn_037__directives_must_be_an_array_of_strings() {
        assert!(
            bad("+++\ndirectives = \"use tt.b\"\n+++\n")
                .message
                .contains("array of strings")
        );
        assert!(
            bad("+++\ndirectives = [1, 2]\n+++\n")
                .message
                .contains("array of strings")
        );
        assert!(
            bad("+++\ndirectives = [['use tt.b']]\n+++\n")
                .message
                .contains("array of strings")
        );
    }

    #[test]
    fn r_syn_037__unterminated_frontmatter_is_bad() {
        let e = bad("+++\ntitle = \"x\"\n# no closing fence\n");
        assert!(e.message.contains("unterminated"));
        assert_eq!((e.at, e.len), (0, 3));
    }
}
