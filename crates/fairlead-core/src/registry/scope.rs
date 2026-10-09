//! Scope tree: stored ids, parents, depth, membership (R-SCOPE-001..005, 054).
use std::collections::BTreeMap;
use std::sync::Arc;

use fairlead_syntax::diag::Severity;
use fairlead_syntax::ids::ScopeSlug;

use crate::config::{EligibilityConfig, OvrDecl};
use crate::manifest::ManifestHints;
use crate::registry::ovr::ovr_fingerprint;
use crate::registry::{ItemId, OvrId, Registry};
use crate::time::AsOf;

use crate::config::{ConfigFailure, ControlFile, ControlSet, Owner};
use crate::diag::{Diagnostic, codes};
use crate::discover::Root;
use crate::facts::Facts;
use crate::hash::{base32, hash_raw};
use crate::path::RepoPath;
use crate::registry::Dg;

/// Identity is the id, never the path (R-SCOPE-002).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ScopeId(pub ScopeSlug);

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScopeState {
    Resolved,
    Unresolved { reason: Vec<Diagnostic> },
    AbsentFromTree,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Scope {
    pub id: ScopeId,
    pub dir: RepoPath,
    pub parent: Option<ScopeId>,
    pub depth: u32,
    pub control: Option<Arc<ControlFile>>,
    pub state: ScopeState,
    pub owners: Vec<Owner>,
}

/// Directory helpers on normalised repo-relative strings; the root directory is "".
pub(crate) fn parent_dir(dir: &str) -> Option<&str> {
    if dir.is_empty() {
        return None;
    }
    Some(match dir.rfind('/') {
        Some(i) => dir.get(..i).unwrap_or(""),
        None => "",
    })
}

pub(crate) fn dir_depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

pub(crate) fn is_within(path: &str, dir: &str) -> bool {
    dir.is_empty()
        || path == dir
        || path
            .strip_prefix(dir)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Owner directory of `<dir>/<folder>/control.toml`.
pub(crate) fn dir_of_control(control_path: &str, folder: &str) -> String {
    let tail = format!("{folder}/control.toml");
    if control_path == tail {
        return String::new();
    }
    match control_path.strip_suffix(&format!("/{tail}")) {
        Some(d) => d.to_string(),
        None => {
            let mut it = control_path.rsplitn(3, '/');
            it.next();
            it.next();
            it.next().unwrap_or("").to_string()
        }
    }
}

#[allow(clippy::expect_used)] // a fixed lowercase prefix plus 8 base32 characters always matches the slug grammar
pub(crate) fn synth_slug(prefix: &str, key: &str) -> ScopeSlug {
    let s = format!("{prefix}-{}", base32(&hash_raw(key.as_bytes()).0, 8));
    ScopeSlug::parse(&s).expect("synthetic slug is valid") // Verify: ScopeSlug::parse accepts "dup-xxxxxxxx"
}

/// The scope tree. Internal; `Registry` exposes it through accessors.
#[derive(Clone, Debug)]
pub(crate) struct ScopeTable {
    pub(crate) root: ScopeId,
    pub(crate) scopes: BTreeMap<ScopeId, Scope>,
    pub(crate) by_dir: BTreeMap<String, ScopeId>,
}

impl ScopeTable {
    pub(crate) fn get(&self, id: &ScopeId) -> Option<&Scope> {
        self.scopes.get(id)
    }

    /// `root ..= id`.
    pub(crate) fn chain_ids(&self, id: &ScopeId) -> Vec<&ScopeId> {
        let mut out = Vec::new();
        let mut cur = self.scopes.get(id);
        while let Some(s) = cur {
            out.push(&s.id);
            cur = s.parent.as_ref().and_then(|p| self.scopes.get(p));
        }
        out.reverse();
        out
    }

    /// Nearest ancestor-or-self scope (R-SCOPE-003). Pure function of the path.
    pub(crate) fn scope_of(&self, path: &RepoPath) -> Option<&ScopeId> {
        let mut cur: &str = path.as_str();
        loop {
            if let Some(id) = self.by_dir.get(cur) {
                return Some(id);
            }
            cur = parent_dir(cur)?;
        }
    }

    pub(crate) fn is_ancestor_or_self(&self, anc: &ScopeId, of: &ScopeId) -> bool {
        self.chain_ids(of).into_iter().any(|s| s == anc)
    }

    pub(crate) fn is_resolved(&self, id: &ScopeId) -> bool {
        matches!(
            self.scopes.get(id).map(|s| &s.state),
            Some(ScopeState::Resolved)
        )
    }

    /// Scopes ordered by (depth, directory): parents always precede children.
    pub(crate) fn in_depth_order(&self) -> Vec<&Scope> {
        let mut v: Vec<&Scope> = self.scopes.values().collect();
        v.sort_by(|a, b| (a.depth, a.dir.as_str()).cmp(&(b.depth, b.dir.as_str())));
        v
    }

    pub(crate) fn root_control(&self) -> Option<&ControlFile> {
        self.scopes
            .get(&self.root)
            .and_then(|s| s.control.as_deref())
    }
}

struct Raw {
    dir: String,
    dir_path: RepoPath,
    control_path: RepoPath,
    res: Result<Arc<ControlFile>, ConfigFailure>,
}

fn nearest_scope_dir(dir: &str, by_dir: &BTreeMap<String, ScopeId>) -> Option<ScopeId> {
    let mut cur = dir;
    while let Some(p) = parent_dir(cur) {
        if let Some(id) = by_dir.get(p) {
            return Some(id.clone());
        }
        cur = p;
    }
    None
}

pub(crate) fn build_scope_table(
    root: &Root,
    controls: &ControlSet,
    facts: &Facts,
    diags: &mut Vec<Diagnostic>,
) -> ScopeTable {
    let folder = root.folder.as_str();
    let mut raws: Vec<Raw> = Vec::new();
    for (cpath, res) in &controls.files {
        match res {
            Ok(cf) => raws.push(Raw {
                dir: cf.dir.as_str().trim_matches('/').to_string(),
                dir_path: cf.dir.clone(),
                control_path: cpath.clone(),
                res: Ok(Arc::new(cf.clone())),
            }),
            Err(f) => {
                let dir = dir_of_control(cpath.as_str(), folder);
                let dir_path = RepoPath::new(&dir).unwrap_or_else(|_| cpath.clone());
                raws.push(Raw {
                    dir,
                    dir_path,
                    control_path: cpath.clone(),
                    res: Err(f.clone()),
                });
            }
        }
    }
    raws.sort_by(|a, b| {
        (dir_depth(&a.dir), a.dir.as_str()).cmp(&(dir_depth(&b.dir), b.dir.as_str()))
    });

    let mut scopes: BTreeMap<ScopeId, Scope> = BTreeMap::new();
    let mut by_dir: BTreeMap<String, ScopeId> = BTreeMap::new();
    let mut claimed: BTreeMap<ScopeId, RepoPath> = BTreeMap::new();
    for raw in raws {
        let parent = nearest_scope_dir(&raw.dir, &by_dir);
        let (mut id, control, mut state, owners) = match &raw.res {
            Ok(cf) => {
                let owners = if raw.dir.is_empty() {
                    cf.settings.owners.clone()
                } else {
                    Vec::new()
                };
                (
                    ScopeId(cf.fairlead.scope.clone()),
                    Some(Arc::clone(cf)),
                    ScopeState::Resolved,
                    owners,
                )
            }
            Err(f) => {
                let from_shard = parent
                    .as_ref()
                    .and_then(|p| member_id(&scopes, p, &raw.dir, folder, facts));
                let id = from_shard.unwrap_or_else(|| ScopeId(synth_slug("unresolved", &raw.dir)));
                (
                    id,
                    None,
                    ScopeState::Unresolved {
                        reason: f.diags.clone(),
                    },
                    Vec::new(),
                )
            }
        };
        let first_claim = claimed.get(&id).map(|p| p.as_str().to_string());
        if let Some(first) = first_claim {
            let d = Dg::new(codes::HN_C007, Severity::Error, &raw.control_path, 1, 1)
                .scope(&id)
                .msg(format!(
                    "scope id \"{}\" is already declared by {first}",
                    id.0.as_str()
                ))
                .hint(
                    "scope ids are unique: change [fairlead] scope in one of the two control files",
                )
                .text(id.0.as_str())
                .done();
            diags.push(d.clone());
            id = ScopeId(synth_slug("dup", raw.control_path.as_str()));
            state = ScopeState::Unresolved { reason: vec![d] };
        } else {
            claimed.insert(id.clone(), raw.control_path.clone());
        }
        if state == ScopeState::Resolved {
            let inherited = parent
                .as_ref()
                .and_then(|p| scopes.get(p))
                .map(|s| s.state.clone());
            if let Some(ScopeState::Unresolved { reason }) = inherited {
                state = ScopeState::Unresolved { reason };
            }
        }
        let depth = parent
            .as_ref()
            .and_then(|p| scopes.get(p))
            .map(|s| s.depth + 1)
            .unwrap_or(0);
        if state == ScopeState::Resolved
            && let Some(pid) = parent.as_ref()
        {
            let registered = scopes
                .get(pid)
                .filter(|p| p.state == ScopeState::Resolved)
                .map(|p| {
                    members_of(p, folder, facts)
                        .unwrap_or_default()
                        .iter()
                        .any(|m| m.id == id.0.as_str())
                });
            if registered == Some(false) {
                diags.push(
                        Dg::new(codes::HN_C008, Severity::Error, &raw.control_path, 1, 1)
                            .scope(&id)
                            .msg(format!(
                                "scope \"{}\" is not registered in the membership shard of its parent",
                                id.0.as_str()
                            ))
                            .hint("run `fairlead sync` to register it, or remove the stray managerial folder")
                            .text(id.0.as_str())
                            .done(),
                    );
            }
        }
        by_dir.insert(raw.dir.clone(), id.clone());
        scopes.insert(
            id.clone(),
            Scope {
                id,
                dir: raw.dir_path.clone(),
                parent,
                depth,
                control,
                state,
                owners,
            },
        );
    }

    let absent: Vec<String> = facts
        .git
        .sparse_absent
        .iter()
        .map(|p| p.as_str().to_string())
        .collect();
    if !absent.is_empty() {
        let parents: Vec<Scope> = scopes
            .values()
            .filter(|s| s.state == ScopeState::Resolved)
            .cloned()
            .collect();
        for p in parents {
            for m in members_of(&p, folder, facts).unwrap_or_default() {
                let (Some(dir), Ok(slug)) = (m.path.clone(), ScopeSlug::parse(&m.id)) else {
                    continue;
                };
                let id = ScopeId(slug);
                if scopes.contains_key(&id) || !absent.iter().any(|a| is_within(&dir, a)) {
                    continue;
                }
                if let Ok(dp) = RepoPath::new(&dir) {
                    by_dir.insert(dir, id.clone());
                    scopes.insert(
                        id.clone(),
                        Scope {
                            id,
                            dir: dp,
                            parent: Some(p.id.clone()),
                            depth: p.depth + 1,
                            control: None,
                            state: ScopeState::AbsentFromTree,
                            owners: Vec::new(),
                        },
                    );
                }
            }
        }
    }

    let root_id = match by_dir.get("") {
        Some(id) => id.clone(),
        None => synth_root(&mut scopes, &mut by_dir, folder, diags),
    };
    ScopeTable {
        root: root_id,
        scopes,
        by_dir,
    }
}

/// Only reachable when `discover` was bypassed: no control file at the root directory.
fn synth_root(
    scopes: &mut BTreeMap<ScopeId, Scope>,
    by_dir: &mut BTreeMap<String, ScopeId>,
    folder: &str,
    diags: &mut Vec<Diagnostic>,
) -> ScopeId {
    let id = ScopeId(synth_slug("unresolved", ""));
    // Verify: P3's RepoPath::new("") denotes the root directory.
    if let (Ok(cpath), Ok(dir)) = (
        RepoPath::new(&format!("{folder}/control.toml")),
        RepoPath::new(""),
    ) {
        let d = Dg::new(codes::HN_C001, Severity::Error, &cpath, 1, 1)
            .scope(&id)
            .msg("no root control file (root = true) was found")
            .hint("run `fairlead init` at the repository root")
            .done();
        diags.push(d.clone());
        scopes.insert(
            id.clone(),
            Scope {
                id: id.clone(),
                dir,
                parent: None,
                depth: 0,
                control: None,
                state: ScopeState::Unresolved { reason: vec![d] },
                owners: Vec::new(),
            },
        );
        by_dir.insert(String::new(), id.clone());
    }
    id
}

// --- membership shards (the HN-C008 check arrives in Task 3) -------------------------------

/// One line of a `gen/members` shard (P4-2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MemberLine {
    pub id: String,
    pub path: Option<String>,
}

pub fn parse_members(bytes: &[u8]) -> Vec<MemberLine> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let id = it.next().unwrap_or("").to_string();
        let path = it.find_map(|t| t.strip_prefix("path=")).map(str::to_string);
        out.push(MemberLine { id, path });
    }
    out
}

fn shard_path(parent: &Scope, folder: &str) -> String {
    let gen_dir = parent
        .control
        .as_ref()
        .map(|c| c.layout.r#gen.clone()) // Verify: contract field `gen` must be spelled r#gen in edition 2024
        .unwrap_or_else(|| "gen".to_string());
    let base = parent.dir.as_str().trim_matches('/');
    if base.is_empty() {
        format!("{folder}/{gen_dir}/members")
    } else {
        format!("{base}/{folder}/{gen_dir}/members")
    }
}

/// `None` when the parent has no shard on disk.
pub(crate) fn members_of(parent: &Scope, folder: &str, facts: &Facts) -> Option<Vec<MemberLine>> {
    let key = RepoPath::new(&shard_path(parent, folder)).ok()?;
    facts.generated_on_disk.get(&key).map(|b| parse_members(b))
}

fn member_id(
    scopes: &BTreeMap<ScopeId, Scope>,
    parent: &ScopeId,
    dir: &str,
    folder: &str,
    facts: &Facts,
) -> Option<ScopeId> {
    let lines = members_of(scopes.get(parent)?, folder, facts)?;
    let m = lines.into_iter().find(|m| m.path.as_deref() == Some(dir))?;
    ScopeSlug::parse(&m.id).ok().map(ScopeId)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EligibilityReason {
    FileLocalLines { files: u32 },
    ManifestMember,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EligibilityHint {
    pub dir: RepoPath,
    pub reason: EligibilityReason,
}

/// R-SCOPE-011, R-SCOPE-016: hints only, never automatic; the manifest never overrides a declaration.
pub fn scope_eligibility(
    reg: &Registry,
    cfg: &EligibilityConfig,
    hints: &ManifestHints,
) -> Vec<EligibilityHint> {
    let mut lines: BTreeMap<&str, u32> = BTreeMap::new();
    for p in reg
        .edges
        .iter()
        .map(|e| &e.from.path)
        .chain(reg.sites.values().flatten().map(|s| &s.path))
        .chain(
            reg.overrides
                .iter()
                .zip(&reg.meta)
                .filter(|(_, m)| matches!(m.extent, crate::registry::ovr::OvrExtent::File(_)))
                .map(|(o, _)| &o.at.path),
        )
    {
        *lines.entry(p.as_str()).or_insert(0) += 1;
    }
    let mut per_dir: BTreeMap<&str, u32> = BTreeMap::new();
    for (file, _) in lines.iter().filter(|(_, n)| **n >= cfg.min_lines) {
        if let Some(d) = parent_dir(file) {
            *per_dir.entry(d).or_insert(0) += 1;
        }
    }
    let mut out: BTreeMap<String, EligibilityReason> = BTreeMap::new();
    for (d, files) in per_dir {
        if files >= cfg.min_files && !d.is_empty() && !reg.table.by_dir.contains_key(d) {
            out.insert(d.to_string(), EligibilityReason::FileLocalLines { files });
        }
    }
    for m in &hints.workspace_members {
        if !reg.table.by_dir.contains_key(m.as_str()) {
            out.entry(m.as_str().to_string())
                .or_insert(EligibilityReason::ManifestMember);
        }
    }
    out.into_iter()
        .filter_map(|(d, reason)| {
            RepoPath::new(&d)
                .ok()
                .map(|dir| EligibilityHint { dir, reason })
        })
        .collect()
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tombstone {
    pub id: ScopeSlug,
    pub former_path: RepoPath,
    pub written_in: ScopeId,
    pub date: jiff::civil::Date,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PromotePlan {
    pub ancestor: ScopeId,
    pub new_scope: ScopeId,
    pub dir: RepoPath,
    pub moved: Vec<OvrDecl>,
    pub kept: Vec<OvrDecl>,
    pub rebinds: Vec<(OvrId, OvrId)>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PromoteRefusal {
    NotBelowAncestor,
    ScopeExists,
    IdInUse,
    AncestorUnresolved,
}

fn rel_dir<'a>(dir: &'a str, base: &str) -> Option<&'a str> {
    if base.is_empty() {
        return Some(dir);
    }
    dir.strip_prefix(base)?.strip_prefix('/')
}

/// Old and new `OvrId` of a declaration that moves from scope `from` to scope `to` (R-CHK-026).
fn rebind(
    reg: &Registry,
    resolve_from: &ScopeId,
    d: &OvrDecl,
    from: &ScopeId,
    to: &ScopeId,
) -> Option<(OvrId, OvrId)> {
    let (target, _) = reg.resolve_target(resolve_from, &d.target)?;
    let exp = d
        .exp
        .as_deref()
        .and_then(|s| s.parse::<jiff::civil::Date>().ok());
    let fp = |s: &ScopeId| {
        OvrId(ovr_fingerprint(
            &target,
            s,
            d.why.as_deref().unwrap_or(""),
            d.owner.as_deref().unwrap_or(""),
            exp.as_ref(),
        ))
    };
    Some((fp(from), fp(to)))
}

pub fn promote_model(
    reg: &Registry,
    ancestor: &ScopeId,
    dir: &RepoPath,
    new_id: &ScopeSlug,
) -> Result<PromotePlan, PromoteRefusal> {
    let anc = reg
        .table
        .get(ancestor)
        .filter(|s| s.state == ScopeState::Resolved)
        .ok_or(PromoteRefusal::AncestorUnresolved)?;
    let cf = anc
        .control
        .as_ref()
        .ok_or(PromoteRefusal::AncestorUnresolved)?;
    let rel = rel_dir(dir.as_str(), anc.dir.as_str().trim_matches('/'))
        .filter(|r| !r.is_empty())
        .ok_or(PromoteRefusal::NotBelowAncestor)?;
    if reg.table.by_dir.contains_key(dir.as_str()) {
        return Err(PromoteRefusal::ScopeExists);
    }
    let new_scope = ScopeId(new_id.clone());
    if reg.table.get(&new_scope).is_some() {
        return Err(PromoteRefusal::IdInUse);
    }
    let (mut moved, mut kept, mut rebinds) = (Vec::new(), Vec::new(), Vec::new());
    for d in &cf.ovrs {
        let rest = d
            .applies
            .as_deref()
            .and_then(|a| a.strip_prefix(rel))
            .and_then(|a| a.strip_prefix('/'));
        match rest {
            Some(rest) => {
                let mut m = d.clone();
                m.applies = if rest == "**" {
                    None
                } else {
                    Some(rest.to_string())
                };
                rebinds.extend(rebind(reg, ancestor, d, ancestor, &new_scope));
                moved.push(m);
            }
            None => kept.push(d.clone()),
        }
    }
    Ok(PromotePlan {
        ancestor: ancestor.clone(),
        new_scope,
        dir: dir.clone(),
        moved,
        kept,
        rebinds,
    })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DemotePlan {
    pub scope: ScopeId,
    pub into: ScopeId,
    pub tombstone: Tombstone,
    pub moved: Vec<OvrDecl>,
    pub items_moved: Vec<ItemId>,
    pub reparented: Vec<ScopeId>,
    pub rebinds: Vec<(OvrId, OvrId)>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DemoteRefusal {
    Root,
    Unresolved,
    DuplicateId(ItemId),
}

/// R-SCOPE-014: tombstone data plus the inverse of `promote_model`; refuses on duplicate IDs.
pub fn demote_model(
    reg: &Registry,
    scope: &ScopeId,
    as_of: AsOf,
) -> Result<DemotePlan, DemoteRefusal> {
    let s = reg.table.get(scope).ok_or(DemoteRefusal::Unresolved)?;
    let into = s.parent.clone().ok_or(DemoteRefusal::Root)?;
    let cf = s
        .control
        .as_ref()
        .filter(|_| s.state == ScopeState::Resolved)
        .ok_or(DemoteRefusal::Unresolved)?;
    let anc = reg.table.get(&into).ok_or(DemoteRefusal::Unresolved)?;
    let rel = rel_dir(s.dir.as_str(), anc.dir.as_str().trim_matches('/'))
        .unwrap_or("")
        .to_string();
    let items_moved: Vec<ItemId> = reg
        .items
        .keys()
        .filter(|k| k.scope == *scope)
        .cloned()
        .collect();
    for m in &items_moved {
        let clash = reg.items.keys().find(|k| {
            k.scope != *scope
                && k.kind == m.kind
                && k.name == m.name
                && (reg.table.is_ancestor_or_self(&k.scope, &into)
                    || reg.table.is_ancestor_or_self(&into, &k.scope))
                && !reg.table.is_ancestor_or_self(scope, &k.scope)
        });
        if clash.is_some() {
            return Err(DemoteRefusal::DuplicateId(m.clone()));
        }
    }
    let mut moved = Vec::new();
    let mut rebinds = Vec::new();
    for d in &cf.ovrs {
        let mut m = d.clone();
        m.applies = Some(match d.applies.as_deref() {
            None | Some("**") => format!("{rel}/**"),
            Some(a) => format!("{rel}/{a}"),
        });
        rebinds.extend(rebind(reg, scope, d, scope, &into));
        moved.push(m);
    }
    let reparented = reg
        .table
        .scopes
        .values()
        .filter(|c| c.parent.as_ref() == Some(scope))
        .map(|c| c.id.clone())
        .collect();
    Ok(DemotePlan {
        scope: scope.clone(),
        into: into.clone(),
        tombstone: Tombstone {
            id: scope.0.clone(),
            former_path: s.dir.clone(),
            written_in: into,
            date: as_of.0,
        },
        moved,
        items_moved,
        reparented,
        rebinds,
    })
}
