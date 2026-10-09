//! Line-ending, BOM and tab normalisation with an offset map back to the original bytes
//! (R-ARCH-033, R-SYN-007).
use crate::diag::Span;
use std::borrow::Cow;
use unicode_normalization::UnicodeNormalization;

/// Panic-free `&s[a..b]`; empty when out of range or not on char boundaries.
pub fn slice(s: &str, a: usize, b: usize) -> &str {
    s.get(a..b).unwrap_or("")
}

pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// BOM stripped, CRLF and lone CR to LF (R-ARCH-076). Works on arbitrary bytes.
pub fn lf_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if body.len() == bytes.len() && !body.contains(&b'\r') {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(body.len());
    let mut it = body.iter().peekable();
    while let Some(&b) = it.next() {
        if b == b'\r' {
            if it.peek() == Some(&&b'\n') {
                it.next();
            }
            out.push(b'\n');
        } else {
            out.push(b);
        }
    }
    Cow::Owned(out)
}

/// NFC for semantic strings (quoted values, notes). R-SYN-007.
pub fn nfc(s: &str) -> String {
    if s.is_ascii() {
        s.to_owned()
    } else {
        s.nfc().collect()
    }
}

/// Tabs to one space outside double quotes (R-SYN-007). Length-preserving, so offsets stay valid.
pub fn tabs_to_spaces_outside_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let (mut in_q, mut esc) = (false, false);
    for c in s.chars() {
        if in_q {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_q = false;
            }
            out.push(c);
        } else if c == '"' {
            in_q = true;
            out.push(c);
        } else if c == '\t' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// LF-only text plus the maps needed to report original-byte spans.
#[derive(Clone, Debug)]
pub struct Normalized {
    pub text: String,
    /// `orig[i]` = original byte offset of normalized byte `i`; one extra entry for the end.
    orig: Vec<u32>,
    /// Byte offset of the start of each line in `text`.
    starts: Vec<usize>,
}

impl Normalized {
    pub fn new(src: &str) -> Normalized {
        let body = src.strip_prefix('\u{feff}').unwrap_or(src);
        let bom = src.len() - body.len();
        let mut text = String::with_capacity(body.len());
        let mut orig: Vec<u32> = Vec::with_capacity(body.len() + 1);
        let mut it = body.char_indices().peekable();
        while let Some((i, c)) = it.next() {
            let o = to_u32(bom + i);
            if c == '\r' {
                if matches!(it.peek(), Some((_, '\n'))) {
                    it.next();
                }
                text.push('\n');
                orig.push(o);
            } else {
                text.push(c);
                for k in 0..c.len_utf8() {
                    orig.push(o.saturating_add(to_u32(k)));
                }
            }
        }
        orig.push(to_u32(src.len()));
        let mut starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        Normalized { text, orig, starts }
    }

    pub fn orig_offset(&self, off: usize) -> u32 {
        self.orig
            .get(off)
            .copied()
            .or_else(|| self.orig.last().copied())
            .unwrap_or(0)
    }

    /// 1-based line holding byte `off` (offsets past the end map to the last line).
    pub fn line_of(&self, off: usize) -> u32 {
        to_u32(self.starts.partition_point(|&s| s <= off).max(1))
    }

    /// Number of lines; a trailing newline does not open another line.
    pub fn line_count(&self) -> u32 {
        let n = if self.text.ends_with('\n') {
            self.starts.len() - 1
        } else {
            self.starts.len()
        };
        to_u32(n.max(1))
    }

    pub fn line_start(&self, line: u32) -> usize {
        let i = (line as usize).saturating_sub(1);
        self.starts.get(i).copied().unwrap_or(self.text.len())
    }

    /// End of the line, excluding the `\n`.
    pub fn line_end(&self, line: u32) -> usize {
        match self.starts.get(line as usize) {
            Some(&next) => next.saturating_sub(1),
            None => self.text.len(),
        }
    }

    pub fn line_text(&self, line: u32) -> &str {
        if line == 0 || line > self.line_count() {
            return "";
        }
        slice(&self.text, self.line_start(line), self.line_end(line))
    }

    /// Original-byte span of the normalized range `a..b`, with 1-based line and scalar column of `a`.
    pub fn span(&self, a: usize, b: usize) -> Span {
        let line = self.line_of(a);
        let col = slice(&self.text, self.line_start(line), a).chars().count() + 1;
        Span {
            start: self.orig_offset(a),
            end: self.orig_offset(b.max(a)),
            line,
            col: to_u32(col),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn r_arch_033__crlf_and_lone_cr_become_lf_with_original_offsets() {
        let n = Normalized::new("a\r\nb\rc");
        assert_eq!(n.text, "a\nb\nc");
        assert_eq!(n.orig_offset(0), 0);
        assert_eq!(n.orig_offset(1), 1, "the LF of a CRLF maps to the CR");
        assert_eq!(n.orig_offset(2), 3);
        assert_eq!(n.orig_offset(3), 4, "a lone CR maps to itself");
        assert_eq!(n.orig_offset(4), 5);
        assert_eq!(
            n.orig_offset(5),
            6,
            "one past the end maps to the original length"
        );
        assert_eq!(n.line_count(), 3);
    }

    #[test]
    fn r_syn_007__bom_is_stripped_and_offsets_stay_original() {
        let n = Normalized::new("\u{feff}x\n");
        assert_eq!(n.text, "x\n");
        assert_eq!(n.orig_offset(0), 3);
    }

    #[test]
    fn r_arch_033__span_columns_count_scalar_values() {
        let n = Normalized::new("\u{e9} = 1\r\n// x");
        let eq = n.text.find('=').unwrap();
        let sp = n.span(eq, eq + 1);
        assert_eq!((sp.line, sp.col, sp.start, sp.end), (1, 3, 3, 4));
        let slash = n.text.find("//").unwrap();
        let sp2 = n.span(slash, slash + 2);
        assert_eq!((sp2.line, sp2.col), (2, 1));
        assert_eq!(sp2.start, 8, "e-acute 2 + ' = 1' 4 + CRLF 2 original bytes");
    }

    #[test]
    fn r_arch_033__line_helpers() {
        let n = Normalized::new("one\ntwo\n");
        assert_eq!(
            n.line_count(),
            2,
            "a trailing newline does not open a third line"
        );
        assert_eq!(n.line_text(1), "one");
        assert_eq!(n.line_text(2), "two");
        assert_eq!(n.line_text(3), "");
        assert_eq!(n.line_of(4), 2);
        assert_eq!(Normalized::new("").line_count(), 1);
    }

    #[test]
    fn r_arch_033__lf_bytes_strips_bom_and_cr() {
        assert_eq!(&*lf_bytes(b"\xef\xbb\xbfa\r\nb\rc"), b"a\nb\nc");
        assert!(matches!(lf_bytes(b"plain\n"), Cow::Borrowed(_)));
        assert_eq!(
            &*lf_bytes(b"\xff\r\n"),
            b"\xff\n",
            "non-UTF-8 bytes pass through"
        );
    }

    #[test]
    fn r_syn_007__nfc_composes_and_ascii_is_untouched() {
        assert_eq!(nfc("e\u{301}"), "\u{e9}");
        assert_eq!(nfc("plain"), "plain");
    }

    #[test]
    fn r_syn_007__tabs_outside_quotes_become_one_space() {
        assert_eq!(
            tabs_to_spaces_outside_quotes("a\t\"b\tc\"\td"),
            "a \"b\tc\" d"
        );
        assert_eq!(
            tabs_to_spaces_outside_quotes("\"a\\\"\tb\"\tc"),
            "\"a\\\"\tb\" c"
        );
    }

    #[test]
    fn r_arch_032__slice_never_panics() {
        assert_eq!(slice("h\u{e9}llo", 1, 2), "", "not a char boundary");
        assert_eq!(slice("abc", 2, 9), "");
        assert_eq!(slice("abc", 1, 3), "bc");
    }
}
