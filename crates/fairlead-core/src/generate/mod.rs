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
    /// `gen/alias`: draft ID to ordinal of sealed change docs (R-VER-046).
    Alias,
    /// `gen/scope.lock`: reconciled version and ancestor digest (R-VER-015); kept current by
    /// `sync` and judged by `H-SYNC-ANCESTOR`, not by the byte comparison of `HN-G001`.
    ScopeLock,
    /// `changelog.file`: the aggregate of the version files (R-VER-053), a Markdown file whose
    /// header line is an HTML comment.
    Changelog,
    /// `gen/hier.suggest`: the hierarchy refactor suggestions of a scope (R-HIER-072), written
    /// only when the root's `[hier] level` is 2 or more and the scope has suggestions.
    Suggest,
    /// `gen/requires`: what a scope requires from other scopes (R-PORT-020), written only for a
    /// scope that declares `[scope] requires` or is portable.
    Requires,
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
        if self.kind == GenKind::Changelog {
            return header::render_markdown(&kind, edition.0, &self.body).into_bytes();
        }
        header::render_file(&kind, edition.0, &self.body).into_bytes()
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GeneratedSet {
    pub files: BTreeMap<RepoPath, GeneratedFile>,
}

impl GeneratedSet {
    /// The changelog aggregate's path, when the set holds one (R-VER-053).
    pub fn changelog_path(&self) -> Option<RepoPath> {
        self.files
            .values()
            .find(|g| g.kind == GenKind::Changelog)
            .map(|g| g.path.clone())
    }
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

use crate::hash::{ContentHash, base32, hash_raw};
use crate::render::line::TerseLine;

/// Join a scope directory (empty or `.` for the root) and a relative path into a `RepoPath`.
pub(crate) fn join_dir(dir: &str, rest: &str) -> Option<RepoPath> {
    let full = if dir.is_empty() || dir == "." {
        rest.to_string()
    } else {
        format!("{dir}/{rest}")
    };
    RepoPath::new(&full).ok()
}

/// One vendor-owned file: scope-relative path, tool name, full blake3 content hash (D-50).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VendorRow {
    pub path: String,
    pub tool: String,
    pub hash: ContentHash,
}

fn to_hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[allow(
    dead_code,
    reason = "vendored.lock readers (P7-T15) parse the hex hash"
)]
fn from_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// `vendor.<id> path=<scope-relative> tool=<name> b3=<64 hex>`; the id is 10 base32 characters of
/// the path hash so it does not change when other files are added or removed.
pub fn vendor_lock_body(rows: &[VendorRow]) -> String {
    let mut lines: Vec<(String, String)> = rows
        .iter()
        .map(|r| {
            let id = format!("vendor.{}", base32(&hash_raw(r.path.as_bytes()).0, 10));
            let line = TerseLine::new(&id)
                .with("path", &crate::render::sanitize::clean_data(&r.path, 240))
                .with("tool", &crate::render::sanitize::clean_data(&r.tool, 60))
                .with("b3", &to_hex(&r.hash.0))
                .render();
            (id, line)
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (_, l) in lines {
        out.push_str(&l);
        out.push('\n');
    }
    out
}

pub use crate::facts::parse_vendor_lock; // A-35: one reader, the P3 one

use crate::check::Baseline;
use crate::config::{IndexValues, VendorDecl};
use crate::extract::Corpus;
use crate::facts::Facts;
use crate::registry::{InertReason, OvrState, Registry, ScopeState};
use members::{MembersView, members_body};
use shard::{ExcRow, ShardItem, TokSite, exc_body, shard_kind};

fn rel_to(dir: &str, path: &str) -> String {
    if dir.is_empty() || dir == "." {
        return path.to_string();
    }
    path.strip_prefix(dir)
        .and_then(|r| r.strip_prefix('/'))
        .unwrap_or(path)
        .to_string()
}

fn inert_word(r: &InertReason) -> &'static str {
    match r {
        InertReason::SealedNoAllow => "sealed-no-allow",
        InertReason::Incomplete => "incomplete",
        InertReason::FloorRule => "floor-rule",
        InertReason::NonDelegable => "non-delegable",
        InertReason::UnknownField => "unknown-field",
        InertReason::DuplicateOvr => "duplicate-ovr",
        InertReason::VersionExpiry => "version-expiry",
        InertReason::OutsideSubtree => "outside-subtree",
        InertReason::UnknownOwner => "unknown-owner",
    }
}

fn ovr_state_word(s: &OvrState) -> Option<String> {
    match s {
        OvrState::Effective => None,
        OvrState::Expired => Some("expired".to_string()),
        OvrState::Stale => Some("stale".to_string()),
        OvrState::Inert(r) => Some(format!("inert-{}", inert_word(r))),
    }
}

fn tool_for(vendors: &[VendorDecl], rel: &str) -> String {
    for v in vendors {
        if let Ok(g) = globset::Glob::new(&v.path)
            && g.compile_matcher().is_match(rel)
        {
            return v.tool.clone();
        }
    }
    String::new()
}

/// Token value sites from the stylesheet extraction, keyed by (scope, custom property name),
/// in path then source order. Foreign-region sites are indexed elsewhere, not here.
fn token_sites(reg: &Registry, corpus: &Corpus) -> BTreeMap<(ScopeId, String), Vec<TokSite>> {
    let mut out: BTreeMap<(ScopeId, String), Vec<TokSite>> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        if rec.css_tokens.is_empty() {
            continue;
        }
        let Some(scope_id) = reg.scope_of(path) else {
            continue;
        };
        let Some(scope) = reg.scope(scope_id) else {
            continue;
        };
        let rel = rel_to(scope.dir.as_str(), path.as_str());
        for site in rec.css_tokens.iter().filter(|s| !s.in_foreign) {
            out.entry((scope_id.clone(), site.name.clone()))
                .or_default()
                .push(TokSite {
                    value: site.value.clone(),
                    context: site.context.clone(),
                    src: rel.clone(),
                });
        }
    }
    out
}

fn put(
    files: &mut BTreeMap<RepoPath, GeneratedFile>,
    dir: &str,
    rest: &str,
    scope: &ScopeId,
    kind: GenKind,
    body: String,
) {
    if let Some(path) = join_dir(dir, rest) {
        files.insert(
            path.clone(),
            GeneratedFile {
                path,
                scope: scope.clone(),
                kind,
                body,
            },
        );
    }
}

/// R-VER-053: the changelog aggregate from the version files' bodies, newest version first. Each
/// body is the version file after its frontmatter, kept as it is (its `# <version>` heading becomes
/// a second-level one).
pub fn changelog_body<'a>(
    folder: &str,
    versions: impl IntoIterator<Item = (crate::version::Version, &'a str)>,
) -> String {
    let mut v: Vec<(crate::version::Version, &str)> = versions.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.0));
    let mut out = format!(
        "# Changelog\n\nGenerated by fairlead from `{folder}/versions/`: edit the change docs, not this file.\n"
    );
    for (_, text) in v {
        let text = text.replace("\r\n", "\n");
        let body = text
            .strip_prefix("+++\n")
            .and_then(|r| r.split_once("\n+++"))
            .map_or(text.as_str(), |(_, b)| b)
            .trim();
        out.push('\n');
        for line in body.lines() {
            match line.strip_prefix("# ") {
                Some(h) => out.push_str(&format!("## {h}\n")),
                None => {
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// Pure (S10): the generated files `sync` would write, computed from the registry, the corpus
/// and the facts. Never reads `facts.generated_on_disk` (R-FILE-008). Headers carry the edition and
/// are added by [`GeneratedFile::render`]; `edition` is kept so every caller uses the one call shape.
pub fn generate(
    reg: &Registry,
    corpus: &Corpus,
    facts: &Facts,
    baseline: Option<&Baseline>,
    edition: Edition,
) -> GeneratedSet {
    let mut files: BTreeMap<RepoPath, GeneratedFile> = BTreeMap::new();
    let Some(root_scope) = reg.scope(reg.root_scope()) else {
        return GeneratedSet { files };
    };
    let root_ctl = root_scope.control.as_deref();
    let folder = root_ctl
        .and_then(|c| c.settings.folder.clone())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "fairlead".to_string());
    let gen_dir = root_ctl
        .map(|c| c.layout.r#gen.clone())
        .filter(|g| !g.is_empty())
        .unwrap_or_else(|| "gen".to_string());
    let mode = root_ctl
        .map(|c| c.settings.index_values)
        .unwrap_or(IndexValues::Value);
    let tokens = token_sites(reg, corpus);
    let (change_docs, _) = crate::docs::collect_change_docs(corpus);
    let start = root_ctl
        .and_then(|c| crate::version::Version::parse(&c.version.start))
        .unwrap_or(crate::version::Version::new(0, 1, 0));
    let sealed = crate::docs::line_of(corpus, &folder, start).sealed;
    let suggestions = crate::hier::config(reg)
        .filter(|h| h.settings.level.is_some_and(|l| l >= 2))
        .map(|h| {
            let m = crate::hier::Model::build(reg);
            let paths: Vec<&RepoPath> = corpus.records.keys().collect();
            let edges = m.edges(reg, &paths);
            m.suggestions(
                reg,
                &edges,
                h,
                &crate::hier::digest_history(reg, &facts.ledgers),
            )
        });

    for scope in reg.scopes() {
        if !matches!(scope.state, ScopeState::Resolved) {
            continue;
        }
        let dir = scope.dir.as_str();
        let view = reg.view(&scope.id);

        // 1. one shard per kind, from this scope's own items (R-FILE-021, R-FILE-022)
        let mut by_kind: BTreeMap<String, (Kind, Vec<ShardItem>)> = BTreeMap::new();
        for item in reg.items().filter(|i| i.id.scope == scope.id) {
            let effective = view.items.get(&item.id);
            let mut fields = item.fields.clone();
            if let Some(e) = effective {
                for (k, f) in &e.fields {
                    fields.insert(k.clone(), f.value.clone());
                }
            }
            let sites = match fields.get("css") {
                Some(crate::registry::FieldValue::Scalar(css)) => tokens
                    .get(&(scope.id.clone(), css.clone()))
                    .cloned()
                    .unwrap_or_default(),
                _ => Vec::new(),
            };
            let kind_text = item.id.kind.as_str().to_string();
            let entry = by_kind
                .entry(kind_text.clone())
                .or_insert_with(|| (item.id.kind.clone(), Vec::new()));
            entry.1.push(ShardItem {
                kind: kind_text,
                name: item.id.name.as_str().to_string(),
                policy: effective.map_or_else(|| item.policy, |e| e.policy),
                sum: item.sum.clone(),
                fields,
                sites,
            });
        }
        for (kind_text, (kind, items)) in &by_kind {
            for out in shard_kind(kind_text, items, &mode) {
                put(
                    &mut files,
                    dir,
                    &format!("{folder}/{gen_dir}/reg/{}", out.file_name),
                    &scope.id,
                    GenKind::Shard(kind.clone()),
                    out.body,
                );
            }
        }

        // 2. the scope's own exception ledger facts (R-SCOPE-044)
        let rows: Vec<ExcRow> = reg
            .ledger(&scope.id)
            .entries
            .iter()
            .filter(|e| e.varied_in == scope.id)
            .map(|e| ExcRow {
                ovr: e.ovr.0.as_str().to_string(),
                target: e.target.to_id().to_string(),
                home: e.home.0.as_str().to_string(),
                varied_in: e.varied_in.0.as_str().to_string(),
                depth: e.depth,
                state: ovr_state_word(&e.state),
                why: e.why.clone(),
                owner: e.owner.clone(),
                exp: e.exp.map(|d| d.to_string()),
            })
            .collect();
        let exc = exc_body(&rows);
        if !exc.is_empty() {
            put(
                &mut files,
                dir,
                &format!("{folder}/{gen_dir}/reg/exc.reg"),
                &scope.id,
                GenKind::Exceptions,
                exc,
            );
        }

        // 3. membership, separate from the shards (R-FILE-022)
        let view_members = MembersView {
            id: scope.id.0.as_str().to_string(),
            dir: dir.to_string(),
            parent: scope.parent.as_ref().map(|p| p.0.as_str().to_string()),
            chain: reg
                .chain(&scope.id)
                .iter()
                .map(|s| s.0.as_str().to_string())
                .collect(),
            children: reg
                .scopes()
                .filter(|s| s.parent.as_ref() == Some(&scope.id))
                .map(|s| (s.id.0.as_str().to_string(), s.dir.as_str().to_string()))
                .collect(),
        };
        put(
            &mut files,
            dir,
            &format!("{folder}/{gen_dir}/members"),
            &scope.id,
            GenKind::Members,
            members_body(&view_members),
        );

        // 3b. alias map of sealed change docs homed here (R-VER-046), only when one exists
        let here: Vec<&crate::docs::ChangeDoc> = change_docs
            .iter()
            .filter(|d| reg.scope_of(&d.path) == Some(&scope.id))
            .collect();
        let alias = crate::docs::alias_body(&here);
        if !alias.is_empty() {
            put(
                &mut files,
                dir,
                &format!("{folder}/{gen_dir}/alias"),
                &scope.id,
                GenKind::Alias,
                alias,
            );
        }

        // 3c. scope.lock, once a version is sealed (R-VER-015); a never-sealed tree has none, so
        // edition-1 trees from 0.1 keep their bytes (R-EDN-002)
        let ported = crate::port::opted_in(reg, &scope.id);
        if let Some(s) = sealed {
            let anc = crate::merkle::ancestor_digest(reg, &scope.id);
            let mut body = crate::merkle::scope_lock_body(s, &anc);
            // R-PORT-022 (0.3): the pinned digests of what the scope requires.
            if ported {
                body.push_str(&crate::port::lock_lines(
                    reg, corpus, &scope.id, s, edition.0, &anc,
                ));
            }
            // R-PORT-051 (0.3): the root pins each mount by commit (a submodule) and rollup digest.
            if scope.id == *reg.root_scope() {
                body.push_str(&crate::port::pin_lines(&facts.mounts));
            }
            put(
                &mut files,
                dir,
                &format!("{folder}/{gen_dir}/scope.lock"),
                &scope.id,
                GenKind::ScopeLock,
                body,
            );
        }
        // R-PORT-020 (0.3): the generated `requires`
        if ported {
            put(
                &mut files,
                dir,
                &format!("{folder}/{gen_dir}/requires"),
                &scope.id,
                GenKind::Requires,
                crate::port::requires_body(reg, corpus, &scope.id),
            );
        }

        // 3d. hierarchy suggestions (R-HIER-072), only at level 2 and when there are any
        if let Some(sg) = suggestions.as_ref() {
            let body = crate::hier::suggest_body(sg, &scope.id);
            if !body.is_empty() {
                put(
                    &mut files,
                    dir,
                    &format!("{folder}/{gen_dir}/hier.suggest"),
                    &scope.id,
                    GenKind::Suggest,
                    body,
                );
            }
        }

        // 4. vendor lock (R-FILE-040)
        if let Some(ctl) = scope.control.as_ref()
            && !ctl.vendors.is_empty()
        {
            let rows: Vec<VendorRow> = facts
                .vendor_hashes
                .iter()
                .filter(|(p, _)| reg.scope_of(p) == Some(&scope.id))
                .map(|(p, h)| {
                    let rel = rel_to(dir, p.as_str());
                    VendorRow {
                        tool: tool_for(&ctl.vendors, &rel),
                        path: rel,
                        hash: *h,
                    }
                })
                .collect();
            if !rows.is_empty() {
                put(
                    &mut files,
                    dir,
                    &format!("{folder}/vendored.lock"),
                    &scope.id,
                    GenKind::VendorLock,
                    vendor_lock_body(&rows),
                );
            }
        }
    }

    // 4b. the changelog aggregate, once a version is sealed (R-VER-053)
    if let Some(file) = root_ctl.and_then(|c| c.changelog.file.as_deref())
        && sealed.is_some()
    {
        let versions = facts.version_texts.iter().filter_map(|(p, t)| {
            crate::version::version_of_file_name(p.file_name()).map(|v| (v, &**t))
        });
        put(
            &mut files,
            "",
            file,
            reg.root_scope(),
            GenKind::Changelog,
            changelog_body(&folder, versions),
        );
    }

    // 5. the baseline, when the caller holds one (written only by `sync --baseline`, D-38)
    if let Some(b) = baseline {
        let configured = root_ctl
            .map(|c| c.baseline.file.clone())
            .filter(|f| !f.is_empty());
        let rel = configured.unwrap_or_else(|| format!("{folder}/{gen_dir}/baseline.txt"));
        put(
            &mut files,
            "",
            &rel,
            reg.root_scope(),
            GenKind::Baseline,
            b.body(),
        );
    }
    GeneratedSet { files }
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
    use fairlead_syntax::ids::ScopeSlug;

    fn root() -> ScopeId {
        ScopeId(ScopeSlug::parse("root").unwrap())
    }

    fn file(kind: GenKind, body: &str) -> GeneratedFile {
        GeneratedFile {
            path: RepoPath::new("fairlead/gen/members").unwrap(),
            scope: root(),
            kind,
            body: body.to_string(),
        }
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
        assert_eq!(
            first_differing_lines(b"a\nb\n", b"a\n", 3),
            vec!["+b".to_string()]
        );
        assert!(first_differing_lines(b"a\nb\n", b"a\nb\n", 3).is_empty());
        assert!(
            first_differing_lines(b"a\r\nb\r\n", b"a\nb\n", 3).is_empty(),
            "a CRLF checkout is not a difference"
        );
    }

    #[test]
    fn r_chk_023__differing_text_is_sanitised() {
        let diff = first_differing_lines(b"a\n", b"\x1b[31ma\n", 3);
        assert!(diff.iter().all(|l| !l.contains('\u{1b}')), "{diff:?}");
    }

    #[test]
    fn r_file_040__vendor_lock_line_and_reader_roundtrip() {
        let rows = vec![VendorRow {
            path: "components.json".to_string(),
            tool: "shadcn".to_string(),
            hash: ContentHash([7u8; 32]),
        }];
        let body = vendor_lock_body(&rows);
        let id = crate::hash::base32(&crate::hash::hash_raw(b"components.json").0, 10);
        assert_eq!(
            body,
            format!(
                "vendor.{id} path=components.json tool=shadcn b3={}\n",
                "07".repeat(32)
            )
        );
        let text = format!("# scope console edition=1 body=aaaaaaaaaaaaaaaaaaaa\n{body}");
        let map = parse_vendor_lock("apps/console", &text);
        assert_eq!(
            map.get(&RepoPath::new("apps/console/components.json").unwrap()),
            Some(&ContentHash([7u8; 32]))
        );
        let root_map = parse_vendor_lock("", &text);
        assert!(root_map.contains_key(&RepoPath::new("components.json").unwrap()));
    }

    #[test]
    fn r_file_040__lock_ids_are_stable_when_other_files_are_added() {
        let a = VendorRow {
            path: "a.json".to_string(),
            tool: "t".to_string(),
            hash: ContentHash([1u8; 32]),
        };
        let b = VendorRow {
            path: "b.json".to_string(),
            tool: "t".to_string(),
            hash: ContentHash([2u8; 32]),
        };
        let only_a = vendor_lock_body(std::slice::from_ref(&a));
        let both = vendor_lock_body(&[a, b]);
        assert!(both.contains(only_a.trim_end()));
    }

    #[test]
    fn r_file_040__reader_skips_garbled_lines() {
        let text = "vendor.x path=a.json tool=t b3=zz\nvendor.y path=b.json\nnot a line \"\n";
        assert!(parse_vendor_lock("", text).is_empty());
    }
}
