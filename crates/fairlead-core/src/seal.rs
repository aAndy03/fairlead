//! `fairlead seal`, steps 1 to 3 of R-VER-050: preflight, compute, due gate. Pure: every fact
//! about git and the check run is passed in, so the planner reads no disk and no clock.

use std::collections::{BTreeMap, BTreeSet};

use crate::docs::{ChangeDoc, DraftId, Status, assign_ordinals, readiness_problems};
use crate::path::RepoPath;
use crate::version::{Version, VersionLine};

/// Git state the preflight needs (R-GIT-063 to R-GIT-065).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SealGit {
    pub available: bool,
    pub detached: bool,
    pub branch: Option<String>,
    pub unmerged: Vec<RepoPath>,
    /// Changed files against HEAD, untracked included (the 0.1 changed set).
    pub changed: Vec<RepoPath>,
}

/// Everything the planner reads.
#[derive(Clone, Debug)]
pub struct SealInput<'a> {
    pub line: VersionLine,
    pub docs: &'a [ChangeDoc],
    /// Findings at or above the seal gate (errors) in the check run.
    pub gate_errors: usize,
    /// `HN-F001` secret-like values, which always refuse (R-RET-006).
    pub secrets: usize,
    pub git: SealGit,
    pub allow_dirty: bool,
    /// `version.seal-where`: `true` for `any`, `false` for `integration` (R-VER-055).
    pub seal_anywhere: bool,
    pub integration_branch: String,
    /// The root managerial folder, to tell the seal's own write-set from other changes.
    pub folder: String,
    /// Every path in the tree, for the sweep report (R-VER-045).
    pub paths: &'a BTreeSet<RepoPath>,
    /// `version.sweep-after` (default 3).
    pub sweep_after: usize,
    /// Every sealed version, for the carried counts of the sweep report.
    pub sealed_versions: &'a [Version],
}

/// The result of steps 1 to 3: what the transaction (P1-T9) writes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SealPlan {
    pub prev: Option<Version>,
    pub open: Version,
    pub bump: &'static str,
    pub sealing: Vec<ChangeDoc>,
    pub carried: Vec<ChangeDoc>,
    pub ordinals: BTreeMap<DraftId, u32>,
    /// Advisory lines (R-VER-045): orphaned or long-carried drafts.
    pub sweep: Vec<String>,
    /// `--allow-dirty` sealed a dirty tree (recorded as `dirty = true`).
    pub dirty: bool,
}

/// Why the seal refused, before anything was written (exit 4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SealRefusal {
    pub message: String,
    pub hint: String,
}

fn refuse(message: impl Into<String>, hint: impl Into<String>) -> SealRefusal {
    SealRefusal {
        message: message.into(),
        hint: hint.into(),
    }
}

fn bump_word(prev: Option<Version>, open: Version) -> &'static str {
    match prev {
        None => "start",
        Some(p) if open.major > p.major => "major",
        Some(p) if open.minor > p.minor => "minor",
        Some(_) => "patch",
    }
}

/// Files a seal may find changed: its own outputs (generated, ledger, version files). Anything
/// else changed means a dirty tree (R-GIT-063).
fn is_seal_output(p: &RepoPath, folder: &str) -> bool {
    let s = p.as_str();
    let mine = |rest: &str| s.contains(&format!("{folder}/{rest}"));
    mine("gen/") || mine("ledger/") || s.starts_with(&format!("{folder}/versions/"))
}

/// A `covers` entry that is an ID counts as existing (IDs are validated by check); a glob must
/// match a path of the tree.
fn covers_anything(d: &ChangeDoc, paths: &BTreeSet<RepoPath>) -> bool {
    d.covers.iter().any(|c| {
        fairlead_syntax::ids::Id::parse(c).is_ok()
            || globset::Glob::new(c).is_ok_and(|g| {
                let m = g.compile_matcher();
                paths.iter().any(|p| m.is_match(p.as_str()))
            })
    })
}

/// Steps 1 to 3 of R-VER-050. Any refusal leaves the tree unchanged.
pub fn plan(input: &SealInput<'_>) -> Result<SealPlan, SealRefusal> {
    // 1. preflight
    if input.git.available {
        if input.git.detached {
            return Err(refuse(
                "HEAD is detached",
                "check out a branch first (R-GIT-065)",
            ));
        }
        if !input.git.unmerged.is_empty() {
            return Err(refuse(
                format!("{} unmerged paths", input.git.unmerged.len()),
                "finish the merge first (R-GIT-064)",
            ));
        }
        if !input.seal_anywhere
            && input.git.branch.as_deref() != Some(input.integration_branch.as_str())
        {
            return Err(refuse(
                format!(
                    "seal runs on the integration branch {} (version.seal-where = \"integration\")",
                    input.integration_branch
                ),
                "seal on the integration branch, or set seal-where = \"any\" for solo work (R-VER-055)",
            ));
        }
    }
    let dirty: Vec<&RepoPath> = input
        .git
        .changed
        .iter()
        .filter(|p| !is_seal_output(p, &input.folder))
        .collect();
    if !dirty.is_empty() && !input.allow_dirty {
        return Err(refuse(
            format!(
                "{} changed files outside the seal write-set, first: {}",
                dirty.len(),
                dirty[0].as_str()
            ),
            "commit or stash them, or pass --allow-dirty (recorded as dirty = true)",
        ));
    }
    if input.secrets > 0 {
        return Err(refuse(
            format!("{} secret-like values in records (HN-F001)", input.secrets),
            "remove them; a seal never records a secret (R-RET-006)",
        ));
    }
    if input.gate_errors > 0 {
        return Err(refuse(
            format!("check reports {} errors", input.gate_errors),
            "fix them, or run fairlead check to see them",
        ));
    }
    let ready: Vec<&ChangeDoc> = input
        .docs
        .iter()
        .filter(|d| d.status == Status::Ready)
        .collect();
    if let Some(d) = ready.iter().find(|d| !readiness_problems(d).is_empty()) {
        return Err(refuse(
            format!(
                "ready doc {} is incomplete: {}",
                d.id,
                readiness_problems(d).join(", ")
            ),
            "fill it, or set it back to draft (R-VER-031)",
        ));
    }
    if ready.is_empty() {
        return Err(refuse(
            "no change doc is ready",
            "set status = \"ready\" on the docs this version closes",
        ));
    }
    if input.sealed_versions.contains(&input.line.open) {
        return Err(refuse(
            format!("version {} is already sealed", input.line.open),
            "a version file is immutable (R-VER-054)",
        ));
    }

    // 2. compute: O from the impacts of the docs being sealed, ordinals
    let impact = ready.iter().filter_map(|d| d.impact).max();
    let open = match impact {
        Some(i) => crate::version::bump(input.line.sealed, i, input.line.open),
        None => input.line.open,
    };
    let existing: Vec<&ChangeDoc> = input
        .docs
        .iter()
        .filter(|d| d.status == Status::Sealed)
        .collect();
    let ordinals = assign_ordinals(&ready, &existing);

    // 3. due gate: lifecycle due items and expiring exceptions arrive with P3-T6.

    let carried: Vec<ChangeDoc> = input
        .docs
        .iter()
        .filter(|d| d.status == Status::Draft)
        .cloned()
        .collect();
    let mut sweep = Vec::new();
    for d in &carried {
        if !d.covers.is_empty() && !covers_anything(d, input.paths) {
            sweep.push(format!(
                "{} covers nothing that exists ({})",
                d.id,
                d.path.as_str()
            ));
        }
        let created = d.created.as_deref().and_then(Version::parse);
        let seals = input
            .sealed_versions
            .iter()
            .filter(|v| created.is_some_and(|c| **v >= c))
            .count();
        if seals > input.sweep_after {
            sweep.push(format!(
                "{} carried through {seals} seals ({})",
                d.id,
                d.path.as_str()
            ));
        }
    }
    Ok(SealPlan {
        prev: input.line.sealed,
        open,
        bump: bump_word(input.line.sealed, open),
        sealing: ready.into_iter().cloned().collect(),
        carried,
        ordinals,
        sweep,
        dirty: !dirty.is_empty(),
    })
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::version::Impact;

    fn doc(id: &str, status: Status, impact: Option<Impact>, created: &str) -> ChangeDoc {
        ChangeDoc {
            path: RepoPath::new(&format!("fairlead/docs/changes/{id}-x.md")).unwrap(),
            id: DraftId::parse(id).unwrap(),
            kind: "change".into(),
            status,
            impact,
            scope: Some("root".into()),
            covers: vec!["src/**".into()],
            created: Some(created.into()),
            sealed: None,
            body_hash: None,
            ordinal: None,
            rolling: false,
            hotfix: false,
            extra: BTreeMap::new(),
        }
    }

    struct Fx {
        docs: Vec<ChangeDoc>,
        paths: BTreeSet<RepoPath>,
        versions: Vec<Version>,
        git: SealGit,
        allow_dirty: bool,
        gate_errors: usize,
    }

    impl Fx {
        fn new(docs: Vec<ChangeDoc>) -> Fx {
            Fx {
                docs,
                paths: [RepoPath::new("src/a.rs").unwrap()].into_iter().collect(),
                versions: vec![Version::new(0, 2, 0)],
                git: SealGit {
                    available: true,
                    branch: Some("main".into()),
                    ..SealGit::default()
                },
                allow_dirty: false,
                gate_errors: 0,
            }
        }
        fn run(&self) -> Result<SealPlan, SealRefusal> {
            let line = crate::version::derive_line(
                self.versions.iter().copied(),
                self.docs
                    .iter()
                    .filter(|d| d.status.is_pending())
                    .map(|d| d.impact.unwrap_or(Impact::Fix)),
                Version::new(0, 1, 0),
            );
            plan(&SealInput {
                line,
                docs: &self.docs,
                gate_errors: self.gate_errors,
                secrets: 0,
                git: self.git.clone(),
                allow_dirty: self.allow_dirty,
                seal_anywhere: false,
                integration_branch: "main".into(),
                folder: "fairlead".into(),
                paths: &self.paths,
                sweep_after: 3,
                sealed_versions: &self.versions,
            })
        }
    }

    fn ready() -> ChangeDoc {
        doc("d-00000001", Status::Ready, Some(Impact::Add), "0.2.1")
    }

    #[test]
    fn r_ver_050__seals_ready_docs_at_the_bumped_version() {
        let p = Fx::new(vec![
            ready(),
            doc("d-00000002", Status::Draft, None, "0.2.1"),
        ])
        .run()
        .unwrap();
        assert_eq!(
            (p.prev, p.open, p.bump),
            (Some(Version::new(0, 2, 0)), Version::new(0, 2, 1), "patch")
        );
        assert_eq!(p.sealing.len(), 1);
        assert_eq!(p.ordinals[&DraftId::parse("d-00000001").unwrap()], 1);
        assert_eq!(p.carried.len(), 1, "R-VER-043: the draft is carried");
    }

    #[test]
    fn r_ver_050__refuses_when_head_is_detached() {
        let mut f = Fx::new(vec![ready()]);
        f.git.detached = true;
        assert!(f.run().unwrap_err().message.contains("detached"));
    }

    #[test]
    fn r_ver_050__refuses_when_paths_are_unmerged() {
        let mut f = Fx::new(vec![ready()]);
        f.git.unmerged = vec![RepoPath::new("src/a.rs").unwrap()];
        assert!(f.run().unwrap_err().message.contains("unmerged"));
    }

    #[test]
    fn r_ver_050__refuses_a_dirty_tree_unless_allowed_and_records_it() {
        let mut f = Fx::new(vec![ready()]);
        f.git.changed = vec![
            RepoPath::new("src/a.rs").unwrap(),
            RepoPath::new("fairlead/gen/members").unwrap(),
        ];
        assert!(
            f.run()
                .unwrap_err()
                .message
                .contains("outside the seal write-set")
        );
        f.allow_dirty = true;
        assert!(f.run().unwrap().dirty);
        f.git.changed = vec![RepoPath::new("fairlead/gen/members").unwrap()];
        f.allow_dirty = false;
        assert!(
            !f.run().unwrap().dirty,
            "the seal's own outputs are not dirt"
        );
    }

    #[test]
    fn r_ver_050__refuses_on_check_errors_and_with_nothing_ready() {
        let mut f = Fx::new(vec![ready()]);
        f.gate_errors = 2;
        assert!(f.run().unwrap_err().message.contains("2 errors"));
        let f = Fx::new(vec![doc("d-00000002", Status::Draft, None, "0.2.1")]);
        assert!(
            f.run()
                .unwrap_err()
                .message
                .contains("no change doc is ready")
        );
    }

    #[test]
    fn r_ver_055__integration_branch_only_by_default() {
        let mut f = Fx::new(vec![ready()]);
        f.git.branch = Some("feature/x".into());
        assert!(f.run().unwrap_err().message.contains("integration branch"));
    }

    #[test]
    fn r_ver_031__incomplete_ready_doc_refuses() {
        let mut d = ready();
        d.covers.clear();
        assert!(
            Fx::new(vec![d])
                .run()
                .unwrap_err()
                .message
                .contains("incomplete")
        );
    }

    #[test]
    fn r_ver_045__sweep_report_is_advisory() {
        let mut orphan = doc("d-00000002", Status::Draft, None, "0.1.0");
        orphan.covers = vec!["gone/**".into()];
        let mut f = Fx::new(vec![ready(), orphan]);
        f.versions = vec![
            Version::new(0, 1, 0),
            Version::new(0, 1, 1),
            Version::new(0, 1, 2),
            Version::new(0, 2, 0),
        ];
        let p = f.run().unwrap();
        assert!(
            p.sweep.iter().any(|s| s.contains("covers nothing")),
            "{:?}",
            p.sweep
        );
        assert!(
            p.sweep
                .iter()
                .any(|s| s.contains("carried through 4 seals")),
            "{:?}",
            p.sweep
        );
        assert_eq!(p.carried.len(), 1, "never abandoned automatically");
    }

    #[test]
    fn r_ver_006__first_seal_of_a_fresh_codebase_is_the_start_version() {
        let mut f = Fx::new(vec![doc(
            "d-00000001",
            Status::Ready,
            Some(Impact::Break),
            "0.1.0",
        )]);
        f.versions.clear();
        let p = f.run().unwrap();
        assert_eq!(
            (p.prev, p.open, p.bump),
            (None, Version::new(0, 1, 0), "start")
        );
    }
}
