//! Write layer (P7). The only write path is `Transaction` (R-ARCH-141..146).
mod approve;
mod init;
mod journal;
mod lock;
mod new;
mod sync;
mod tx;
mod writeset;
pub use {approve::*, init::*, journal::*, lock::*, new::*, sync::*, tx::*, writeset::*};

use crate::error::{Error, IoFailure, Refusal, RefusalKind};
use crate::path::RepoPath;

pub(crate) fn refuse(
    kind: RefusalKind,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> Error {
    Error::Refused(Refusal {
        kind,
        message: message.into(),
        hint: hint.into(),
    })
}

/// Single place that builds `Error::Io`. Verify: P3 defines `IoFailure`; this assumes
/// `IoFailure::from_io(op, path, &io::Error)`. Adapt only this helper if P3 shaped it otherwise.
pub(crate) fn io_err(op: &'static str, path: &str, e: &std::io::Error) -> Error {
    Error::Io(IoFailure {
        path: path.to_string(),
        op,
        kind: e.kind(),
        detail: e.to_string(),
        user_path: false,
    })
}

pub(crate) fn repo_path(s: &str) -> Result<RepoPath, Error> {
    RepoPath::new(s).map_err(|e| Error::Internal(format!("invalid planned path {s:?}: {e:?}")))
}

#[allow(dead_code, reason = "used by the new and sync planners (P7-T10, T14)")]
/// Line diff for plan display: ` ` context, `-` removed, `+` added; at most `cap` lines.
pub(crate) fn plain_diff(old: &str, new: &str, cap: usize) -> String {
    use similar::{ChangeTag, TextDiff};
    let mut out = String::new();
    let mut shown = 0usize;
    let mut total = 0usize;
    for ch in TextDiff::from_lines(old, new).iter_all_changes() {
        total += 1;
        if shown < cap {
            let sign = match ch.tag() {
                ChangeTag::Equal => ' ',
                ChangeTag::Delete => '-',
                ChangeTag::Insert => '+',
            };
            out.push(sign);
            out.push_str(ch.value().trim_end_matches('\n'));
            out.push('\n');
            shown += 1;
        }
    }
    if total > shown {
        out.push_str(&format!("... {} more lines\n", total - shown));
    }
    out
}

use crate::hash::ContentHash;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PlanAction {
    Create,
    Edit,
    Skip,
    Quarantine,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlanLine {
    pub action: PlanAction,
    pub path: RepoPath,
    pub detail: String,
    pub diff: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Plan {
    pub verb: WriteSetKind,
    pub set: WriteSet,
    pub lines: Vec<PlanLine>,
    pub plan_hash: ContentHash,
    pub notes: Vec<String>,
}

fn rank(a: PlanAction) -> u8 {
    match a {
        PlanAction::Create => 0,
        PlanAction::Edit => 1,
        PlanAction::Quarantine => 2,
        PlanAction::Skip => 3,
    }
}

fn kind_tag(k: &WriteKind) -> String {
    match k {
        WriteKind::Create => "create".into(),
        WriteKind::Replace => "replace".into(),
        WriteKind::AppendBlock { marker } => format!("append:{marker}"),
        WriteKind::Delete => "delete".into(),
    }
}

/// blake3 over verb, then (path, kind, length, bytes) of every write in path order (R-CLI-005).
pub fn plan_hash(set: &WriteSet) -> ContentHash {
    let mut h = blake3::Hasher::new();
    h.update(format!("{:?}\0", set.kind).as_bytes());
    for w in set.writes.values() {
        h.update(w.path.as_str().as_bytes());
        h.update(b"\0");
        h.update(kind_tag(&w.kind).as_bytes());
        h.update(b"\0");
        h.update(&(w.bytes.len() as u64).to_le_bytes());
        h.update(&w.bytes);
    }
    ContentHash(*h.finalize().as_bytes())
}

impl Plan {
    /// Sorts lines (create, edit, quarantine, skip; then path) and computes the plan hash.
    pub fn new(set: WriteSet, mut lines: Vec<PlanLine>, notes: Vec<String>) -> Plan {
        lines.sort_by(|a, b| (rank(a.action), &a.path).cmp(&(rank(b.action), &b.path)));
        Plan {
            verb: set.kind,
            plan_hash: plan_hash(&set),
            set,
            lines,
            notes,
        }
    }
    pub fn is_noop(&self) -> bool {
        self.set.is_empty()
    }

    /// R-CLI-026: a Quarantine line whose file has no Replace in the set means "refused, not overwritten".
    pub fn refusal(&self) -> Option<Refusal> {
        let blocked: Vec<&str> = self
            .lines
            .iter()
            .filter(|l| {
                l.action == PlanAction::Quarantine && !self.set.writes.contains_key(&l.path)
            })
            .map(|l| l.path.as_str())
            .collect();
        if blocked.is_empty() {
            return None;
        }
        Some(Refusal {
            kind: RefusalKind::HandEditedGenerated,
            message: format!("{} hand-edited generated file(s) not overwritten: {}", blocked.len(), blocked.join(", ")),
            hint: "review the copy under .fairlead/quarantine/, then re-run with `fairlead sync --force`".into(),
        })
    }

    pub fn render_text(&self, title: &str) -> String {
        let mut s = format!("{title}\n");
        for l in &self.lines {
            let word = match l.action {
                PlanAction::Create => "create",
                PlanAction::Edit => "edit",
                PlanAction::Skip => "skip",
                PlanAction::Quarantine => "quarantine",
            };
            s.push_str(&format!("{word:<11}{}  ({})\n", l.path.as_str(), l.detail));
        }
        for n in &self.notes {
            s.push_str(n);
            s.push('\n');
        }
        s
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Gate {
    Apply,
    DryRun,
    NotConfirmed,
}

/// R-CLI-002, R-CLI-043: `--dry-run` always wins; only `init` is plan-first; `--yes` is a no-op elsewhere.
pub fn gate(verb: WriteSetKind, yes: bool, dry_run: bool) -> Gate {
    if dry_run {
        return Gate::DryRun;
    }
    match verb {
        WriteSetKind::Init if !yes => Gate::NotConfirmed,
        _ => Gate::Apply,
    }
}
pub fn gate_error(g: Gate) -> Option<Error> {
    (g == Gate::NotConfirmed).then(|| {
        refuse(
            RefusalKind::PlanNotConfirmed,
            "plan not applied",
            "re-run with --yes to apply, or --dry-run to preview",
        )
    })
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use crate::classify::FileClass;
    use crate::config::{ApprovalsFile, ControlSet, parse_control};
    use crate::discover::{Edition, Pin, Root, RootSource};
    use crate::extract::Corpus;
    use crate::facts::{Facts, GitFacts};
    use crate::hash::ContentHash;
    use crate::manifest::ManifestHints;
    use crate::registry::{Registry, ResolveInput, resolve};
    use crate::time::AsOf;
    use camino::{Utf8Path, Utf8PathBuf};
    use std::collections::{BTreeMap, BTreeSet};

    pub const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n\
[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1\"\nowners = [\"@lead\"]\n\n\
[kinds.adr]\ntitle = \"decision\"\nshape = \"doc\"\n\n[kinds.cmp]\ntitle = \"component\"\nshape = \"code\"\n";
    pub const WEB_CONTROL: &str = "[fairlead]\nscope = \"web\"\n";

    pub fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }
    pub fn test_root(dir: &std::path::Path) -> Root {
        Root {
            dir: Utf8PathBuf::from_path_buf(dir.to_path_buf()).unwrap(),
            folder: "fairlead".into(),
            source: RootSource::Flag,
            edition: Edition(1),
            pin: Pin {
                version: None,
                edition: Edition(1),
            },
            config_hash: ContentHash([0; 32]),
        }
    }
    pub fn pw(path: &str, kind: WriteKind, bytes: &str) -> PlannedWrite {
        PlannedWrite {
            path: rp(path),
            kind,
            bytes: bytes.as_bytes().to_vec(),
            class: FileClass::Authored,
            expect_prior: None,
        }
    }
    pub fn facts_empty() -> Facts {
        Facts {
            generated_on_disk: BTreeMap::new(),
            agent_files: BTreeMap::new(),
            codeowners: None,
            vendor_hashes: BTreeMap::new(),
            vendor_lock: BTreeMap::new(),
            approvals: ApprovalsFile { approvals: vec![] },
            git: GitFacts {
                available: false,
                changed: None,
                sparse_absent: vec![],
                unreadable: None,
                coverage: None,
            },
            ledgers: Default::default(),
            doc_body_hashes: Default::default(),
            version_texts: Default::default(),
            pending_journal: false,
            pending_move: false,
            tombstones: Default::default(),
            authority: Vec::new(),
            manifest_hints: ManifestHints {
                workspace_members: BTreeSet::new(),
                default_excludes: vec![],
            },
        }
    }
    /// In-memory registry from `(repo path of control.toml, text)` pairs; no disk.
    pub fn registry(controls: &[(&str, &str)], as_of: &str) -> (Root, Registry) {
        registry_with(controls, as_of, facts_empty())
    }
    pub fn registry_with(controls: &[(&str, &str)], as_of: &str, facts: Facts) -> (Root, Registry) {
        let root = test_root(std::path::Path::new("/repo"));
        let mut files = BTreeMap::new();
        for (path, text) in controls {
            let p = rp(path);
            files.insert(
                p.clone(),
                parse_control(&p, text.as_bytes(), *path == "fairlead/control.toml"),
            );
        }
        let controls = ControlSet { files };
        let corpus = Corpus {
            records: BTreeMap::new(),
        };
        let reg = resolve(&ResolveInput {
            root: &root,
            controls: &controls,
            corpus: &corpus,
            facts: &facts,
            as_of: AsOf::parse(as_of).unwrap(),
        });
        (root, reg)
    }
    use std::cell::Cell;
    pub struct FailingFs {
        inner: RealFs,
        fail_write_on: Option<usize>,
        fail_renames: Option<(usize, usize)>, // inclusive 1-based call range
        writes: Cell<usize>,
        renames: Cell<usize>,
    }
    impl FailingFs {
        pub fn new() -> Self {
            FailingFs {
                inner: RealFs,
                fail_write_on: None,
                fail_renames: None,
                writes: Cell::new(0),
                renames: Cell::new(0),
            }
        }
        pub fn write_fails_on(mut self, n: usize) -> Self {
            self.fail_write_on = Some(n);
            self
        }
        pub fn renames_fail(mut self, from: usize, to: usize) -> Self {
            self.fail_renames = Some((from, to));
            self
        }
        pub fn rename_calls(&self) -> usize {
            self.renames.get()
        }
    }
    impl Fs for FailingFs {
        fn read(&self, p: &Utf8Path) -> std::io::Result<Option<Vec<u8>>> {
            self.inner.read(p)
        }
        fn create_dir_all(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.inner.create_dir_all(p)
        }
        fn write_new(&self, p: &Utf8Path, b: &[u8]) -> std::io::Result<()> {
            self.writes.set(self.writes.get() + 1);
            if Some(self.writes.get()) == self.fail_write_on {
                return Err(std::io::Error::other("injected write failure"));
            }
            self.inner.write_new(p, b)
        }
        fn rename(&self, a: &Utf8Path, b: &Utf8Path) -> std::io::Result<()> {
            self.renames.set(self.renames.get() + 1);
            let n = self.renames.get();
            if self.fail_renames.is_some_and(|(f, t)| (f..=t).contains(&n)) {
                return Err(std::io::Error::other("injected rename failure"));
            }
            self.inner.rename(a, b)
        }
        fn remove_file(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.inner.remove_file(p)
        }
        fn set_executable(&self, p: &Utf8Path) -> std::io::Result<()> {
            self.inner.set_executable(p)
        }
        fn pause(&self, _ms: u64) {}
    }
    pub fn tree(dir: &std::path::Path) -> Vec<String> {
        fn walk(d: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(d).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, base, out)
                } else {
                    out.push(
                        p.strip_prefix(base)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    )
                }
            }
        }
        let mut v = Vec::new();
        walk(dir, dir, &mut v);
        v.sort();
        v
    }
    use crate::write::{LivenessAnswer, PidLiveness};
    pub struct FakeProbe {
        pub host: String,
        pub answers: BTreeMap<u32, LivenessAnswer>,
        pub tokens: BTreeMap<u32, String>,
    }
    impl FakeProbe {
        pub fn new(host: &str) -> Self {
            FakeProbe {
                host: host.into(),
                answers: BTreeMap::new(),
                tokens: BTreeMap::new(),
            }
        }
    }
    impl PidLiveness for FakeProbe {
        fn is_alive(&self, pid: u32, host: &str) -> LivenessAnswer {
            if host != self.host {
                return LivenessAnswer::OtherHost;
            }
            self.answers
                .get(&pid)
                .copied()
                .unwrap_or(LivenessAnswer::Dead)
        }
        fn local_host(&self) -> String {
            self.host.clone()
        }
        fn start_token(&self, pid: u32) -> Option<String> {
            self.tokens.get(&pid).cloned()
        }
    }
    use crate::check::{CheckReport, Finding, FindingState, ReportHeader, Summary};
    pub fn finding(code: fairlead_syntax::diag::DiagCode, path: &str, fp: &str) -> Finding {
        Finding {
            code,
            severity: fairlead_syntax::diag::Severity::Error,
            stage: crate::config::Stage::Enforce,
            location: crate::diag::Location {
                path: rp(path),
                line: 1,
                col: 1,
            },
            scope: crate::registry::ScopeId(
                fairlead_syntax::ids::ScopeSlug::parse("root").unwrap(),
            ),
            rule: None,
            related: None,
            message: "m".into(),
            excerpt: None,
            intent: None,
            fix: "f".into(),
            occurrence: 0,
            fingerprint: crate::hash::Fingerprint::parse(fp).unwrap(),
            state: FindingState::New,
            waived_by: None,
        }
    }
    pub fn report(findings: Vec<Finding>) -> CheckReport {
        CheckReport {
            header: ReportHeader {
                fairlead_version: "0.1.0".into(),
                edition: Edition(1),
                as_of: AsOf::parse("2026-10-09").unwrap(),
                scopes: 1,
                files: 0,
                rules: 0,
            },
            findings,
            summary: Summary {
                errors: 0,
                warns: 0,
                infos: 0,
                baselined: 0,
                waived: 0,
                observe: 0,
                accepted: 0,
                waivers: BTreeMap::new(),
                relaxations: vec![],
                exceptions_delta: 0,
                codeowners_uncovered: 0,
                requests_open: 0,
                oldest_request_days: None,
            },
            partial: false,
            unknown_scopes: vec![],
            absent_scopes: vec![],
            zero_declarations: false,
        }
    }
    use crate::registry::{
        ApprovalStatus, FieldChange, FieldValue, ItemId, Override, OvrId, OvrOp, OvrState, ScopeId,
    };
    use fairlead_syntax::ids::{Id, ScopeSlug};
    pub fn item_id(scope: &str, kind_name: &str) -> ItemId {
        let id = Id::parse(kind_name).unwrap();
        ItemId {
            scope: ScopeId(ScopeSlug::parse(scope).unwrap()),
            kind: id.kind,
            name: id.name,
        }
    }
    pub fn ovr_fixture(file: &str, line: u32, why: &str) -> Override {
        Override {
            id: OvrId(crate::hash::Fingerprint::parse("aaaaaaaaaaaa").unwrap()),
            target: item_id("root", "rule.no-raw-hex"),
            field: None,
            op: OvrOp::Loosen,
            set: BTreeMap::from([(
                "sev".to_owned(),
                FieldChange::Replace(FieldValue::Scalar("warn".into())),
            )]),
            why: Some(why.into()),
            owner: Some("@lead".into()),
            exp: Some(jiff::civil::date(2026, 12, 31)),
            base: None,
            declared_in: ScopeId(ScopeSlug::parse("web").unwrap()),
            applies: None,
            at: crate::diag::Location {
                path: rp(file),
                line,
                col: 1,
            },
            state: OvrState::Effective,
            approval: ApprovalStatus::Missing,
            target_hash: None,
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::testutil::*;
    use super::*;

    #[test]
    fn r_arch_112__plain_diff_marks_added_and_removed_lines_with_lf_only() {
        let d = plain_diff("a\nb\n", "a\nc\n", 40);
        assert_eq!(d, " a\n-b\n+c\n");
    }

    #[test]
    fn r_arch_112__plain_diff_is_capped() {
        let new: String = (0..100).map(|i| format!("l{i}\n")).collect();
        let d = plain_diff("", &new, 5);
        assert_eq!(d.lines().count(), 6); // 5 lines + the "... N more" line
        assert!(d.ends_with("... 95 more lines\n"));
    }

    #[test]
    fn r_scope_050__testutil_registry_resolves_root_and_child() {
        let (_root, reg) = registry(
            &[
                ("fairlead/control.toml", ROOT_CONTROL),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        );
        assert!(reg.scopes().count() >= 2);
    }

    fn line(a: PlanAction, p: &str) -> PlanLine {
        PlanLine {
            action: a,
            path: rp(p),
            detail: "d".into(),
            diff: None,
        }
    }

    #[test]
    fn r_cli_012__plan_text_lists_creates_then_edits_then_skips_then_notes() {
        let plan = Plan::new(
            WriteSet::new(WriteSetKind::Init),
            vec![
                line(PlanAction::Skip, "Cargo.toml"),
                line(PlanAction::Edit, "AGENTS.md"),
                line(PlanAction::Create, "fairlead/control.toml"),
            ],
            vec!["hooks: not installed (re-run with --hooks)".into()],
        );
        let t = plan.render_text("fairlead init (plan, nothing written)");
        let order: Vec<&str> = t
            .lines()
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(order, ["fairlead", "create", "edit", "skip", "hooks:"]);
        assert!(!t.contains('\r') && t.ends_with('\n'));
    }

    #[test]
    fn r_cli_012__plan_hash_is_blake3_over_paths_and_bytes() {
        let mk = |b: &str| {
            let mut s = WriteSet::new(WriteSetKind::New);
            s.add(pw("fairlead/docs/adr/x.md", WriteKind::Create, b))
                .unwrap();
            plan_hash(&s)
        };
        assert_eq!(mk("a\n"), mk("a\n"));
        assert_ne!(mk("a\n"), mk("b\n"));
    }

    #[test]
    fn r_cli_043__gate_init_needs_yes_dry_run_wins_sync_and_new_apply_directly() {
        use WriteSetKind::*;
        assert_eq!(gate(Init, false, false), Gate::NotConfirmed);
        assert_eq!(gate(Init, true, false), Gate::Apply);
        assert_eq!(gate(Init, true, true), Gate::DryRun);
        assert_eq!(gate(Sync, false, false), Gate::Apply);
        assert_eq!(gate(New, true, false), Gate::Apply, "--yes is a no-op");
        assert_eq!(gate(New, false, true), Gate::DryRun);
        assert!(gate_error(Gate::NotConfirmed).is_some() && gate_error(Gate::Apply).is_none());
    }

    #[test]
    fn r_cli_026__quarantine_line_without_a_replace_is_a_refusal() {
        let mut blocked = Plan::new(
            WriteSet::new(WriteSetKind::Sync),
            vec![line(PlanAction::Quarantine, "fairlead/gen/reg/rule.reg")],
            vec![],
        );
        assert_eq!(
            blocked.refusal().unwrap().kind,
            RefusalKind::HandEditedGenerated
        );
        let mut set = WriteSet::new(WriteSetKind::Sync);
        set.add(pw("fairlead/gen/reg/rule.reg", WriteKind::Replace, "x\n"))
            .unwrap();
        blocked.set = set; // with --force the replace is present: no refusal
        assert!(blocked.refusal().is_none());
    }
}
