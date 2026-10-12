//! Everything a pure check, context or sync-plan needs from disk or git, gathered once per run
//! (contract 4.4). Task 24 creates the git-facing types; Task 25 adds `Facts` and `collect_facts`.
use crate::path::RepoPath;
use std::collections::BTreeSet;

/// Working tree plus index against HEAD, untracked included (C-36).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ChangedSet(pub BTreeSet<RepoPath>);

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct GitFacts {
    pub available: bool,
    pub changed: Option<ChangedSet>,
    pub sparse_absent: Vec<RepoPath>,
    /// Set when git state could not be read; under `--changed` this means exit 5 (R-ARCH-075).
    pub unreadable: Option<String>,
    /// Docs-first changed set against the coverage base; `None` before the first seal.
    pub coverage: Option<Coverage>,
}

/// R-VER-033: the files changed since the commit that introduced `versions/<S>.md`, or why that
/// set cannot be computed (no git, shallow history: never a silent pass, R-GIT-040).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Coverage {
    Changed {
        base: String,
        files: BTreeSet<RepoPath>,
    },
    Unavailable(String),
}

/// The highest `versions/<v>.md` on disk, as a root-relative path.
fn latest_version_file(root: &Root) -> Option<String> {
    let dir = root.dir.join(&root.folder).join("versions");
    let entries = std::fs::read_dir(dir.as_std_path()).ok()?;
    entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| crate::version::version_of_file_name(&n).map(|v| (v, n)))
        .max()
        .map(|(_, n)| format!("{}/versions/{n}", root.folder))
}

use crate::classify::{ClassifyRules, FileClass, classify};
use crate::config::{ApprovalsFile, ControlSet, parse_approvals};
use crate::discover::Root;
use crate::error::Error;
use crate::git::GitRepo;
use crate::hash::{ContentHash, hash_content};
use crate::manifest::{ManifestHints, read_manifest_hints};
use crate::walk::WalkResult;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The rollup the outer root reads of a registered mount (R-PORT-031): never its rules.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct MountFacts {
    /// The mount directory exists (a submodule that is not checked out does not).
    pub present: bool,
    /// The inner control file declares `root = true`.
    pub is_root: bool,
    /// The inner `[fairlead] scope`.
    pub scope: Option<String>,
    /// The inner `[fairlead] edition` (1 when absent).
    pub edition: Option<u32>,
    /// The inner scope ids, from its membership shard `gen/members`.
    pub scopes: Vec<String>,
    /// R-PORT-051: the submodule commit the parent pins (a gitlink), if it is one.
    pub commit: Option<String>,
    /// R-PORT-051: digest of the rollup (inner control file and membership shard).
    pub digest: String,
}

/// Reads the rollup of each `[[mount]]` of the root control file.
pub fn read_mounts(root: &Root, controls: &ControlSet) -> BTreeMap<String, MountFacts> {
    let mut out = BTreeMap::new();
    let Some(rc) = controls.root_control() else {
        return out;
    };
    for m in &rc.mounts {
        let dir = root.dir.join(&m.path);
        let mut f = MountFacts {
            present: dir.as_std_path().is_dir(),
            ..MountFacts::default()
        };
        let ctl = dir.join(&root.folder).join("control.toml");
        if let Ok(text) = std::fs::read_to_string(ctl.as_std_path())
            && let Ok(t) = text.parse::<toml::Table>()
            && let Some(h) = t.get("fairlead").and_then(toml::Value::as_table)
        {
            f.is_root = h.get("root").and_then(toml::Value::as_bool) == Some(true);
            f.scope = h
                .get("scope")
                .and_then(toml::Value::as_str)
                .map(str::to_owned);
            f.edition = Some(
                h.get("edition")
                    .and_then(toml::Value::as_integer)
                    .and_then(|e| u32::try_from(e).ok())
                    .unwrap_or(1),
            );
        }
        let members = dir.join(&root.folder).join("gen").join("members");
        let rollup = format!(
            "{}\n--\n{}",
            std::fs::read_to_string(ctl.as_std_path())
                .unwrap_or_default()
                .replace("\r\n", "\n"),
            std::fs::read_to_string(members.as_std_path())
                .unwrap_or_default()
                .replace("\r\n", "\n")
        );
        f.digest = crate::hash::base32(&crate::hash::hash_raw(rollup.as_bytes()).0, 12);
        if let Some(repo) = crate::git::GitRepo::open(root)
            && let Ok(p) = RepoPath::new(&m.path)
        {
            f.commit = repo.gitlink(&p);
        }
        if let Ok(text) = std::fs::read_to_string(members.as_std_path()) {
            f.scopes = text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| {
                    l.split_whitespace()
                        .next()?
                        .strip_prefix("scope.")
                        .map(str::to_owned)
                })
                .collect();
        }
        out.insert(m.scope.clone(), f);
    }
    out
}

/// Files above this size are streamed and hashed raw instead of read into memory.
const LARGE_FILE: u64 = 16 * 1024 * 1024;

/// Everything a pure check, context or sync plan needs from disk or git (contract 4.4).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Facts {
    pub generated_on_disk: BTreeMap<RepoPath, Arc<[u8]>>,
    pub agent_files: BTreeMap<RepoPath, Arc<str>>,
    pub codeowners: Option<Arc<str>>,
    pub vendor_hashes: BTreeMap<RepoPath, ContentHash>,
    pub vendor_lock: BTreeMap<RepoPath, ContentHash>,
    pub approvals: ApprovalsFile,
    pub git: GitFacts,
    pub manifest_hints: ManifestHints,
    /// R-PORT-031 (0.3): the rollup of each registered mount, by mount scope id.
    pub mounts: BTreeMap<String, MountFacts>,
    /// Ledger year files (`ledger/hist/<year>.hl`), raw text, for the continuity check (R-RET-036).
    pub ledgers: BTreeMap<RepoPath, Arc<str>>,
    /// `docs::body_hash` of every Markdown file under a `docs/changes/` folder, for the seal
    /// tamper check (R-VER-054).
    pub doc_body_hashes: BTreeMap<RepoPath, String>,
    /// Raw text of the root's version files, for the collision and gap checks (R-GIT-030, R-GIT-032).
    pub version_texts: BTreeMap<RepoPath, Arc<str>>,
    /// A write journal is left over: an interrupted writer (R-GIT-051, H-TX-INCOMPLETE).
    pub pending_journal: bool,
    /// The left-over journal belongs to a `move` (R-PORT-005, MOVE-INCOMPLETE).
    pub pending_move: bool,
    /// Tombstone year files (`<folder>/tombstones/<year>.toml`), raw text (R-LC-020).
    pub tombstones: BTreeMap<RepoPath, Arc<str>>,
    /// Foreign regions whose in-file anchor was at HEAD but is gone now (R-FILE-043): path, the
    /// `foreign.<name>` ID, and its line at HEAD.
    pub foreign_lost: Vec<(RepoPath, String, u32)>,
    /// Authority records and the signature state of the commit that last wrote each, filled only
    /// by `check --ci` with `authority.signed-records = true` (R-LC-040).
    pub authority: Vec<AuthorityRecord>,
}

/// One authority record file checked for a signed commit (R-LC-040).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AuthorityRecord {
    pub path: RepoPath,
    /// The commit that last wrote the file, if git knows one.
    pub commit: Option<String>,
    pub signed: bool,
}

/// Bytes of a regular file; `None` for a missing file, a symlink or a directory (never followed).
fn read_regular(root: &Root, rel: &RepoPath) -> Result<Option<Vec<u8>>, Error> {
    let abs = root.dir.join(rel.as_str());
    match std::fs::symlink_metadata(abs.as_std_path()) {
        Ok(m) if m.is_file() => std::fs::read(abs.as_std_path())
            .map(Some)
            .map_err(|e| Error::io(rel.as_str(), "read", &e, false)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(rel.as_str(), "stat", &e, false)),
    }
}

fn hash_file(root: &Root, rel: &RepoPath, size: u64) -> Result<Option<ContentHash>, Error> {
    if size <= LARGE_FILE {
        return Ok(read_regular(root, rel)?.map(|b| hash_content(&b)));
    }
    let abs = root.dir.join(rel.as_str());
    let file = match std::fs::File::open(abs.as_std_path()) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(rel.as_str(), "open", &e, false)),
    };
    let mut h = blake3::Hasher::new();
    h.update_reader(file)
        .map_err(|e| Error::io(rel.as_str(), "read", &e, false))?;
    Ok(Some(ContentHash(*h.finalize().as_bytes())))
}

/// `vendored.lock` lines (amendment A-35): `vendor.<id> path=<scope-relative path> tool=<tool>
/// b3=<64 hex>`. `#` lines, blank lines and malformed lines are ignored (the file is generated, so a
/// malformed line surfaces as drift in P5). `scope_dir` is the directory of the scope that owns the
/// lock ("" for the root); keys of the result are repo-relative.
pub fn parse_vendor_lock(scope_dir: &str, text: &str) -> BTreeMap<RepoPath, ContentHash> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        if !parts.next().is_some_and(|id| id.starts_with("vendor.")) {
            continue;
        }
        let (mut path, mut hex) = (None, None);
        for t in parts {
            if let Some(v) = t.strip_prefix("path=") {
                path = Some(v);
            } else if let Some(v) = t.strip_prefix("b3=") {
                hex = Some(v);
            }
        }
        let (Some(rel), Some(hex)) = (path, hex) else {
            continue;
        };
        let full = if scope_dir.is_empty() {
            rel.to_string()
        } else {
            format!("{scope_dir}/{rel}")
        };
        let Ok(path) = RepoPath::new(&full) else {
            continue;
        };
        if let Ok(h) = ContentHash::from_hex(hex) {
            out.insert(path, h);
        }
    }
    out
}

pub fn git_facts(root: &Root, want_changed: bool) -> GitFacts {
    let latest = latest_version_file(root);
    let Some(repo) = GitRepo::open(root) else {
        return GitFacts {
            unreadable: want_changed
                .then(|| "not inside a git repository: git state is unavailable".to_string()),
            coverage: latest
                .map(|_| Coverage::Unavailable("not inside a git repository".to_string())),
            ..GitFacts::default()
        };
    };
    let mut git = GitFacts {
        available: true,
        ..GitFacts::default()
    };
    match repo.sparse_absent() {
        Ok(v) => git.sparse_absent = v,
        Err(e) => git.unreadable = Some(e.to_string()),
    }
    git.coverage = latest.map(|path| match repo.commit_adding(&path) {
        Ok(Some(base)) => match repo.changed_since(&base) {
            Ok(files) => Coverage::Changed { base, files },
            Err(e) => Coverage::Unavailable(e.to_string()),
        },
        Ok(None) => Coverage::Unavailable(format!(
            "history does not reach the commit that added {path} (shallow clone?)"
        )),
        Err(e) => Coverage::Unavailable(e.to_string()),
    });
    if want_changed {
        match repo.changed_files() {
            Ok(c) => git.changed = Some(c),
            Err(e) => git.unreadable = Some(e.to_string()),
        }
    }
    git
}

/// S5: gather everything once so checks stay pure. I/O (disk, git).
pub fn collect_facts(
    root: &Root,
    walk: &WalkResult,
    controls: &ControlSet,
    want_changed: bool,
) -> Result<Facts, Error> {
    collect_facts_with(root, walk, controls, git_facts(root, want_changed))
}

/// `collect_facts` with git state gathered by the caller (the pipeline runs it beside the walk).
pub fn collect_facts_with(
    root: &Root,
    walk: &WalkResult,
    controls: &ControlSet,
    git: GitFacts,
) -> Result<Facts, Error> {
    let rules = ClassifyRules::from_controls(root, controls);
    let mut facts = Facts::default();
    for e in &walk.files {
        match classify(&e.path, &rules) {
            FileClass::Generated => {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    if e.path.file_name() == "vendored.lock" {
                        // `<scope-dir>/<folder>/vendored.lock`: the scope directory is two levels up.
                        let scope_dir = e
                            .path
                            .parent()
                            .and_then(|f| f.parent())
                            .map(|d| d.as_str().to_string())
                            .unwrap_or_default();
                        facts.vendor_lock.extend(parse_vendor_lock(
                            &scope_dir,
                            &String::from_utf8_lossy(&bytes),
                        ));
                    }
                    facts
                        .generated_on_disk
                        .insert(e.path.clone(), Arc::<[u8]>::from(bytes));
                }
            }
            FileClass::Ledger if e.path.as_str().ends_with(".hl") => {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    facts.ledgers.insert(
                        e.path.clone(),
                        Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()),
                    );
                }
            }
            FileClass::Authored
                if e.path.as_str().ends_with(".toml")
                    && e.path.parent().is_some_and(|d| {
                        d.file_name() == "tombstones"
                            && d.parent().is_some_and(|f| f.file_name() == root.folder)
                    }) =>
            {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    facts.tombstones.insert(
                        e.path.clone(),
                        Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()),
                    );
                }
            }
            FileClass::Authored
                if e.path.as_str().ends_with(".md")
                    && e.path.as_str().contains("docs/changes/") =>
            {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    facts.doc_body_hashes.insert(
                        e.path.clone(),
                        crate::docs::body_hash(&String::from_utf8_lossy(&bytes)),
                    );
                }
            }
            FileClass::Authored
                if e.path.as_str().ends_with(".md")
                    && e.path
                        .parent()
                        .is_some_and(|d| d.as_str() == format!("{}/versions", root.folder)) =>
            {
                if let Some(bytes) = read_regular(root, &e.path)? {
                    facts.version_texts.insert(
                        e.path.clone(),
                        Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()),
                    );
                }
            }
            FileClass::VendorOwned => {
                if let Some(h) = hash_file(root, &e.path, e.size)? {
                    facts.vendor_hashes.insert(e.path.clone(), h);
                }
            }
            _ => {}
        }
    }
    if let Some(rc) = controls.root_control() {
        for target in &rc.agents {
            let Ok(p) = RepoPath::new(target) else {
                continue;
            };
            if let Some(bytes) = read_regular(root, &p)? {
                facts.agent_files.insert(
                    p,
                    Arc::<str>::from(String::from_utf8_lossy(&bytes).into_owned()),
                );
            }
        }
    }
    for candidate in [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"] {
        let Ok(p) = RepoPath::new(candidate) else {
            continue;
        };
        if let Some(bytes) = read_regular(root, &p)? {
            facts.codeowners = Some(Arc::<str>::from(
                String::from_utf8_lossy(&bytes).into_owned(),
            ));
            break;
        }
    }
    let approvals_rel = RepoPath::new(&format!("{}/approvals.toml", root.folder))
        .map_err(|e| Error::Internal(format!("approvals path: {e}")))?;
    if let Some(bytes) = read_regular(root, &approvals_rel)? {
        facts.approvals =
            parse_approvals(&bytes).map_err(|f| Error::from(f.with_path(&approvals_rel)))?;
    }
    facts.git = git;
    let journal = root.dir.join(".fairlead").join("sync.journal");
    facts.pending_journal = journal.as_std_path().is_file();
    facts.pending_move = facts.pending_journal
        && std::fs::read(journal.as_std_path())
            .ok()
            .and_then(|b| serde_json::from_slice::<crate::write::JournalDoc>(&b).ok())
            .is_some_and(|d| d.kind.as_deref() == Some("Move"));
    facts.manifest_hints = read_manifest_hints(root, walk);
    facts.mounts = read_mounts(root, controls);
    Ok(facts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_controls;
    use crate::hash::{hash_normalised, hash_raw};
    use crate::testutil::{ROOT_CONTROL, test_root, utf8_tmp, write_bytes, write_file};
    use crate::walk::{WalkOptions, walk};

    struct F {
        _g: tempfile::TempDir,
        dir: camino::Utf8PathBuf,
        root: Root,
        walked: WalkResult,
        set: ControlSet,
    }

    fn build(extra_root: &str, files: &[(&str, &str)]) -> F {
        let (g, dir) = utf8_tmp();
        write_file(
            &dir,
            "fairlead/control.toml",
            &format!("{ROOT_CONTROL}{extra_root}"),
        );
        for (p, c) in files {
            write_file(&dir, p, c);
        }
        let root = test_root(&dir);
        let walked = walk(&root, &WalkOptions::default()).unwrap();
        let set = load_controls(&root, &walked).unwrap();
        F {
            _g: g,
            dir,
            root,
            walked,
            set,
        }
    }

    fn facts(f: &F, want_changed: bool) -> Facts {
        collect_facts(&f.root, &f.walked, &f.set, want_changed).unwrap()
    }

    fn p(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    #[test]
    fn r_file_007__generated_files_are_read_raw_and_include_the_baseline_and_lock() {
        let f = build(
            "",
            &[
                (
                    "fairlead/gen/reg/tok.reg",
                    "# scope root edition=1\r\ntok.a css=--a\r\n",
                ),
                ("fairlead/gen/baseline.txt", "# baseline\n"),
                ("fairlead/vendored.lock", "# lock\n"),
                ("fairlead/docs/adr.md", "authored"),
                ("src/a.rs", "authored"),
            ],
        );
        let x = facts(&f, false);
        let keys: Vec<&str> = x.generated_on_disk.keys().map(RepoPath::as_str).collect();
        assert_eq!(
            keys,
            [
                "fairlead/gen/baseline.txt",
                "fairlead/gen/reg/tok.reg",
                "fairlead/vendored.lock"
            ]
        );
        assert_eq!(
            &*x.generated_on_disk[&p("fairlead/gen/reg/tok.reg")],
            b"# scope root edition=1\r\ntok.a css=--a\r\n"
        );
    }

    #[test]
    fn r_agt_002__agent_files_are_the_existing_targets_only() {
        let f = build(
            "[agents]\ntargets = [\"AGENTS.md\", \"CLAUDE.md\", \"docs/AI.md\"]\n",
            &[("AGENTS.md", "hello\r\n"), ("docs/AI.md", "x")],
        );
        let x = facts(&f, false);
        assert_eq!(
            x.agent_files
                .keys()
                .map(RepoPath::as_str)
                .collect::<Vec<_>>(),
            ["AGENTS.md", "docs/AI.md"]
        );
        assert_eq!(&*x.agent_files[&p("AGENTS.md")], "hello\r\n");
    }

    #[cfg(unix)]
    #[test]
    fn r_agt_002__a_symlinked_agent_file_is_not_followed() {
        let f = build(
            "[agents]\ntargets = [\"AGENTS.md\"]\n",
            &[("real.md", "secret")],
        );
        std::os::unix::fs::symlink(
            f.dir.join("real.md").as_std_path(),
            f.dir.join("AGENTS.md").as_std_path(),
        )
        .unwrap();
        assert!(facts(&f, false).agent_files.is_empty());
    }

    #[test]
    fn r_chk_053__codeowners_uses_githubs_search_order() {
        let f = build("", &[("CODEOWNERS", "root"), ("docs/CODEOWNERS", "docs")]);
        assert_eq!(facts(&f, false).codeowners.as_deref(), Some("root"));
        let g = build("", &[(".github/CODEOWNERS", "gh"), ("CODEOWNERS", "root")]);
        assert_eq!(facts(&g, false).codeowners.as_deref(), Some("gh"));
        assert_eq!(facts(&build("", &[]), false).codeowners, None);
    }

    #[test]
    fn r_file_005__vendor_files_are_hashed_normalised_for_text_and_raw_for_binary() {
        let f = build(
            "[[vendor]]\npath = \"ui/**\"\ntool = \"shadcn\"\n",
            &[
                ("ui/a.tsx", "line\r\nline\r\n"),
                ("ui/b.tsx", "line\nline\n"),
                ("src/own.rs", "x"),
            ],
        );
        write_bytes(&f.dir, "ui/font.woff", b"\x00\r\n\xff");
        let walked = walk(&f.root, &WalkOptions::default()).unwrap();
        let x = collect_facts(&f.root, &walked, &f.set, false).unwrap();
        assert_eq!(
            x.vendor_hashes[&p("ui/a.tsx")],
            hash_normalised(b"line\nline\n")
        );
        assert_eq!(
            x.vendor_hashes[&p("ui/a.tsx")],
            x.vendor_hashes[&p("ui/b.tsx")],
            "autocrlf cannot cause drift (R-FILE-024)"
        );
        assert_eq!(
            x.vendor_hashes[&p("ui/font.woff")],
            hash_raw(b"\x00\r\n\xff")
        );
        assert!(!x.vendor_hashes.contains_key(&p("src/own.rs")));
    }

    #[test]
    fn r_file_005__vendored_lock_lines_are_parsed_tolerantly() {
        let h = hash_raw(b"a");
        let text = format!(
            "# scope root edition=1 body=00000000000000000000
vendor.aaaaaaaaaa path=ui/a.tsx tool=shadcn b3={h}

broken line
vendor.bbbbbbbbbb path=../escape tool=x b3={h}
vendor.cccccccccc path=ui/b.tsx tool=x b3=nothex
vendor.dddddddddd path=ui/c.tsx b3={h}
ui/old.tsx tool=x h={h}
",
            h = h.to_hex()
        );
        let m = parse_vendor_lock("", &text);
        assert_eq!(
            m.keys().map(RepoPath::as_str).collect::<Vec<_>>(),
            ["ui/a.tsx", "ui/c.tsx"]
        );
        assert_eq!(m[&p("ui/a.tsx")], h);
        assert_eq!(
            parse_vendor_lock("web", &text)
                .keys()
                .next()
                .map(RepoPath::as_str),
            Some("web/ui/a.tsx")
        );
        let f = build(
            "[[vendor]]
path = \"ui/**\"
tool = \"x\"
",
            &[("fairlead/vendored.lock", text.as_str()), ("ui/a.tsx", "x")],
        );
        assert_eq!(facts(&f, false).vendor_lock.len(), 2);
    }

    #[test]
    fn r_cli_037__approvals_load_absent_means_empty_and_broken_fails_closed() {
        let good = "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"root\"\napprover = \"@a\"\ndate = 2026-10-09\n";
        let f = build("", &[("fairlead/approvals.toml", good)]);
        assert_eq!(facts(&f, false).approvals.approvals.len(), 1);
        assert!(facts(&build("", &[]), false).approvals.approvals.is_empty());
        let bad = build("", &[("fairlead/approvals.toml", "[[approval]\n")]);
        let e = collect_facts(&bad.root, &bad.walked, &bad.set, false).unwrap_err();
        assert!(
            matches!(&e, Error::Config(c) if c.path == p("fairlead/approvals.toml")),
            "{e}"
        );
        assert_eq!(e.class(), crate::error::ErrorClass::Usage);
    }

    #[test]
    fn r_arch_025__outside_a_repository_git_is_unavailable_and_changed_can_never_pass() {
        let f = build("", &[("src/a.rs", "x")]);
        let quiet = facts(&f, false);
        assert!(
            !quiet.git.available && quiet.git.changed.is_none() && quiet.git.unreadable.is_none()
        );
        let asked = facts(&f, true);
        assert!(!asked.git.available);
        assert!(asked.git.changed.is_none());
        assert!(
            asked
                .git
                .unreadable
                .as_deref()
                .is_some_and(|m| m.contains("not inside a git repository")),
            "{:?}",
            asked.git
        );
    }

    #[test]
    fn r_arch_075__changed_is_computed_only_when_asked() {
        let f = build("", &[("src/a.rs", "1")]);
        crate::git::test_support::init(&f.dir);
        crate::git::test_support::commit_all(&f.dir);
        write_file(&f.dir, "src/a.rs", "22");
        write_file(&f.dir, "src/new.rs", "x");
        let without = facts(&f, false);
        assert!(
            without.git.available
                && without.git.changed.is_none()
                && without.git.unreadable.is_none()
        );
        let walked = walk(&f.root, &WalkOptions::default()).unwrap();
        let with = collect_facts(&f.root, &walked, &f.set, true).unwrap();
        let changed: Vec<&str> = with
            .git
            .changed
            .as_ref()
            .unwrap()
            .0
            .iter()
            .map(RepoPath::as_str)
            .collect();
        assert_eq!(changed, ["src/a.rs", "src/new.rs"]);
        assert!(with.git.sparse_absent.is_empty());
    }

    #[test]
    fn r_arch_102__unreadable_git_state_is_reported_not_swallowed() {
        let f = build("", &[("src/a.rs", "1")]);
        crate::git::test_support::init(&f.dir);
        crate::git::test_support::commit_all(&f.dir);
        crate::git::test_support::corrupt_index(&f.dir);
        let x = facts(&f, true);
        assert!(x.git.available);
        assert!(x.git.changed.is_none());
        assert!(
            x.git.unreadable.is_some(),
            "P5 turns this into exit 5 under --changed"
        );
    }

    #[test]
    fn r_arch_006__manifest_hints_are_part_of_the_facts() {
        let f = build(
            "",
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n"),
                ("crates/a/Cargo.toml", "[package]\nname = \"a\"\n"),
            ],
        );
        let x = facts(&f, false);
        assert_eq!(
            x.manifest_hints
                .workspace_members
                .iter()
                .map(RepoPath::as_str)
                .collect::<Vec<_>>(),
            ["crates/a"]
        );
    }

    #[test]
    fn r_arch_060__facts_default_is_empty_for_pure_tests_in_other_phases() {
        let d = Facts::default();
        assert!(
            d.generated_on_disk.is_empty()
                && d.agent_files.is_empty()
                && d.vendor_hashes.is_empty()
        );
        assert!(!d.git.available && d.codeowners.is_none());
    }
}
