//! Generated-file header: `# scope <id> edition=<e> body=<hash>` (C-17, R-FILE-003,
//! R-CHK-024). The hash is blake3 over the LF-normalised body (D-50, R-ARCH-076).

use fairlead_syntax::normalize::lf_bytes;

use super::HeaderVerdict;
use crate::hash::{base32, hash_normalised};

pub const BODY_HASH_CHARS: usize = 20;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HeaderKind {
    /// `# scope <id> ...`
    Scope(String),
    /// `# baseline ...`
    Baseline,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Header {
    pub kind: HeaderKind,
    pub edition: u32,
    pub body: String,
}

/// 100 bits of blake3 over the LF-normalised body as 20 base32 characters.
pub fn body_hash(body: &str) -> String {
    base32(&hash_normalised(body.as_bytes()).0, BODY_HASH_CHARS)
}

pub fn header_line(kind: &HeaderKind, edition: u32, body: &str) -> String {
    let hash = body_hash(body);
    match kind {
        HeaderKind::Scope(id) => format!("# scope {id} edition={edition} body={hash}"),
        HeaderKind::Baseline => format!("# baseline edition={edition} body={hash}"),
    }
}

/// Header line, LF, then the body with every line LF-terminated. The body is normalised first
/// (CR removed, one trailing newline) so the bytes never depend on the platform (R-ARCH-112).
pub fn render_file(kind: &HeaderKind, edition: u32, body: &str) -> String {
    let mut body = body.replace("\r\n", "\n").replace('\r', "\n");
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    let mut out = header_line(kind, edition, &body);
    out.push('\n');
    out.push_str(&body);
    out
}

pub fn parse_header(line: &str) -> Option<Header> {
    let (kind, rest) = if let Some(r) = line.strip_prefix("# scope ") {
        let (id, rest) = r.split_once(' ')?;
        (HeaderKind::Scope(id.to_string()), rest)
    } else {
        let r = line.strip_prefix("# baseline ")?;
        (HeaderKind::Baseline, r)
    };
    let mut parts = rest.split(' ');
    let edition = parts
        .next()?
        .strip_prefix("edition=")?
        .parse::<u32>()
        .ok()?;
    let body = parts.next()?.strip_prefix("body=")?.to_string();
    if parts.next().is_some() || body.chars().count() != BODY_HASH_CHARS {
        return None;
    }
    Some(Header {
        kind,
        edition,
        body,
    })
}

/// Both a start and an end marker line are required, so a lone `=======` is not a conflict.
pub fn has_conflict_markers(text: &str) -> bool {
    let mut start = false;
    let mut end = false;
    for line in text.lines() {
        start |= line.starts_with("<<<<<<<");
        end |= line.starts_with(">>>>>>>");
    }
    start && end
}

/// R-CHK-024 / R-FILE-007. Conflict markers win over a hash mismatch (they are regenerable).
pub fn verify_header(bytes: &[u8]) -> HeaderVerdict {
    let norm = lf_bytes(bytes);
    let Ok(text) = std::str::from_utf8(&norm) else {
        return HeaderVerdict::HandEdited;
    };
    if has_conflict_markers(text) {
        return HeaderVerdict::HasConflictMarkers;
    }
    let Some((first, body)) = text.split_once('\n') else {
        return HeaderVerdict::HandEdited;
    };
    match parse_header(first) {
        Some(h) if h.body == body_hash(body) => HeaderVerdict::Ok,
        _ => HeaderVerdict::HandEdited,
    }
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

    const BODY: &str = "tok.a pol=open\ntok.b pol=open\n";

    fn web() -> HeaderKind {
        HeaderKind::Scope("web".to_string())
    }

    #[test]
    fn r_chk_024__body_hash_is_20_base32_chars_and_ignores_cr() {
        let h = body_hash("a\n");
        assert_eq!(h.chars().count(), 20);
        assert!(
            h.chars()
                .all(|c| "0123456789abcdefghjkmnpqrstvwxyz".contains(c))
        );
        assert_eq!(h, body_hash("a\r\n"));
        assert_ne!(h, body_hash("b\n"));
    }

    #[test]
    fn r_file_003__header_line_layout_and_roundtrip() {
        let text = render_file(&web(), 1, BODY);
        let (first, rest) = text.split_once('\n').unwrap();
        assert_eq!(rest, BODY);
        assert_eq!(
            first,
            format!("# scope web edition=1 body={}", body_hash(BODY))
        );
        assert_eq!(
            parse_header(first),
            Some(Header {
                kind: web(),
                edition: 1,
                body: body_hash(BODY)
            })
        );
        let base = render_file(&HeaderKind::Baseline, 1, "");
        assert!(base.starts_with("# baseline edition=1 body="));
        assert!(base.ends_with('\n') && base.matches('\n').count() == 1);
    }

    #[test]
    fn r_arch_091__render_ends_with_one_newline_and_has_no_cr() {
        let text = render_file(&web(), 1, "a b\r\nc d");
        assert!(text.ends_with("c d\n") && !text.contains('\r'));
        assert!(!text.ends_with("\n\n"));
    }

    #[test]
    fn r_chk_024__untouched_file_verifies_ok() {
        let text = render_file(&web(), 1, BODY);
        assert_eq!(verify_header(text.as_bytes()), HeaderVerdict::Ok);
        assert_eq!(
            verify_header(render_file(&HeaderKind::Baseline, 1, "").as_bytes()),
            HeaderVerdict::Ok
        );
    }

    #[test]
    fn r_file_007__a_hand_edited_line_is_detected() {
        let text = render_file(&web(), 1, BODY).replace("tok.b", "tok.c");
        assert_eq!(verify_header(text.as_bytes()), HeaderVerdict::HandEdited);
        let appended = format!("{}tok.z pol=open\n", render_file(&web(), 1, BODY));
        assert_eq!(
            verify_header(appended.as_bytes()),
            HeaderVerdict::HandEdited
        );
    }

    #[test]
    fn r_file_007__missing_or_garbled_header_counts_as_hand_edited() {
        assert_eq!(verify_header(b""), HeaderVerdict::HandEdited);
        assert_eq!(
            verify_header(b"tok.a pol=open\n"),
            HeaderVerdict::HandEdited
        );
        assert_eq!(
            verify_header(b"# scope web edition=x body=aaaaaaaaaaaaaaaaaaaa\n"),
            HeaderVerdict::HandEdited
        );
        assert_eq!(
            verify_header(&[0xff, 0xfe, 0x00]),
            HeaderVerdict::HandEdited
        );
        let no_newline = format!("# scope web edition=1 body={}", body_hash(""));
        assert_eq!(
            verify_header(no_newline.as_bytes()),
            HeaderVerdict::HandEdited
        );
    }

    #[test]
    fn r_chk_024__conflict_markers_are_regenerable() {
        let good = render_file(&web(), 1, BODY);
        let conflicted = format!("<<<<<<< HEAD\n{good}=======\n{good}>>>>>>> branch\n");
        assert_eq!(
            verify_header(conflicted.as_bytes()),
            HeaderVerdict::HasConflictMarkers
        );
        assert!(!has_conflict_markers(
            "a line with ======= alone\n=======\n"
        ));
    }

    #[test]
    fn r_file_024__crlf_checkout_and_bom_still_verify() {
        let text = render_file(&web(), 1, BODY).replace('\n', "\r\n");
        assert_eq!(verify_header(text.as_bytes()), HeaderVerdict::Ok);
        let with_bom = format!("\u{feff}{}", render_file(&web(), 1, BODY));
        assert_eq!(verify_header(with_bom.as_bytes()), HeaderVerdict::Ok);
    }
}
