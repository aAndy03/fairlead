//! The single extraction entry point (R-ARCH-030..033). Pure, total, deterministic.
use crate::delimiters::{CommentTable, Leader};
use crate::diag::{Span, SyntaxDiag, codes};
use crate::directive::Directive;
use crate::frontmatter::Frontmatter;
use crate::{GRAMMAR_EDITION, SYNTAX_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExtractConfig {
    /// `[fairlead] marker`, default `"fairlead"`.
    pub marker: String,
    pub comments: CommentTable,
    /// `[scan] header-skip` patterns (`*` wildcard) skipped before the file-form block (R-SYN-016).
    pub header_skip: Vec<String>,
    /// Default 2_000_000 (R-SYN-030).
    pub max_file: u64,
    /// Default 500 (R-SYN-031).
    pub max_directives: u32,
}

impl Default for ExtractConfig {
    fn default() -> Self {
        ExtractConfig {
            marker: "fairlead".to_owned(),
            comments: CommentTable::builtin(),
            header_skip: Vec::new(),
            max_file: 2_000_000,
            max_directives: 500,
        }
    }
}

fn put(b: &mut Vec<u8>, tag: u8, s: &[u8]) {
    b.push(tag);
    b.extend_from_slice(&(s.len() as u64).to_le_bytes());
    b.extend_from_slice(s);
}

fn fnv_lane(data: &[u8], basis: u64) -> u64 {
    let mut h = basis;
    for &x in data {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

impl ExtractConfig {
    /// Length-prefixed, order-stable encoding of everything that influences extraction.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        put(&mut b, b'V', &SYNTAX_VERSION.to_le_bytes());
        put(&mut b, b'E', &GRAMMAR_EDITION.to_le_bytes());
        put(&mut b, b'M', self.marker.as_bytes());
        for (ext, leaders) in self.comments.iter() {
            put(&mut b, b'X', ext.as_bytes());
            for l in leaders {
                match l {
                    Leader::Line(s) => put(&mut b, b'L', s.as_bytes()),
                    Leader::Block { open, close } => {
                        put(&mut b, b'O', open.as_bytes());
                        put(&mut b, b'C', close.as_bytes());
                    }
                    Leader::Html => put(&mut b, b'H', b""),
                }
            }
        }
        for p in &self.header_skip {
            put(&mut b, b'S', p.as_bytes());
        }
        put(&mut b, b'F', &self.max_file.to_le_bytes());
        put(&mut b, b'D', &self.max_directives.to_le_bytes());
        b
    }

    /// Cache-key part (contract 4.2). Not a security primitive.
    pub fn config_fingerprint(&self) -> [u8; 32] {
        let data = self.canonical_bytes();
        let mut out = [0u8; 32];
        for (k, chunk) in out.chunks_mut(8).enumerate() {
            let basis =
                0xcbf2_9ce4_8422_2325u64 ^ (k as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            chunk.copy_from_slice(&fnv_lane(&data, basis).to_le_bytes());
        }
        out
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ScanStatus {
    Scanned,
    NoMarker,
    SkippedTooLarge,
    SkippedBinary,
    SkippedNotUtf8,
}

/// Cached, keyed by content hash + config (R-ARCH-073).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Extraction {
    pub status: ScanStatus,
    pub directives: Vec<Directive>,
    pub frontmatter: Option<Frontmatter>,
    pub diags: Vec<SyntaxDiag>,
    /// Marker candidates seen (a well-formed `end` that closes a `+end` region is structure, not counted).
    pub candidates: u32,
    /// Invariant: `candidates == directives.len() + dropped` (R-ARCH-031).
    pub dropped: u32,
}

impl Extraction {
    pub fn empty(status: ScanStatus) -> Extraction {
        Extraction {
            status,
            directives: Vec::new(),
            frontmatter: None,
            diags: Vec::new(),
            candidates: 0,
            dropped: 0,
        }
    }
    fn skipped(status: ScanStatus, diag: SyntaxDiag) -> Extraction {
        let mut e = Extraction::empty(status);
        e.diags.push(diag);
        e
    }
}

/// Case-insensitive ASCII search for the marker (so near-misses still reach the scanner).
fn contains_marker_ci(hay: &[u8], marker: &str) -> bool {
    let m = marker.as_bytes();
    let Some(&first) = m.first() else {
        return false;
    };
    let (lo, up) = (first.to_ascii_lowercase(), first.to_ascii_uppercase());
    let mut from = 0usize;
    while let Some(p) = memchr::memchr2(lo, up, hay.get(from..).unwrap_or(&[])) {
        let at = from + p;
        if hay
            .get(at..at + m.len())
            .is_some_and(|w| w.eq_ignore_ascii_case(m))
        {
            return true;
        }
        from = at + 1;
    }
    false
}

/// THE entry point. Pure function of bytes, extension and config (R-ARCH-030). Total: never panics,
/// never loops, any byte input (R-ARCH-032). Offsets in results are original-byte offsets.
pub fn extract(bytes: &[u8], ext: &str, cfg: &ExtractConfig) -> Extraction {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if len > cfg.max_file || u32::try_from(bytes.len()).is_err() {
        let d = SyntaxDiag::new(
            codes::HN_I001,
            Span::whole_file(),
            format!("file skipped: {len} bytes exceed max-file {}", cfg.max_file),
            "raise [scan] max-file, or add the path to [scan] skip",
        );
        return Extraction::skipped(ScanStatus::SkippedTooLarge, d);
    }
    if bytes.iter().take(8192).any(|&b| b == 0) {
        let d = SyntaxDiag::new(
            codes::HN_I001,
            Span::whole_file(),
            "file skipped: contains a NUL byte in the first 8 KB (binary)",
            "add the path to [scan] skip or exclude to silence this notice",
        );
        return Extraction::skipped(ScanStatus::SkippedBinary, d);
    }
    let Ok(src) = std::str::from_utf8(bytes) else {
        let d = SyntaxDiag::new(
            codes::HN_I003,
            Span::whole_file(),
            "file skipped: not valid UTF-8 (never decoded by guess)",
            "re-save the file as UTF-8, or add it to [scan] skip",
        );
        return Extraction::skipped(ScanStatus::SkippedNotUtf8, d);
    };
    let ext_l = ext.trim_start_matches('.').to_ascii_lowercase();
    let body = src.strip_prefix('\u{feff}').unwrap_or(src);
    let has_frontmatter = ext_l == "md" && body.starts_with("+++");
    if !has_frontmatter && !contains_marker_ci(bytes, &cfg.marker) {
        return Extraction::empty(ScanStatus::NoMarker);
    }
    let Some(leaders) = cfg.comments.leaders_for(&ext_l) else {
        let d = if ext_l.is_empty() {
            SyntaxDiag::new(
                codes::HN_I002,
                Span::whole_file(),
                "no comment syntax known for a file without an extension; the file was not scanned",
                "add the path to [scan] skip if it holds no directives",
            )
        } else {
            SyntaxDiag::new(
                codes::HN_I002,
                Span::whole_file(),
                format!(
                    "no comment syntax known for extension `.{ext_l}`; the file was not scanned"
                ),
                format!(
                    "add `{ext_l} = [\"//\"]` (or the right leader) under [comments] in the root control file"
                ),
            )
        };
        let mut e = Extraction::empty(ScanStatus::Scanned);
        e.diags.push(d);
        return e;
    };
    if leaders.is_empty() {
        return Extraction::empty(ScanStatus::Scanned); // comment-free host such as JSON (R-SYN-006)
    }
    scan_text(src, &ext_l, cfg, leaders)
}

use crate::attach::{Outcome, Site, attach, code_text};
use crate::diag::DiagCode;
use crate::directive::{ArgValue, BodyError, Carrier, Extent, ParsedBody, Verb, parse_body};
use crate::frontmatter::{FmOutcome, parse_frontmatter};
use crate::lexer::{Candidate, CommentKind, lex, scan_markers};
use crate::normalize::{Normalized, slice, tabs_to_spaces_outside_quotes};

fn hint(code: DiagCode) -> &'static str {
    match code.0 {
        "HN-S001" => "the six verbs are def, use, ovr, flag, env, end (R-SYN-009)",
        "HN-S002" => "IDs are [scope:]kind.name in ASCII lowercase; kinds have 2 to 8 letters",
        "HN-S004" => "frontmatter must be valid TOML and `directives` an array of strings",
        "HN-S005" => "the marker is case-sensitive and `:` must follow it directly (R-SYN-003)",
        "HN-S006" => "shorten the directive: at most 8 continuation lines and 1 KB",
        "HN-S007" => {
            "quote with \"...\"; only \\\" and \\\\ are escapes and a quote may not span lines"
        }
        "HN-S008" => "close every `+end` region with `end`, and write no `end` without one",
        "HN-S010" => "put code after the directive, or give it +N or +end",
        "HN-S012" => "edition 1 uses exp=YYYY-MM-DD; until= needs seals (0.2)",
        "HN-S013" => {
            "use `//` or `/* */` instead of a doc comment so the text never reaches rustdoc or TSDoc"
        }
        "HN-S014" => "fix the sym= spelling or move the directive onto the item",
        "HN-S015" => "split the file or raise [scan] max-directives",
        "HN-S016" => "see `fairlead explain syntax` for the directive forms",
        "HN-P001" => {
            "rename: con prn aux nul com1..9 lpt1..9 and names ending in `.` or space are reserved"
        }
        _ => "see `fairlead explain` for this code",
    }
}

fn plain(
    n: &Normalized,
    code: DiagCode,
    a: usize,
    b: usize,
    message: impl Into<String>,
) -> SyntaxDiag {
    SyntaxDiag::new(code, n.span(a, b), message, hint(code))
}

fn body_diag(n: &Normalized, c: &Candidate, e: &BodyError) -> SyntaxDiag {
    let a = c.norm_offset(e.at);
    let b = (c.norm_offset(e.at.saturating_add(e.len).saturating_sub(1)) + 1).max(a + 1);
    let mut d = plain(n, e.code, a, b, e.message.clone());
    d.expected = e.expected.clone();
    d.did_you_mean = e.did_you_mean.clone();
    d
}

struct Pending {
    body: ParsedBody,
    span: Span,
    carrier: Carrier,
}

fn extent_text<'a>(n: &Normalized, masked: &'a str, extent: Extent) -> Option<&'a str> {
    match extent {
        Extent::File => Some(masked),
        Extent::Doc => None,
        Extent::Lines { first, last } => Some(slice(masked, n.line_start(first), n.line_end(last))),
    }
}

fn scan_text(src: &str, ext: &str, cfg: &ExtractConfig, leaders: &[Leader]) -> Extraction {
    let n = Normalized::new(src);
    let mut ex = Extraction::empty(ScanStatus::Scanned);
    let mut candidates = 0u32;
    let mut dropped = 0u32;

    let mut fm_end = 0usize;
    if ext == "md" {
        match parse_frontmatter(&n) {
            FmOutcome::Absent => {}
            FmOutcome::Bad(e) => {
                ex.diags
                    .push(plain(&n, codes::HN_S004, e.at, e.at + e.len, e.message))
            }
            FmOutcome::Parsed(p) => {
                fm_end = p.end;
                for (body_text, &off) in p.fm.directive_bodies.iter().zip(p.offsets.iter()) {
                    candidates += 1;
                    let end = off + body_text.len();
                    match parse_body(&tabs_to_spaces_outside_quotes(body_text)) {
                        Err(e) => {
                            let d = SyntaxDiag::new(
                                e.code,
                                n.span(off + e.at, off + e.at + e.len),
                                e.message.clone(),
                                hint(e.code),
                            );
                            ex.diags.push(SyntaxDiag {
                                expected: e.expected.clone(),
                                did_you_mean: e.did_you_mean.clone(),
                                ..d
                            });
                            dropped += 1;
                        }
                        Ok(pb) if pb.verb == Verb::End => {
                            ex.diags.push(plain(
                                &n,
                                codes::HN_S008,
                                off,
                                end,
                                "`end` has no region to close in frontmatter",
                            ));
                            dropped += 1;
                        }
                        Ok(pb) if pb.explicit.is_some() => {
                            let d = plain(
                                &n,
                                codes::HN_S016,
                                off,
                                end,
                                "frontmatter directives bind to the doc and take no +N or +end",
                            );
                            ex.diags.push(SyntaxDiag {
                                expected: Some("verb ID [k=v..]".to_owned()),
                                ..d
                            });
                            dropped += 1;
                        }
                        Ok(mut pb) => {
                            for (a, &(s, e)) in pb.args.iter_mut().zip(pb.arg_pos.iter()) {
                                a.span = n.span(off + s, off + e);
                            }
                            ex.directives.push(Directive {
                                ordinal: 0,
                                verb: pb.verb,
                                must_understand: pb.must_understand,
                                ids: pb.ids,
                                bare: pb.bare,
                                args: pb.args,
                                note: pb.note,
                                carrier: Carrier::Frontmatter,
                                span: n.span(off, end),
                                extent: Extent::Doc,
                                end_span: None,
                            });
                        }
                    }
                }
                ex.frontmatter = Some(p.fm);
            }
        }
    }

    let mut lexed = lex(&n, ext, leaders);
    lexed.comments.retain(|c| c.start >= fm_end);
    let scan = scan_markers(&n, &lexed, &cfg.marker);

    for &off in &scan.near_misses {
        let mut d = plain(
            &n,
            codes::HN_S005,
            off,
            off + cfg.marker.len(),
            "comment looks like a directive but the marker does not match",
        );
        d.expected = Some(format!("{}:", cfg.marker));
        d.did_you_mean = Some(format!("{}:", cfg.marker));
        ex.diags.push(d);
    }

    let mut pend: Vec<Pending> = Vec::new();
    let mut sites: Vec<Site> = Vec::new();
    for c in &scan.candidates {
        candidates += 1;
        let line_end = n.line_end(c.first_line);
        let whole = |code: DiagCode, msg: &str| plain(&n, code, c.marker_start, line_end, msg);
        if c.doc {
            ex.diags.push(whole(
                codes::HN_S013,
                "directive in a doc comment would leak into rustdoc/TSDoc and is ignored",
            ));
            dropped += 1;
            continue;
        }
        if c.orphan {
            let mut d = whole(
                codes::HN_S016,
                "continuation line has no directive to continue",
            );
            d.expected = Some("marker: verb ...".to_owned());
            ex.diags.push(d);
            dropped += 1;
            continue;
        }
        if c.too_long {
            ex.diags.push(whole(
                codes::HN_S006,
                "directive exceeds 8 continuation lines or 1 KB",
            ));
            dropped += 1;
            continue;
        }
        let body = tabs_to_spaces_outside_quotes(&c.body);
        match parse_body(&body) {
            Err(e) => {
                ex.diags.push(body_diag(&n, c, &e));
                dropped += 1;
            }
            Ok(mut pb) => {
                for (a, &(s, e)) in pb.args.iter_mut().zip(pb.arg_pos.iter()) {
                    let (x, y) = (c.norm_offset(s), c.norm_offset(e.saturating_sub(1)) + 1);
                    a.span = n.span(x, y);
                }
                let carrier = match lexed.comments.get(c.comment).map(|k| k.kind) {
                    Some(CommentKind::Block) => Carrier::BlockComment,
                    Some(CommentKind::Html) => Carrier::HtmlComment,
                    _ => Carrier::LineComment,
                };
                sites.push(Site {
                    first_line: c.first_line,
                    last_line: c.last_line,
                    inline: c.inline,
                    explicit: pb.explicit,
                    is_end: pb.verb == Verb::End,
                });
                pend.push(Pending {
                    body: pb,
                    span: n.span(c.marker_start, line_end),
                    carrier,
                });
            }
        }
    }

    let outcomes = attach(&n, &lexed, ext, &cfg.header_skip, &sites);
    let masked = code_text(&n, &lexed);
    let spans: Vec<Span> = pend.iter().map(|p| p.span).collect();
    let mut consumed = 0u32;
    for (p, o) in pend.into_iter().zip(outcomes) {
        match o {
            Outcome::Bound { extent, end_site } => {
                let d = Directive {
                    ordinal: 0,
                    verb: p.body.verb,
                    must_understand: p.body.must_understand,
                    ids: p.body.ids,
                    bare: p.body.bare,
                    args: p.body.args,
                    note: p.body.note,
                    carrier: p.carrier,
                    span: p.span,
                    extent,
                    end_span: end_site.and_then(|j| spans.get(j).copied()),
                };
                if matches!(d.verb, Verb::Def | Verb::Use)
                    && let (Some(a), Some(hay)) = (d.arg("sym"), extent_text(&n, &masked, extent))
                {
                    let wanted: Vec<&str> = match &a.value {
                        ArgValue::Word(w) | ArgValue::Quoted(w) => vec![w.as_str()],
                        ArgValue::List(v) => v.iter().map(String::as_str).collect(),
                    };
                    if let Some(missing) = wanted.iter().find(|w| !hay.contains(**w)) {
                        ex.diags.push(SyntaxDiag::new(
                            codes::HN_S014,
                            a.span,
                            format!("sym `{missing}` not found in the directive's extent"),
                            hint(codes::HN_S014),
                        ));
                    }
                }
                ex.directives.push(d);
            }
            Outcome::ClosesRegion => consumed += 1,
            Outcome::NothingToBind => {
                ex.diags.push(SyntaxDiag::new(
                    codes::HN_S010,
                    p.span,
                    "directive binds to nothing and was dropped",
                    hint(codes::HN_S010),
                ));
                dropped += 1;
            }
            Outcome::UnmatchedEnd => {
                ex.diags.push(SyntaxDiag::new(
                    codes::HN_S008,
                    p.span,
                    "`end` without an open `+end` region",
                    hint(codes::HN_S008),
                ));
                dropped += 1;
            }
            Outcome::UnclosedRegion => {
                ex.diags.push(SyntaxDiag::new(
                    codes::HN_S008,
                    p.span,
                    "`+end` region is never closed; the directive was dropped",
                    hint(codes::HN_S008),
                ));
                dropped += 1;
            }
        }
    }

    ex.directives.sort_by_key(|d| d.span.start);
    for (i, d) in ex.directives.iter_mut().enumerate() {
        d.ordinal = u32::try_from(i).unwrap_or(u32::MAX);
    }
    if u32::try_from(ex.directives.len()).unwrap_or(u32::MAX) > cfg.max_directives {
        ex.diags.push(SyntaxDiag::new(
            codes::HN_S015,
            Span::whole_file(),
            format!(
                "{} directives in one file exceed max-directives {}",
                ex.directives.len(),
                cfg.max_directives
            ),
            hint(codes::HN_S015),
        ));
    }
    ex.diags.sort_by(|a, b| {
        (a.span.line, a.span.col, a.code.0, &a.message).cmp(&(
            b.span.line,
            b.span.col,
            b.code.0,
            &b.message,
        ))
    });
    ex.candidates = candidates - consumed;
    ex.dropped = dropped;
    ex
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::Leader;
    use crate::diag::Severity;

    fn cfg() -> ExtractConfig {
        ExtractConfig::default()
    }

    #[test]
    fn r_syn_030__default_limits_and_marker() {
        let c = cfg();
        assert_eq!(
            (c.marker.as_str(), c.max_file, c.max_directives),
            ("fairlead", 2_000_000, 500)
        );
        assert!(c.header_skip.is_empty());
    }

    #[test]
    fn r_syn_030__too_large_is_skipped_with_i001() {
        let mut c = cfg();
        c.max_file = 10;
        let ex = extract(b"// fairlead: use tt.b\n", "rs", &c);
        assert_eq!(ex.status, ScanStatus::SkippedTooLarge);
        assert_eq!(ex.diags.len(), 1);
        assert_eq!(
            (ex.diags[0].code, ex.diags[0].severity),
            (codes::HN_I001, Severity::Info)
        );
        assert!(ex.directives.is_empty() && ex.candidates == 0 && ex.dropped == 0);
        assert_eq!(
            extract(b"0123456789", "rs", &c).status,
            ScanStatus::NoMarker,
            "exactly max-file is fine"
        );
    }

    #[test]
    fn r_syn_030__nul_in_the_first_8k_means_binary() {
        let mut bytes = b"// fairlead: use tt.b\n".to_vec();
        bytes.resize(100, b' ');
        bytes[50] = 0;
        let ex = extract(&bytes, "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::SkippedBinary);
        assert_eq!(ex.diags[0].code, codes::HN_I001);
        let mut late = vec![b' '; 9000];
        late[8500] = 0;
        assert_eq!(
            extract(&late, "rs", &cfg()).status,
            ScanStatus::NoMarker,
            "a NUL after 8 KB does not count"
        );
    }

    #[test]
    fn r_file_015__non_utf8_is_skipped_with_i003_and_never_decoded() {
        let ex = extract(b"// fairlead: use tt.b \xe9\xe8\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::SkippedNotUtf8);
        assert_eq!(
            (ex.diags[0].code, ex.diags[0].severity),
            (codes::HN_I003, Severity::Warn)
        );
        assert!(ex.directives.is_empty());
    }

    #[test]
    fn r_arch_032__any_bytes_are_accepted() {
        for b in [
            &b""[..],
            b"\xff",
            b"\xef\xbb\xbf",
            b"\x00",
            b"\r",
            b"\xc3",
            b"fairlead",
            b"// fairlead:",
        ] {
            let _ = extract(b, "rs", &cfg());
            let _ = extract(b, "md", &cfg());
            let _ = extract(b, "", &cfg());
        }
    }

    #[test]
    fn r_arch_030__no_marker_skips_scanning_entirely() {
        let ex = extract(b"fn main() {}\n// just a comment\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::NoMarker);
        assert!(ex.diags.is_empty() && ex.directives.is_empty());
        assert_eq!(extract(b"", "rs", &cfg()).status, ScanStatus::NoMarker);
    }

    #[test]
    fn r_syn_006__unknown_extension_yields_one_i002_only_when_the_marker_is_present() {
        let ex = extract(b"x ; fairlead: use tt.b\n", "zzz", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
        assert_eq!(ex.diags.len(), 1);
        assert_eq!(
            (ex.diags[0].code, ex.diags[0].severity),
            (codes::HN_I002, Severity::Info)
        );
        assert!(ex.diags[0].hint.contains("[comments]"));
        assert_eq!(extract(b"nothing here\n", "zzz", &cfg()).diags.len(), 0);
    }

    #[test]
    fn r_syn_006__a_file_without_an_extension_gets_a_readable_i002() {
        let ex = extract(b"/fairlead/ @owner\n", "", &cfg());
        assert_eq!(ex.diags.len(), 1);
        let d = &ex.diags[0];
        assert!(d.message.contains("without an extension") && !d.message.contains("`.`"));
        assert!(d.hint.contains("[scan] skip"));
    }

    #[test]
    fn r_syn_006__json_carries_no_directives() {
        let ex = extract(b"{\"a\": \"// fairlead: use tt.b\"}\n", "json", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
        assert!(ex.diags.is_empty() && ex.directives.is_empty() && ex.candidates == 0);
    }

    #[test]
    fn r_syn_003__prefilter_is_case_insensitive_so_near_misses_reach_the_scanner() {
        let ex = extract(b"// Fairlead: use tt.b\n", "rs", &cfg());
        assert_eq!(ex.status, ScanStatus::Scanned);
    }

    #[test]
    fn r_syn_006__fingerprint_is_stable_and_sensitive_to_every_field() {
        let base = cfg();
        assert_eq!(base.config_fingerprint(), cfg().config_fingerprint());
        let mut variants = Vec::new();
        let mut a = cfg();
        a.marker = "ferrylead".into();
        variants.push(a);
        let mut a = cfg();
        a.header_skip.push("// eslint*".into());
        variants.push(a);
        let mut a = cfg();
        a.max_file = 1;
        variants.push(a);
        let mut a = cfg();
        a.max_directives = 1;
        variants.push(a);
        let mut a = cfg();
        a.comments = a
            .comments
            .with_overrides("foo", vec![Leader::Line(";".into())]);
        variants.push(a);
        for v in &variants {
            assert_ne!(v.config_fingerprint(), base.config_fingerprint());
        }
        let prints: std::collections::BTreeSet<[u8; 32]> =
            variants.iter().map(|v| v.config_fingerprint()).collect();
        assert_eq!(prints.len(), variants.len());
    }

    fn ex(src: &str, ext: &str) -> Extraction {
        extract(src.as_bytes(), ext, &cfg())
    }
    fn codes_of(e: &Extraction) -> Vec<&'static str> {
        e.diags.iter().map(|d| d.code.0).collect()
    }
    fn extent(e: &Extraction, i: usize) -> Extent {
        e.directives[i].extent
    }
    fn assert_invariant(e: &Extraction) {
        assert_eq!(
            e.candidates as usize,
            e.directives.len() + e.dropped as usize,
            "R-ARCH-031: {e:?}"
        );
    }

    #[test]
    fn r_arch_031__every_candidate_is_a_directive_or_a_diagnostic() {
        let src = "// fairlead: def cmp.a\nfn a() {}\n// fairlead: frobnicate xx.y\n// fairlead: use bad\n// fairlead: use cmp.a\nfn b() {}\n// fairlead|  orphan\n";
        let e = ex(src, "rs");
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (5, 2, 3));
        assert_eq!(codes_of(&e), vec!["HN-S001", "HN-S002", "HN-S016"]);
        assert_eq!(
            e.diags.iter().map(|d| d.span.line).collect::<Vec<_>>(),
            vec![3, 4, 7]
        );
        assert_invariant(&e);
    }

    #[test]
    fn r_syn_034__fail_closed_set_drops_the_directive_with_an_error() {
        let mut nine = String::from("// fairlead: use tt.b\n");
        for _ in 0..9 {
            nine.push_str("// fairlead| k=v\n");
        }
        nine.push_str("fn f() {}\n");
        let table: Vec<(String, &str)> = vec![
            (
                "// fairlead: frobnicate tt.b\nfn f() {}\n".into(),
                "HN-S001",
            ),
            ("// fairlead: use Cmp.x\nfn f() {}\n".into(), "HN-S002"),
            (
                "// fairlead: use tt.b why=\"abc\nfn f() {}\n".into(),
                "HN-S007",
            ),
            (
                "// fairlead: ovr rule.x why=\"w\" exp=tomorrow\nfn f() {}\n".into(),
                "HN-S012",
            ),
            (
                "// fairlead: ovr rule.x why=\"w\" until=1.9\nfn f() {}\n".into(),
                "HN-S012",
            ),
            ("fn f() {}\n// fairlead: end\n".into(), "HN-S008"),
            (nine, "HN-S006"),
            ("/// fairlead: use tt.b\nfn f() {}\n".into(), "HN-S013"),
            ("//! fairlead: use tt.b\nfn f() {}\n".into(), "HN-S013"),
            ("/** fairlead: use tt.b */\nfn f() {}\n".into(), "HN-S013"),
            ("// fairlead: use cmp.con\nfn f() {}\n".into(), "HN-P001"),
        ];
        for (src, code) in table {
            let e = ex(&src, "rs");
            assert!(e.directives.is_empty(), "{code}: {e:?}");
            assert_eq!((e.candidates, e.dropped), (1, 1), "{code}");
            assert_eq!(codes_of(&e), vec![code], "{src}");
            assert_eq!(e.diags[0].severity, Severity::Error, "{code}");
            assert!(!e.diags[0].hint.is_empty() && !e.diags[0].message.is_empty());
        }
    }

    #[test]
    fn r_syn_032__diagnostic_carries_position_expected_form_and_suggestion() {
        let e = ex("fn a() {}\n// fairlead: deff cmp.a\nfn b() {}\n", "rs");
        let d = &e.diags[0];
        assert_eq!(d.code, codes::HN_S001);
        assert_eq!((d.span.line, d.span.col), (2, 14), "points at the verb");
        assert_eq!(d.did_you_mean.as_deref(), Some("def"));
        assert_eq!(
            d.expected.as_deref(),
            Some("one of: def use ovr flag env end")
        );
        assert_eq!(
            &"fn a() {}\n// fairlead: deff cmp.a\nfn b() {}\n"
                [d.span.start as usize..d.span.end as usize],
            "deff"
        );
    }

    #[test]
    fn r_syn_034__warn_only_set() {
        let e = ex("// Fairlead: use tt.b\nfn f() {}\n", "rs");
        assert_eq!((e.candidates, e.directives.len()), (0, 0));
        assert_eq!(
            (codes_of(&e), e.diags[0].severity),
            (vec!["HN-S005"], Severity::Warn)
        );
        assert_eq!(e.diags[0].did_you_mean.as_deref(), Some("fairlead:"));
        let e2 = ex("fn f() {}\n// fairlead: use tt.b\n", "rs");
        assert_eq!(
            (codes_of(&e2), e2.diags[0].severity),
            (vec!["HN-S010"], Severity::Warn)
        );
        assert_eq!(
            (e2.candidates, e2.dropped),
            (1, 1),
            "bind-to-nothing is dropped (R-SYN-021)"
        );
        assert_invariant(&e2);
    }

    #[test]
    fn r_syn_004__escaped_marker_is_silent() {
        let e = ex("// fairlead\\: use tt.b\nfn f() {}\n", "rs");
        assert!(e.diags.is_empty() && e.candidates == 0);
    }

    #[test]
    fn r_file_013__marker_inside_a_string_literal_does_not_register() {
        let e = ex(
            "let s = \"// fairlead: use tt.b\";\nlet r = r#\"// fairlead: use uu.d\"#;\n",
            "rs",
        );
        assert_eq!(
            (e.status, e.candidates, e.diags.len()),
            (ScanStatus::Scanned, 0, 0)
        );
    }

    #[test]
    fn r_syn_020__sym_is_a_text_presence_check_inside_the_extent() {
        let ok = ex(
            "x\n\n// fairlead: def cmd.x sym=sync_now\nfn sync_now() {}\n",
            "rs",
        );
        assert!(ok.diags.is_empty(), "{:?}", ok.diags);
        let missing = ex(
            "x\n\n// fairlead: def cmd.x sym=other_name\nfn sync_now() {}\n",
            "rs",
        );
        assert_eq!(codes_of(&missing), vec!["HN-S014"]);
        assert_eq!(missing.diags[0].severity, Severity::Warn);
        assert_eq!(
            missing.directives.len(),
            1,
            "sym never drops or reshapes the directive"
        );
        assert_eq!(extent(&missing, 0), Extent::Lines { first: 4, last: 4 });
    }

    #[test]
    fn r_syn_031__directive_count_explosion_warns_once_and_keeps_everything() {
        let mut c = cfg();
        c.max_directives = 2;
        let src =
            "x\n\n// fairlead: use tt.b\nA\n// fairlead: use tt.c\nB\n// fairlead: use tt.d\nC\n";
        let e = extract(src.as_bytes(), "rs", &c);
        assert_eq!(e.directives.len(), 3);
        assert_eq!(codes_of(&e), vec!["HN-S015"]);
        assert_eq!(e.diags[0].severity, Severity::Warn);
    }

    #[test]
    fn r_syn_007__ordinals_follow_source_order_and_notes_are_data() {
        let src = "x\n\n// fairlead: use tt.b -- first\nA\n// fairlead: use tt.c\nB\n";
        let e = ex(src, "rs");
        assert_eq!(
            e.directives.iter().map(|d| d.ordinal).collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(e.directives[0].note.as_deref(), Some("first"));
        assert_eq!(e.directives[0].carrier, Carrier::LineComment);
    }

    #[test]
    fn r_syn_023__where_a_directive_binds_never_changes_what_it_says() {
        let file = ex("// fairlead: use cmp.a layer=molecule\n\nfn f() {}\n", "rs");
        let standalone = ex(
            "x\n\n// fairlead: use cmp.a layer=molecule\nfn f() {}\n",
            "rs",
        );
        let trailing = ex("fn f() {} // fairlead: use cmp.a layer=molecule\n", "rs");
        assert_eq!(extent(&file, 0), Extent::File);
        assert_eq!(extent(&standalone, 0), Extent::Lines { first: 4, last: 4 });
        assert_eq!(extent(&trailing, 0), Extent::Lines { first: 1, last: 1 });
        for e in [&standalone, &trailing] {
            assert_eq!(
                (e.directives[0].verb, &e.directives[0].ids),
                (file.directives[0].verb, &file.directives[0].ids)
            );
            let strip = |d: &Directive| {
                d.args
                    .iter()
                    .map(|a| (a.key.clone(), a.value.clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(strip(&e.directives[0]), strip(&file.directives[0]));
        }
    }

    #[test]
    fn r_syn_014__foreign_regions_and_env_regions_in_css() {
        let css = "/* fairlead: def tok.primary sym=--primary */\n:root { --primary: #2563eb; }\n.btn { color: var(--primary); } /* fairlead: use tok.primary */\n/* fairlead: env dev +end */\n.debug-grid { outline: 1px solid red; }\n/* fairlead: end */\n/* fairlead: def foreign.shadcn tool=shadcn +end */\n.shadcn-block { color: #111; }\n/* fairlead: end */\n";
        let e = ex(css, "css");
        assert!(e.diags.is_empty(), "{:?}", e.diags);
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (4, 4, 0));
        assert_eq!(extent(&e, 0), Extent::Lines { first: 2, last: 2 });
        assert_eq!(extent(&e, 1), Extent::Lines { first: 3, last: 3 });
        assert_eq!(extent(&e, 2), Extent::Lines { first: 5, last: 5 });
        assert_eq!(e.directives[2].end_span.map(|s| s.line), Some(6));
        assert_eq!(extent(&e, 3), Extent::Lines { first: 8, last: 8 });
        assert_eq!(e.directives[3].ids[0].to_string(), "foreign.shadcn");
        assert_eq!(e.directives[0].carrier, Carrier::BlockComment);
        assert_invariant(&e);
    }

    #[test]
    fn r_arch_033__crlf_and_lf_checkouts_extract_identically() {
        let lf = "// License\n// fairlead: def cmp.a\n\nfn a() {}\n// fairlead: use cmp.a sym=zz\nfn b() {}\n// fairlead: nope\n";
        let crlf = lf.replace('\n', "\r\n");
        let (a, b) = (ex(lf, "rs"), ex(&crlf, "rs"));
        let shape = |e: &Extraction| {
            (
                e.directives
                    .iter()
                    .map(|d| {
                        (
                            d.ordinal,
                            d.verb,
                            d.ids.clone(),
                            d.extent,
                            d.span.line,
                            d.span.col,
                        )
                    })
                    .collect::<Vec<_>>(),
                e.diags
                    .iter()
                    .map(|d| (d.code, d.span.line, d.span.col, d.message.clone()))
                    .collect::<Vec<_>>(),
                (e.candidates, e.dropped),
            )
        };
        assert_eq!(shape(&a), shape(&b));
    }

    #[test]
    fn r_arch_033__offsets_refer_to_the_original_bytes() {
        let src = "// x\r\n// fairlead: def cmp.a\r\n\r\nfn a() {}\r\n";
        let e = ex(src, "rs");
        let d = &e.directives[0];
        assert_eq!(
            &src.as_bytes()[d.span.start as usize..d.span.start as usize + 9],
            b"fairlead:"
        );
        assert_eq!(
            &src[d.span.start as usize..d.span.end as usize],
            "fairlead: def cmp.a"
        );
        assert_eq!((d.span.line, d.span.col), (2, 4));
        let bom = ex("\u{feff}// fairlead: def cmp.a\n\nfn a() {}\n", "rs");
        assert_eq!(
            bom.directives[0].span.start, 6,
            "BOM bytes count in original offsets"
        );
    }

    #[test]
    fn r_syn_016__file_form_through_extract() {
        let e = ex(
            "#!/usr/bin/env python\n# fairlead: use tt.b\n\nimport os\n",
            "py",
        );
        assert_eq!(extent(&e, 0), Extent::File);
    }

    #[test]
    fn r_syn_006__jsx_child_carrier_binds_to_the_next_element() {
        let e = ex(
            "<Button>\n  {/* fairlead: use tok.surface */}\n  <Icon />\n</Button>\n",
            "tsx",
        );
        assert_eq!(extent(&e, 0), Extent::Lines { first: 3, last: 3 });
        // With other code on the line it stays trailing.
        let e = ex(
            "<Icon /> {/* fairlead: use tok.surface */}\n<Next />\n",
            "tsx",
        );
        assert_eq!(extent(&e, 0), Extent::Lines { first: 1, last: 1 });
    }

    #[test]
    fn r_syn_044__blanks_around_the_leader_and_marker_are_tolerated() {
        let e = ex(
            "x

	// 	fairlead:	use tok.a	 
fn f() {}
",
            "rs",
        );
        assert_eq!(e.directives.len(), 1, "{:?}", e.diags);
        assert_eq!(e.directives[0].ids[0].to_string(), "tok.a");
        assert_eq!(extent(&e, 0), Extent::Lines { first: 4, last: 4 });
    }

    #[test]
    fn r_syn_035__extraction_is_pure_and_repeatable() {
        let src = "x

// fairlead: use tok.a
fn f() {}
// fairlead: frobnicate
";
        assert_eq!(ex(src, "rs"), ex(src, "rs"));
    }

    #[test]
    fn r_syn_036__frontmatter_directives_bind_to_the_doc() {
        let src = "+++\ntitle = \"T\"\ndirectives = [\n  'def tok.primary applies=color',\n  'use rule.no-raw-hex',\n  'end',\n  'frobnicate xx.y',\n]\n+++\n\n# Colour\n<!-- fairlead: use tok.primary -- example only -->\nSome text\n";
        let e = ex(src, "md");
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (5, 3, 2));
        assert_eq!(codes_of(&e), vec!["HN-S008", "HN-S001"]);
        let fm = &e.directives[0];
        assert_eq!(
            (fm.carrier, fm.extent, fm.ordinal),
            (Carrier::Frontmatter, Extent::Doc, 0)
        );
        assert_eq!((fm.span.line, fm.span.col), (4, 4));
        assert_eq!(e.directives[1].ids[0].to_string(), "rule.no-raw-hex");
        let body = &e.directives[2];
        assert_eq!(
            (body.carrier, body.extent, body.note.as_deref()),
            (
                Carrier::HtmlComment,
                Extent::Lines {
                    first: 13,
                    last: 13
                },
                Some("example only")
            )
        );
        assert!(e.frontmatter.as_ref().unwrap().table.contains_key("title"));
        assert_eq!(e.frontmatter.as_ref().unwrap().directive_bodies.len(), 4);
        assert_invariant(&e);
    }

    #[test]
    fn r_syn_037__bad_frontmatter_is_s004_and_loses_no_body_comment() {
        let src = "+++\ndirectives = 5\n+++\n\nText\n<!-- fairlead: use tt.b -->\nMore\n";
        let e = ex(src, "md");
        assert_eq!(codes_of(&e), vec!["HN-S004"]);
        assert_eq!(e.diags[0].severity, Severity::Error);
        assert!(e.frontmatter.is_none());
        assert_eq!(e.directives.len(), 1, "the body comment still extracts");
        let e2 = ex("+++\ntitle = \"x\"\n", "md");
        assert_eq!(codes_of(&e2), vec!["HN-S004"], "unterminated frontmatter");
    }

    #[test]
    fn r_syn_036__frontmatter_without_a_marker_anywhere_is_still_scanned() {
        let e = ex("+++\ndirectives = ['use tt.b']\n+++\n# T\n", "md");
        assert_eq!((e.status, e.directives.len()), (ScanStatus::Scanned, 1));
    }

    #[test]
    fn r_syn_019__extents_are_meaningless_in_frontmatter() {
        let e = ex(
            "+++\ndirectives = ['use tt.b +3', 'env dev +end']\n+++\n",
            "md",
        );
        assert_eq!((e.candidates, e.directives.len(), e.dropped), (2, 0, 2));
        assert_eq!(codes_of(&e), vec!["HN-S016", "HN-S016"]);
    }

    #[test]
    fn r_syn_005__markdown_fenced_comments_do_not_register_through_extract() {
        let src = "# T\n\n```md\n<!-- fairlead: use no.fence -->\n```\n\n<!-- fairlead: use yes.one -->\ntext\n";
        let e = ex(src, "md");
        assert_eq!(e.directives.len(), 1);
        assert_eq!(e.directives[0].ids[0].to_string(), "yes.one");
        assert_eq!(extent(&e, 0), Extent::Lines { first: 8, last: 8 });
    }

    #[test]
    fn r_syn_005__comments_inside_frontmatter_strings_are_not_body_comments() {
        let src = "+++\ntitle = \"<!-- fairlead: use no.fm -->\"\n+++\n\ntext\n";
        let e = ex(src, "md");
        assert!(e.directives.is_empty() && e.candidates == 0 && e.diags.is_empty());
    }
}
