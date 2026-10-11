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
    /// `--reseal`: sealed docs of the integration line; their ordinals are taken (R-GIT-031).
    pub also_docs: &'a [ChangeDoc],
    /// Every path in the tree, for the sweep report (R-VER-045).
    pub paths: &'a BTreeSet<RepoPath>,
    /// `version.sweep-after` (default 3).
    pub sweep_after: usize,
    /// Every sealed version, for the carried counts of the sweep report.
    pub sealed_versions: &'a [Version],
    /// The home scope directory of each doc (its `covers` globs are relative to it).
    pub doc_dirs: &'a BTreeMap<RepoPath, String>,
    /// Lifecycle plans with their due state (R-LC-006).
    pub due: &'a [DueItem],
    /// Ledger records of symbolic targets this seal resolves (R-LC-003), by the flag's path.
    pub plan_records: &'a [(RepoPath, String)],
    /// Completed lifecycle removals; the seal writes their tombstones (R-LC-020).
    pub tombs: &'a [crate::lifecycle::Tomb],
    /// Graveyard entries fingerprinted by this seal: `(path, old text, new text)` (R-LC-031).
    pub graves: &'a [(RepoPath, String, String)],
}

/// A lifecycle plan as the due gate sees it (R-LC-006).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DueItem {
    /// `kind.name`.
    pub id: String,
    /// `scope:kind.name`.
    pub key: String,
    pub word: String,
    pub target: Option<Version>,
    pub state: crate::lifecycle::DueState,
    /// `path:line` of every remaining reference.
    pub refs: Vec<String>,
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
    /// Plans approaching their target (R-LC-004, info).
    pub approaching: Vec<String>,
    /// Ledger records of symbolic targets resolved by this seal (R-LC-003).
    pub plan_records: Vec<(RepoPath, String)>,
    /// Tombstones this seal writes, `removed` set to the sealed version (R-LC-020).
    pub tombs: Vec<crate::lifecycle::Tomb>,
    /// Graveyard entries this seal fingerprints (R-LC-031).
    pub graves: Vec<(RepoPath, String, String)>,
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
/// match a path of the tree under the doc's home scope.
fn covers_anything(d: &ChangeDoc, dir: &str, paths: &BTreeSet<RepoPath>) -> bool {
    d.covers
        .iter()
        .any(|c| fairlead_syntax::ids::Id::parse(c).is_ok())
        || paths.iter().any(|p| crate::docs::covers_path(d, dir, p))
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
        .chain(input.also_docs)
        .collect();
    let ordinals = assign_ordinals(&ready, &existing);

    // 3. due gate (R-LC-006): a due plan blocks unless its references are gone and a doc in this
    // seal covers the item, or a retarget doc in this seal moves it.
    let names = |d: &ChangeDoc, it: &DueItem, key: &str| {
        let list: Vec<String> = match key {
            "covers" => d.covers.clone(),
            k => d
                .extra
                .get(k)
                .map(|v| crate::lifecycle::doc_ids(v))
                .unwrap_or_default(),
        };
        list.iter().any(|c| *c == it.id || *c == it.key)
    };
    let blocked: Vec<String> = input
        .due
        .iter()
        .filter(|it| it.state == crate::lifecycle::DueState::Due)
        .filter(|it| {
            let moved = ready
                .iter()
                .any(|d| d.kind == "retarget" && names(d, it, "retargets"));
            let finished = it.refs.is_empty() && ready.iter().any(|d| names(d, it, "covers"));
            !moved && !finished
        })
        .map(|it| {
            let at = it.target.map_or_else(String::new, |t| format!(" at {t}"));
            if it.refs.is_empty() {
                format!("{} ({}{at})", it.id, it.word)
            } else {
                format!("{} ({}{at}; refs: {})", it.id, it.word, it.refs.join(", "))
            }
        })
        .collect();
    if !blocked.is_empty() {
        return Err(refuse(
            format!(
                "{} due lifecycle plans: {}",
                blocked.len(),
                blocked.join("; ")
            ),
            "finish them (remove the references, seal a doc covering the ID), add an exception with until, or seal a retarget doc (R-LC-006)",
        ));
    }
    let approaching: Vec<String> = input
        .due
        .iter()
        .filter(|it| it.state == crate::lifecycle::DueState::Approaching)
        .map(|it| {
            let at = it.target.map_or_else(String::new, |t| format!(" at {t}"));
            format!("{} ({}{at}) is approaching", it.id, it.word)
        })
        .collect();

    let carried: Vec<ChangeDoc> = input
        .docs
        .iter()
        .filter(|d| d.status == Status::Draft)
        .cloned()
        .collect();
    let mut sweep = Vec::new();
    for d in &carried {
        let dir = input.doc_dirs.get(&d.path).map_or("", String::as_str);
        if !d.covers.is_empty() && !covers_anything(d, dir, input.paths) {
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
        sealing: ready.iter().map(|d| (*d).clone()).collect(),
        carried,
        ordinals,
        sweep,
        dirty: !dirty.is_empty(),
        approaching,
        plan_records: input.plan_records.to_vec(),
        graves: input.graves.to_vec(),
        tombs: input
            .tombs
            .iter()
            .map(|t| {
                let mut t = t.clone();
                t.removed = open.to_string();
                if let Some(d) = ready.iter().find(|d| {
                    d.covers
                        .iter()
                        .any(|c| *c == t.id || c.ends_with(&format!(":{}", t.id)))
                }) && let Some(title) = &d.title
                {
                    t.why = title.clone();
                }
                t
            })
            .collect(),
    })
}

/// What the write-set builder reads besides the plan.
pub struct SealWriteCx<'a> {
    pub reg: &'a crate::registry::Registry,
    /// Every change doc of the tree (the sealed ones feed the alias maps).
    pub all_docs: &'a [ChangeDoc],
    /// The current text of each doc being sealed.
    pub texts: &'a BTreeMap<RepoPath, String>,
    /// Ledger year files on disk (`Facts::ledgers`).
    pub ledgers: &'a BTreeMap<RepoPath, std::sync::Arc<str>>,
    /// UTC timestamp recorded as `sealed-at` (`YYYY-MM-DDTHH:MM:SSZ`), the seal's one clock read.
    pub sealed_at: &'a str,
    pub edition: u32,
    pub folder: &'a str,
    pub gen_dir: &'a str,
    /// Merkle digest of every scope (`merkle::scope_digests`), for the rollup records.
    pub digests: &'a BTreeMap<crate::registry::ScopeId, String>,
}

/// The sealed text of a doc: `status`, `sealed`, `body-hash` and `ordinal` set in its
/// frontmatter, every other key and the body kept byte for byte (R-VER-012, R-VER-041).
fn sealed_text(text: &str, open: Version, ordinal: u32) -> Result<String, String> {
    let rest = text
        .strip_prefix("+++\n")
        .ok_or("doc does not start with +++ frontmatter")?;
    let (fm, body) = rest
        .split_once("\n+++")
        .ok_or("frontmatter has no closing +++")?;
    let mut doc: toml_edit::DocumentMut = fm.parse().map_err(|e| format!("{e}"))?;
    doc["status"] = toml_edit::value("sealed");
    doc["sealed"] = toml_edit::value(open.to_string());
    doc["body-hash"] = toml_edit::value(crate::docs::body_hash(text));
    doc["ordinal"] = toml_edit::value(i64::from(ordinal));
    let mut fm = doc.to_string();
    if !fm.ends_with('\n') {
        fm.push('\n');
    }
    Ok(format!("+++\n{fm}+++{body}"))
}

/// `<ordinal>-<slug>.md` beside the draft-named file (R-VER-041).
fn ordinal_path(p: &RepoPath, ordinal: u32) -> Result<RepoPath, String> {
    let name = p.file_name();
    let slug = name
        .strip_suffix(".md")
        .unwrap_or(name)
        .strip_prefix("d-")
        .and_then(|r| r.split_once('-'))
        .map_or_else(
            || name.trim_end_matches(".md").to_owned(),
            |(_, s)| s.to_owned(),
        );
    let dir = p
        .parent()
        .map(|d| d.as_str().to_owned())
        .unwrap_or_default();
    let rel = format!("{ordinal:04}-{slug}.md");
    let full = if dir.is_empty() {
        rel
    } else {
        format!("{dir}/{rel}")
    };
    RepoPath::new(&full).map_err(|e| format!("{e:?}"))
}

fn toml_list(v: &[String]) -> String {
    let items: Vec<String> = v.iter().map(|s| format!("{s:?}")).collect();
    format!("[{}]", items.join(", "))
}

/// Steps 4 to 6 of R-VER-050 as one write-set (R-VER-059), committed by the 0.1 transaction with
/// `versions/<O>.md` as the commit point.
pub fn write_set(plan: &SealPlan, cx: &SealWriteCx<'_>) -> Result<crate::write::WriteSet, String> {
    use crate::classify::FileClass;
    use crate::generate::{GenKind, GeneratedFile};
    use crate::hist::YearFile;
    use crate::render::line::TerseLine;
    use crate::write::{PlannedWrite, WriteKind, WriteSet, WriteSetKind};

    let mut set = WriteSet::new(WriteSetKind::Seal);
    let mut add = |w: PlannedWrite| set.add(w).map_err(|e| e.to_string());
    let year: u32 = cx
        .sealed_at
        .get(..4)
        .and_then(|y| y.parse().ok())
        .ok_or("sealed-at has no year")?;
    let mut sealed_now: Vec<ChangeDoc> = Vec::new();
    let mut records: BTreeMap<ScopeKey, Vec<String>> = BTreeMap::new();
    for d in &plan.sealing {
        let ord = *plan.ordinals.get(&d.id).ok_or("doc without an ordinal")?;
        let text = cx
            .texts
            .get(&d.path)
            .ok_or_else(|| format!("{} was not read", d.path.as_str()))?;
        let new_text = sealed_text(text, plan.open, ord)?;
        let target = ordinal_path(&d.path, ord)?;
        add(PlannedWrite {
            path: target.clone(),
            kind: WriteKind::Create,
            bytes: new_text.into_bytes(),
            class: FileClass::Authored,
            expect_prior: None,
        })?;
        if target != d.path {
            add(PlannedWrite {
                path: d.path.clone(),
                kind: WriteKind::Delete,
                bytes: Vec::new(),
                class: FileClass::Authored,
                expect_prior: Some(crate::hash::hash_normalised(text.as_bytes())),
            })?;
        }
        let scope = cx
            .reg
            .scope_of(&d.path)
            .cloned()
            .unwrap_or_else(|| cx.reg.root_scope().clone());
        let line = TerseLine::new(d.id.as_str())
            .with("kind", &d.kind)
            .with("ord", &format!("{ord:04}"))
            .with(
                "impact",
                match d.impact {
                    Some(crate::version::Impact::Break) => "break",
                    Some(crate::version::Impact::Add) => "add",
                    _ => "fix",
                },
            )
            .with("title", d.title.as_deref().unwrap_or(""))
            .render();
        records
            .entry(ScopeKey(scope.clone()))
            .or_default()
            .push(line);
        if d.kind == "revive"
            && let (Some(old), Some(new)) = (d.extra.get("revives"), d.extra.get("as"))
        {
            // R-LC-024: the link from the tombstoned ID to its versioned successor.
            records
                .entry(ScopeKey(scope))
                .or_default()
                .push(format!("revive {old} as={new} doc={}", d.id));
        }
        if d.rolling {
            // R-VER-035: a rolling doc is recorded in this version and a successor draft with the
            // same covers opens the next one. Its draft ID is derived from the old ID and the
            // version, so the seal stays deterministic.
            let digest = crate::hash::hash_raw(format!("{} {}", d.id, plan.open).as_bytes());
            let mut bits = [0u8; 5];
            bits.copy_from_slice(&digest.0[..5]);
            let next = crate::docs::DraftId::from_bits(bits);
            let slug = d
                .title
                .as_deref()
                .map_or_else(|| "chore".to_owned(), crate::docs::slug);
            let dir = d
                .path
                .parent()
                .map(|p| p.as_str().to_owned())
                .unwrap_or_default();
            let path =
                RepoPath::new(&format!("{dir}/{next}-{slug}.md")).map_err(|e| format!("{e:?}"))?;
            let text = format!(
                "+++\nid = \"{next}\"\nkind = \"{}\"\nstatus = \"draft\"\nrolling = true\nscope = \"{}\"\ntitle = \"{}\"\ncovers = {}\ncreated = \"{}\"\n+++\n\n# {}\n\nSuccessor of {} (rolling).\n",
                d.kind,
                d.scope.as_deref().unwrap_or("root"),
                d.title.as_deref().unwrap_or("chore").replace('"', "'"),
                toml_list(&d.covers),
                plan.open,
                d.title.as_deref().unwrap_or("chore"),
                d.id
            );
            add(PlannedWrite {
                path,
                kind: WriteKind::Create,
                bytes: text.into_bytes(),
                class: FileClass::Authored,
                expect_prior: None,
            })?;
        }
        let mut sealed = d.clone();
        sealed.status = Status::Sealed;
        sealed.ordinal = Some(ord);
        sealed.path = target;
        sealed_now.push(sealed);
    }

    // Fingerprints of graveyard entries, computed once, here (R-LC-031).
    for (path, old, new) in &plan.graves {
        add(PlannedWrite {
            path: path.clone(),
            kind: WriteKind::Replace,
            bytes: new.clone().into_bytes(),
            class: FileClass::Authored,
            expect_prior: Some(crate::hash::hash_normalised(old.as_bytes())),
        })?;
    }

    // Tombstones of completed removals (R-LC-020), one append per year file.
    let mut tomb_files: BTreeMap<RepoPath, String> = BTreeMap::new();
    for t in &plan.tombs {
        let Some(scope) = t.scope.as_ref() else {
            continue;
        };
        let year = i16::try_from(year).map_err(|_| "year out of range")?;
        let file = crate::lifecycle::tomb_file(cx.reg, scope, cx.folder, year)
            .ok_or("bad tombstone folder")?;
        tomb_files
            .entry(file)
            .or_default()
            .push_str(&crate::lifecycle::tomb_text(t, None));
        let line = format!("tomb {}:{} removed={}", scope.0.as_str(), t.id, t.removed);
        records
            .entry(ScopeKey(scope.clone()))
            .or_default()
            .push(line);
    }
    for (path, text) in tomb_files {
        add(PlannedWrite {
            path,
            kind: WriteKind::AppendBlock {
                marker: String::new(),
            },
            bytes: text.into_bytes(),
            class: FileClass::Authored,
            expect_prior: None,
        })?;
    }

    // Symbolic plan targets resolved by this seal (R-LC-003).
    for (path, rec) in &plan.plan_records {
        let scope = cx
            .reg
            .scope_of(path)
            .cloned()
            .unwrap_or_else(|| cx.reg.root_scope().clone());
        records
            .entry(ScopeKey(scope))
            .or_default()
            .push(rec.clone());
    }

    // Rollups (R-VER-052): every touched scope and every ancestor of one gets a rollup record:
    // its own waivers, the subtree's waivers and its Merkle digest, so a reader of the root never
    // descends. Computed only here, at seal (D-10).
    let own_waivers = |s: &crate::registry::ScopeId| {
        cx.reg
            .ledger(s)
            .entries
            .iter()
            .filter(|e| &e.varied_in == s && e.state == crate::registry::OvrState::Effective)
            .count()
    };
    let subtree_of = |s: &crate::registry::ScopeId| {
        cx.reg
            .scopes()
            .filter(|x| cx.reg.chain(&x.id).contains(&s))
            .map(|x| own_waivers(&x.id))
            .sum::<usize>()
    };
    let touched: Vec<crate::registry::ScopeId> = records.keys().map(|k| k.0.clone()).collect();
    let mut rolled: BTreeSet<crate::registry::ScopeId> = BTreeSet::new();
    for s in &touched {
        for a in cx.reg.chain(s) {
            rolled.insert(a.clone());
        }
    }
    for s in rolled {
        let line = TerseLine::new(&format!("rollup.{}", plan.open))
            .with("kind", "rollup")
            .with("waivers", &own_waivers(&s).to_string())
            .with("subtree-waivers", &subtree_of(&s).to_string())
            .with("digest", cx.digests.get(&s).map_or("", String::as_str))
            .render();
        records.entry(ScopeKey(s)).or_default().push(line);
    }

    // Ledger year files of the touched scopes (R-VER-059, R-RET-031).
    for (ScopeKey(scope), recs) in &records {
        let dir = cx
            .reg
            .scope(scope)
            .map(|s| s.dir.as_str().to_owned())
            .unwrap_or_default();
        let base = crate::generate::join_dir(&dir, &format!("{}/ledger/hist", cx.folder))
            .ok_or("bad ledger folder")?;
        let path =
            RepoPath::new(&format!("{}/{year}.hl", base.as_str())).map_err(|e| format!("{e:?}"))?;
        let mut years: Vec<(u32, &RepoPath, YearFile)> = cx
            .ledgers
            .iter()
            .filter_map(|(p, t)| YearFile::parse(t).ok().map(|f| (f.year, p, f)))
            .filter(|(_, p, _)| p.parent().as_ref() == Some(&base))
            .collect();
        years.sort_by_key(|(y, _, _)| *y);
        let mut file = match years.iter().find(|(y, _, _)| *y == year) {
            Some((_, _, f)) => f.clone(),
            None => {
                let prev = match years.iter().rev().find(|(y, _, _)| *y < year) {
                    Some((_, p, f)) if !f.is_final() => {
                        let mut closed = f.clone();
                        closed.finalise();
                        add(PlannedWrite {
                            path: (*p).clone(),
                            kind: WriteKind::Replace,
                            bytes: closed.render().into_bytes(),
                            class: FileClass::Ledger,
                            expect_prior: None,
                        })?;
                        Some(closed.last_digest())
                    }
                    Some((_, _, f)) => Some(f.last_digest()),
                    None => None,
                };
                YearFile::new(scope.0.as_str(), year, prev, cx.edition)
            }
        };
        file.append_version(plan.open, recs)?;
        add(PlannedWrite {
            path,
            kind: WriteKind::Replace,
            bytes: file.render().into_bytes(),
            class: FileClass::Ledger,
            expect_prior: None,
        })?;
    }

    // Generated files the seal changes: alias maps and scope locks (R-VER-046, R-VER-015).
    let mut docs_after: Vec<ChangeDoc> = cx
        .all_docs
        .iter()
        .filter(|d| {
            !plan
                .sealing
                .iter()
                .any(|s| s.id == d.id && s.path == d.path)
        })
        .cloned()
        .collect();
    docs_after.extend(sealed_now);
    for scope in cx.reg.scopes() {
        if !matches!(scope.state, crate::registry::ScopeState::Resolved) {
            continue;
        }
        let dir = scope.dir.as_str();
        let here: Vec<&ChangeDoc> = docs_after
            .iter()
            .filter(|d| cx.reg.scope_of(&d.path) == Some(&scope.id))
            .collect();
        let mut files = vec![(
            "scope.lock",
            GenKind::ScopeLock,
            crate::merkle::scope_lock_body(
                plan.open,
                &crate::merkle::ancestor_digest(cx.reg, &scope.id),
            ),
        )];
        let alias = crate::docs::alias_body(&here);
        if !alias.is_empty() {
            files.push(("alias", GenKind::Alias, alias));
        }
        for (name, kind, body) in files {
            let path =
                crate::generate::join_dir(dir, &format!("{}/{}/{name}", cx.folder, cx.gen_dir))
                    .ok_or("bad generated path")?;
            let g = GeneratedFile {
                path: path.clone(),
                scope: scope.id.clone(),
                kind,
                body,
            };
            add(PlannedWrite {
                path,
                kind: WriteKind::Replace,
                bytes: g.render(crate::discover::Edition(cx.edition)),
                class: FileClass::Generated,
                expect_prior: None,
            })?;
        }
    }

    // The version file, written last: the commit point (R-VER-050 step 6, R-VER-053).
    let version_path = RepoPath::new(&format!("{}/versions/{}.md", cx.folder, plan.open))
        .map_err(|e| format!("{e:?}"))?;
    let mut scopes: Vec<String> = records.keys().map(|k| k.0.0.as_str().to_owned()).collect();
    scopes.dedup();
    let ids = |v: &[ChangeDoc]| v.iter().map(|d| d.id.to_string()).collect::<Vec<_>>();
    let mut text = format!("+++\nversion = \"{}\"\n", plan.open);
    if let Some(p) = plan.prev {
        text.push_str(&format!("prev = \"{p}\"\n"));
    }
    // R-EDN-006: the pins in force at this seal, the floor a later lowering is measured against.
    if let Some(c) = cx
        .reg
        .scope(cx.reg.root_scope())
        .and_then(|s| s.control.as_ref())
    {
        text.push_str(&crate::edition::Pins::of(c).version_lines());
    }
    text.push_str(&format!(
        "bump = \"{}\"\nsealed-at = \"{}\"\nscopes = {}\ndocs = {}\ncarried = {}\ndirty = {}\n+++\n\n# {}\n\n",
        plan.bump,
        cx.sealed_at,
        toml_list(&scopes),
        toml_list(&ids(&plan.sealing)),
        toml_list(&ids(&plan.carried)),
        plan.dirty,
        plan.open
    ));
    for d in &plan.sealing {
        let ord = plan.ordinals.get(&d.id).copied().unwrap_or_default();
        let impact = match d.impact {
            Some(crate::version::Impact::Break) => "break",
            Some(crate::version::Impact::Add) => "add",
            _ => "fix",
        };
        text.push_str(&format!(
            "- {impact}: {} ({}, {} {ord:04})\n",
            d.title.as_deref().unwrap_or("untitled"),
            d.scope.as_deref().unwrap_or("root"),
            d.id
        ));
    }
    add(PlannedWrite {
        path: version_path.clone(),
        kind: WriteKind::Create,
        bytes: text.into_bytes(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    set.commit_point = Some(version_path);
    Ok(set)
}

/// Ordering wrapper so scopes can key a `BTreeMap`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ScopeKey(crate::registry::ScopeId);

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
            title: None,
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
        due: Vec<DueItem>,
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
                due: Vec::new(),
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
                also_docs: &[],
                paths: &self.paths,
                sweep_after: 3,
                sealed_versions: &self.versions,
                doc_dirs: &BTreeMap::new(),
                due: &self.due,
                plan_records: &[],
                tombs: &[],
                graves: &[],
            })
        }
    }

    fn ready() -> ChangeDoc {
        doc("d-00000001", Status::Ready, Some(Impact::Add), "0.2.1")
    }

    fn due_item(refs: &[&str]) -> DueItem {
        DueItem {
            id: "engine.old".into(),
            key: "root:engine.old".into(),
            word: "remove".into(),
            target: Some(Version::new(0, 2, 1)),
            state: crate::lifecycle::DueState::Due,
            refs: refs.iter().map(|r| (*r).to_string()).collect(),
        }
    }

    #[test]
    fn r_lc_006__due_item_blocks_seal() {
        let mut fx = Fx::new(vec![ready()]);
        fx.due = vec![due_item(&["src/a.ts:3", "src/b.ts:9"])];
        let r = fx.run().unwrap_err();
        assert!(
            r.message
                .contains("engine.old (remove at 0.2.1; refs: src/a.ts:3, src/b.ts:9)"),
            "{}",
            r.message
        );
        // References gone, and a doc in this seal covers the ID: the removal completes.
        let mut removal = ready();
        removal.covers.push("engine.old".into());
        let mut fx = Fx::new(vec![removal]);
        fx.due = vec![due_item(&[])];
        assert!(fx.run().is_ok());
    }

    #[test]
    fn r_lc_006__retarget_doc_unblocks() {
        let mut rt = ready();
        rt.kind = "retarget".into();
        rt.extra.insert("retargets".into(), "engine.old".into());
        rt.extra.insert("target".into(), "0.4.0".into());
        let mut fx = Fx::new(vec![rt]);
        fx.due = vec![due_item(&["src/a.ts:3"])];
        assert!(fx.run().is_ok());
    }

    #[test]
    fn r_lc_004__approaching_plans_are_listed_not_blocking() {
        let mut fx = Fx::new(vec![ready()]);
        let mut it = due_item(&["src/a.ts:3"]);
        it.state = crate::lifecycle::DueState::Approaching;
        fx.due = vec![it];
        let p = fx.run().unwrap();
        assert_eq!(
            p.approaching,
            ["engine.old (remove at 0.2.1) is approaching"]
        );
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
