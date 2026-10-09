//! Terse `ID k=v` line writer and reader (R-FILE-003, R-ARCH-091, D-23, D-32).
//! Shared by `generate`, `render` and `context`. Pure; callers sanitise text first.

use core::fmt::Write as _;

/// One value of a `k=v` pair.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TerseValue {
    Word(String),
    Quoted(String),
}

impl TerseValue {
    /// `Word` when the text is a non-empty run of ASCII graphic characters without `"` and `\`;
    /// otherwise `Quoted` (R-CTX-031: ASCII except inside quoted text).
    pub fn auto(s: &str) -> TerseValue {
        if is_word(s) {
            TerseValue::Word(s.to_string())
        } else {
            TerseValue::Quoted(s.to_string())
        }
    }

    /// The unescaped text.
    pub fn text(&self) -> &str {
        match self {
            TerseValue::Word(s) | TerseValue::Quoted(s) => s,
        }
    }

    fn render(&self) -> String {
        match self {
            TerseValue::Word(s) => s.clone(),
            TerseValue::Quoted(s) => quote(s),
        }
    }
}

fn is_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_graphic() && c != '"' && c != '\\')
}

/// Always-quoted form with escapes: `\\`, `\"`, `\n`, `\r`, `\t`, other controls as `\u{hex}`.
pub fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `ID k=v k=v ...`; `st=active` is omitted by the caller (R-CTX-030).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TerseLine {
    pub id: String,
    pub pairs: Vec<(String, TerseValue)>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LineError {
    Empty,
    BadKey { at: usize },
    UnterminatedQuote,
    BadEscape { at: usize },
    TrailingAfterQuote { at: usize },
}

impl TerseLine {
    pub fn new(id: &str) -> TerseLine {
        TerseLine { id: id.to_string(), pairs: Vec::new() }
    }

    pub fn with(mut self, key: &str, value: &str) -> TerseLine {
        self.pairs.push((key.to_string(), TerseValue::auto(value)));
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.text())
    }

    pub fn render(&self) -> String {
        let mut out = self.id.clone();
        for (k, v) in &self.pairs {
            out.push(' ');
            out.push_str(k);
            out.push('=');
            out.push_str(&v.render());
        }
        out
    }

    /// Inverse of [`TerseLine::render`]. Total: never panics on any input.
    pub fn parse(line: &str) -> Result<TerseLine, LineError> {
        let chars: Vec<char> = line.chars().collect();
        let id: String = chars.iter().take_while(|c| **c != ' ').collect();
        if id.is_empty() {
            return Err(LineError::Empty);
        }
        let mut i = id.chars().count();
        let mut pairs = Vec::new();
        loop {
            while chars.get(i) == Some(&' ') {
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            let mut key = String::new();
            loop {
                match chars.get(i) {
                    Some('=') if !key.is_empty() => break,
                    Some(c) if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') => {
                        key.push(*c);
                        i += 1;
                    }
                    _ => return Err(LineError::BadKey { at: i }),
                }
            }
            i += 1; // the '='
            if chars.get(i) == Some(&'"') {
                i += 1;
                let mut val = String::new();
                loop {
                    match chars.get(i) {
                        None => return Err(LineError::UnterminatedQuote),
                        Some('"') => {
                            i += 1;
                            break;
                        }
                        Some('\\') => {
                            i += 1;
                            match chars.get(i) {
                                Some('\\') => val.push('\\'),
                                Some('"') => val.push('"'),
                                Some('n') => val.push('\n'),
                                Some('r') => val.push('\r'),
                                Some('t') => val.push('\t'),
                                Some('u') if chars.get(i + 1) == Some(&'{') => {
                                    let mut j = i + 2;
                                    let mut hex = String::new();
                                    while let Some(c) = chars.get(j) {
                                        if *c == '}' {
                                            break;
                                        }
                                        hex.push(*c);
                                        j += 1;
                                    }
                                    if chars.get(j) != Some(&'}') {
                                        return Err(LineError::BadEscape { at: i });
                                    }
                                    let ch = u32::from_str_radix(&hex, 16)
                                        .ok()
                                        .and_then(char::from_u32)
                                        .ok_or(LineError::BadEscape { at: i })?;
                                    val.push(ch);
                                    i = j; // at '}', consumed by the increment below
                                }
                                _ => return Err(LineError::BadEscape { at: i }),
                            }
                            i += 1;
                        }
                        Some(c) => {
                            val.push(*c);
                            i += 1;
                        }
                    }
                }
                if let Some(c) = chars.get(i) {
                    if *c != ' ' {
                        return Err(LineError::TrailingAfterQuote { at: i });
                    }
                }
                pairs.push((key, TerseValue::Quoted(val)));
            } else {
                let mut val = String::new();
                while let Some(c) = chars.get(i) {
                    if *c == ' ' {
                        break;
                    }
                    val.push(*c);
                    i += 1;
                }
                pairs.push((key, TerseValue::Word(val)));
            }
        }
        Ok(TerseLine { id, pairs })
    }
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
    use super::*;

    #[test]
    fn r_file_003__word_values_are_bare() {
        let line = TerseLine::new("tok.radius-base").with("css", "--radius").with("light", "0.25rem");
        assert_eq!(line.render(), "tok.radius-base css=--radius light=0.25rem");
    }

    #[test]
    fn r_file_012__space_quote_newline_backslash_and_non_ascii_force_quoted_escaped_values() {
        let line = TerseLine::new("rule.x")
            .with("why", "say \"hi\"\nthen\\go")
            .with("sum", "caf\u{e9} au lait")
            .with("empty", "");
        assert_eq!(
            line.render(),
            "rule.x why=\"say \\\"hi\\\"\\nthen\\\\go\" sum=\"caf\u{e9} au lait\" empty=\"\""
        );
    }

    #[test]
    fn r_file_012__control_characters_are_escaped_as_unicode() {
        assert_eq!(quote("a\u{1b}b"), "\"a\\u{1b}b\"");
        assert_eq!(quote("tab\there"), "\"tab\\there\"");
    }

    #[test]
    fn r_arch_091__a_value_cannot_start_a_second_line() {
        let line = TerseLine::new("rule.x").with("why", "a\nHN-R001 error evil");
        let out = line.render();
        assert!(!out.contains('\n'));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn r_file_003__parse_inverts_render() {
        let samples = [
            TerseLine::new("tok.a").with("css", "--a").with("note", "x y").with("e", ""),
            TerseLine::new("exc.k3m9q2x7a1bz").with("why", "line1\nline2 \"q\" \\ end\u{1b}"),
            TerseLine::new("aaaaaaaaaaaa").with("code", "HN-T001").with("path", "dir with space/f.rs"),
        ];
        for line in samples {
            let parsed = TerseLine::parse(&line.render()).unwrap();
            assert_eq!(parsed.id, line.id);
            for (k, v) in &line.pairs {
                assert_eq!(parsed.get(k), Some(v.text()), "key {k}");
            }
            assert_eq!(parsed.render(), line.render());
        }
    }

    #[test]
    fn r_file_012__parse_rejects_unterminated_quote_and_bad_escape() {
        assert_eq!(TerseLine::parse("a.b k=\"open"), Err(LineError::UnterminatedQuote));
        assert!(matches!(TerseLine::parse("a.b k=\"x\\q\""), Err(LineError::BadEscape { .. })));
        assert!(matches!(TerseLine::parse("a.b k=\"x\"junk"), Err(LineError::TrailingAfterQuote { .. })));
        assert_eq!(TerseLine::parse(""), Err(LineError::Empty));
        assert!(matches!(TerseLine::parse("a.b novalue"), Err(LineError::BadKey { .. })));
    }
}
