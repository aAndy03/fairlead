//! Generated-file model and generator (P5b).
pub mod header;
pub mod members;
pub mod shard;

pub use header::verify_header;

/// Result of the header check of a generated file (R-CHK-024).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderVerdict {
    Ok,
    /// Header hash does not match the body, or the header is missing or garbled: `HN-G002`.
    HandEdited,
    /// Git conflict markers: the file is regenerable without `--force` (R-CLI-026).
    HasConflictMarkers,
}

use std::collections::BTreeMap;

use fairlead_syntax::ids::Kind;
use fairlead_syntax::normalize::lf_bytes;

use crate::discover::Edition;
use crate::path::RepoPath;
use crate::registry::ScopeId;
use crate::render::sanitize::sanitize_data;

/// What a generated file holds (C-8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GenKind {
    Shard(Kind),
    Exceptions,
    Members,
    Baseline,
    VendorLock,
}

/// One generated file. `body` is the sorted terse lines, each ending in `\n`, without the header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GeneratedFile {
    pub path: RepoPath,
    pub scope: ScopeId,
    pub kind: GenKind,
    pub body: String,
}

impl GeneratedFile {
    /// Header line + body, LF-only, byte-exact (R-FILE-003, R-ARCH-091).
    pub fn render(&self, edition: Edition) -> Vec<u8> {
        let kind = match self.kind {
            GenKind::Baseline => header::HeaderKind::Baseline,
            _ => header::HeaderKind::Scope(self.scope.0.as_str().to_string()),
        };
        header::render_file(&kind, edition.0, &self.body).into_bytes()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GeneratedSet {
    pub files: BTreeMap<RepoPath, GeneratedFile>,
}

/// R-CHK-023: the first `n` differing lines between what `sync` would write (`want`) and what is
/// on disk (`have`), as `+<line>` (missing on disk) and `-<line>` (extra on disk). LF-normalised
/// first, so a CRLF checkout is not a difference; each line is sanitised and cut at 120 characters.
pub fn first_differing_lines(want: &[u8], have: &[u8], n: usize) -> Vec<String> {
    let want = String::from_utf8_lossy(&lf_bytes(want)).into_owned();
    let have = String::from_utf8_lossy(&lf_bytes(have)).into_owned();
    let diff = similar::TextDiff::from_lines(have.as_str(), want.as_str());
    let mut out = Vec::new();
    for change in diff.iter_all_changes() {
        if out.len() >= n {
            break;
        }
        let sign = match change.tag() {
            similar::ChangeTag::Delete => '-',
            similar::ChangeTag::Insert => '+',
            similar::ChangeTag::Equal => continue,
        };
        out.push(format!("{sign}{}", sanitize_data(change.value(), 120)));
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
    use super::*;
    use fairlead_syntax::ids::ScopeSlug;

    fn root() -> ScopeId {
        ScopeId(ScopeSlug::parse("root").unwrap())
    }

    fn file(kind: GenKind, body: &str) -> GeneratedFile {
        GeneratedFile { path: RepoPath::new("fairlead/gen/members").unwrap(), scope: root(), kind, body: body.to_string() }
    }

    #[test]
    fn r_file_003__scope_header_then_sorted_body() {
        let f = file(GenKind::Members, "self.root dir=. up=- chain=root\n");
        let text = String::from_utf8(f.render(Edition(1))).unwrap();
        assert!(text.starts_with("# scope root edition=1 body="));
        assert!(text.ends_with("self.root dir=. up=- chain=root\n"));
        assert_eq!(verify_header(text.as_bytes()), HeaderVerdict::Ok);
    }

    #[test]
    fn r_chk_030__baseline_kind_uses_the_baseline_header() {
        let text = String::from_utf8(file(GenKind::Baseline, "").render(Edition(1))).unwrap();
        assert!(text.starts_with("# baseline edition=1 body="));
    }

    #[test]
    fn r_arch_112__render_is_lf_only_whatever_the_body_holds() {
        let bytes = file(GenKind::Members, "a b\r\nc d\r\n").render(Edition(1));
        assert!(!bytes.contains(&b'\r'));
        assert_eq!(bytes.last(), Some(&b'\n'));
    }

    #[test]
    fn r_chk_023__first_differing_lines_reports_at_most_n() {
        let want = b"a\nb\nc\nd\ne\n";
        let have = b"a\nx\nc\ny\nz\n";
        let diff = first_differing_lines(want, have, 3);
        assert_eq!(diff.len(), 3);
        assert!(diff.iter().any(|l| l == "-x"), "{diff:?}");
        assert!(diff.iter().any(|l| l == "+b"), "{diff:?}");
    }

    #[test]
    fn r_chk_023__pure_additions_and_identical_inputs() {
        assert_eq!(first_differing_lines(b"a\nb\n", b"a\n", 3), vec!["+b".to_string()]);
        assert!(first_differing_lines(b"a\nb\n", b"a\nb\n", 3).is_empty());
        assert!(first_differing_lines(b"a\r\nb\r\n", b"a\nb\n", 3).is_empty(), "a CRLF checkout is not a difference");
    }

    #[test]
    fn r_chk_023__differing_text_is_sanitised() {
        let diff = first_differing_lines(b"a\n", b"\x1b[31ma\n", 3);
        assert!(diff.iter().all(|l| !l.contains('\u{1b}')), "{diff:?}");
    }
}
