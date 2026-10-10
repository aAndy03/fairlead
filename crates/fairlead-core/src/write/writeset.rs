//! Write sets, validation and text splice (R-ARCH-141, R-ARCH-144, R-CLI-016).
use super::refuse;
use crate::classify::FileClass;
use crate::discover::Root;
use crate::error::{Error, RefusalKind};
use crate::hash::ContentHash;
use crate::path::RepoPath;
use crate::secrets::scan_secrets;
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WriteKind {
    Create,
    Replace,
    AppendBlock {
        marker: String,
    },
    /// Remove the file (seal's ordinal rename: the draft-named doc goes, R-VER-041). `bytes` is empty.
    Delete,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlannedWrite {
    pub path: RepoPath,
    pub kind: WriteKind,
    pub bytes: Vec<u8>,
    pub class: FileClass,
    pub expect_prior: Option<ContentHash>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum WriteSetKind {
    Init,
    Sync,
    New,
    Approve,
    Seal,
    /// R-EDN-032: the root control file (renamed last) and generated files.
    Upgrade,
    /// R-EDN-007: marker and folder rename; directive lines anywhere and managerial files move.
    Rename,
    /// R-CLI-040: `move` relocates any file and rewrites fairlead's own text.
    Move,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WriteSet {
    pub kind: WriteSetKind,
    pub writes: BTreeMap<RepoPath, PlannedWrite>,
    /// Applied after every other write: seal's version file (R-VER-050 step 6, D-14).
    pub commit_point: Option<RepoPath>,
}

impl WriteSet {
    pub fn new(kind: WriteSetKind) -> Self {
        WriteSet {
            kind,
            writes: BTreeMap::new(),
            commit_point: None,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }

    pub fn add(&mut self, w: PlannedWrite) -> Result<(), Error> {
        let key = w.path.fold_key();
        if self.writes.keys().any(|p| p.fold_key() == key) {
            return Err(Error::Internal(format!(
                "path planned twice: {}",
                w.path.as_str()
            )));
        }
        self.writes.insert(w.path.clone(), w);
        Ok(())
    }

    /// Path allow-list per verb, LF-only bytes, secret scan (R-ARCH-141, R-ARCH-144, R-ARCH-112).
    pub fn validate(&self, root: &Root) -> Result<(), Error> {
        for w in self.writes.values() {
            if !allowed(self.kind, w, &root.folder) {
                return Err(Error::Internal(format!(
                    "{:?} may not write {}",
                    self.kind,
                    w.path.as_str()
                )));
            }
            check_bytes(w, &root.folder)?;
        }
        Ok(())
    }
}

fn allowed(kind: WriteSetKind, w: &PlannedWrite, folder: &str) -> bool {
    let p = w.path.as_str();
    let segs: Vec<&str> = p.split('/').collect();
    if segs.contains(&".git") {
        return kind == WriteSetKind::Init && p == ".git/hooks/pre-commit";
    }
    let name = segs.last().copied().unwrap_or("");
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    let fpos = segs.iter().position(|s| *s == folder);
    let after: Vec<&str> = fpos
        .map(|i| segs.iter().skip(i + 1).copied().collect())
        .unwrap_or_default();
    match kind {
        WriteSetKind::Sync => match (fpos, after.as_slice()) {
            (Some(_), ["gen", ..]) | (Some(_), ["vendored.lock"]) => {
                w.class == FileClass::Generated
            }
            (Some(_), ["tombstones", _]) => w.class == FileClass::Authored, // plan_demote (C-9), R-CLI-046
            (None, _) => {
                segs.len() == 3
                    && segs.starts_with(&[".fairlead", "quarantine"])
                    && w.kind == WriteKind::Create
            }
            _ => false,
        },
        WriteSetKind::New => {
            fpos.is_some()
                && !matches!(
                    after.as_slice(),
                    ["gen", ..] | ["approvals.toml"] | ["vendored.lock"] | ["tombstones", ..]
                )
        }
        WriteSetKind::Approve => fpos == Some(0) && after.as_slice() == ["approvals.toml"],
        // R-VER-059: ledger shards, generated files, sealed docs (rewritten or renamed in
        // place) and the root's version file; nothing else.
        WriteSetKind::Seal => match (fpos, after.as_slice()) {
            (Some(_), ["gen", ..]) => w.class == FileClass::Generated,
            (Some(_), ["ledger", "hist", _]) => w.class == FileClass::Ledger,
            (Some(_), ["tombstones", _]) => w.class == FileClass::Authored && ext == "toml",
            (Some(_), ["docs", ..]) => w.class == FileClass::Authored && ext == "md",
            (Some(0), ["versions", _]) => w.class == FileClass::Authored && ext == "md",
            _ => false,
        },
        WriteSetKind::Rename | WriteSetKind::Move => !segs.starts_with(&[".fairlead"]),
        WriteSetKind::Upgrade => match (fpos, after.as_slice()) {
            (Some(_), ["gen", ..]) => w.class == FileClass::Generated,
            (Some(0), ["control.toml"]) => w.class == FileClass::Authored,
            _ => false,
        },
        WriteSetKind::Init => {
            matches!(
                (fpos, after.as_slice()),
                (Some(0), ["control.toml"]) | (Some(0), ["gen", "baseline.txt"])
            ) || matches!(
                p,
                ".gitignore"
                    | ".gitattributes"
                    | "CODEOWNERS"
                    | ".github/CODEOWNERS"
                    | "docs/CODEOWNERS"
                    | ".claude/settings.json"
            ) || (matches!(ext, "md" | "mdc" | "txt") && w.kind != WriteKind::Replace)
        }
    }
}

fn check_bytes(w: &PlannedWrite, folder: &str) -> Result<(), Error> {
    let p = w.path.as_str();
    if p.starts_with(".fairlead/quarantine/") {
        return Ok(()); // verbatim copy of an existing hand-edited file; disposable cache
    }
    if w.bytes.contains(&b'\r') {
        return Err(Error::Internal(format!(
            "{p}: planned bytes contain CR (LF-only, R-ARCH-112)"
        )));
    }
    if p.rsplit('/').next() == Some("control.toml")
        && w.kind != WriteKind::Delete
        && w.bytes.iter().all(u8::is_ascii_whitespace)
    {
        return Err(Error::Internal(format!(
            "{p}: tools never create an empty control file (R-SCOPE-050)"
        )));
    }
    match &w.kind {
        WriteKind::Delete if !w.bytes.is_empty() => {
            return Err(Error::Internal(format!("{p}: a delete carries no bytes")));
        }
        WriteKind::Create if w.expect_prior.is_some() => {
            return Err(Error::Internal(format!(
                "{p}: Create cannot carry expect_prior"
            )));
        }
        WriteKind::AppendBlock { marker } if !marker.is_empty() => {
            let text = String::from_utf8_lossy(&w.bytes);
            let begins = text
                .lines()
                .filter(|l| tag_match(l, "BEGIN", marker))
                .count();
            let ends = text.lines().filter(|l| tag_match(l, "END", marker)).count();
            if begins != 1 || ends != 1 {
                return Err(Error::Internal(format!(
                    "{p}: block must hold exactly one BEGIN and one END {marker}"
                )));
            }
        }
        _ => {}
    }
    if p.split('/').any(|s| s == folder) {
        let text = String::from_utf8_lossy(&w.bytes);
        if let Some(hit) = scan_secrets(&text).first() {
            return Err(refuse(
                RefusalKind::Floor,
                format!(
                    "HN-F001 secret-like value (class {}) at {p}:{}",
                    hit.class, hit.span.line
                ),
                "declare secrets by environment-variable name only, for example value=env:API_KEY",
            ));
        }
    }
    Ok(())
}

// ---- text splice (pure) ----------------------------------------------------------------------

struct Line {
    start: usize,
    end: usize,
}

fn sl(b: &[u8], a: usize, z: usize) -> &[u8] {
    b.get(a..z).unwrap_or_default()
}

fn split_lines(b: &[u8]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < b.len() {
        let rest = sl(b, start, b.len());
        let (raw_end, next) = match rest.iter().position(|&c| c == b'\n') {
            Some(i) => (start + i, start + i + 1),
            None => (b.len(), b.len()),
        };
        let end = if raw_end > start && b.get(raw_end - 1) == Some(&b'\r') {
            raw_end - 1
        } else {
            raw_end
        };
        out.push(Line { start, end });
        start = next;
    }
    out
}

/// `CRLF` when CRLF line ends are at least half of all line ends, else `LF` (R-CLI-016).
pub fn detect_eol(b: &[u8]) -> &'static str {
    let crlf = b.windows(2).filter(|w| *w == *b"\r\n").count();
    let lf = b.iter().filter(|&&c| c == b'\n').count();
    if crlf > 0 && crlf * 2 >= lf {
        "\r\n"
    } else {
        "\n"
    }
}

/// `tag` ("BEGIN"/"END") followed by exactly `marker` as a token inside `line`.
pub(crate) fn tag_match(line: &str, tag: &str, marker: &str) -> bool {
    let needle = format!("{tag} {marker}");
    let mut from = 0;
    while let Some(i) = line.get(from..).and_then(|s| s.find(&needle)) {
        let end = from + i + needle.len();
        let next = line.get(end..).and_then(|s| s.chars().next());
        if next.is_none_or(|c| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))) {
            return true;
        }
        from = end;
    }
    false
}

/// Replace the `BEGIN <marker>`..`END <marker>` lines of `existing` by `block`, or append `block`.
/// Bytes outside the block are untouched; the file's EOL style is used for the block.
pub fn splice_block(existing: &[u8], marker: &str, block: &str) -> Result<Vec<u8>, Error> {
    let lines = split_lines(existing);
    let text = |l: &Line| std::str::from_utf8(sl(existing, l.start, l.end)).unwrap_or("");
    let idx = |tag: &str| -> Vec<usize> {
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| tag_match(text(l), tag, marker))
            .map(|(i, _)| i)
            .collect()
    };
    let (begins, ends) = (idx("BEGIN"), idx("END"));
    let eol = detect_eol(existing);
    let rendered = block
        .trim_end_matches('\n')
        .split('\n')
        .collect::<Vec<_>>()
        .join(eol);
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => {
            let mut out = existing.to_vec();
            if !out.is_empty() && !out.ends_with(b"\n") {
                out.extend_from_slice(eol.as_bytes());
            }
            out.extend_from_slice(rendered.as_bytes());
            out.extend_from_slice(eol.as_bytes());
            Ok(out)
        }
        ([b], [e]) if b < e => {
            let (Some(bl), Some(el)) = (lines.get(*b), lines.get(*e)) else {
                return Err(Error::Internal("block line index".into()));
            };
            let mut out = sl(existing, 0, bl.start).to_vec();
            out.extend_from_slice(rendered.as_bytes());
            out.extend_from_slice(sl(existing, el.end, existing.len()));
            Ok(out)
        }
        _ => Err(refuse(
            RefusalKind::UnbalancedBlock,
            format!("unbalanced BEGIN/END {marker} block"),
            "fix or remove the block by hand, then re-run",
        )),
    }
}

/// Append `text` (LF) with the file's EOL; unchanged when the text is already present.
pub fn append_plain(existing: &[u8], text: &str) -> Vec<u8> {
    let eol = detect_eol(existing);
    let needle = text.trim().replace("\r\n", "\n");
    if String::from_utf8_lossy(existing)
        .replace("\r\n", "\n")
        .contains(&needle)
        && !needle.is_empty()
    {
        return existing.to_vec();
    }
    let body = if existing.is_empty() {
        text.trim_start_matches('\n')
    } else {
        text
    };
    let mut out = existing.to_vec();
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.extend_from_slice(eol.as_bytes());
    }
    out.extend_from_slice(body.trim_end_matches('\n').replace('\n', eol).as_bytes());
    out.extend_from_slice(eol.as_bytes());
    out
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::error::RefusalKind;
    use crate::write::testutil::*;

    fn root() -> (tempfile::TempDir, Root) {
        let d = tempfile::tempdir().unwrap();
        let r = test_root(d.path());
        (d, r)
    }
    fn one(kind: WriteSetKind, w: PlannedWrite) -> WriteSet {
        let mut s = WriteSet::new(kind);
        s.add(w).unwrap();
        s
    }
    const KINDS: [WriteSetKind; 4] = [
        WriteSetKind::Init,
        WriteSetKind::Sync,
        WriteSetKind::New,
        WriteSetKind::Approve,
    ];

    #[test]
    fn r_arch_141__git_config_is_never_writable() {
        let (_d, r) = root();
        for k in KINDS {
            assert!(
                one(k, pw(".git/config", WriteKind::Replace, "[core]\n"))
                    .validate(&r)
                    .is_err(),
                "{k:?}"
            );
        }
    }

    #[test]
    fn r_arch_141__only_init_may_write_the_pre_commit_hook() {
        let (_d, r) = root();
        let w = || pw(".git/hooks/pre-commit", WriteKind::Create, "#!/bin/sh\n");
        assert!(one(WriteSetKind::Init, w()).validate(&r).is_ok());
        assert!(one(WriteSetKind::Sync, w()).validate(&r).is_err());
        assert!(one(WriteSetKind::New, w()).validate(&r).is_err());
    }

    #[test]
    fn r_cli_014__init_never_edits_manifests_ci_or_source() {
        let (_d, r) = root();
        for p in [
            "Cargo.toml",
            "package.json",
            "pnpm-workspace.yaml",
            "Cargo.lock",
            ".github/workflows/ci.yml",
            "src/main.rs",
            "web/app.tsx",
        ] {
            let w = pw(
                p,
                WriteKind::AppendBlock {
                    marker: String::new(),
                },
                "x\n",
            );
            assert!(one(WriteSetKind::Init, w).validate(&r).is_err(), "{p}");
        }
    }

    #[test]
    fn r_file_002__new_may_not_write_generated_or_approval_files() {
        let (_d, r) = root();
        for p in [
            "fairlead/gen/baseline.txt",
            "fairlead/approvals.toml",
            "fairlead/vendored.lock",
        ] {
            assert!(
                one(WriteSetKind::New, pw(p, WriteKind::Create, "x\n"))
                    .validate(&r)
                    .is_err(),
                "{p}"
            );
        }
        assert!(
            one(
                WriteSetKind::New,
                pw("fairlead/docs/adr/x.md", WriteKind::Create, "x\n")
            )
            .validate(&r)
            .is_ok()
        );
    }

    #[test]
    fn r_scope_061__sync_writes_only_generated_files_never_authored() {
        let (_d, r) = root();
        assert!(
            one(
                WriteSetKind::Sync,
                pw("fairlead/control.toml", WriteKind::Replace, "[fairlead]\n")
            )
            .validate(&r)
            .is_err()
        );
        let mut g = pw(
            "fairlead/gen/reg/rule.reg",
            WriteKind::Replace,
            "# scope root\n",
        );
        assert!(
            one(WriteSetKind::Sync, g.clone()).validate(&r).is_err(),
            "class Authored is refused in gen/"
        );
        g.class = crate::classify::FileClass::Generated;
        assert!(one(WriteSetKind::Sync, g).validate(&r).is_ok());
    }

    #[test]
    fn r_arch_112__carriage_returns_are_rejected_in_planned_bytes() {
        let (_d, r) = root();
        assert!(
            one(
                WriteSetKind::New,
                pw("fairlead/docs/adr/x.md", WriteKind::Create, "a\r\nb\n")
            )
            .validate(&r)
            .is_err()
        );
    }

    #[test]
    fn r_arch_144__secret_like_value_blocks_the_write_and_is_not_echoed() {
        let (_d, r) = root();
        let w = pw(
            "fairlead/docs/adr/x.md",
            WriteKind::Create,
            "-----BEGIN PRIVATE KEY-----\nabcdef\n",
        );
        let err = one(WriteSetKind::New, w).validate(&r).unwrap_err();
        match err {
            Error::Refused(f) => {
                assert_eq!(f.kind, RefusalKind::Floor);
                assert!(!f.message.contains("-----BEGIN") && !f.message.contains("abcdef"));
                assert!(f.message.contains("fairlead/docs/adr/x.md"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn r_scope_050__an_empty_control_file_is_never_written() {
        let (_d, r) = root();
        assert!(
            one(
                WriteSetKind::New,
                pw("apps/web/fairlead/control.toml", WriteKind::Create, "\n")
            )
            .validate(&r)
            .is_err()
        );
    }

    #[test]
    fn r_arch_143__a_path_may_be_planned_once() {
        let mut s = WriteSet::new(WriteSetKind::New);
        s.add(pw("fairlead/docs/adr/x.md", WriteKind::Create, "a\n"))
            .unwrap();
        assert!(
            s.add(pw("fairlead/docs/adr/x.md", WriteKind::Create, "b\n"))
                .is_err()
        );
        assert!(
            s.add(pw("fairlead/docs/ADR/x.md", WriteKind::Create, "b\n"))
                .is_err(),
            "case-fold collision"
        );
    }

    const BLOCK: &str =
        "<!-- BEGIN fairlead (managed block; edit outside it) -->\nnew\n<!-- END fairlead -->\n";

    #[test]
    fn r_cli_016__block_replaces_only_the_text_between_markers_and_keeps_crlf() {
        let old =
            b"intro\r\n<!-- BEGIN fairlead (old) -->\r\nold\r\n<!-- END fairlead -->\r\noutro\r\n";
        let got = splice_block(old, "fairlead", BLOCK).unwrap();
        let want = b"intro\r\n<!-- BEGIN fairlead (managed block; edit outside it) -->\r\nnew\r\n<!-- END fairlead -->\r\noutro\r\n";
        assert_eq!(got, want);
        assert_eq!(
            splice_block(&got, "fairlead", BLOCK).unwrap(),
            got,
            "idempotent"
        );
    }

    #[test]
    fn r_cli_016__block_is_appended_with_the_file_eol() {
        assert_eq!(
            splice_block(b"# T", "fairlead", BLOCK).unwrap(),
            format!("# T\n{BLOCK}").into_bytes()
        );
        let crlf = splice_block(b"a\r\nb\r\n", "fairlead", BLOCK).unwrap();
        assert!(
            crlf.starts_with(b"a\r\nb\r\n<!-- BEGIN fairlead")
                && crlf.ends_with(b"<!-- END fairlead -->\r\n")
        );
    }

    #[test]
    fn r_cli_016__unbalanced_markers_refuse_with_exit_4_kind() {
        for bad in [
            "<!-- BEGIN fairlead -->\nx\n",
            "x\n<!-- END fairlead -->\n",
            "<!-- END fairlead -->\n<!-- BEGIN fairlead -->\n",
        ] {
            match splice_block(bad.as_bytes(), "fairlead", BLOCK) {
                Err(Error::Refused(f)) => assert_eq!(f.kind, RefusalKind::UnbalancedBlock),
                other => panic!("{bad:?} -> {other:?}"),
            }
        }
    }

    #[test]
    fn r_cli_016__marker_match_is_token_exact() {
        // "fairlead-x" must not be taken for "fairlead"
        let existing = b"<!-- BEGIN fairlead-x -->\nkeep\n<!-- END fairlead-x -->\n";
        let got = splice_block(existing, "fairlead", BLOCK).unwrap();
        assert!(got.starts_with(existing));
    }

    #[test]
    fn r_arch_143__plain_append_is_idempotent_and_keeps_eol() {
        let once = append_plain(b"a\r\n", "\n[x]\nk = 1\n");
        assert_eq!(once, b"a\r\n\r\n[x]\r\nk = 1\r\n");
        assert_eq!(append_plain(&once, "\n[x]\nk = 1\n"), once);
        assert_eq!(append_plain(b"", "\n[x]\n"), b"[x]\n");
    }
}
