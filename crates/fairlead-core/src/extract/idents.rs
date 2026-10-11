//! Identifier evidence for `sym=` (R-HIER-030, R-HIER-031): the whole identifiers of a file,
//! comments excluded, kept as 64-bit hashes so a large tree costs 8 bytes per distinct name.
//! Collected only when the root's `[hier] level` is 2 or more.

/// `sym` must be at least this long (R-HIER-030); shorter names are never looked up.
pub const MIN_SYM: usize = 6;

/// FNV-1a, 64 bits: stable across platforms and runs, so cached hashes stay valid.
pub fn ident_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Sorted, distinct hashes of the identifiers of `text` that are at least `MIN_SYM` long, start
/// with a letter, `_` or `$`, and lie outside the comments of `ext`.
pub fn ident_hashes(text: &str, ext: &str) -> Vec<u64> {
    let comments = fairlead_syntax::lexer::comment_spans(text, ext);
    // The spans are in order: a binary search per identifier.
    let in_comment = |i: usize| {
        let k = comments.partition_point(|r| r.end <= i);
        comments.get(k).is_some_and(|r| r.start <= i)
    };
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let flush = |a: usize, b: usize, out: &mut Vec<u64>| {
        let word = text.get(a..b).unwrap_or("");
        if word.len() >= MIN_SYM
            && word.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
            && !in_comment(a)
        {
            out.push(ident_hash(word));
        }
    };
    for (i, c) in text.char_indices() {
        match (is_ident_char(c), start) {
            (true, None) => start = Some(i),
            (false, Some(a)) => {
                flush(a, i, &mut out);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        flush(a, text.len(), &mut out);
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether a file's identifier hashes contain `sym`.
pub fn mentions(hashes: &[u64], sym: &str) -> bool {
    hashes.binary_search(&ident_hash(sym)).is_ok()
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_hier_031__match_in_comment_ignored() {
        let h = ident_hashes(
            "// verify_manifest is described here\nfn run() { transfer_batch(); }\n",
            "rs",
        );
        assert!(mentions(&h, "transfer_batch"));
        assert!(!mentions(&h, "verify_manifest"), "comments are excluded");
    }

    #[test]
    fn r_hier_031__a_match_is_a_whole_identifier() {
        let h = ident_hashes("let x = verify_manifest_v2();\n", "ts");
        assert!(!mentions(&h, "verify_manifest"), "not a substring");
        assert!(mentions(&h, "verify_manifest_v2"));
        assert!(
            !mentions(&ident_hashes("let short = 1;\n", "ts"), "short"),
            "below 6 characters"
        );
    }
}
