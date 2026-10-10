//! Environments, maturity and lifecycle (chapter 21): the root vocabulary, item axes and due dates.
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::directive::{ArgValue, Directive, Verb};
use fairlead_syntax::ids::Id;

use crate::config::VocabConfig;
use crate::docs::{ChangeDoc, Status};
use crate::extract::Corpus;
use crate::path::RepoPath;
use crate::registry::{ItemId, Lookup, Registry};
use crate::version::Version;

/// Theme variant words: values of a variation axis, never envs (R-ENV-037).
pub const THEME_WORDS: [&str; 6] = ["light", "dark", "brand", "theme", "high-contrast", "dim"];

/// The fixed lifecycle states (R-ENV-004).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum State {
    Proposed,
    Active,
    Deprecated,
    Removed,
    Rejected,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Proposed => "proposed",
            State::Active => "active",
            State::Deprecated => "deprecated",
            State::Removed => "removed",
            State::Rejected => "rejected",
        }
    }
    fn parse(w: &str) -> Option<State> {
        Some(match w {
            "proposed" => State::Proposed,
            "active" => State::Active,
            "deprecated" => State::Deprecated,
            "removed" => State::Removed,
            "rejected" => State::Rejected,
            _ => return None,
        })
    }
}

/// The root vocabulary, with aliases resolved (R-ENV-002, R-ENV-003).
#[derive(Clone, Copy, Debug)]
pub struct Vocab<'a> {
    pub cfg: &'a VocabConfig,
}

/// What a word means on the env axis.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EnvWord {
    /// The slot (index in `env.order`, 0 = least exposed).
    Slot(usize),
    /// `all`: every declared env.
    All,
    /// A theme variant word (R-ENV-037).
    Theme,
    Unknown,
}

impl<'a> Vocab<'a> {
    pub fn new(cfg: &'a VocabConfig) -> Self {
        Self { cfg }
    }
    /// True once the root declares an env path; until then the env axis is not checked.
    pub fn has_envs(&self) -> bool {
        !self.cfg.env_order.is_empty()
    }
    pub fn has_maturity(&self) -> bool {
        !self.cfg.maturity.is_empty()
    }
    pub fn env(&self, word: &str) -> EnvWord {
        let canon = self.cfg.env_alias.get(word).map_or(word, String::as_str);
        if let Some(i) = self.cfg.env_order.iter().position(|e| e == canon) {
            EnvWord::Slot(i)
        } else if word == "all" {
            EnvWord::All
        } else if THEME_WORDS.contains(&word) {
            EnvWord::Theme
        } else {
            EnvWord::Unknown
        }
    }
    pub fn env_name(&self, slot: usize) -> &str {
        self.cfg.env_order.get(slot).map_or("?", String::as_str)
    }
    pub fn all_envs(&self) -> BTreeSet<usize> {
        (0..self.cfg.env_order.len()).collect()
    }
    /// The canonical word of an env alias (`qa` -> `staging`), if `word` is an alias.
    pub fn canonical_env(&self, word: &str) -> Option<&str> {
        self.cfg.env_alias.get(word).map(String::as_str)
    }
    pub fn rank(&self, word: &str) -> Option<i64> {
        self.cfg.maturity.get(word).copied()
    }
    /// The lowest declared rank (R-ENV-006).
    pub fn lowest_rank(&self) -> Option<i64> {
        self.cfg.maturity.values().min().copied()
    }
    /// A maturity word for a rank (the first word in key order with that rank).
    pub fn rank_word(&self, rank: i64) -> String {
        self.cfg
            .maturity
            .iter()
            .find(|(_, r)| **r == rank)
            .map_or_else(|| rank.to_string(), |(w, _)| w.clone())
    }
    /// A lifecycle state word or its display alias (R-ENV-004).
    pub fn state(&self, word: &str) -> Option<State> {
        let canon = self
            .cfg
            .lifecycle_alias
            .get(word)
            .map_or(word, String::as_str);
        State::parse(canon)
    }
}

/// The resolved env and maturity axes of one item.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Axes {
    pub envs: BTreeSet<usize>,
    /// Rank per env slot; an env without an entry has no maturity.
    pub mat: BTreeMap<usize, i64>,
}

/// A word that resolved to nothing (H-VOC-UNKNOWN, R-ENV-003, R-ENV-037).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnknownWord {
    pub key: &'static str,
    pub word: String,
    pub theme: bool,
}

/// `in=` words to slots. Unknown and theme words are returned, never guessed.
pub fn parse_in(v: &Vocab<'_>, words: &[String], bad: &mut Vec<UnknownWord>) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for w in words {
        match v.env(w) {
            EnvWord::Slot(i) => {
                out.insert(i);
            }
            EnvWord::All => out.extend(v.all_envs()),
            EnvWord::Theme => bad.push(UnknownWord {
                key: "in",
                word: w.clone(),
                theme: true,
            }),
            EnvWord::Unknown => bad.push(UnknownWord {
                key: "in",
                word: w.clone(),
                theme: false,
            }),
        }
    }
    out
}

/// `mat=<word>` (every env of the item) or `mat=<env>:<word>,...` (per env, R-ENV-010).
pub fn parse_mat(
    v: &Vocab<'_>,
    words: &[String],
    envs: &BTreeSet<usize>,
    bad: &mut Vec<UnknownWord>,
) -> BTreeMap<usize, i64> {
    let mut out = BTreeMap::new();
    let unknown_mat = |w: &str, bad: &mut Vec<UnknownWord>| {
        bad.push(UnknownWord {
            key: "mat",
            word: w.to_string(),
            theme: false,
        });
    };
    for w in words {
        match w.split_once(':') {
            Some((env, m)) => {
                let slots: Vec<usize> = match v.env(env) {
                    EnvWord::Slot(i) => vec![i],
                    EnvWord::All => envs.iter().copied().collect(),
                    EnvWord::Theme | EnvWord::Unknown => {
                        bad.push(UnknownWord {
                            key: "mat",
                            word: env.to_string(),
                            theme: v.env(env) == EnvWord::Theme,
                        });
                        continue;
                    }
                };
                match v.rank(m) {
                    Some(r) => out.extend(slots.into_iter().map(|s| (s, r))),
                    None => unknown_mat(m, bad),
                }
            }
            None => match v.rank(w) {
                Some(r) => out.extend(envs.iter().map(|s| (*s, r))),
                None => unknown_mat(w, bad),
            },
        }
    }
    out
}

/// Envs ordered by exposure where a later env is MORE mature than an earlier one (R-ENV-011):
/// `(earlier, later)` slot pairs.
pub fn order_violations(axes: &Axes) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let pairs: Vec<(usize, i64)> = axes.mat.iter().map(|(s, r)| (*s, *r)).collect();
    for (i, (ea, ra)) in pairs.iter().enumerate() {
        for (eb, rb) in pairs.iter().skip(i + 1) {
            if rb > ra {
                out.push((*ea, *eb));
            }
        }
    }
    out
}

/// The lowest rank an item has in any env (what a dependent relies on, R-ENV-020).
pub fn floor_rank(axes: &Axes) -> Option<i64> {
    axes.mat.values().min().copied()
}

/// The plan words of `flag` (R-LC-001).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PlanWord {
    Deprecate,
    Replace,
    Edit,
    Remove,
}

impl PlanWord {
    pub fn parse(w: &str) -> Option<PlanWord> {
        Some(match w {
            "deprecate" => PlanWord::Deprecate,
            "replace" => PlanWord::Replace,
            "edit" => PlanWord::Edit,
            "remove" => PlanWord::Remove,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PlanWord::Deprecate => "deprecate",
            PlanWord::Replace => "replace",
            PlanWord::Edit => "edit",
            PlanWord::Remove => "remove",
        }
    }
    /// References to an item under this plan are deprecated references (R-LC-008).
    pub fn retires(self) -> bool {
        !matches!(self, PlanWord::Edit)
    }
}

/// A `target=` value: concrete, or symbolic until the seal that first contains it (R-LC-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Version(Version),
    NextMajor,
    NextMinor,
}

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        match s {
            "next-major" => Some(Target::NextMajor),
            "next-minor" => Some(Target::NextMinor),
            _ => Version::parse(s).map(Target::Version),
        }
    }
    /// The concrete version against `sealed` (the version sealed before the seal that records it;
    /// `start` when nothing is sealed yet).
    pub fn resolve(self, sealed: Option<Version>, start: Version) -> Version {
        let s = sealed.unwrap_or(start);
        match self {
            Target::Version(v) => v,
            Target::NextMajor => Version::new(s.major + 1, 0, 0),
            Target::NextMinor => Version::new(s.major, s.minor + 1, 0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DueState {
    NotDue,
    Approaching,
    Due,
}

/// R-LC-004, R-LC-005: due when `O >= T` (never `==` only) or the `by-date` has passed, whichever
/// fires first; approaching when `T` is within `warn_window` minors of `O` on the same major.
pub fn due(
    target: Option<Version>,
    by_date: Option<jiff::civil::Date>,
    open: Version,
    as_of: jiff::civil::Date,
    warn_window: u32,
) -> DueState {
    if by_date.is_some_and(|d| as_of >= d) || target.is_some_and(|t| open >= t) {
        return DueState::Due;
    }
    match target {
        Some(t) if t.major == open.major && t.minor - open.minor <= u64::from(warn_window) => {
            DueState::Approaching
        }
        Some(t) if t.major == open.major + 1 && t.minor == 0 && warn_window > 0 => {
            DueState::Approaching
        }
        _ => DueState::NotDue,
    }
}

/// R-LC-007: follow `replace ... to=` edges to the final ID. `Err` carries the cycle in order.
pub fn resolve_chain<T: Ord + Clone>(
    start: &T,
    next: &std::collections::BTreeMap<T, T>,
) -> Result<T, Vec<T>> {
    let mut seen: Vec<T> = vec![start.clone()];
    let mut cur = start.clone();
    while let Some(n) = next.get(&cur) {
        if let Some(i) = seen.iter().position(|s| s == n) {
            let mut cycle = seen.split_off(i);
            cycle.push(n.clone());
            return Err(cycle);
        }
        seen.push(n.clone());
        cur = n.clone();
    }
    Ok(cur)
}

/// One plan flag on a declared item (R-LC-001).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Plan {
    pub word: PlanWord,
    pub item: ItemId,
    /// The `to=` successor, resolved from the flag's scope.
    pub to: Option<ItemId>,
    pub target: Option<Target>,
    pub by_date: Option<jiff::civil::Date>,
    pub path: RepoPath,
    pub line: u32,
    pub col: u32,
}

fn arg_text(d: &Directive, k: &str) -> Option<String> {
    match &d.arg(k)?.value {
        ArgValue::Word(w) | ArgValue::Quoted(w) => Some(w.clone()),
        ArgValue::List(l) => Some(l.join(",")),
    }
}

/// Every plan flag of the tree whose ID resolves (an unresolved ID is `HN-R001`'s).
pub fn collect_plans(reg: &Registry, corpus: &Corpus) -> Vec<Plan> {
    let mut out = Vec::new();
    for (path, rec) in &corpus.records {
        let (Some(ex), Some(scope)) = (rec.extraction.as_ref(), reg.scope_of(path)) else {
            continue;
        };
        for d in ex.directives.iter().filter(|d| d.verb == Verb::Flag) {
            let Some(word) = d.bare.first().and_then(|w| PlanWord::parse(w)) else {
                continue;
            };
            let Some(id) = d.ids.first() else { continue };
            let Lookup::Found(item) = reg.lookup(scope, id) else {
                continue;
            };
            let to = arg_text(d, "to")
                .and_then(|t| Id::parse(&t).ok())
                .and_then(|t| match reg.lookup(scope, &t) {
                    Lookup::Found(i) => Some(i),
                    _ => None,
                });
            out.push(Plan {
                word,
                item,
                to,
                target: arg_text(d, "target").and_then(|t| Target::parse(&t)),
                by_date: arg_text(d, "by-date").and_then(|t| t.parse().ok()),
                path: path.clone(),
                line: d.span.line,
                col: d.span.col,
            });
        }
    }
    out
}

/// The `<scope>:<kind>.<name>` key used in ledger records and retarget docs.
pub fn item_key(i: &ItemId) -> String {
    format!(
        "{}:{}.{}",
        i.scope.0.as_str(),
        i.kind.as_str(),
        i.name.as_str()
    )
}

/// Ledger record of a plan, written by the seal that first contains it (R-LC-002, R-LC-003):
/// `plan <scope:kind.name> <word> [target=<v>] [to=<scope:kind.name>]`.
pub fn plan_record(p: &Plan, target: Option<Version>) -> String {
    let mut out = format!("plan {} {}", item_key(&p.item), p.word.as_str());
    if let Some(t) = target {
        out.push_str(&format!(" target={t}"));
    }
    if let Some(to) = &p.to {
        out.push_str(&format!(" to={}", item_key(to)));
    }
    out
}

/// A plan as an earlier seal recorded it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RecordedPlan {
    pub word: String,
    pub target: Option<Version>,
    pub to: Option<String>,
}

/// Plans recorded at earlier seals, by item key; the first record of a key wins.
pub fn recorded_plans<'a>(
    ledgers: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, RecordedPlan> {
    let mut out = BTreeMap::new();
    for text in ledgers {
        for line in text.lines() {
            let mut w = line.split_whitespace();
            if w.next() != Some("plan") {
                continue;
            }
            let (Some(key), Some(word)) = (w.next(), w.next()) else {
                continue;
            };
            let rest: Vec<&str> = w.collect();
            let kv = |k: &str| rest.iter().find_map(|x| x.strip_prefix(k));
            out.entry(key.to_string()).or_insert(RecordedPlan {
                word: word.to_string(),
                target: kv("target=").and_then(Version::parse),
                to: kv("to=").map(str::to_string),
            });
        }
    }
    out
}

/// Targets recorded at earlier seals, by item key (R-LC-003).
pub fn recorded_targets<'a>(
    ledgers: impl IntoIterator<Item = &'a str>,
) -> BTreeMap<String, Version> {
    recorded_plans(ledgers)
        .into_iter()
        .filter_map(|(k, r)| Some((k, r.target?)))
        .collect()
}

/// `<scope dir>/<folder>/tombstones/<year>.toml` (R-LC-020).
pub fn tomb_file(
    reg: &Registry,
    scope: &crate::registry::ScopeId,
    folder: &str,
    year: i16,
) -> Option<RepoPath> {
    let dir = reg.scope(scope)?.dir.as_str().to_owned();
    let rel = crate::generate::join_dir(&dir, &format!("{folder}/tombstones/{year}.toml"))?;
    Some(rel)
}

/// Recorded remove or replace plans whose item is declared nowhere any more and has no
/// tombstone: the removals the next seal records (R-LC-020). `removed` is left empty for the seal.
pub fn completed_removals(reg: &Registry, recorded: &BTreeMap<String, RecordedPlan>) -> Vec<Tomb> {
    let declared: std::collections::BTreeSet<String> = reg
        .items()
        .map(|i| format!("{}.{}", i.id.kind.as_str(), i.id.name.as_str()))
        .collect();
    let mut out = Vec::new();
    for (key, r) in recorded {
        if !matches!(r.word.as_str(), "remove" | "replace") {
            continue;
        }
        let bare = bare_of(key);
        let Some((scope, _)) = key.split_once(':') else {
            continue;
        };
        let Ok(scope) = fairlead_syntax::ids::ScopeSlug::parse(scope) else {
            continue;
        };
        let scope = crate::registry::ScopeId(scope);
        let Ok(id) = Id::parse(bare) else { continue };
        if declared.contains(bare)
            || reg.tomb_for(&scope, &id).is_some()
            || reg.scope(&scope).is_none()
        {
            continue;
        }
        out.push(Tomb {
            file: RepoPath::root(),
            scope: Some(scope),
            id: bare.to_string(),
            removed: String::new(),
            by: r.to.as_deref().map(|t| bare_of(t).to_string()),
            why: format!("{} plan completed", r.word),
            paths: Vec::new(),
            deleted_by: None,
        });
    }
    out
}

/// The approval fingerprint of a revive doc (R-LC-024): over its draft ID, the revived ID and
/// the new ID, so editing either needs a new approval.
pub fn revive_fp(draft: &str, revives: &str, new: &str) -> crate::hash::Fingerprint {
    crate::hash::Fingerprint::from_hash(&crate::hash::hash_raw(
        format!("revive {draft} {revives} {new}").as_bytes(),
    ))
}

/// A revived item's new ID is the old one with a version suffix `-v<N>`, N >= 2 (R-LC-024).
pub fn revive_id_ok(old: &str, new: &str) -> bool {
    new.strip_prefix(old)
        .and_then(|r| r.strip_prefix("-v"))
        .and_then(|d| {
            d.parse::<u32>()
                .ok()
                .filter(|n| *n >= 2)
                .map(|n| n.to_string() == d)
        })
        .unwrap_or(false)
}

/// What is wrong with a revive doc: `revives` must name a tombstoned ID, `as` the new versioned
/// ID, and an approval must exist for its fingerprint. Empty when valid.
pub fn revive_problems(
    d: &ChangeDoc,
    reg: &Registry,
    approvals: &crate::config::ApprovalsFile,
) -> Vec<String> {
    let mut out = Vec::new();
    let Some(old) = d.extra.get("revives") else {
        return vec!["a revive doc needs `revives = \"<tombstoned id>\"`".into()];
    };
    let scope = reg
        .scope_of(&d.path)
        .cloned()
        .unwrap_or_else(|| reg.root_scope().clone());
    let tombed = Id::parse(old)
        .ok()
        .is_some_and(|id| reg.tomb_for(&scope, &id).is_some());
    if !tombed {
        out.push(format!("revives = {old:?} names no tombstoned ID"));
    }
    let new = d.extra.get("as").map(String::as_str).unwrap_or("");
    if !revive_id_ok(old, new) {
        out.push(format!(
            "`as` must be a new ID {old}-v<N> (N >= 2), not {new:?}"
        ));
    }
    let fp = revive_fp(d.id.as_str(), old, new);
    if !approvals.approvals.iter().any(|a| a.fp == fp) {
        out.push(format!(
            "not approved: a human runs `fairlead approve {}` (fingerprint {})",
            d.id.as_str(),
            fp.as_str()
        ));
    }
    out
}

/// `kind.name` of a `scope:kind.name` key.
pub fn bare_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, b)| b)
}

/// IDs in the committed registry shards (`gen/reg/*.reg`) with no declaration left in any scope
/// and no tombstone: deleted since the last `sync` (R-CLI-046). `(scope, kind.name)`.
pub fn vanished(
    reg: &Registry,
    on_disk: &BTreeMap<RepoPath, std::sync::Arc<[u8]>>,
) -> Vec<(crate::registry::ScopeId, String)> {
    let declared: std::collections::BTreeSet<String> = reg
        .items()
        .map(|i| format!("{}.{}", i.id.kind.as_str(), i.id.name.as_str()))
        .collect();
    let mut out = Vec::new();
    for (p, bytes) in on_disk {
        if !p.as_str().ends_with(".reg") || !p.as_str().contains("/gen/reg/") {
            continue;
        }
        let Some(scope) = reg.scope_of(p) else {
            continue;
        };
        for line in String::from_utf8_lossy(bytes).lines() {
            let Ok(t) = crate::render::line::TerseLine::parse(line) else {
                continue;
            };
            let Ok(id) = Id::parse(&t.id) else { continue };
            if declared.contains(&t.id) || reg.tomb_for(scope, &id).is_some() {
                continue;
            }
            out.push((scope.clone(), t.id.clone()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// IDs named by a doc key whose value is a word or a TOML list kept as text.
pub fn doc_ids(value: &str) -> Vec<String> {
    if value.contains('"') {
        value
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    } else {
        vec![value.trim().to_string()]
    }
}

/// Sealed `retarget` docs: per item key, the latest new target and how many times it moved
/// (R-LC-006). A doc names its items in `retargets` and the new version in `target`.
pub fn retargets(docs: &[ChangeDoc], reg: &Registry) -> BTreeMap<String, (Version, u32)> {
    let mut sealed: Vec<&ChangeDoc> = docs
        .iter()
        .filter(|d| d.kind == "retarget" && d.status == Status::Sealed)
        .collect();
    sealed.sort_by_key(|d| (d.sealed, d.ordinal));
    let mut out: BTreeMap<String, (Version, u32)> = BTreeMap::new();
    for d in sealed {
        let Some(t) = d.extra.get("target").and_then(|t| Version::parse(t)) else {
            continue;
        };
        for raw in d
            .extra
            .get("retargets")
            .map(|v| doc_ids(v))
            .unwrap_or_default()
        {
            let key = Id::parse(&raw)
                .ok()
                .and_then(|id| match reg.lookup(reg.root_scope(), &id) {
                    Lookup::Found(i) => Some(item_key(&i)),
                    _ => None,
                })
                .unwrap_or(raw);
            let e = out.entry(key).or_insert((t, 0));
            *e = (t, e.1 + 1);
        }
    }
    out
}

/// The concrete target of a plan: a sealed retarget wins, then the written version, then the
/// version recorded at seal, then the symbolic target against the current sealed version.
pub fn effective_target(
    p: &Plan,
    recorded: &BTreeMap<String, Version>,
    moved: &BTreeMap<String, (Version, u32)>,
    sealed: Option<Version>,
    start: Version,
) -> Option<Version> {
    let key = item_key(&p.item);
    if let Some((v, _)) = moved.get(&key) {
        return Some(*v);
    }
    match p.target? {
        Target::Version(v) => Some(v),
        t => Some(
            recorded
                .get(&key)
                .copied()
                .unwrap_or_else(|| t.resolve(sealed, start)),
        ),
    }
}

/// One `[[tomb]]` of `<folder>/tombstones/<year>.toml` (R-LC-020): an item ID (`kind.name`) or,
/// for a demoted scope, a scope slug. Tombstones never expire (R-RET-041).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tomb {
    pub file: RepoPath,
    /// The scope whose managerial folder holds the file (set by the registry).
    pub scope: Option<crate::registry::ScopeId>,
    pub id: String,
    /// The version (lifecycle removal) or date (scope demotion, user deletion) of the removal.
    pub removed: String,
    /// The permanent `replaced-by` alias (R-LC-011).
    pub by: Option<String>,
    pub why: String,
    pub paths: Vec<String>,
    /// `sync` for an ID deleted outside the lifecycle flow (R-CLI-046).
    pub deleted_by: Option<String>,
}

impl Tomb {
    /// An item tombstone (`kind.name`), not a scope's.
    pub fn is_item(&self) -> bool {
        self.id.contains('.') && !self.id.contains(':')
    }
}

/// Parses a tombstone year file. `Err` names the first problem; the caller fails closed.
pub fn parse_tombs(file: &RepoPath, text: &str) -> Result<Vec<Tomb>, String> {
    let t: toml::Table = text
        .parse()
        .map_err(|e: toml::de::Error| e.message().to_string())?;
    let Some(arr) = t.get("tomb") else {
        return Ok(Vec::new());
    };
    let arr = arr.as_array().ok_or("`tomb` must be an array of tables")?;
    let mut out = Vec::new();
    for (i, v) in arr.iter().enumerate() {
        let tb = v.as_table().ok_or("every `tomb` entry must be a table")?;
        let s = |k: &str| tb.get(k).and_then(|x| x.as_str()).map(str::to_string);
        let removed = match tb.get("removed") {
            Some(toml::Value::String(x)) => x.clone(),
            Some(toml::Value::Datetime(d)) => d.to_string(),
            _ => return Err(format!("tomb {} has no `removed`", i + 1)),
        };
        out.push(Tomb {
            file: file.clone(),
            scope: None,
            id: s("id").ok_or_else(|| format!("tomb {} has no `id`", i + 1))?,
            removed,
            by: s("by").filter(|b| b.contains('.')),
            why: s("why").unwrap_or_default(),
            paths: tb
                .get("paths")
                .and_then(|p| p.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            deleted_by: s("deleted-by"),
        });
    }
    Ok(out)
}

/// The `[[tomb]]` text seal and sync append (R-LC-020, R-CLI-046).
pub fn tomb_text(t: &Tomb, at: Option<&str>) -> String {
    let q = |s: &str| format!("{s:?}");
    let mut out = format!(
        "\n[[tomb]]\nid = {}\nremoved = {}\n",
        q(&t.id),
        q(&t.removed)
    );
    if let Some(b) = &t.by {
        out.push_str(&format!("by = {}\n", q(b)));
    }
    out.push_str(&format!("why = {}\n", q(&t.why)));
    if !t.paths.is_empty() {
        let items: Vec<String> = t.paths.iter().map(|p| q(p)).collect();
        out.push_str(&format!("paths = [{}]\n", items.join(", ")));
    }
    if let Some(d) = &t.deleted_by {
        out.push_str(&format!("deleted-by = {}\n", q(d)));
    }
    if let Some(a) = at {
        out.push_str(&format!("at = {}\n", q(a)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VocabConfig {
        let mut c = VocabConfig {
            env_order: vec!["dev".into(), "staging".into(), "prod".into()],
            ..Default::default()
        };
        c.env_alias.insert("qa".into(), "staging".into());
        for (w, r) in [("alpha", 0), ("beta", 1), ("rc", 2), ("stable", 3)] {
            c.maturity.insert(w.into(), r);
        }
        c.lifecycle_alias
            .insert("sunsetting".into(), "deprecated".into());
        c
    }

    #[test]
    fn r_env_003__alias_valid_unknown_word_reported() {
        let c = cfg();
        let v = Vocab::new(&c);
        let mut bad = Vec::new();
        let envs = parse_in(&v, &["qa".into(), "canary".into()], &mut bad);
        assert_eq!(envs, BTreeSet::from([1]), "qa is staging forever");
        assert_eq!(bad.len(), 1);
        assert_eq!((bad[0].word.as_str(), bad[0].theme), ("canary", false));
        assert_eq!(v.state("sunsetting"), Some(State::Deprecated));
        assert_eq!(v.state("archived"), None);
    }

    #[test]
    fn r_env_037__theme_word_is_not_an_env() {
        let c = cfg();
        let mut bad = Vec::new();
        parse_in(&Vocab::new(&c), &["dark".into()], &mut bad);
        assert!(bad[0].theme);
    }

    #[test]
    fn r_env_010__mat_applies_to_every_env_or_per_env() {
        let c = cfg();
        let v = Vocab::new(&c);
        let mut bad = Vec::new();
        let envs = parse_in(&v, &["all".into()], &mut bad);
        assert_eq!(parse_mat(&v, &["beta".into()], &envs, &mut bad).len(), 3);
        let per = parse_mat(
            &v,
            &["dev:beta".into(), "staging:alpha".into()],
            &envs,
            &mut bad,
        );
        assert_eq!(per, BTreeMap::from([(0, 1), (1, 0)]));
        assert!(bad.is_empty());
    }

    #[test]
    fn r_env_011__later_env_more_mature_is_an_order_violation() {
        let ok = Axes {
            envs: BTreeSet::from([0, 2]),
            mat: BTreeMap::from([(0, 3), (2, 1)]),
        };
        let bad = Axes {
            envs: BTreeSet::from([0, 2]),
            mat: BTreeMap::from([(0, 1), (2, 3)]),
        };
        assert!(order_violations(&ok).is_empty());
        assert_eq!(order_violations(&bad), [(0, 2)]);
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn d(s: &str) -> jiff::civil::Date {
        s.parse().unwrap()
    }

    #[test]
    fn r_lc_004__due_when_open_reaches_target_never_equal_only() {
        let today = d("2026-10-10");
        let t = Some(v("2.0.0"));
        assert_eq!(due(t, None, v("2.0.0"), today, 1), DueState::Due);
        assert_eq!(
            due(t, None, v("2.1.0"), today, 1),
            DueState::Due,
            "a bump jumped over"
        );
        assert_eq!(due(t, None, v("1.9.0"), today, 1), DueState::Approaching);
        assert_eq!(
            due(Some(v("1.6.0")), None, v("1.4.0"), today, 1),
            DueState::NotDue
        );
        assert_eq!(
            due(Some(v("1.5.0")), None, v("1.4.0"), today, 1),
            DueState::Approaching
        );
    }

    #[test]
    fn r_lc_005__first_of_date_and_version_wins() {
        let t = Some(v("3.0.0"));
        assert_eq!(
            due(t, Some(d("2027-03-01")), v("1.0.0"), d("2027-03-01"), 1),
            DueState::Due
        );
        assert_eq!(
            due(t, Some(d("2027-03-01")), v("1.0.0"), d("2027-02-28"), 1),
            DueState::NotDue
        );
        assert_eq!(
            due(
                Some(v("1.0.0")),
                Some(d("2099-01-01")),
                v("1.0.0"),
                d("2026-01-01"),
                1
            ),
            DueState::Due
        );
    }

    #[test]
    fn r_lc_003__symbolic_target_resolves_against_the_sealed_version() {
        let start = v("0.1.0");
        assert_eq!(
            Target::parse("next-major")
                .unwrap()
                .resolve(Some(v("1.4.2")), start),
            v("2.0.0")
        );
        assert_eq!(
            Target::parse("next-minor")
                .unwrap()
                .resolve(Some(v("1.4.2")), start),
            v("1.5.0")
        );
        assert_eq!(
            Target::parse("next-minor").unwrap().resolve(None, start),
            v("0.2.0")
        );
        assert_eq!(Target::parse("1.6"), None, "two parts is not a version");
    }

    #[test]
    fn r_lc_007__chain_resolves_cycle_errors() {
        let next: BTreeMap<&str, &str> = BTreeMap::from([("a", "b"), ("b", "c")]);
        assert_eq!(resolve_chain(&"a", &next), Ok("c"));
        let cyc: BTreeMap<&str, &str> = BTreeMap::from([("a", "b"), ("b", "a")]);
        assert_eq!(resolve_chain(&"a", &cyc), Err(vec!["a", "b", "a"]));
    }
}
