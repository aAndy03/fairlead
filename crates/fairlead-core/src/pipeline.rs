//! Composes the seams of contract Section 5. The only module that calls several stages.
//! Sequencing only: no rule, path or file-format logic lives here (R-ARCH-001).

use std::collections::BTreeSet;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use fairlead_syntax::GRAMMAR_EDITION;

use crate::cache::{Cache, CacheKey};
use crate::check::baseline::{Baseline, BaselineIssue};
use crate::check::{CheckInput, CheckOptions, CheckReport, preflight, run as check_run};
use crate::config::{ControlFile, ControlSet, config_hash_of, control_diagnostics, load_controls};
use crate::context::{
    ContextArgError, ContextInput, ContextOutput, ContextRequest, ContextTarget, NO_MATCH_MESSAGE,
    build as build_context, parse_target, precheck,
};
use crate::diag::{Diagnostic, codes, sort_diagnostics};
use crate::discover::{DiscoverRequest, Discovered, Root, discover};
use crate::error::Error;
use crate::explain::{ExplainTarget, parse_explain_target};
use crate::extract::{Corpus, ExtractPlan, extract_files};
use crate::facts::{Facts, GitFacts, collect_facts, collect_facts_with, git_facts};
use crate::path::RepoPath;
use crate::registry::{Registry, ResolveInput, resolve};
use crate::time::AsOf;
use crate::walk::{WalkOptions, WalkResult, apply_child_excludes, walk};
use crate::write::{
    ApprovePreview, ApproveRequest, CommitReport, NewRequest, PidLiveness, Plan, Recovery,
    SyncRequest, Transaction, WriterLock, approve_plan, approve_preview, new_plan, sync_plan,
};

pub struct LoadRequest {
    pub discover: DiscoverRequest,
    pub use_cache: bool,
    pub threads: Option<usize>,
    pub want_changed: bool,
    pub only: Option<BTreeSet<RepoPath>>,
}

/// Everything a reader verb needs, gathered once. `root_control` is the A-11 extension.
pub struct Loaded {
    pub root: Root,
    pub root_control: ControlFile,
    pub controls: ControlSet,
    pub walk: WalkResult,
    pub corpus: Corpus,
    pub facts: Facts,
    pub load_diags: Vec<Diagnostic>,
    pub cache: Cache,
}

/// A-11: discover -> walk -> controls -> child excludes -> extract plan -> cache -> extract -> facts.
/// Reader verbs call this; only `WriteSession` takes the writer lock.
pub fn load(req: &LoadRequest) -> Result<Loaded, Error> {
    let Discovered {
        root, root_control, ..
    } = discover(&req.discover)?;
    // Git status depends only on the root, so it runs beside walk and extract (E-46: on a
    // 100,000-file Windows tree it was a third of a `--changed` run).
    std::thread::scope(|s| {
        let git = s.spawn(|| git_facts(&root, req.want_changed));
        load_rest(req, root.clone(), root_control, git)
    })
}

fn load_rest(
    req: &LoadRequest,
    root: Root,
    root_control: ControlFile,
    git: std::thread::ScopedJoinHandle<'_, GitFacts>,
) -> Result<Loaded, Error> {
    let walked = walk(
        &root,
        &WalkOptions::from_root_control(&root_control, req.threads),
    )?;
    let controls = load_controls(&root, &walked)?;
    let walked = apply_child_excludes(walked, &controls)?;
    let plan = ExtractPlan::from_controls(&root, &controls)?;
    let key = CacheKey {
        fairlead_version: env!("CARGO_PKG_VERSION").to_string(),
        grammar: GRAMMAR_EDITION,
        config_hash: config_hash_of(&controls),
        as_of: None,
    };
    let cache = Cache::open(&root, req.use_cache, key);
    let corpus = in_pool(req.threads, || {
        extract_files(&walked, &root, &plan, &cache, req.only.as_ref())
    })?;
    let git = git
        .join()
        .map_err(|_| Error::Internal("the git status thread panicked".into()))?;
    let facts = collect_facts_with(&root, &walked, &controls, git)?;
    let mut load_diags = walked.diags.clone();
    load_diags.extend(control_diagnostics(&controls));
    sort_diagnostics(&mut load_diags);
    Ok(Loaded {
        root,
        root_control,
        controls,
        walk: walked,
        corpus,
        facts,
        load_diags,
        cache,
    })
}

/// R-ARCH-080: `--threads N` bounds the extraction pool; without it the global pool is used.
fn in_pool<T: Send>(threads: Option<usize>, f: impl FnOnce() -> T + Send) -> T {
    match threads.and_then(|n| rayon::ThreadPoolBuilder::new().num_threads(n).build().ok()) {
        Some(pool) => pool.install(f),
        None => f(),
    }
}

pub fn resolve_loaded(loaded: &Loaded, as_of: AsOf) -> Registry {
    resolve(&ResolveInput {
        root: &loaded.root,
        controls: &loaded.controls,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        as_of,
    })
}

/// Load-time diagnostics plus the baseline of this run (R-CHK-030). Bytes come from
/// `Facts.generated_on_disk`, so no second read occurs.
pub struct CheckContext {
    pub diags: Vec<Diagnostic>,
    pub baseline: Option<Baseline>,
}

/// An unreadable baseline is floor `HN-B002`. Header failures are already reported by the
/// generated-file check, so only the body issues are added here.
fn baseline_issue(issue: &BaselineIssue, path: &RepoPath) -> Option<Diagnostic> {
    if matches!(
        issue,
        BaselineIssue::HashMismatch | BaselineIssue::MissingHeader
    ) {
        return None;
    }
    Some(
        crate::check::finding::DiagB::new(codes::HN_B002, path, 0, 0, issue.message())
            .hint("restore the file from git, or regenerate it with fairlead sync --baseline")
            .text(path.as_str())
            .build(),
    )
}

pub fn check_context(loaded: &Loaded, registry: &Registry, options: &CheckOptions) -> CheckContext {
    let mut diags = loaded.load_diags.clone();
    let mut baseline = None;
    if !options.no_baseline
        && let Ok(path) = RepoPath::new(&crate::check::builtin::baseline_rel(registry))
        && let Some(bytes) = loaded.facts.generated_on_disk.get(&path)
    {
        match Baseline::parse(bytes) {
            Ok(parsed) => baseline = Some(parsed),
            Err(issue) => diags.extend(baseline_issue(&issue, &path)),
        }
    }
    CheckContext { diags, baseline }
}

/// A-17: `preflight` first (unreadable git under `--changed` is exit 5), then the pure run.
pub fn check_loaded(
    loaded: &Loaded,
    registry: &Registry,
    options: &CheckOptions,
) -> Result<CheckReport, Error> {
    let context = check_context(loaded, registry, options);
    let input = CheckInput {
        registry,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        load_diags: &context.diags,
        baseline: context.baseline.as_ref(),
        options,
    };
    preflight(&input)?;
    Ok(check_run(&input))
}

/// A-18: `precheck` (unresolved chain, `--exact-tokens`, unknown `--scope`) then `build`.
pub fn context_loaded(
    loaded: &Loaded,
    registry: &Registry,
    report: &CheckReport,
    request: &ContextRequest,
) -> Result<ContextOutput, Error> {
    precheck(registry, &loaded.root_control.context, request)?;
    Ok(build_context(&ContextInput {
        registry,
        corpus: &loaded.corpus,
        report,
        config: &loaded.root_control.context,
        request,
        cache: &loaded.cache,
    }))
}

/// Lexical normalisation: `.` dropped, `..` removes the previous component. Containment is decided
/// later by `strip_prefix`.
pub fn normalise(path: &Utf8Path) -> Utf8PathBuf {
    let mut out = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {}
            Utf8Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_str()),
        }
    }
    out
}

/// A path argument as a repo-relative path (C-13, A-31): relative to the working directory. The root
/// itself is `RepoPath::root()`; outside the root is `PathOutsideRoot`.
pub fn to_root_relative(root: &Root, cwd: &Utf8Path, text: &str) -> Result<RepoPath, Error> {
    let typed = Utf8Path::new(text);
    let joined = if typed.is_absolute() {
        typed.to_path_buf()
    } else {
        cwd.join(typed)
    };
    let absolute = normalise(&joined);
    let outside = || Error::PathOutsideRoot {
        path: text.to_string(),
    };
    let relative = absolute
        .strip_prefix(normalise(&root.dir))
        .map_err(|_| outside())?;
    let rel = relative.as_str().replace('\\', "/");
    if rel.is_empty() || rel == "." {
        return Ok(RepoPath::root());
    }
    RepoPath::new(&rel).map_err(|_| outside())
}

pub fn context_target(
    root: &Root,
    cwd: &Utf8Path,
    arg: Option<&str>,
) -> Result<ContextTarget, Error> {
    let exists = |path: &RepoPath| root.dir.join(path.as_str()).exists();
    let root_relative =
        |text: &str| to_root_relative(root, cwd, text).map_err(|_| ContextArgError::OutsideRoot);
    parse_target(arg, &exists, &root_relative).map_err(|error| match error {
        ContextArgError::NoMatchTaskQuery => Error::Usage(NO_MATCH_MESSAGE.to_string()),
        ContextArgError::OutsideRoot => Error::PathOutsideRoot {
            path: arg.unwrap_or_default().to_string(),
        },
    })
}

/// C-21 order: topic and `HN-` code words are never paths; otherwise an existing path wins over an ID.
/// R-VER-014, R-CLI-036: the virtual version of a file, never written into it: the newest sealed
/// change doc whose `covers` matches the file, plus the last commit that touched it. Either part
/// says "unavailable" when it has no source (no seal covers it, no git history).
pub fn file_version_section(loaded: &Loaded, path: &RepoPath) -> crate::explain::ExplainSection {
    use crate::render::line::TerseLine;
    let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
    let covering = docs
        .iter()
        .filter(|d| d.status == crate::docs::Status::Sealed)
        .filter(|d| {
            let dir = loaded_scope_dir(loaded, &d.path);
            crate::docs::covers_path(d, &dir, path)
        })
        .max_by_key(|d| (d.sealed, d.ordinal));
    let mut line = TerseLine::new("file");
    line = match covering {
        Some(d) => line
            .with(
                "version",
                &d.sealed.map(|v| v.to_string()).unwrap_or_default(),
            )
            .with("doc", d.id.as_str()),
        None => line.with("version", "unavailable"),
    };
    let git = crate::git::GitRepo::open(&loaded.root).and_then(|r| r.last_commit_touching(path));
    line = match git {
        Some((id, date)) => line.with("commit", &id).with("date", &date),
        None => line.with("history", "unavailable"),
    };
    crate::explain::ExplainSection {
        title: "version".into(),
        prose: vec![
            "Derived from sealed change docs and git history; never stamped into the file.".into(),
        ],
        lines: vec![line],
    }
}

/// The directory of the scope that holds `path`, from the membership the corpus walk saw: the
/// deepest managerial folder above it (empty for the root).
fn loaded_scope_dir(loaded: &Loaded, path: &RepoPath) -> String {
    let folder = &loaded.root.folder;
    let mut best = String::new();
    for c in loaded.controls.files.keys() {
        if let Some(dir) = c.as_str().strip_suffix(&format!("{folder}/control.toml")) {
            let dir = dir.trim_end_matches('/');
            if (dir.is_empty() || path.as_str().starts_with(&format!("{dir}/")))
                && dir.len() >= best.len()
            {
                best = dir.to_owned();
            }
        }
    }
    best
}

pub fn explain_target(root: &Root, cwd: &Utf8Path, arg: &str) -> Result<ExplainTarget, Error> {
    let exists = |path: &RepoPath| root.dir.join(path.as_str()).exists();
    let topic_or_code = crate::explain::topics::TOPICS.contains(&arg) || arg.starts_with("HN-");
    let text = if topic_or_code {
        arg.to_string()
    } else {
        match to_root_relative(root, cwd, arg) {
            Ok(path) if exists(&path) => path.as_str().to_string(),
            _ => arg.to_string(),
        }
    };
    parse_explain_target(&text, &exists)
}

/// A-63: a writer verb takes the lock and recovers an interrupted run BEFORE it loads, so the
/// planners read the recovered files; the commit happens under the same lock.
pub struct WriteSession<'p> {
    pub root: Root,
    pub lock: WriterLock<'p>,
    /// The repository-wide allocation lock, for `seal`, `move` and `upgrade` (R-GIT-052).
    pub alloc: Option<WriterLock<'p>>,
    pub recovery: Recovery,
}

impl<'p> WriteSession<'p> {
    pub fn open(req: &DiscoverRequest, probe: &'p dyn PidLiveness) -> Result<Self, Error> {
        let d = discover(req)?;
        Self::open_at(d.root, probe)
    }

    /// `seal`, `move` and `upgrade`: the worktree lock, then the allocation lock in the git
    /// common directory, so two worktrees never allocate versions or IDs at once (D-52).
    pub fn open_allocating(
        req: &DiscoverRequest,
        probe: &'p dyn PidLiveness,
    ) -> Result<Self, Error> {
        let mut s = Self::open(req, probe)?;
        s.alloc = WriterLock::acquire_alloc(&s.root, probe)?;
        Ok(s)
    }

    /// For `init`, whose root does not exist yet (`write::init_root`).
    pub fn open_at(root: Root, probe: &'p dyn PidLiveness) -> Result<Self, Error> {
        let lock = WriterLock::acquire(&root, probe)?;
        let recovery = Transaction::recover(&root, &lock)?;
        Ok(WriteSession {
            root,
            lock,
            alloc: None,
            recovery,
        })
    }

    /// `seal --abort` (R-VER-051): lock, then roll an interrupted journal BACK instead of forward.
    pub fn open_abort(req: &DiscoverRequest, probe: &'p dyn PidLiveness) -> Result<Self, Error> {
        let root = discover(req)?.root;
        let lock = WriterLock::acquire(&root, probe)?;
        let recovery = Transaction::abort(&root, &lock)?;
        Ok(WriteSession {
            root,
            lock,
            alloc: None,
            recovery,
        })
    }
}

/// `check --base <ref>` (R-CLI-022, R-GIT-041): the coverage base and the changed set are the
/// files changed since `ref` instead of since the latest version file.
pub fn apply_base(loaded: &mut Loaded, base: &str) -> Result<(), Error> {
    let repo = crate::git::GitRepo::open(&loaded.root)
        .ok_or_else(|| Error::Usage("--base needs a git repository".into()))?;
    let id = repo.resolve(base)?;
    let files = repo.changed_since(&id)?;
    loaded.facts.git.changed = Some(crate::facts::ChangedSet(files.clone()));
    loaded.facts.git.coverage = Some(crate::facts::Coverage::Changed { base: id, files });
    Ok(())
}

/// `check --staged`: the staged paths, used as the report filter.
pub fn staged_paths(loaded: &Loaded) -> Result<BTreeSet<RepoPath>, Error> {
    crate::git::GitRepo::open(&loaded.root)
        .ok_or_else(|| Error::Usage("--staged needs a git repository".into()))?
        .staged_paths()
}

/// `check --since <rev>` (R-AGT-014, R-AGT-020): what a reviewer must look at, rule-bearing
/// changes first: a lowered pin, added and deleted `ovr`s, control files, instruction files and
/// authored managerial docs.
pub fn must_review(loaded: &Loaded, rev: &str) -> Result<Vec<String>, Error> {
    let repo = crate::git::GitRepo::open(&loaded.root)
        .ok_or_else(|| Error::Usage("--since needs a git repository".into()))?;
    let mut rules: Vec<String> = Vec::new();
    let mut rest: Vec<String> = Vec::new();
    let ctl = &loaded.root_control.path;
    if let Some(old) = repo.file_at(rev, ctl)
        && let Ok(then) = crate::config::parse_control(ctl, &old, true)
    {
        for l in crate::edition::lowered(
            &crate::edition::Pins::of(&loaded.root_control),
            &crate::edition::Pins::of(&then),
        ) {
            rules.push(format!("must-review {l} (since {rev})"));
        }
    }
    let lines = repo.changed_lines(rev)?;
    for (p, added, text) in &lines {
        let t = text.trim();
        let is_ovr = t.contains("[[ovr]]")
            || t.split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|w| w[0].ends_with(':') && w[1] == "ovr");
        if is_ovr {
            rules.push(format!(
                "must-review {} ovr in {}: {}",
                if *added { "added" } else { "deleted" },
                p.as_str(),
                crate::render::sanitize::sanitize_data(t, 120)
            ));
        }
    }
    let agents: BTreeSet<String> = loaded.root_control.agents.iter().cloned().collect();
    let changed: BTreeSet<&RepoPath> = lines.iter().map(|(p, _, _)| p).collect();
    for p in changed {
        let s = p.as_str();
        if p.file_name() == "control.toml" {
            rules.push(format!("must-review control file {s}"));
        } else if agents.contains(s) || matches!(p.file_name(), "AGENTS.md" | "CLAUDE.md") {
            rest.push(format!("must-review instruction file {s}"));
        } else if s.split('/').any(|seg| seg == loaded.root.folder) && !s.contains("/gen/") {
            rest.push(format!("must-review managerial file {s}"));
        }
    }
    rules.extend(rest);
    Ok(rules)
}

/// `check --against <ref>` (R-GIT-042): local sealed version files whose content differs from the
/// copy at `ref` (a force-push divergence).
pub fn against(loaded: &Loaded, rev: &str) -> Result<Vec<String>, Error> {
    let repo = crate::git::GitRepo::open(&loaded.root)
        .ok_or_else(|| Error::Usage("--against needs a git repository".into()))?;
    repo.resolve(rev)?;
    let mut out = Vec::new();
    for (p, text) in &loaded.facts.version_texts {
        if let Some(theirs) = repo.file_at(rev, p)
            && crate::hash::hash_normalised(&theirs)
                != crate::hash::hash_normalised(text.as_bytes())
        {
            out.push(format!(
                "warning: sealed version file {} differs from {rev} (force-push divergence?)",
                p.as_str()
            ));
        }
    }
    Ok(out)
}

/// R-GIT-064, R-GIT-011: during a merge or rebase (unmerged paths), `sync` regenerates conflicted
/// generated files but refuses while an authored file is unmerged, listing them; every other
/// writer of authored files refuses outright.
pub fn merge_gate(loaded: &Loaded, verb: &str, generated_ok: bool) -> Result<(), Error> {
    let Some(repo) = crate::git::GitRepo::open(&loaded.root) else {
        return Ok(());
    };
    let unmerged = repo.unmerged_paths()?;
    let generated = |p: &RepoPath| {
        loaded
            .corpus
            .records
            .get(p)
            .is_some_and(|r| r.class == crate::classify::FileClass::Generated)
            || p.as_str().contains(&format!("{}/gen/", loaded.root.folder))
    };
    let blocking: Vec<&RepoPath> = unmerged
        .iter()
        .filter(|p| !(generated_ok && generated(p)))
        .collect();
    if blocking.is_empty() {
        return Ok(());
    }
    let list: Vec<&str> = blocking.iter().map(|p| p.as_str()).collect();
    Err(Error::Refused(crate::error::Refusal {
        kind: crate::error::RefusalKind::PlanNotConfirmed,
        message: format!(
            "{verb} refuses during a merge: {} unmerged authored file(s): {}",
            list.len(),
            list.join(", ")
        ),
        hint: "resolve them and `git add` them first; generated files are regenerated by sync"
            .into(),
    }))
}

/// What a writer reports: the recovery it performed before writing, and what it wrote (R-CLI-007).
pub struct WriteOutcome {
    pub recovery: Recovery,
    pub report: CommitReport,
}

/// The `upgrade` plan (R-EDN-030): the edit write-set, its hash, the readable edit list and the
/// fields nobody may invent.
pub struct UpgradeOut {
    pub from: crate::discover::Edition,
    pub to: crate::discover::Edition,
    pub hash: String,
    pub plan: Plan,
    pub edits: Vec<String>,
    pub needs: Vec<crate::upgrade::NeedsHuman>,
}

/// R-EDN-030, R-EDN-037: the deterministic one-edition edit set. The control file carries the new
/// `edition` and is the commit point; every generated file is re-rendered by this binary for the
/// new edition. Authored files other than the control file are never touched.
pub fn plan_upgrade(
    loaded: &Loaded,
    registry: &Registry,
    options: &CheckOptions,
    to: crate::discover::Edition,
) -> Result<UpgradeOut, Error> {
    use crate::write::{PlanAction, PlanLine, PlannedWrite, WriteKind, WriteSet, WriteSetKind};
    let from = loaded.root.edition;
    let spec = crate::upgrade::one_step(from, to).map_err(|m| {
        Error::Refused(crate::error::Refusal {
            kind: crate::error::RefusalKind::PlanNotConfirmed,
            message: m,
            hint: "install the binary of the next edition and upgrade one step at a time".into(),
        })
    })?;
    let ctl = loaded.root_control.path.clone();
    let abs = loaded.root.dir.join(ctl.as_str());
    let old = std::fs::read_to_string(abs.as_std_path())
        .map_err(|e| Error::io(ctl.as_str(), "read", &e, false))?
        .replace("\r\n", "\n");
    let new = crate::upgrade::rewrite_edition(&old, to).map_err(Error::Internal)?;
    let table: toml::Table = old.parse().unwrap_or_default();
    let needs = crate::upgrade::needs_human(&ctl, &table, spec);
    let context = check_context(loaded, registry, options);
    let generated = crate::generate::generate(
        registry,
        &loaded.corpus,
        &loaded.facts,
        context.baseline.as_ref(),
        to,
    );
    let mut set = WriteSet::new(WriteSetKind::Upgrade);
    let mut lines = Vec::new();
    let mut edits = Vec::new();
    for (path, g) in &generated.files {
        let bytes = g.render(to);
        let have = loaded.facts.generated_on_disk.get(path);
        if have.is_some_and(|h| **h == *bytes) {
            continue;
        }
        set.add(PlannedWrite {
            path: path.clone(),
            kind: if have.is_some() {
                WriteKind::Replace
            } else {
                WriteKind::Create
            },
            bytes,
            class: crate::classify::FileClass::Generated,
            expect_prior: have.map(|h| crate::hash::hash_normalised(h)),
        })?;
        edits.push(format!("regenerate {} for edition {}", path.as_str(), to.0));
        lines.push(PlanLine {
            action: PlanAction::Edit,
            path: path.clone(),
            detail: format!("regenerated for edition {}", to.0),
            diff: None,
        });
    }
    set.add(PlannedWrite {
        path: ctl.clone(),
        kind: WriteKind::Replace,
        bytes: new.into_bytes(),
        class: crate::classify::FileClass::Authored,
        expect_prior: Some(crate::hash::hash_normalised(old.as_bytes())),
    })?;
    set.commit_point = Some(ctl.clone());
    edits.push(format!(
        "{}: [fairlead] edition {} -> {} (written last)",
        ctl.as_str(),
        from.0,
        to.0
    ));
    lines.push(PlanLine {
        action: PlanAction::Edit,
        path: ctl,
        detail: format!("edition {} -> {} (commit point)", from.0, to.0),
        diff: None,
    });
    let plan = Plan::new(set, lines, Vec::new());
    Ok(UpgradeOut {
        from,
        to,
        hash: crate::upgrade::hash_text(&plan.plan_hash),
        plan,
        edits,
        needs,
    })
}

/// R-EDN-007, D-30: the rename step of an upgrade plan. Every directive's marker is rewritten in
/// place (at the extracted marker offsets, so text that only looks like a directive is left
/// alone), every file under a managerial folder moves to the new folder name, the root control
/// file gets the new `marker` and `folder` (and is the commit point), and CODEOWNERS,
/// `.gitattributes` and `.gitignore` follow the folder. The edition is unchanged.
pub fn plan_rename(
    loaded: &Loaded,
    marker: Option<&str>,
    folder: Option<&str>,
) -> Result<UpgradeOut, Error> {
    use crate::write::{PlanAction, PlanLine, PlannedWrite, WriteKind, WriteSet, WriteSetKind};
    for n in marker.into_iter().chain(folder) {
        if !crate::upgrade::valid_name(n) {
            return Err(Error::Usage(format!(
                "{n:?} is not a valid name: lowercase a-z, 0-9, - and _, at most 32 characters"
            )));
        }
    }
    let old_marker = loaded
        .root_control
        .fairlead
        .marker
        .clone()
        .unwrap_or_else(|| "fairlead".to_owned());
    let old_folder = loaded.root.folder.clone();
    let marker = marker.filter(|m| *m != old_marker);
    let folder = folder.filter(|f| *f != old_folder);
    if marker.is_none() && folder.is_none() {
        return Err(Error::Usage(
            "nothing to rename: the names are unchanged".into(),
        ));
    }
    let managerial: Vec<String> = loaded
        .controls
        .files
        .keys()
        .filter_map(|p| p.parent())
        .map(|d| d.as_str().to_owned())
        .collect();
    let moved = |p: &RepoPath| -> Option<RepoPath> {
        let new = folder?;
        let s = p.as_str();
        managerial.iter().find_map(|m| {
            let rest = s.strip_prefix(&format!("{m}/"))?;
            let parent = m.rsplit_once('/').map_or("", |(a, _)| a);
            let to = if parent.is_empty() {
                format!("{new}/{rest}")
            } else {
                format!("{parent}/{new}/{rest}")
            };
            RepoPath::new(&to).ok()
        })
    };
    // Text is moved LF-normalised (writes are LF-only, R-ARCH-112); bytes otherwise.
    let read = |p: &RepoPath| -> Result<Vec<u8>, Error> {
        let b = std::fs::read(loaded.root.dir.join(p.as_str()).as_std_path())
            .map_err(|e| Error::io(p.as_str(), "read", &e, false))?;
        Ok(match String::from_utf8(b) {
            Ok(t) => t.replace("\r\n", "\n").into_bytes(),
            Err(e) => e.into_bytes(),
        })
    };
    // R-EDN-037 for a rename: the hash covers every move and every rewritten content, but not the
    // content of change docs and approvals that are only moved, so marking the upgrade doc ready
    // and approving it does not make its own plan stale.
    let mut items: Vec<String> = Vec::new();
    let only_moved =
        |p: &RepoPath| p.as_str().contains("/docs/changes/") || p.file_name() == "approvals.toml";
    let mut set = WriteSet::new(WriteSetKind::Rename);
    let mut lines = Vec::new();
    let mut edits = Vec::new();
    let root_ctl = loaded.root_control.path.clone();
    let mut new_root_ctl = root_ctl.clone();
    for e in &loaded.walk.files {
        let p = &e.path;
        let offsets: Vec<usize> = match marker {
            Some(_) => loaded
                .corpus
                .records
                .get(p)
                .and_then(|r| r.extraction.as_ref())
                .map(|x| {
                    x.directives
                        .iter()
                        .filter(|d| d.carrier != fairlead_syntax::directive::Carrier::Frontmatter)
                        .filter_map(|d| usize::try_from(d.span.start).ok())
                        .collect()
                })
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let target = moved(p);
        let mention = folder.is_some()
            && matches!(
                p.as_str(),
                "CODEOWNERS"
                    | ".github/CODEOWNERS"
                    | "docs/CODEOWNERS"
                    | ".gitattributes"
                    | ".gitignore"
            );
        if offsets.is_empty() && target.is_none() && !mention && *p != root_ctl {
            continue;
        }
        let old = read(p)?;
        let mut new = old.clone();
        if let Some(m) = marker {
            new = crate::upgrade::rewrite_markers(&new, &offsets, &old_marker, m);
        }
        if mention && let Some(f) = folder {
            new = crate::upgrade::rewrite_folder_mentions(
                &String::from_utf8_lossy(&new),
                &old_folder,
                f,
            )
            .into_bytes();
        }
        if *p == root_ctl {
            let text = String::from_utf8_lossy(&new).replace("\r\n", "\n");
            new = crate::upgrade::rewrite_names(
                &text,
                marker,
                folder.map(|f| (old_folder.as_str(), f)),
            )
            .map_err(Error::Internal)?
            .into_bytes();
        }
        let class = loaded
            .corpus
            .records
            .get(p)
            .map_or(crate::classify::FileClass::Authored, |r| r.class);
        match target {
            Some(to) => {
                if *p == root_ctl {
                    new_root_ctl = to.clone();
                }
                set.add(PlannedWrite {
                    path: to.clone(),
                    kind: WriteKind::Create,
                    bytes: new.clone(),
                    class,
                    expect_prior: None,
                })?;
                set.add(PlannedWrite {
                    path: p.clone(),
                    kind: WriteKind::Delete,
                    bytes: Vec::new(),
                    class,
                    expect_prior: Some(crate::hash::hash_normalised(&old)),
                })?;
                edits.push(format!("move {} -> {}", p.as_str(), to.as_str()));
                if only_moved(p) {
                    // Change docs and approvals come and go between the draft and the apply.
                } else if new == old {
                    items.push(format!("move {} {}", p.as_str(), to.as_str()));
                } else {
                    items.push(format!(
                        "move {} {} {}",
                        p.as_str(),
                        to.as_str(),
                        crate::hash::hash_normalised(&new)
                            .0
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    ));
                }
                lines.push(PlanLine {
                    action: PlanAction::Create,
                    path: to,
                    detail: format!("moved from {}", p.as_str()),
                    diff: None,
                });
            }
            None if new != old => {
                set.add(PlannedWrite {
                    path: p.clone(),
                    kind: WriteKind::Replace,
                    bytes: new.clone(),
                    class,
                    expect_prior: Some(crate::hash::hash_normalised(&old)),
                })?;
                edits.push(format!("rewrite {}", p.as_str()));
                items.push(format!(
                    "rewrite {} {}",
                    p.as_str(),
                    crate::hash::hash_normalised(&new)
                        .0
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                ));
                lines.push(PlanLine {
                    action: PlanAction::Edit,
                    path: p.clone(),
                    detail: "marker or folder renamed".into(),
                    diff: None,
                });
            }
            None => {}
        }
    }
    set.commit_point = Some(new_root_ctl);
    let plan = Plan::new(set, lines, Vec::new());
    items.sort();
    let hash = crate::hash::ContentHash(*blake3::hash(items.join("\n").as_bytes()).as_bytes());
    Ok(UpgradeOut {
        from: loaded.root.edition,
        to: loaded.root.edition,
        hash: crate::upgrade::hash_text(&hash),
        plan,
        edits,
        needs: Vec::new(),
    })
}

/// `move SRC DST` (R-CLI-040).
#[derive(Clone, Debug)]
pub struct MoveRequest {
    pub src: RepoPath,
    pub dst: RepoPath,
    pub allow_dirty: bool,
}

/// The `move` plan: the write-set, the obligations diff (`+ rule.x`, `- rule.y`, inert
/// exceptions), the code sites to fix by hand, and what refuses the move.
pub struct MoveOut {
    pub plan: Plan,
    pub hash: String,
    pub obligations: Vec<String>,
    pub code_sites: Vec<String>,
    /// R-PORT-004: a loosening reparent; applying needs `fairlead approve move-<fp>`.
    pub loosening: bool,
    pub fp: crate::hash::Fingerprint,
}

/// The deepest resolved scope whose directory holds `p`, skipping `moving` scopes.
use crate::registry::ScopeId;

fn owner_of(reg: &Registry, p: &str, moving: &BTreeSet<ScopeId>) -> ScopeId {
    reg.scopes()
        .filter(|s| !moving.contains(&s.id))
        .filter(|s| {
            let d = s.dir.as_str();
            d.is_empty() || p == d || p.starts_with(&format!("{d}/"))
        })
        .max_by_key(|s| s.dir.as_str().len())
        .map_or_else(|| reg.root_scope().clone(), |s| s.id.clone())
}

fn rule_ids(rules: &crate::check::RuleSet, s: &ScopeId) -> BTreeSet<String> {
    rules
        .for_scope(s)
        .iter()
        .map(|r| format!("rule.{}", r.id.0.as_str()))
        .collect()
}

/// R-CLI-040, R-CLI-044, R-PORT-004 to R-PORT-006: plan a move of a file, a folder or a scope.
/// Files move (create plus delete); quoted paths and globs in control files and docs follow;
/// code sites are listed; a draft restructure doc covering both paths is written (R-VER-032).
pub fn plan_move(loaded: &Loaded, reg: &Registry, req: &MoveRequest) -> Result<MoveOut, Error> {
    use crate::restructure::{code_sites, moved_path, needles, rel_to, rewrite_quoted};
    use crate::write::{PlanAction, PlanLine, PlannedWrite, WriteKind, WriteSet, WriteSetKind};
    let (src, dst) = (req.src.as_str().to_owned(), req.dst.as_str().to_owned());
    if src.is_empty() || dst.is_empty() || src == dst {
        return Err(Error::Usage(
            "move needs two different paths under the root".into(),
        ));
    }
    if moved_path(&dst, &src, &dst).is_some() && dst.starts_with(&format!("{src}/")) {
        return Err(Error::Usage(format!("cannot move {src} into itself")));
    }
    let files: Vec<&RepoPath> = loaded
        .walk
        .files
        .iter()
        .map(|e| &e.path)
        .filter(|p| moved_path(p.as_str(), &src, &dst).is_some())
        .collect();
    if files.is_empty() {
        return Err(Error::Usage(format!(
            "{src} names no file or folder in the tree"
        )));
    }
    if loaded
        .walk
        .files
        .iter()
        .any(|e| moved_path(e.path.as_str(), &dst, &dst).is_some())
    {
        return Err(refusal(
            crate::error::RefusalKind::NoClobber,
            format!("{dst} already exists"),
            "pick a new destination; move never overwrites",
        ));
    }
    let moving: BTreeSet<ScopeId> = reg
        .scopes()
        .filter(|s| !s.dir.as_str().is_empty() && moved_path(s.dir.as_str(), &src, &dst).is_some())
        .map(|s| s.id.clone())
        .collect();
    let rules = crate::check::RuleSet::collect(reg);
    // Obligations: what applies to each moved scope, or to the moved files' owner, before and after.
    let mut obligations: Vec<String> = Vec::new();
    let mut loosening = false;
    let mut diff =
        |label: &str, before: BTreeSet<String>, after: BTreeSet<String>, out: &mut Vec<String>| {
            for r in after.difference(&before) {
                out.push(format!("+ {r} ({label})"));
            }
            for r in before.difference(&after) {
                out.push(format!("- {r} ({label})"));
                loosening = true;
            }
        };
    for s in reg.scopes().filter(|s| moving.contains(&s.id)) {
        let new_dir = moved_path(s.dir.as_str(), &src, &dst).unwrap_or_default();
        let new_parent = owner_of(reg, &new_dir, &moving);
        let own: BTreeSet<String> = rules
            .for_scope(&s.id)
            .iter()
            .filter(|r| r.home == s.id)
            .map(|r| format!("rule.{}", r.id.0.as_str()))
            .collect();
        let before = rule_ids(&rules, &s.id);
        let after: BTreeSet<String> = rule_ids(&rules, &new_parent).union(&own).cloned().collect();
        diff(
            &format!("scope {}", s.id.0.as_str()),
            before,
            after,
            &mut obligations,
        );
        // R-PORT-006: grants from ancestors that are no longer ancestors lapse.
        let old_chain: BTreeSet<&ScopeId> = reg.chain(&s.id).into_iter().collect();
        let mut new_chain: BTreeSet<&ScopeId> = reg.chain(&new_parent).into_iter().collect();
        new_chain.insert(&s.id);
        for a in reg.grants().iter().filter(|a| {
            a.for_scopes.contains(&s.id)
                && old_chain.contains(&a.granting)
                && !new_chain.contains(&a.granting)
        }) {
            for o in reg
                .overrides()
                .iter()
                .filter(|o| o.declared_in == s.id && o.target == a.target)
            {
                obligations.push(format!(
                    "~ inert ovr {}.{} at {}:{} (the [[allow]] {} of {} lapses)",
                    o.target.kind.as_str(),
                    o.target.name.as_str(),
                    o.at.path.as_str(),
                    o.at.line,
                    a.id,
                    a.granting.0.as_str()
                ));
            }
        }
    }
    if moving.is_empty() {
        let old_owner = reg
            .scope_of(files[0])
            .cloned()
            .unwrap_or_else(|| reg.root_scope().clone());
        let new_owner = owner_of(reg, &dst, &moving);
        if old_owner != new_owner {
            diff(
                &format!(
                    "{src} from scope {} to {}",
                    old_owner.0.as_str(),
                    new_owner.0.as_str()
                ),
                rule_ids(&rules, &old_owner),
                rule_ids(&rules, &new_owner),
                &mut obligations,
            );
        }
    }
    let read = |p: &RepoPath| -> Result<Vec<u8>, Error> {
        let b = std::fs::read(loaded.root.dir.join(p.as_str()).as_std_path())
            .map_err(|e| Error::io(p.as_str(), "read", &e, false))?;
        Ok(match String::from_utf8(b) {
            Ok(t) => t.replace("\r\n", "\n").into_bytes(),
            Err(e) => e.into_bytes(),
        })
    };
    let managerial = |p: &RepoPath| {
        p.as_str().split('/').any(|seg| seg == loaded.root.folder)
            && (p.as_str().ends_with(".toml") || p.as_str().ends_with(".md"))
            && !p.as_str().contains("/gen/")
    };
    let mut set = WriteSet::new(WriteSetKind::Move);
    let mut lines = Vec::new();
    let mut code = Vec::new();
    let pats = needles(&src);
    for e in &loaded.walk.files {
        let p = &e.path;
        let target = moved_path(p.as_str(), &src, &dst);
        let mut text_rewrite = false;
        let old = if target.is_some() || managerial(p) {
            Some(read(p)?)
        } else {
            None
        };
        let mut new = old.clone();
        if managerial(p)
            && let Some(o) = &old
            && let Ok(t) = std::str::from_utf8(o)
        {
            let dir = reg
                .scope_of(p)
                .and_then(|s| reg.scope(s))
                .map(|s| s.dir.as_str().to_owned())
                .unwrap_or_default();
            let mut r = rewrite_quoted(t, &src, &dst);
            if let (Some(a), Some(b)) = (rel_to(&dir, &src), rel_to(&dir, &dst))
                && !dir.is_empty()
            {
                r = rewrite_quoted(&r, &a, &b);
            }
            text_rewrite = r != t;
            new = Some(r.into_bytes());
        }
        let class = loaded
            .corpus
            .records
            .get(p)
            .map_or(crate::classify::FileClass::Authored, |r| r.class);
        match (target, old, new) {
            (Some(to), Some(old), Some(new)) => {
                let to = RepoPath::new(&to).map_err(|e| Error::Usage(format!("{to}: {e:?}")))?;
                set.add(PlannedWrite {
                    path: to.clone(),
                    kind: WriteKind::Create,
                    bytes: new,
                    class,
                    expect_prior: None,
                })?;
                set.add(PlannedWrite {
                    path: p.clone(),
                    kind: WriteKind::Delete,
                    bytes: Vec::new(),
                    class,
                    expect_prior: Some(crate::hash::hash_normalised(&old)),
                })?;
                lines.push(PlanLine {
                    action: PlanAction::Create,
                    path: to,
                    detail: format!("moved from {}", p.as_str()),
                    diff: None,
                });
            }
            (None, Some(old), Some(new)) if text_rewrite => {
                set.add(PlannedWrite {
                    path: p.clone(),
                    kind: WriteKind::Replace,
                    bytes: new,
                    class,
                    expect_prior: Some(crate::hash::hash_normalised(&old)),
                })?;
                lines.push(PlanLine {
                    action: PlanAction::Edit,
                    path: p.clone(),
                    detail: format!("paths {src} -> {dst}"),
                    diff: None,
                });
            }
            _ => {}
        }
        if class == crate::classify::FileClass::Authored && !managerial(p) {
            let text = std::fs::read_to_string(loaded.root.dir.join(p.as_str()).as_std_path())
                .unwrap_or_default();
            code.extend(code_sites(p, &text, &pats));
        }
    }
    // R-VER-032, R-CHK-044: a generated restructure doc covers the move under docs-first.
    let id = crate::docs::DraftId::from_bits({
        let h = crate::hash::hash_raw(format!("move {src} {dst}").as_bytes());
        let mut b = [0u8; 5];
        b.copy_from_slice(&h.0[..5]);
        b
    });
    let slug = crate::docs::slug(&format!("restructure {dst}"));
    let doc_path = RepoPath::new(&format!(
        "{}/docs/changes/{id}-{slug}.md",
        loaded.root.folder
    ))
    .map_err(|e| Error::Internal(format!("{e:?}")))?;
    let glob = |p: &str, file: bool| {
        if file {
            p.to_owned()
        } else {
            format!("{p}/**")
        }
    };
    let is_file = files.len() == 1 && files[0].as_str() == src;
    let doc = format!(
        "+++\nid = \"{id}\"\nkind = \"chore\"\nstatus = \"draft\"\nimpact = \"fix\"\nscope = \"root\"\ntitle = \"Restructure: move {src} to {dst}\"\ncovers = [\"{}\", \"{}\"]\ncreated = \"{}\"\nrestructure = true\n+++\n\n# Restructure: move {src} to {dst}\n\nWritten by `fairlead move`. Obligations:\n\n{}\n",
        glob(&src, is_file),
        glob(&dst, is_file),
        version_line(loaded).open,
        if obligations.is_empty() {
            "- none".to_owned()
        } else {
            obligations
                .iter()
                .map(|o| format!("- {o}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    );
    set.add(PlannedWrite {
        path: doc_path.clone(),
        kind: WriteKind::Create,
        bytes: doc.into_bytes(),
        class: crate::classify::FileClass::Authored,
        expect_prior: None,
    })?;
    lines.push(PlanLine {
        action: PlanAction::Create,
        path: doc_path,
        detail: "restructure doc (draft)".into(),
        diff: None,
    });
    let plan = Plan::new(set, lines, Vec::new());
    let hash = crate::upgrade::hash_text(&plan.plan_hash);
    let fp = crate::hash::Fingerprint::from_hash(&plan.plan_hash);
    Ok(MoveOut {
        plan,
        hash,
        obligations,
        code_sites: code,
        loosening,
        fp,
    })
}

/// Scope operations of `move` without a destination (R-PORT-010, R-PORT-012, R-LC-023).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScopeOp {
    /// Delete: items tombstoned into the parent, the managerial folder removed.
    Delete { force: bool },
    /// Demote: docs move into the parent with `applies`, a scope tombstone is left.
    Demote,
}

/// Adds ` applies=<glob>` to every frontmatter `def` that has none, so a demoted scope's items
/// keep applying to its former directory only.
fn with_applies(text: &str, glob: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let (mut in_fm, mut fences) = (false, 0);
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "+++" && fences < 2 {
            fences += 1;
            in_fm = fences == 1;
            out.push_str(line);
            continue;
        }
        let t = line.trim_start();
        let quoted = t.starts_with("'def ") || t.starts_with("\"def ");
        if in_fm && quoted && !line.contains("applies=") {
            let q = if t.starts_with('\'') { '\'' } else { '"' };
            if let Some(end) = line.rfind(q) {
                let (head, tail) = line.split_at(end);
                out.push_str(head);
                out.push_str(&format!(" applies={glob}"));
                out.push_str(tail);
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

/// `move --delete SRC` and `move --demote SRC` (R-PORT-010, R-LC-023).
pub fn plan_scope_op(
    loaded: &Loaded,
    reg: &Registry,
    src: &RepoPath,
    op: ScopeOp,
) -> Result<MoveOut, Error> {
    use crate::write::{PlanAction, PlanLine, PlannedWrite, WriteKind, WriteSet, WriteSetKind};
    let sc = reg
        .scopes()
        .find(|s| s.dir.as_str() == src.as_str() && !s.dir.as_str().is_empty())
        .ok_or_else(|| {
            Error::Usage(format!(
                "{} is not a scope directory (other than the root)",
                src.as_str()
            ))
        })?;
    let parent_id = sc
        .parent
        .clone()
        .ok_or_else(|| Error::Usage("the scope has no ancestor".into()))?;
    if reg.scopes().any(|s| s.parent.as_ref() == Some(&sc.id)) {
        return Err(refusal(
            crate::error::RefusalKind::PlanNotConfirmed,
            format!("{} has child scopes", sc.id.0.as_str()),
            "delete, demote or move the child scopes first",
        ));
    }
    let folder = &loaded.root.folder;
    let mgr = if sc.dir.as_str().is_empty() {
        folder.clone()
    } else {
        format!("{}/{folder}", sc.dir.as_str())
    };
    let parent = reg
        .scope(&parent_id)
        .ok_or_else(|| Error::Internal("parent scope vanished".into()))?;
    let parent_mgr = if parent.dir.as_str().is_empty() {
        folder.clone()
    } else {
        format!("{}/{folder}", parent.dir.as_str())
    };
    let read = |p: &RepoPath| -> Result<Vec<u8>, Error> {
        let b = std::fs::read(loaded.root.dir.join(p.as_str()).as_std_path())
            .map_err(|e| Error::io(p.as_str(), "read", &e, false))?;
        Ok(match String::from_utf8(b) {
            Ok(t) => t.replace("\r\n", "\n").into_bytes(),
            Err(e) => e.into_bytes(),
        })
    };
    let date = reg.as_of().0;
    let mut set = WriteSet::new(WriteSetKind::Move);
    let mut lines = Vec::new();
    let mut obligations = Vec::new();
    let mut loosening = false;
    let own: Vec<&RepoPath> = loaded
        .walk
        .files
        .iter()
        .map(|e| &e.path)
        .filter(|p| p.as_str().starts_with(&format!("{mgr}/")))
        .collect();
    let delete_file =
        |p: &RepoPath, set: &mut WriteSet, lines: &mut Vec<PlanLine>| -> Result<(), Error> {
            let old = read(p)?;
            set.add(PlannedWrite {
                path: p.clone(),
                kind: WriteKind::Delete,
                bytes: Vec::new(),
                class: loaded
                    .corpus
                    .records
                    .get(p)
                    .map_or(crate::classify::FileClass::Authored, |r| r.class),
                expect_prior: Some(crate::hash::hash_normalised(&old)),
            })?;
            lines.push(PlanLine {
                action: PlanAction::Edit,
                path: p.clone(),
                detail: "removed".into(),
                diff: None,
            });
            Ok(())
        };
    match op {
        ScopeOp::Delete { force } => {
            // R-PORT-010: zero live references into the scope, or `--force` with an approval.
            let live: Vec<String> = reg
                .edges()
                .iter()
                .filter(|e| {
                    e.from.scope != sc.id && e.resolved.as_ref().is_some_and(|r| r.scope == sc.id)
                })
                .map(|e| {
                    format!(
                        "live reference {}:{} -> {}",
                        e.from.path.as_str(),
                        e.from.span.line,
                        e.to
                    )
                })
                .collect();
            if !live.is_empty() {
                obligations.extend(live.iter().cloned());
                loosening = true;
                if !force {
                    return Err(refusal(
                        crate::error::RefusalKind::PlanNotConfirmed,
                        format!(
                            "{} live references into {}: {}",
                            live.len(),
                            sc.id.0.as_str(),
                            live.join("; ")
                        ),
                        "remove them first, or pass --force with an approval of the plan (R-PORT-010)",
                    ));
                }
            }
            let mut tombs = String::new();
            let mut ids: Vec<String> = reg
                .items()
                .filter(|i| i.id.scope == sc.id)
                .map(|i| format!("{}.{}", i.id.kind.as_str(), i.id.name.as_str()))
                .collect();
            ids.sort();
            for id in &ids {
                tombs.push_str(&crate::lifecycle::tomb_text(
                    &crate::lifecycle::Tomb {
                        file: RepoPath::root(),
                        scope: None,
                        id: id.clone(),
                        removed: date.to_string(),
                        by: None,
                        why: format!("scope {} deleted", sc.id.0.as_str()),
                        paths: Vec::new(),
                        deleted_by: Some("delete".into()),
                    },
                    Some(&date.to_string()),
                ));
                obligations.push(format!("tombstone {id}"));
            }
            tombs.push_str(&crate::lifecycle::tomb_text(
                &crate::lifecycle::Tomb {
                    file: RepoPath::root(),
                    scope: None,
                    id: sc.id.0.as_str().to_owned(),
                    removed: date.to_string(),
                    by: Some("delete".into()),
                    why: format!(
                        "scope deleted; its items are tombstoned in {}",
                        parent_id.0.as_str()
                    ),
                    paths: vec![sc.dir.as_str().to_owned()],
                    deleted_by: None,
                },
                None,
            ));
            let tomb_path = RepoPath::new(&format!("{parent_mgr}/tombstones/{}.toml", date.year()))
                .map_err(|e| Error::Internal(format!("{e:?}")))?;
            set.add(PlannedWrite {
                path: tomb_path.clone(),
                kind: WriteKind::AppendBlock {
                    marker: String::new(),
                },
                bytes: tombs.into_bytes(),
                class: crate::classify::FileClass::Authored,
                expect_prior: None,
            })?;
            lines.push(PlanLine {
                action: PlanAction::Edit,
                path: tomb_path,
                detail: format!("{} item tombstones and the scope tombstone", ids.len()),
                diff: None,
            });
            for p in &own {
                delete_file(p, &mut set, &mut lines)?;
            }
        }
        ScopeOp::Demote => {
            let ctl = sc
                .control
                .as_ref()
                .ok_or_else(|| Error::Usage("the scope's control file did not parse".into()))?;
            let busy: Vec<&str> = [
                ("rules", !ctl.rules.is_empty()),
                ("kinds", !ctl.kinds.is_empty()),
                ("[[ovr]]", !ctl.ovrs.is_empty()),
                ("[[allow]]", !ctl.allows.is_empty()),
                ("[[file]]", !ctl.files.is_empty()),
                ("[[vendor]]", !ctl.vendors.is_empty()),
                ("[[convention]]", !ctl.conventions.is_empty()),
            ]
            .into_iter()
            .filter_map(|(n, b)| b.then_some(n))
            .collect();
            if !busy.is_empty() {
                return Err(refusal(
                    crate::error::RefusalKind::PlanNotConfirmed,
                    format!(
                        "{} declares {} in its control file",
                        sc.id.0.as_str(),
                        busy.join(", ")
                    ),
                    "move those tables into the parent's control file first; demote moves docs, not control tables",
                ));
            }
            if own.iter().any(|p| p.as_str().contains("/ledger/")) {
                return Err(refusal(
                    crate::error::RefusalKind::PlanNotConfirmed,
                    format!("{} has sealed ledger history", sc.id.0.as_str()),
                    "a scope with sealed history is kept; detach or delete it instead",
                ));
            }
            // The 0.1 demote model: duplicate-ID refusal and the scope tombstone.
            let tomb = crate::write::plan_demote(reg, &sc.id)?;
            for (_, w) in tomb.set.writes {
                lines.push(PlanLine {
                    action: PlanAction::Edit,
                    path: w.path.clone(),
                    detail: format!("tombstone for {}", sc.id.0.as_str()),
                    diff: None,
                });
                set.add(w)?;
            }
            let rel_dir = crate::restructure::rel_to(parent.dir.as_str(), sc.dir.as_str())
                .unwrap_or_else(|| sc.dir.as_str().to_owned());
            let glob = format!("{rel_dir}/**");
            for p in &own {
                let rest = p.as_str().strip_prefix(&format!("{mgr}/")).unwrap_or("");
                if let Some(doc) = rest.strip_prefix("docs/") {
                    let to =
                        RepoPath::new(&format!("{parent_mgr}/docs/{}/{doc}", sc.id.0.as_str()))
                            .map_err(|e| Error::Internal(format!("{e:?}")))?;
                    let old = read(p)?;
                    let new = with_applies(&String::from_utf8_lossy(&old), &glob);
                    set.add(PlannedWrite {
                        path: to.clone(),
                        kind: WriteKind::Create,
                        bytes: new.into_bytes(),
                        class: crate::classify::FileClass::Authored,
                        expect_prior: None,
                    })?;
                    obligations.push(format!(
                        "~ {} -> {} (applies={glob})",
                        p.as_str(),
                        to.as_str()
                    ));
                    lines.push(PlanLine {
                        action: PlanAction::Create,
                        path: to,
                        detail: format!("moved from {}", p.as_str()),
                        diff: None,
                    });
                }
                delete_file(p, &mut set, &mut lines)?;
            }
        }
    }
    let plan = Plan::new(set, lines, Vec::new());
    Ok(MoveOut {
        hash: crate::upgrade::hash_text(&plan.plan_hash),
        fp: crate::hash::Fingerprint::from_hash(&plan.plan_hash),
        plan,
        obligations,
        code_sites: Vec::new(),
        loosening,
    })
}

/// R-PORT-004, R-GIT-062: a loosening move needs `fairlead approve move-<fp>`; the write-set must
/// equal HEAD unless `--allow-dirty`.
pub fn move_gate(loaded: &Loaded, out: &MoveOut, allow_dirty: bool) -> Result<(), Error> {
    if out.loosening
        && !loaded
            .facts
            .approvals
            .approvals
            .iter()
            .any(|a| a.fp == out.fp)
    {
        return Err(refusal(
            crate::error::RefusalKind::PlanNotConfirmed,
            format!(
                "the move loosens the restrictions of the moved subtree (rules stop applying); a human runs `fairlead approve move-{}` to accept it",
                out.fp.as_str()
            ),
            &format!(
                "a human runs `fairlead approve move-{}` to accept it (R-PORT-004)",
                out.fp.as_str()
            ),
        ));
    }
    if !allow_dirty && let Some(repo) = crate::git::GitRepo::open(&loaded.root) {
        let changed = repo.changed_files()?.0;
        if let Some(p) = out
            .plan
            .set
            .writes
            .iter()
            .filter(|(_, w)| w.kind != crate::write::WriteKind::Create)
            .map(|(p, _)| p)
            .find(|p| changed.contains(*p))
        {
            return Err(refusal(
                crate::error::RefusalKind::PlanNotConfirmed,
                format!(
                    "{} differs from HEAD; the write-set must equal HEAD",
                    p.as_str()
                ),
                "commit or stash it, or pass --allow-dirty",
            ));
        }
    }
    Ok(())
}

fn refusal(kind: crate::error::RefusalKind, message: String, hint: &str) -> Error {
    Error::Refused(crate::error::Refusal {
        kind,
        message,
        hint: hint.into(),
    })
}

/// What `upgrade --yes` may do (R-EDN-031).
pub enum UpgradeGate {
    /// A ready, approved doc matches the plan hash and the write-set is clean.
    Apply,
    /// No doc for this plan: write the draft (the only file this run writes) and stop.
    Draft { id: String, plan: Plan },
}

fn upgrade_refusal(message: String, hint: &str) -> Error {
    Error::Refused(crate::error::Refusal {
        kind: crate::error::RefusalKind::PlanNotConfirmed,
        message,
        hint: hint.into(),
    })
}

/// R-EDN-031, R-EDN-034: needs-human refuses; a plan hash no doc approves writes a draft; an older
/// doc is "plan stale, re-plan"; a doc that is not ready and approved is "approval missing"; the
/// write-set must equal HEAD unless `allow_dirty`.
pub fn upgrade_gate(
    loaded: &Loaded,
    out: &UpgradeOut,
    allow_dirty: bool,
) -> Result<UpgradeGate, Error> {
    if !out.needs.is_empty() {
        let list: Vec<String> = out
            .needs
            .iter()
            .map(|n| format!("{} in {}", n.field, n.path.as_str()))
            .collect();
        return Err(upgrade_refusal(
            format!("needs-human: {}", list.join(", ")),
            "fill these by hand, then run fairlead upgrade again; an upgrade never invents a value (R-EDN-034)",
        ));
    }
    let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
    let pending: Vec<&crate::docs::ChangeDoc> = docs
        .iter()
        .filter(|d| d.kind == "upgrade" && d.status.is_pending())
        .collect();
    let matching = pending
        .iter()
        .find(|d| d.extra.get("plan-hash").map(String::as_str) == Some(out.hash.as_str()));
    let Some(doc) = matching else {
        if let Some(old) = pending.first() {
            return Err(upgrade_refusal(
                format!(
                    "plan stale, re-plan: {} approves {}, the tree now plans {}",
                    old.id,
                    old.extra.get("plan-hash").map_or("nothing", String::as_str),
                    out.hash
                ),
                "set the old doc's status = \"abandoned\", then run fairlead upgrade --yes for a fresh draft",
            ));
        }
        let id = crate::docs::DraftId::from_bits(random_bits())
            .as_str()
            .to_owned();
        let path = RepoPath::new(&format!(
            "{}/docs/changes/{id}-upgrade-edition-{}-to-{}.md",
            loaded.root.folder, out.from.0, out.to.0
        ))
        .map_err(|e| Error::Internal(format!("{e:?}")))?;
        let text = crate::upgrade::draft_doc(
            &id,
            &loaded.root.folder,
            out.from,
            out.to,
            &version_line(loaded).open.to_string(),
            &out.hash,
            &out.edits,
            &out.needs,
        );
        let mut set = crate::write::WriteSet::new(crate::write::WriteSetKind::New);
        set.add(crate::write::PlannedWrite {
            path: path.clone(),
            kind: crate::write::WriteKind::Create,
            bytes: text.into_bytes(),
            class: crate::classify::FileClass::Authored,
            expect_prior: None,
        })?;
        let lines = vec![crate::write::PlanLine {
            action: crate::write::PlanAction::Create,
            path,
            detail: format!("upgrade doc {id}"),
            diff: None,
        }];
        return Ok(UpgradeGate::Draft {
            id,
            plan: Plan::new(set, lines, Vec::new()),
        });
    };
    let fp = crate::upgrade::upgrade_fp(doc.id.as_str(), &out.hash);
    let approved = loaded.facts.approvals.approvals.iter().any(|a| a.fp == fp);
    if doc.status != crate::docs::Status::Ready || !approved {
        return Err(upgrade_refusal(
            format!("approval missing: {} must be ready and approved", doc.id),
            &format!(
                "set status = \"ready\" in {}, then a human runs `fairlead approve {}`",
                doc.path.as_str(),
                doc.id
            ),
        ));
    }
    if !allow_dirty && let Some(repo) = crate::git::GitRepo::open(&loaded.root) {
        let changed = repo.changed_files()?.0;
        if let Some(p) = out.plan.set.writes.keys().find(|p| changed.contains(*p)) {
            return Err(upgrade_refusal(
                format!(
                    "{} differs from HEAD; the write-set must equal HEAD",
                    p.as_str()
                ),
                "commit or stash it, or pass --allow-dirty (rollback is git, R-EDN-035)",
            ));
        }
    }
    Ok(UpgradeGate::Apply)
}

/// R-LC-032: a proposal whose title matches the effective graveyard of its scope is refused, with
/// the entry printed, unless it names the entry in `revisits` and gives a one-line `revisit-why`.
fn graveyard_gate(loaded: &Loaded, registry: &Registry, request: &NewRequest) -> Result<(), Error> {
    let scope = request
        .scope
        .clone()
        .unwrap_or_else(|| registry.root_scope().clone());
    let (entries, _) = crate::graveyard::collect(&loaded.corpus);
    let here = crate::check::builtin::grave::effective(registry, &entries, &scope);
    let named = request.fields.get("revisits").map(String::as_str);
    let why = request
        .fields
        .get("revisit-why")
        .is_some_and(|w| !w.trim().is_empty());
    for e in here {
        let hits = crate::graveyard::text_hits(e, &request.name);
        if hits.is_empty() || (named == Some(e.id.as_str()) && why) {
            continue;
        }
        let terms: Vec<&str> = hits.iter().map(|t| t.text()).collect();
        return Err(Error::Refused(crate::error::Refusal {
            kind: crate::error::RefusalKind::PlanNotConfirmed,
            message: format!(
                "the title matches the graveyard entry {} ({}; {} at {}): {} -- {}",
                e.id,
                terms.join(", "),
                e.state.as_str(),
                e.at.as_deref().unwrap_or("?"),
                e.what,
                e.why
            ),
            hint: format!(
                "to revisit it, pass --field revisits={} --field revisit-why=\"<one line>\" (R-LC-032)",
                e.id
            ),
        }));
    }
    Ok(())
}

/// R-LC-040: under `check --ci` with `authority.signed-records = true`, find the commit that last
/// wrote each authority record (graveyard entries that are rejected or revisited or carry
/// `do-not-reintroduce`, and the approvals file) and ask `git verify-commit` about it. Without
/// the setting nothing runs and no subprocess starts.
pub fn verify_authority(loaded: &mut Loaded) -> Result<(), Error> {
    if !loaded.root_control.authority.signed_records {
        return Ok(());
    }
    let mut paths: Vec<RepoPath> = Vec::new();
    let (entries, _) = crate::graveyard::collect(&loaded.corpus);
    for e in &entries {
        let dnr = loaded
            .corpus
            .records
            .get(&e.path)
            .and_then(|r| r.extraction.as_ref())
            .and_then(|x| x.frontmatter.as_ref())
            .is_some_and(|f| f.table.contains_key("do-not-reintroduce"));
        if dnr
            || matches!(
                e.state,
                crate::graveyard::GrvState::Rejected | crate::graveyard::GrvState::Revisited
            )
        {
            paths.push(e.path.clone());
        }
    }
    let approvals = RepoPath::new(&format!("{}/approvals.toml", loaded.root.folder))
        .map_err(|e| Error::Internal(format!("approvals path: {e}")))?;
    if !loaded.facts.approvals.approvals.is_empty() {
        paths.push(approvals);
    }
    let Some(repo) = crate::git::GitRepo::open(&loaded.root) else {
        return Ok(());
    };
    let mut out = Vec::new();
    for p in paths {
        let commit = repo.last_commit_touching(&p).map(|(c, _)| c);
        let signed = match &commit {
            Some(c) => crate::git::GitRepo::verify_commit(&loaded.root, c)?,
            None => false,
        };
        out.push(crate::facts::AuthorityRecord {
            path: p,
            commit,
            signed,
        });
    }
    loaded.facts.authority = out;
    Ok(())
}

/// `new` plan (R-CLI-028). Pure: never touches disk.
pub fn plan_new(
    loaded: &Loaded,
    registry: &Registry,
    request: &NewRequest,
    as_of: AsOf,
) -> Result<Plan, Error> {
    if request.kind == "change" {
        graveyard_gate(loaded, registry, request)?;
        let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
        let taken = docs.iter().map(|d| d.id.clone()).collect();
        return crate::write::new_change_plan(
            request,
            registry,
            version_line(loaded).open,
            random_bits(),
            &taken,
            as_of,
        );
    }
    new_plan(request, registry, &loaded.facts, as_of)
}

/// The version line of the loaded tree (R-VER-001 to R-VER-003): version files under the root's
/// `<folder>/versions/`, pending change docs from the corpus. `version.start` is `0.1.0` until the
/// `[version]` table is parsed (0.2 errata E2-5).
pub fn version_line(loaded: &Loaded) -> crate::version::VersionLine {
    let start = crate::version::Version::parse(&loaded.root_control.version.start)
        .unwrap_or(crate::version::Version::new(0, 1, 0));
    crate::docs::line_of(&loaded.corpus, &loaded.root.folder, start)
}

/// 40 random bits for a draft ID (R-VER-040), from the standard library's randomly keyed hasher
/// (seeded by the OS); no clock, no extra crate.
fn random_bits() -> [u8; 5] {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u32(std::process::id());
    let b = h.finish().to_le_bytes();
    [b[0], b[1], b[2], b[3], b[4]]
}

/// `sync` plan (R-CLI-025). The check runs first, so `sync_plan` sees the findings `check` reports.
pub fn plan_sync(
    loaded: &Loaded,
    registry: &Registry,
    options: &CheckOptions,
    request: &SyncRequest,
) -> Result<Plan, Error> {
    let context = check_context(loaded, registry, options);
    let input = CheckInput {
        registry,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        load_diags: &context.diags,
        baseline: context.baseline.as_ref(),
        options,
    };
    let report = check_run(&input);
    sync_plan(request, &input, &report, loaded.root.edition)
}

/// `approve`, step 1: the preview the reviewer confirms (R-CLI-037).
pub fn approve_preview_loaded(
    loaded: &Loaded,
    registry: &Registry,
    request: &ApproveRequest,
) -> Result<ApprovePreview, Error> {
    let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
    if let Some(p) = crate::write::revive_preview(request, registry, &docs, &loaded.facts)? {
        return Ok(p);
    }
    if let Some(p) = crate::write::pin_preview(request, registry, &loaded.facts)? {
        return Ok(p);
    }
    if let Some(p) = crate::write::move_preview(request, registry, &loaded.facts)? {
        return Ok(p);
    }
    approve_preview(request, registry, &loaded.facts)
}

/// `approve`, step 2: the plan, built only after the terminal gate has passed (A-21).
pub fn plan_approve_loaded(
    loaded: &Loaded,
    preview: &ApprovePreview,
    approver: &str,
    as_of: AsOf,
) -> Result<Plan, Error> {
    approve_plan(
        &loaded.root.folder,
        preview,
        approver,
        as_of,
        &loaded.facts.approvals,
    )
}

/// Begin and commit under the session's lock (R-CLI-007, R-CLI-008). The only function that
/// reaches `Transaction::begin`.
/// What `seal` is asked for (R-CLI-041).
#[derive(Clone, Debug)]
pub struct SealRequest {
    pub allow_dirty: bool,
    /// `--anywhere`: seal off the integration branch even when `version.seal-where` says
    /// `integration` (the control key `any` has the same effect).
    pub seal_anywhere: bool,
}

/// R-LC-031: every graveyard entry without a fingerprint gets one from its rejected material,
/// read here once: the `material` paths and the tree files its glob terms match (relative to the
/// entry's scope directory). Later checks compare against the stored value and never rescan.
fn grave_prints(loaded: &Loaded) -> Result<Vec<(RepoPath, String, String)>, Error> {
    let (entries, _) = crate::graveyard::collect(&loaded.corpus);
    let mut out = Vec::new();
    for e in entries.iter().filter(|e| e.fingerprint.is_none()) {
        let dir = loaded_scope_dir(loaded, &e.path);
        let rel_of = |p: &RepoPath| -> Option<String> {
            if dir.is_empty() || dir == "." {
                Some(p.as_str().to_owned())
            } else {
                p.as_str()
                    .strip_prefix(&format!("{dir}/"))
                    .map(str::to_owned)
            }
        };
        let mut paths: BTreeSet<RepoPath> = e
            .material
            .iter()
            .filter_map(|m| RepoPath::new(m).ok())
            .collect();
        for p in loaded.corpus.records.keys() {
            if crate::graveyard::is_graveyard_path(p) {
                continue;
            }
            if rel_of(p).is_some_and(|r| !crate::graveyard::path_hits(e, &r).is_empty()) {
                paths.insert(p.clone());
            }
        }
        let mut files = Vec::new();
        for p in &paths {
            let abs = loaded.root.dir.join(p.as_str());
            if let Ok(text) = std::fs::read_to_string(abs.as_std_path()) {
                files.push((p.as_str().to_owned(), text));
            }
        }
        let fp = crate::graveyard::fingerprint(&files);
        let abs = loaded.root.dir.join(e.path.as_str());
        let old = std::fs::read_to_string(abs.as_std_path())
            .map_err(|err| Error::io(e.path.as_str(), "read", &err, false))?;
        let new = crate::graveyard::with_fingerprint(&old, &fp).map_err(Error::Internal)?;
        out.push((e.path.clone(), old, new));
    }
    // R-LC-024, R-LC-035: a revive doc in this seal moves the entries it names in `revisits` to
    // `revisited`, linked to the doc; entries are never deleted.
    let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
    for d in docs
        .iter()
        .filter(|d| d.kind == "revive" && d.status == crate::docs::Status::Ready)
    {
        for id in crate::check::builtin::grave::revisits(d) {
            let Some(e) = entries.iter().find(|e| e.id == id) else {
                continue;
            };
            let draft = d.id.as_str().to_owned();
            let edit = |text: &str| {
                crate::graveyard::set_keys(text, |doc| {
                    doc["state"] = toml_edit::value("revisited");
                    doc["revisited-by"] = toml_edit::value(draft.as_str());
                })
                .map_err(Error::Internal)
            };
            if let Some(slot) = out.iter_mut().find(|(p, _, _)| *p == e.path) {
                slot.2 = edit(&slot.2)?;
            } else {
                let abs = loaded.root.dir.join(e.path.as_str());
                let old = std::fs::read_to_string(abs.as_std_path())
                    .map_err(|err| Error::io(e.path.as_str(), "read", &err, false))?;
                let new = edit(&old)?;
                out.push((e.path.clone(), old, new));
            }
        }
    }
    Ok(out)
}

/// `seal` steps 1 to 4 (R-VER-050): run the check, read git state, plan, build the write-set.
/// `sealed-at` is the `--as-of` date at 00:00 UTC: retention needs days (R-RET-011), and the
/// planner stays deterministic with no clock read of its own.
pub fn plan_seal(
    loaded: &Loaded,
    registry: &Registry,
    options: &CheckOptions,
    request: &SealRequest,
) -> Result<(crate::seal::SealPlan, Plan), Error> {
    use crate::check::FindingState;
    use crate::config::Stage;
    use crate::diag::Severity;
    use crate::diag::codes::HN_F001;
    let context = check_context(loaded, registry, options);
    let input = CheckInput {
        registry,
        corpus: &loaded.corpus,
        facts: &loaded.facts,
        load_diags: &context.diags,
        baseline: context.baseline.as_ref(),
        options,
    };
    let report = check_run(&input);
    let blocking = |f: &&crate::check::Finding| {
        f.severity == Severity::Error
            && f.stage == Stage::Enforce
            && matches!(f.state, FindingState::New | FindingState::WaiverIneffective)
    };
    let gate_errors = report.findings.iter().filter(blocking).count();
    let secrets = report.findings.iter().filter(|f| f.code == HN_F001).count();
    let git = match crate::git::GitRepo::open(&loaded.root) {
        None => crate::seal::SealGit::default(),
        Some(repo) => {
            let (detached, branch) = repo.head_state();
            crate::seal::SealGit {
                available: true,
                detached,
                branch,
                unmerged: repo.unmerged_paths()?,
                changed: repo.changed_files()?.0.into_iter().collect(),
            }
        }
    };
    let (docs, _) = crate::docs::collect_change_docs(&loaded.corpus);
    let paths: BTreeSet<RepoPath> = loaded.corpus.records.keys().cloned().collect();
    let versions = crate::docs::version_files(&loaded.corpus, &loaded.root.folder);
    let (due, plan_records, tombs) = crate::check::builtin::plans::seal_lifecycle(
        registry,
        &loaded.corpus,
        &loaded.facts.ledgers,
        options.as_of.0,
    );
    let seal_plan = crate::seal::plan(&crate::seal::SealInput {
        line: version_line(loaded),
        docs: &docs,
        gate_errors,
        secrets,
        git,
        allow_dirty: request.allow_dirty,
        seal_anywhere: request.seal_anywhere || loaded.root_control.version.seal_anywhere,
        integration_branch: loaded.root_control.version.integration_branch.clone(),
        folder: loaded.root.folder.clone(),
        paths: &paths,
        sweep_after: usize::try_from(loaded.root_control.version.sweep_after).unwrap_or(3),
        sealed_versions: &versions,
        doc_dirs: &docs
            .iter()
            .map(|d| (d.path.clone(), loaded_scope_dir(loaded, &d.path)))
            .collect(),
        due: &due,
        plan_records: &plan_records,
        tombs: &tombs,
        graves: &grave_prints(loaded)?,
    })
    .map_err(|r| {
        Error::Refused(crate::error::Refusal {
            kind: crate::error::RefusalKind::PlanNotConfirmed,
            message: r.message,
            hint: r.hint,
        })
    })?;
    let mut texts = std::collections::BTreeMap::new();
    for d in &seal_plan.sealing {
        let abs = loaded.root.dir.join(d.path.as_str());
        let text = std::fs::read_to_string(abs.as_std_path())
            .map_err(|e| Error::io(d.path.as_str(), "read", &e, false))?;
        texts.insert(d.path.clone(), text);
    }
    let gen_dir = loaded.root_control.layout.r#gen.clone();
    let gen_dir = if gen_dir.is_empty() {
        "gen".to_owned()
    } else {
        gen_dir
    };
    let sealed_at = format!("{}T00:00:00Z", options.as_of);
    let set = crate::seal::write_set(
        &seal_plan,
        &crate::seal::SealWriteCx {
            reg: registry,
            all_docs: &docs,
            texts: &texts,
            ledgers: &loaded.facts.ledgers,
            sealed_at: &sealed_at,
            edition: loaded.root.edition.0,
            folder: &loaded.root.folder,
            gen_dir: &gen_dir,
            digests: &crate::merkle::scope_digests(registry, &loaded.corpus),
        },
    )
    .map_err(Error::Internal)?;
    let lines = set
        .writes
        .values()
        .map(|w| crate::write::PlanLine {
            action: match w.kind {
                crate::write::WriteKind::Create => crate::write::PlanAction::Create,
                _ => crate::write::PlanAction::Edit,
            },
            path: w.path.clone(),
            detail: match w.kind {
                crate::write::WriteKind::Delete => "renamed to its ordinal".into(),
                _ if Some(&w.path) == set.commit_point.as_ref() => {
                    "version file (commit point)".into()
                }
                _ => String::new(),
            },
            diff: None,
        })
        .collect();
    let mut notes = seal_plan.sweep.clone();
    notes.extend(seal_plan.approaching.iter().cloned());
    Ok((seal_plan, Plan::new(set, lines, notes)))
}

pub fn commit_plan(session: &WriteSession<'_>, plan: Plan) -> Result<WriteOutcome, Error> {
    let transaction = Transaction::begin(&session.root, &session.lock, plan.set)?;
    let report = transaction.commit()?;
    Ok(WriteOutcome {
        recovery: session.recovery,
        report,
    })
}

/// R-CLI-019: the first scan `init` reports, with the control file it would write held in memory
/// (A-10: the same bytes `init_plan` writes). Only the walk and file reads touch disk.
pub fn init_first_scan(
    root: &Root,
    control_bytes: &[u8],
    threads: Option<usize>,
    options: &CheckOptions,
) -> Result<CheckReport, Error> {
    let path = RepoPath::new(&format!("{}/control.toml", root.folder))
        .map_err(|_| Error::Internal("the managed control path is not a repo path".into()))?;
    let control = crate::config::parse_control(&path, control_bytes, true)
        .map_err(|failure| Error::Config(Box::new(failure)))?;
    let walked = walk(root, &WalkOptions::from_root_control(&control, threads))?;
    let mut controls = load_controls(root, &walked)?;
    controls.files.insert(path, Ok(control));
    let walked = apply_child_excludes(walked, &controls)?;
    let plan = ExtractPlan::from_controls(root, &controls)?;
    let key = CacheKey {
        fairlead_version: env!("CARGO_PKG_VERSION").to_string(),
        grammar: GRAMMAR_EDITION,
        config_hash: config_hash_of(&controls),
        as_of: None,
    };
    let cache = Cache::open(root, false, key);
    let corpus = in_pool(threads, || {
        extract_files(&walked, root, &plan, &cache, None)
    })?;
    let facts = collect_facts(root, &walked, &controls, false)?;
    let registry = resolve(&ResolveInput {
        root,
        controls: &controls,
        corpus: &corpus,
        facts: &facts,
        as_of: options.as_of,
    });
    let mut load_diags = walked.diags.clone();
    load_diags.extend(control_diagnostics(&controls));
    sort_diagnostics(&mut load_diags);
    Ok(check_run(&CheckInput {
        registry: &registry,
        corpus: &corpus,
        facts: &facts,
        load_diags: &load_diags,
        baseline: None,
        options,
    }))
}
