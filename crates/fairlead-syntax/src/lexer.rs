//! Comment-token lexer honouring string and raw-string literals (R-FILE-013, R-ARCH-030).
use crate::delimiters::Leader;
use crate::normalize::{Normalized, slice};
use core::ops::Range;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CommentKind {
    Line,
    Block,
    Html,
}

/// One logical line of a comment: leader, closing delimiter, leading `*` and outer blanks removed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CLine {
    pub line: u32,
    pub start: usize,
    pub end: usize,
}

impl CLine {
    pub fn text<'a>(&self, n: &'a Normalized) -> &'a str {
        slice(&n.text, self.start, self.end)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Comment {
    pub kind: CommentKind,
    /// Whole token including delimiters.
    pub start: usize,
    pub end: usize,
    /// `///`, `//!`, `/**`, `/*!` (R-SYN-002).
    pub doc: bool,
    /// Code shares a line with this comment (trailing form, R-SYN-017).
    pub inline: bool,
    pub lines: Vec<CLine>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Lexed {
    pub comments: Vec<Comment>,
    /// Byte ranges of string literals (used to mask text for bracket matching).
    pub strings: Vec<Range<usize>>,
}

// ---- byte helpers (all total) -------------------------------------------------------------

fn at(b: &[u8], i: usize) -> u8 {
    b.get(i).copied().unwrap_or(0)
}

fn starts(b: &[u8], i: usize, pat: &[u8]) -> bool {
    !pat.is_empty() && b.get(i..).is_some_and(|r| r.starts_with(pat))
}

fn find(b: &[u8], from: usize, pat: &[u8]) -> Option<usize> {
    let r = b.get(from..)?;
    memchr::memmem::find(r, pat).map(|p| p + from)
}

fn eol(b: &[u8], from: usize) -> usize {
    let r = b.get(from..).unwrap_or(&[]);
    memchr::memchr(b'\n', r).map_or(b.len(), |p| p + from)
}

fn is_blank(b: &[u8]) -> bool {
    b.iter().all(|c| matches!(c, b' ' | b'\t'))
}

fn trim_range(b: &[u8], mut s: usize, mut e: usize) -> (usize, usize) {
    while s < e && matches!(at(b, s), b' ' | b'\t') {
        s += 1;
    }
    while e > s && matches!(at(b, e - 1), b' ' | b'\t') {
        e -= 1;
    }
    (s, e)
}

fn utf8_len(lead: u8) -> usize {
    match lead {
        0..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

fn inline_flag(n: &Normalized, start: usize, end: usize) -> bool {
    let b = n.text.as_bytes();
    let l0 = n.line_of(start);
    let before = b.get(n.line_start(l0)..start).unwrap_or(&[]);
    let l1 = n.line_of(end.saturating_sub(1).max(start));
    let after = b
        .get(end.min(b.len())..n.line_end(l1).max(end.min(b.len())))
        .unwrap_or(&[]);
    !is_blank(before) || !is_blank(after)
}

// ---- per-extension string rules -----------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum Sq {
    #[allow(dead_code)] // reason: reserved for hosts without single-quote strings
    Off,
    Char,
    Str,
}

struct Rules {
    dq: bool,
    sq: Sq,
    backtick: bool,
    triple: bool,
    rust_raw: bool,
    hash_needs_space: bool,
    /// `"..."` may span lines (Rust string literals).
    dq_spans_lines: bool,
}

fn rules_for(ext: &str) -> Rules {
    let r = |sq, backtick, triple, rust_raw, hash_needs_space| Rules {
        dq: true,
        sq,
        backtick,
        triple,
        rust_raw,
        hash_needs_space,
        dq_spans_lines: false,
    };
    match ext {
        "rs" => Rules {
            dq_spans_lines: true,
            ..r(Sq::Char, false, false, true, false)
        },
        "ts" | "tsx" | "js" | "jsx" | "mjs" => r(Sq::Str, true, false, false, false),
        "go" => r(Sq::Char, true, false, false, false),
        "java" | "kt" | "swift" | "cs" | "scala" => r(Sq::Char, false, true, false, false),
        "c" | "h" | "cc" | "cpp" | "zig" => r(Sq::Char, false, false, false, false),
        "py" | "rb" | "toml" | "ps1" | "gitignore" => r(Sq::Str, false, true, false, false),
        "sh" | "yaml" | "yml" => r(Sq::Str, false, false, false, true),
        _ => r(Sq::Str, false, false, false, false),
    }
}

/// `"..."` / `'...'` ending at the closing quote or the newline (unless `spans_lines`); triple
/// quotes span lines.
fn scan_quoted(b: &[u8], i: usize, q: u8, triple: bool, spans_lines: bool) -> usize {
    if triple && at(b, i + 1) == q && at(b, i + 2) == q {
        return find(b, i + 3, &[q, q, q]).map_or(b.len(), |p| p + 3);
    }
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'\\' => j += 2,
            c if c == q => return j + 1,
            b'\n' if !spans_lines => return j,
            _ => j += 1,
        }
    }
    b.len()
}

/// Rust/C-family char literal at `i` (`'x'`, `'\n'`, `'\u{1f600}'`); a lifetime returns `i + 1`.
fn scan_char(b: &[u8], i: usize) -> usize {
    let n1 = at(b, i + 1);
    if n1 == b'\\' {
        let lim = (i + 14).min(b.len());
        let mut j = i + 3;
        while j < lim {
            match at(b, j) {
                b'\'' => return j + 1,
                b'\n' => break,
                _ => j += 1,
            }
        }
        return i + 1;
    }
    let l = utf8_len(n1);
    if n1 != 0 && n1 != b'\n' && n1 != b'\'' && at(b, i + 1 + l) == b'\'' {
        i + 2 + l
    } else {
        i + 1
    }
}

fn scan_backtick(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'\\' => j += 2,
            b'`' => return j + 1,
            _ => j += 1,
        }
    }
    b.len()
}

/// `r"..."`, `r#"..."#`, `br"..."` starting at `i`; `None` when `i` does not start one.
fn scan_rust_raw(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 {
        let p = at(b, i - 1);
        if p.is_ascii_alphanumeric() || p == b'_' {
            return None;
        }
    }
    let mut j = i;
    if at(b, j) == b'b' {
        j += 1;
    }
    if at(b, j) != b'r' {
        return None;
    }
    j += 1;
    let mut hashes = 0usize;
    while at(b, j) == b'#' {
        hashes += 1;
        j += 1;
    }
    if at(b, j) != b'"' {
        return None;
    }
    let mut closer = vec![b'"'];
    closer.extend(std::iter::repeat_n(b'#', hashes));
    Some(find(b, j + 1, &closer).map_or(b.len(), |p| p + closer.len()))
}

// ---- comment tokens -----------------------------------------------------------------------

/// Splits the content `body_start..body_end` of a delimited comment into logical lines.
fn block_lines(n: &Normalized, body_start: usize, body_end: usize, strip_star: bool) -> Vec<CLine> {
    let b = n.text.as_bytes();
    let mut out = Vec::new();
    let mut a = body_start.min(body_end);
    let mut first = true;
    loop {
        let seg = b.get(a..body_end).unwrap_or(&[]);
        let nl = memchr::memchr(b'\n', seg);
        let seg_end = nl.map_or(body_end, |p| a + p);
        let mut s = a;
        if !first {
            while s < seg_end && matches!(at(b, s), b' ' | b'\t') {
                s += 1;
            }
            if strip_star && at(b, s) == b'*' && s < seg_end {
                s += 1;
            }
        }
        let (s, e) = trim_range(b, s, seg_end);
        out.push(CLine {
            line: n.line_of(a),
            start: s,
            end: e.max(s),
        });
        match nl {
            None => break,
            Some(_) => {
                a = seg_end + 1;
                first = false;
            }
        }
    }
    out
}

fn push_line_comment(
    n: &Normalized,
    out: &mut Lexed,
    i: usize,
    leader_len: usize,
    is_slash: bool,
) -> usize {
    let b = n.text.as_bytes();
    let end = eol(b, i);
    let doc = is_slash
        && leader_len == 2
        && ((at(b, i + 2) == b'/' && at(b, i + 3) != b'/') || at(b, i + 2) == b'!');
    let start = if doc { i + 3 } else { i + leader_len };
    let (s, e) = trim_range(b, start.min(end), end);
    out.comments.push(Comment {
        kind: CommentKind::Line,
        start: i,
        end,
        doc,
        inline: inline_flag(n, i, end),
        lines: vec![CLine {
            line: n.line_of(i),
            start: s,
            end: e,
        }],
    });
    end.max(i + 1)
}

fn push_delimited(
    n: &Normalized,
    out: &mut Lexed,
    i: usize,
    open: &str,
    close: &str,
    kind: CommentKind,
) -> usize {
    let b = n.text.as_bytes();
    let body0 = i + open.len();
    let doc = kind == CommentKind::Block
        && open == "/*"
        && ((at(b, body0) == b'*' && at(b, body0 + 1) != b'*' && at(b, body0 + 1) != b'/')
            || at(b, body0) == b'!');
    let body_start = if doc { body0 + 1 } else { body0 };
    let (body_end, end) = match find(b, body_start.min(b.len()), close.as_bytes()) {
        Some(p) => (p, p + close.len()),
        None => (b.len(), b.len()),
    };
    out.comments.push(Comment {
        kind,
        start: i,
        end,
        doc,
        inline: inline_flag(n, i, end),
        lines: block_lines(n, body_start, body_end, kind == CommentKind::Block),
    });
    end.max(i + 1)
}

/// Lexes `n` for the leaders of `ext`. Total and linear.
/// A-28: original byte ranges of every comment token in `text`, in order, with the built-in
/// leaders of `ext` (no leaders known: no spans). The entry point of the lexer fuzz target.
pub fn comment_spans(text: &str, ext: &str) -> Vec<Range<usize>> {
    let table = crate::delimiters::CommentTable::builtin();
    let Some(leaders) = table.leaders_for(&ext.to_ascii_lowercase()) else {
        return Vec::new();
    };
    let n = Normalized::new(text);
    lex(&n, ext, leaders)
        .comments
        .iter()
        .map(|c| {
            let a = usize::try_from(n.orig_offset(c.start)).unwrap_or(usize::MAX);
            let b = usize::try_from(n.orig_offset(c.end)).unwrap_or(usize::MAX);
            a.min(text.len())..b.min(text.len())
        })
        .collect()
}

pub fn lex(n: &Normalized, ext: &str, leaders: &[Leader]) -> Lexed {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    if leaders.iter().any(|l| matches!(l, Leader::Html)) {
        return lex_html(n, matches!(ext.as_str(), "md" | "markdown"));
    }
    lex_code(n, &ext, leaders)
}

fn lex_code(n: &Normalized, ext: &str, leaders: &[Leader]) -> Lexed {
    let b = n.text.as_bytes();
    let rules = rules_for(ext);
    let mut lines: Vec<&str> = leaders
        .iter()
        .filter_map(|l| match l {
            Leader::Line(s) if !s.is_empty() => Some(s.as_str()),
            _ => None,
        })
        .collect();
    lines.sort_by_key(|s| core::cmp::Reverse(s.len()));
    let blocks: Vec<(&str, &str)> = leaders
        .iter()
        .filter_map(|l| match l {
            Leader::Block { open, close } if !open.is_empty() && !close.is_empty() => {
                Some((open.as_str(), close.as_str()))
            }
            _ => None,
        })
        .collect();
    let mut out = Lexed::default();
    let mut i = 0usize;
    'scan: while i < b.len() {
        for l in &lines {
            let word_start = i == 0
                || matches!(
                    at(b, i - 1),
                    b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'('
                );
            if starts(b, i, l.as_bytes()) && (!rules.hash_needs_space || *l != "#" || word_start) {
                i = push_line_comment(n, &mut out, i, l.len(), *l == "//");
                continue 'scan;
            }
        }
        for (open, close) in &blocks {
            if starts(b, i, open.as_bytes()) {
                i = push_delimited(n, &mut out, i, open, close, CommentKind::Block);
                continue 'scan;
            }
        }
        match at(b, i) {
            b'"' if rules.dq => {
                let e = scan_quoted(b, i, b'"', rules.triple, rules.dq_spans_lines).max(i + 1);
                out.strings.push(i..e);
                i = e;
            }
            b'\'' => match rules.sq {
                Sq::Str => {
                    let e = scan_quoted(b, i, b'\'', rules.triple, false).max(i + 1);
                    out.strings.push(i..e);
                    i = e;
                }
                Sq::Char => {
                    let e = scan_char(b, i);
                    if e > i + 1 {
                        out.strings.push(i..e);
                    }
                    i = e.max(i + 1);
                }
                Sq::Off => i += 1,
            },
            b'`' if rules.backtick => {
                let e = scan_backtick(b, i).max(i + 1);
                out.strings.push(i..e);
                i = e;
            }
            b'r' | b'b' if rules.rust_raw => match scan_rust_raw(b, i) {
                Some(e) => {
                    out.strings.push(i..e.max(i + 1));
                    i = e.max(i + 1);
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out
}

/// Skips a backtick code span that starts at `i` (equal-length runs, one line); returns the next index.
fn skip_code_span(b: &[u8], i: usize) -> usize {
    let mut n = 0usize;
    while at(b, i + n) == b'`' {
        n += 1;
    }
    let e = eol(b, i);
    let mut j = i + n;
    while j < e {
        if at(b, j) == b'`' {
            let mut m = 0usize;
            while at(b, j + m) == b'`' {
                m += 1;
            }
            if m == n {
                return j + m;
            }
            j += m;
        } else {
            j += 1;
        }
    }
    i + n
}

/// Skips an HTML start tag at `i` (`<` plus a letter), honouring quoted attribute values.
fn skip_tag(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match at(b, j) {
            b'>' => return j + 1,
            q @ (b'"' | b'\'') => match find(b, j + 1, &[q]) {
                Some(p) => j = p + 1,
                None => return b.len(),
            },
            _ => j += 1,
        }
    }
    b.len()
}

/// `<!-- -->` comments. For Markdown also: fenced code, indented code, inline code spans (R-SYN-005).
fn lex_html(n: &Normalized, markdown: bool) -> Lexed {
    let b = n.text.as_bytes();
    let mut out = Lexed::default();
    let mut i = 0usize;
    let mut fence: Option<(u8, usize)> = None;
    let mut prev_ok = true; // previous line blank or indented code: an indented code block may start
    while i < b.len() {
        let line_start = i == 0 || at(b, i - 1) == b'\n';
        if markdown && line_start {
            let e = eol(b, i);
            let line = b.get(i..e).unwrap_or(&[]);
            let lead = line.iter().take_while(|c| **c == b' ').count();
            let blank = is_blank(line);
            if let Some((ch, len)) = fence {
                let rest = line.get(lead..).unwrap_or(&[]);
                if lead <= 3 && rest.len() >= len && rest.iter().all(|c| *c == ch) {
                    fence = None;
                }
                prev_ok = false;
                i = e + 1;
                continue;
            }
            let ch = at(line, lead);
            if lead <= 3 && (ch == b'`' || ch == b'~') {
                let run = line
                    .get(lead..)
                    .unwrap_or(&[])
                    .iter()
                    .take_while(|c| **c == ch)
                    .count();
                if run >= 3 {
                    fence = Some((ch, run));
                    prev_ok = false;
                    i = e + 1;
                    continue;
                }
            }
            if !blank && prev_ok && (at(line, 0) == b'\t' || lead >= 4) {
                i = e + 1; // indented code line; prev_ok stays true
                continue;
            }
            prev_ok = blank;
        }
        match at(b, i) {
            b'<' if starts(b, i, b"<!--") => {
                i = push_delimited(n, &mut out, i, "<!--", "-->", CommentKind::Html);
            }
            b'`' if markdown => i = skip_code_span(b, i).max(i + 1),
            b'<' if !markdown && at(b, i + 1).is_ascii_alphabetic() => {
                i = skip_tag(b, i).max(i + 1)
            }
            _ => i += 1,
        }
    }
    out
}

/// What one comment line is, with respect to the marker (R-SYN-001, 003, 004).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MarkerLine {
    None,
    Start { body: usize },
    Cont { body: usize },
    NearMiss,
    Escaped,
}

fn skip_blanks(s: &str, from: usize) -> usize {
    let b = s.as_bytes();
    let mut j = from;
    while matches!(at(b, j), b' ' | b'\t') {
        j += 1;
    }
    j.min(s.len())
}

/// `text` is a [`CLine`] text (outer blanks already trimmed).
pub fn classify_marker_line(text: &str, marker: &str) -> MarkerLine {
    let m = marker.len();
    if m == 0 {
        return MarkerLine::None;
    }
    let (Some(head), Some(rest)) = (text.get(..m), text.get(m..)) else {
        return MarkerLine::None;
    };
    let rb = rest.as_bytes();
    if head == marker {
        return match at(rb, 0) {
            b':' => MarkerLine::Start {
                body: skip_blanks(text, m + 1),
            },
            b'|' => MarkerLine::Cont {
                body: skip_blanks(text, m + 1),
            },
            b'\\' if at(rb, 1) == b':' => MarkerLine::Escaped,
            b';' => MarkerLine::NearMiss,
            b' ' | b'\t' => {
                let j = skip_blanks(rest, 0);
                match at(rb, j) {
                    b':' | b';' | b'|' => MarkerLine::NearMiss,
                    b'-' if at(rb, j + 1) != b'-' => MarkerLine::NearMiss,
                    _ => MarkerLine::None,
                }
            }
            _ => MarkerLine::None,
        };
    }
    if head.eq_ignore_ascii_case(marker) {
        let j = skip_blanks(rest, 0);
        if matches!(at(rb, j), b':' | b';' | b'|' | b'-') {
            return MarkerLine::NearMiss;
        }
    }
    MarkerLine::None
}

/// One marker candidate: a start line plus its joined continuation lines (R-SYN-028).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    /// Index into `Lexed::comments` of the comment holding the first line.
    pub comment: usize,
    pub first_line: u32,
    pub last_line: u32,
    /// Normalized offset of the marker on the first line.
    pub marker_start: usize,
    /// Directive body: text after `marker:`, continuation tails joined with one space.
    pub body: String,
    /// `(body offset, normalized offset)` per joined segment, ascending.
    pub segs: Vec<(usize, usize)>,
    /// A `marker|` line with no directive start before it.
    pub orphan: bool,
    /// More than 8 continuation lines or more than 1 KB (HN-S006).
    pub too_long: bool,
    pub doc: bool,
    pub inline: bool,
}

impl Candidate {
    /// Maps an offset inside `body` to an offset in the normalized text.
    pub fn norm_offset(&self, body_off: usize) -> usize {
        let idx = self.segs.partition_point(|s| s.0 <= body_off);
        match idx.checked_sub(1).and_then(|k| self.segs.get(k)) {
            Some(&(b0, n0)) => n0 + (body_off - b0),
            None => self.marker_start,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Scan {
    pub candidates: Vec<Candidate>,
    /// Normalized offsets of near-miss comment texts (HN-S005).
    pub near_misses: Vec<usize>,
    /// Count of silently ignored `marker\:` lines (R-SYN-004).
    pub escaped: u32,
}

pub const MAX_CONTINUATIONS: usize = 8;
pub const MAX_BODY_BYTES: usize = 1024;

/// Finds every marker candidate in the comment lines of `lexed`.
pub fn scan_markers(n: &Normalized, lexed: &Lexed, marker: &str) -> Scan {
    let mut flat: Vec<(usize, &CLine, MarkerLine)> = Vec::new();
    for (ci, c) in lexed.comments.iter().enumerate() {
        for cl in &c.lines {
            flat.push((ci, cl, classify_marker_line(cl.text(n), marker)));
        }
    }
    let mut out = Scan::default();
    let mut k = 0usize;
    while let Some(&(ci, cl, class)) = flat.get(k) {
        k += 1;
        let Some(comment) = lexed.comments.get(ci) else {
            continue;
        };
        match class {
            MarkerLine::None => {}
            MarkerLine::NearMiss => out.near_misses.push(cl.start),
            MarkerLine::Escaped => out.escaped += 1,
            MarkerLine::Start { body } | MarkerLine::Cont { body } => {
                let orphan = matches!(class, MarkerLine::Cont { .. });
                let first = slice(cl.text(n), body, cl.text(n).len());
                let mut cand = Candidate {
                    comment: ci,
                    first_line: cl.line,
                    last_line: cl.line,
                    marker_start: cl.start,
                    body: first.to_owned(),
                    segs: vec![(0, cl.start + body)],
                    orphan,
                    too_long: false,
                    doc: comment.doc,
                    inline: comment.inline,
                };
                let mut conts = 0usize;
                while let Some(&(_, cl2, class2)) = flat.get(k) {
                    let MarkerLine::Cont { body: b2 } = class2 else {
                        break;
                    };
                    if cl2.line != cand.last_line + 1 {
                        break;
                    }
                    k += 1;
                    conts += 1;
                    cand.last_line = cl2.line;
                    let tail = slice(cl2.text(n), b2, cl2.text(n).len());
                    if conts <= MAX_CONTINUATIONS
                        && cand.body.len() + 1 + tail.len() <= MAX_BODY_BYTES
                    {
                        cand.body.push(' ');
                        let off = cand.body.len();
                        cand.body.push_str(tail);
                        cand.segs.push((off, cl2.start + b2));
                    } else {
                        cand.too_long = true;
                    }
                }
                if cand.body.len() > MAX_BODY_BYTES {
                    cand.too_long = true;
                }
                out.candidates.push(cand);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::CommentTable;

    fn lex_str(src: &str, ext: &str) -> (Normalized, Lexed) {
        let n = Normalized::new(src);
        let table = CommentTable::builtin();
        let leaders = table.leaders_for(ext).unwrap().to_vec();
        let l = lex(&n, ext, &leaders);
        (n, l)
    }

    /// Each comment as its line texts joined with `|`.
    fn texts(n: &Normalized, l: &Lexed) -> Vec<String> {
        l.comments
            .iter()
            .map(|c| {
                c.lines
                    .iter()
                    .map(|x| x.text(n))
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect()
    }

    #[test]
    fn r_file_013__double_quoted_string_hides_comment_marker() {
        let (n, l) = lex_str("let s = \"// fairlead: use x\";\n// real\n", "rs");
        assert_eq!(texts(&n, &l), vec!["real"]);
        assert_eq!(l.strings.len(), 1);
    }

    #[test]
    fn r_file_013__rust_multi_line_strings_hide_comments() {
        let src = "let s = \"x\n// fairlead: use tok.a\n\";\n// real\n";
        let (n, l) = lex_str(src, "rs");
        assert_eq!(texts(&n, &l), vec!["real"]);
    }

    #[test]
    fn r_file_013__rust_raw_strings_hide_comments() {
        let src = "let a = r#\"// fairlead: x \"quoted\" \"#;\nlet b = br\"// fairlead: y\";\nlet c = r##\"\"# // z\"##; // tail\n";
        let (n, l) = lex_str(src, "rs");
        assert_eq!(texts(&n, &l), vec!["tail"]);
    }

    #[test]
    fn r_file_013__rust_lifetimes_and_char_literals_are_not_strings() {
        let src = "fn f<'a>(x: &'a str) { let q = '\"'; let e = '\\''; } // tail\n";
        let (n, l) = lex_str(src, "rs");
        assert_eq!(texts(&n, &l), vec!["tail"]);
    }

    #[test]
    fn r_file_013__js_quotes_and_templates() {
        let src = "const a = '// no'; const b = `// no\n// still no`; // yes\n";
        let (n, l) = lex_str(src, "ts");
        assert_eq!(texts(&n, &l), vec!["yes"]);
    }

    #[test]
    fn r_file_013__unterminated_single_line_string_ends_at_newline() {
        let (n, l) = lex_str("x = 'abc\n// real\n", "ts");
        assert_eq!(texts(&n, &l), vec!["real"]);
    }

    #[test]
    fn r_file_013__hash_hosts_python_shell_toml() {
        let (n, l) = lex_str("x = \"# no\"  # yes\ny = '''\n# inside triple\n'''\n", "py");
        assert_eq!(texts(&n, &l), vec!["yes"]);
        let (n2, l2) = lex_str("echo ${#x} a#b # real\n", "sh");
        assert_eq!(texts(&n2, &l2), vec!["real"]);
        let (n3, l3) = lex_str("a = '#no'\n# yes\n", "toml");
        assert_eq!(texts(&n3, &l3), vec!["yes"]);
    }

    #[test]
    fn r_file_013__css_and_sql_hosts() {
        let (n, l) = lex_str("a { content: \"/* no */\"; } /* yes */\n", "css");
        assert_eq!(texts(&n, &l), vec!["yes"]);
        let (n2, l2) = lex_str("select '-- no'; -- yes\n", "sql");
        assert_eq!(texts(&n2, &l2), vec!["yes"]);
    }

    #[test]
    fn r_syn_030__block_comment_lines_strip_leading_star() {
        let (n, l) = lex_str("/*\n * fairlead: def tok.a\n * more\n */\ncode();\n", "rs");
        assert_eq!(l.comments.len(), 1);
        let c = &l.comments[0];
        assert_eq!(c.kind, CommentKind::Block);
        let lines: Vec<(u32, &str)> = c.lines.iter().map(|x| (x.line, x.text(&n))).collect();
        assert_eq!(
            lines,
            vec![(1, ""), (2, "fairlead: def tok.a"), (3, "more"), (4, "")]
        );
    }

    #[test]
    fn r_syn_030__unterminated_block_runs_to_eof() {
        let (n, l) = lex_str("/* fairlead: use x\nstill comment", "rs");
        assert_eq!(l.comments.len(), 1);
        assert_eq!(l.comments[0].end, n.text.len());
    }

    #[test]
    fn r_syn_002__doc_comment_leaders_are_flagged() {
        let src = "/// a\n//! b\n/** c */\n/*! d */\n//// e\n/**/\n/*** f */\n// g\n";
        let (_, l) = lex_str(src, "rs");
        let flags: Vec<bool> = l.comments.iter().map(|c| c.doc).collect();
        assert_eq!(
            flags,
            vec![true, true, true, true, false, false, false, false]
        );
    }

    #[test]
    fn r_syn_017__inline_flag_marks_comments_sharing_a_line_with_code() {
        let (_, l) = lex_str("let x = 1; // a\n// b\n/* c */ foo();\n  // d\n", "rs");
        let flags: Vec<bool> = l.comments.iter().map(|c| c.inline).collect();
        assert_eq!(flags, vec![true, false, true, false]);
    }

    #[test]
    fn r_syn_006__custom_leader_from_overrides() {
        let n = Normalized::new("x ; fairlead: use tt.b\n");
        let l = lex(&n, "foo", &[Leader::Line(";".to_owned())]);
        assert_eq!(texts(&n, &l), vec!["fairlead: use tt.b"]);
    }

    #[test]
    fn r_arch_032__lexer_is_total_on_hostile_input() {
        for src in [
            "'",
            "\"",
            "`",
            "/*",
            "r#\"",
            "br",
            "'\\",
            "<!--",
            "\u{0}\u{1}",
            "\u{e9}'\u{e9}",
            "'''",
            "\"\"\"",
        ] {
            let _ = lex_str(src, "rs");
            let _ = lex_str(src, "py");
            let _ = lex_str(src, "ts");
        }
    }

    #[test]
    fn r_syn_005__markdown_comment_outside_fences_only() {
        let src = "# T\n\n```md\n<!-- fairlead: use no.fence -->\n```\n\n<!-- fairlead: use yes.one -->\n\n~~~~\n<!-- fairlead: use no.tilde -->\n~~~\n<!-- still inside -->\n~~~~\n<!-- fairlead: use yes.two -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(
            texts(&n, &l),
            vec!["fairlead: use yes.one", "fairlead: use yes.two"]
        );
    }

    #[test]
    fn r_syn_005__markdown_indented_code_and_inline_code_are_not_scanned() {
        let src = "para\n\n    <!-- fairlead: use no.indented -->\n\nText `<!-- fairlead: use no.inline -->` and <!-- fairlead: use yes.x -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.x"]);
    }

    #[test]
    fn r_syn_005__indented_text_directly_after_a_paragraph_is_not_code() {
        let src = "para\n    <!-- fairlead: use yes.y -->\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.y"]);
    }

    #[test]
    fn r_syn_005__multi_line_html_comment_yields_one_line_per_source_line() {
        let src = "<!-- fairlead: ovr rule.x\n     why=\"a\"\n-->\ntext\n";
        let (n, l) = lex_str(src, "md");
        assert_eq!(l.comments.len(), 1);
        let got: Vec<(u32, &str)> = l.comments[0]
            .lines
            .iter()
            .map(|x| (x.line, x.text(&n)))
            .collect();
        assert_eq!(
            got,
            vec![(1, "fairlead: ovr rule.x"), (2, "why=\"a\""), (3, "")]
        );
        assert_eq!(l.comments[0].kind, CommentKind::Html);
    }

    #[test]
    fn r_syn_044__html_attribute_strings_do_not_hold_comments() {
        let src = "<a title=\"<!-- fairlead: use no.attr -->\" href='x'>t</a>\n<!-- fairlead: use yes.z -->\n";
        let (n, l) = lex_str(src, "html");
        assert_eq!(texts(&n, &l), vec!["fairlead: use yes.z"]);
    }

    #[test]
    fn r_syn_044__xml_and_svg_use_the_same_comment_syntax() {
        let (n, l) = lex_str(
            "<?xml version=\"1.0\"?>\n<!-- fairlead: def asset.logo -->\n<svg/>\n",
            "svg",
        );
        assert_eq!(texts(&n, &l), vec!["fairlead: def asset.logo"]);
    }

    #[test]
    fn r_syn_001__start_and_continuation_markers() {
        assert_eq!(
            classify_marker_line("fairlead: def cmp.x", "fairlead"),
            MarkerLine::Start { body: 10 }
        );
        assert_eq!(
            classify_marker_line("fairlead:   use tt.b", "fairlead"),
            MarkerLine::Start { body: 12 }
        );
        assert_eq!(
            classify_marker_line("fairlead| why=\"x\"", "fairlead"),
            MarkerLine::Cont { body: 10 }
        );
        assert_eq!(
            classify_marker_line("fairlead:", "fairlead"),
            MarkerLine::Start { body: 9 }
        );
        assert_eq!(
            classify_marker_line("fairlead: use tt.b", "fairlead"),
            MarkerLine::Start { body: 10 }
        );
    }

    #[test]
    fn r_syn_001__ordinary_comments_are_not_markers() {
        for t in [
            "",
            "fairleads: x",
            "the fairlead: x",
            "fairlead is a word",
            "fairleadX: y",
            "fairlead",
            "TODO fairlead: x",
        ] {
            assert_eq!(classify_marker_line(t, "fairlead"), MarkerLine::None, "{t}");
        }
        assert_eq!(
            classify_marker_line("fairlead: x", ""),
            MarkerLine::None,
            "empty marker never matches"
        );
    }

    #[test]
    fn r_syn_003__near_misses_are_reported_not_parsed() {
        for t in [
            "Fairlead: x",
            "FAIRLEAD: x",
            "fairlead; x",
            "fairlead - x",
            "fairlead : x",
            "Fairlead| x",
            "fairlead ; x",
        ] {
            assert_eq!(
                classify_marker_line(t, "fairlead"),
                MarkerLine::NearMiss,
                "{t}"
            );
        }
        assert_eq!(
            classify_marker_line("fairlead -- note", "fairlead"),
            MarkerLine::None
        );
        assert_eq!(
            classify_marker_line("Fairlead is fine", "fairlead"),
            MarkerLine::None
        );
    }

    #[test]
    fn r_syn_004__backslash_colon_is_silently_ignored() {
        assert_eq!(
            classify_marker_line("fairlead\\: def x", "fairlead"),
            MarkerLine::Escaped
        );
    }

    #[test]
    fn r_syn_001__non_ascii_marker_does_not_panic() {
        assert_eq!(
            classify_marker_line("h\u{e4}rna: x", "h\u{e4}rna"),
            MarkerLine::Start { body: 8 }
        );
        assert_eq!(
            classify_marker_line("h\u{e4}", "fairlead"),
            MarkerLine::None
        );
        assert_eq!(classify_marker_line("\u{e9}", "fairlead"), MarkerLine::None);
    }

    fn scan(src: &str, ext: &str) -> (Normalized, Scan) {
        let (n, l) = lex_str(src, ext);
        let s = scan_markers(&n, &l, "fairlead");
        (n, s)
    }

    #[test]
    fn r_syn_028__continuation_lines_join_with_one_space() {
        let src = "// fairlead: ovr rule.x owner=@a exp=2026-12-31\n// fairlead| why=\"legacy\"\n// fairlead|   value=#ff0000\nlet x = 1;\n";
        let (n, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        let c = &s.candidates[0];
        assert_eq!(
            c.body,
            "ovr rule.x owner=@a exp=2026-12-31 why=\"legacy\" value=#ff0000"
        );
        assert_eq!((c.first_line, c.last_line), (1, 3));
        assert!(!c.too_long && !c.orphan && !c.doc);
        let why = c.body.find("why").unwrap();
        assert_eq!(&n.text[c.norm_offset(why)..c.norm_offset(why) + 3], "why");
        let value = c.body.find("value").unwrap();
        assert_eq!(
            &n.text[c.norm_offset(value)..c.norm_offset(value) + 5],
            "value"
        );
        assert_eq!(c.norm_offset(0), c.marker_start + "fairlead: ".len());
    }

    #[test]
    fn r_syn_028__more_than_eight_continuations_is_too_long() {
        let mut src = String::from("// fairlead: ovr rule.x why=\"a\"\n");
        for _ in 0..9 {
            src.push_str("// fairlead| k=v\n");
        }
        let (_, s) = scan(&src, "rs");
        assert_eq!(
            s.candidates.len(),
            1,
            "extra continuations are consumed, not orphaned"
        );
        assert!(s.candidates[0].too_long);
        let mut ok = String::from("// fairlead: ovr rule.x why=\"a\"\n");
        for _ in 0..8 {
            ok.push_str("// fairlead| k=v\n");
        }
        assert!(!scan(&ok, "rs").1.candidates[0].too_long);
    }

    #[test]
    fn r_syn_028__body_over_one_kilobyte_is_too_long() {
        let src = format!(
            "// fairlead: ovr rule.x why=\"{}\"\n// fairlead| k=v\n",
            "a".repeat(1100)
        );
        let (_, s) = scan(&src, "rs");
        assert!(s.candidates[0].too_long);
    }

    #[test]
    fn r_syn_028__a_gap_ends_the_continuation_and_the_stray_line_is_an_orphan() {
        let src = "// fairlead: use tt.b\n\n// fairlead| k=v\n";
        let (_, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 2);
        assert!(!s.candidates[0].orphan);
        assert!(s.candidates[1].orphan);
        assert_eq!(s.candidates[1].body, "k=v");
    }

    #[test]
    fn r_syn_028__block_comment_lines_can_continue() {
        let src = "/*\n * fairlead: use tt.b\n * fairlead| sym=X\n */\n";
        let (_, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.candidates[0].body, "use tt.b sym=X");
        assert_eq!(
            (s.candidates[0].first_line, s.candidates[0].last_line),
            (2, 3)
        );
    }

    #[test]
    fn r_syn_002__doc_and_inline_flags_reach_the_candidate() {
        let (_, s) = scan(
            "/// fairlead: use tt.b\nlet x = 1; // fairlead: use uu.d\n",
            "rs",
        );
        assert!(s.candidates[0].doc && !s.candidates[0].inline);
        assert!(!s.candidates[1].doc && s.candidates[1].inline);
    }

    #[test]
    fn r_syn_003__near_misses_and_escapes_are_not_candidates() {
        let src = "// Fairlead: use tt.b\n// fairlead; use tt.b\n// fairlead\\: use tt.b\n// fairlead: use tt.b\n";
        let (n, s) = scan(src, "rs");
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.near_misses.len(), 2);
        assert_eq!(&n.text[s.near_misses[0]..s.near_misses[0] + 8], "Fairlead");
        assert_eq!(s.escaped, 1);
    }

    #[test]
    fn r_syn_001__marker_in_a_string_is_not_a_candidate() {
        let (_, s) = scan("let s = \"// fairlead: use tt.b\";\n", "rs");
        assert!(s.candidates.is_empty());
    }

    #[test]
    fn r_arch_150__comment_spans_are_ordered_in_bounds_and_skip_strings() {
        let text = "let s = \"// fairlead: def fake\"; // fairlead: def real
/* b */
";
        let spans = comment_spans(text, "rs");
        assert_eq!(spans.len(), 2, "{spans:?}");
        let mut last = 0;
        for sp in &spans {
            assert!(sp.start >= last && sp.start <= sp.end && sp.end <= text.len());
            last = sp.end;
        }
        assert!(text[spans[0].clone()].contains("real"));
        assert!(comment_spans(text, "nope").is_empty());
    }
}
