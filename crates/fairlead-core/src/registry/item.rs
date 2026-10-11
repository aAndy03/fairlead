//! Items, sites, edges, lookups (R-SCOPE-023..026, R-ARCH-062).
use std::collections::BTreeMap;

use fairlead_syntax::diag::Span;
use fairlead_syntax::directive::Extent;
use fairlead_syntax::ids::{Id, IdQualifier, Kind, Name};

use crate::config::Policy;
use crate::path::RepoPath;
use crate::registry::scope::ScopeId;
use std::collections::BTreeSet;

use fairlead_syntax::diag::Severity;
use fairlead_syntax::directive::{Arg, ArgOp, ArgValue, Carrier, Directive, Verb, known_keys};

use crate::config::{ControlFile, KindDecl, RuleCheckDecl, Stage};
use crate::diag::{Diagnostic, Location, codes};
use crate::extract::Corpus;
use crate::registry::Dg;
use crate::registry::Registry;
use crate::registry::scope::{Scope, ScopeState, ScopeTable, is_within};

/// Fully qualified item identity (home scope + kind + name). `Id` is what was WRITTEN.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ItemId {
    pub scope: ScopeId,
    pub kind: Kind,
    pub name: Name,
}

impl ItemId {
    /// Qualified form `scope:kind.name`.
    pub fn to_id(&self) -> Id {
        Id {
            qualifier: IdQualifier::Scope(self.scope.0.clone()),
            kind: self.kind.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Origin {
    ControlTable { control: RepoPath },
    DocDef { path: RepoPath, span: Span },
}

/// A code `def` site (R-SYN-010).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Site {
    pub path: RepoPath,
    pub span: Span,
    pub extent: Extent,
    pub scope: ScopeId,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FieldValue {
    Scalar(String),
    List(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Item {
    pub id: ItemId,
    pub origin: Origin,
    pub policy: Policy,
    pub sum: Option<String>,
    pub fields: BTreeMap<String, FieldValue>,
}

/// `use` edge (R-SCOPE-060).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Edge {
    pub from: Site,
    pub to: Id,
    pub sym: Option<String>,
    pub resolved: Option<ItemId>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Lookup {
    Found(ItemId),
    Ambiguous(Vec<ItemId>),
    Missing { nearest: Vec<Id> },
}

pub const BUILTIN_KINDS: [&str; 4] = ["rule", "handoff", "foreign", "file"];

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev.get(j).copied().unwrap_or(0) + usize::from(ca != *cb);
            let del = prev.get(j + 1).copied().unwrap_or(0) + 1;
            let ins = cur.get(j).copied().unwrap_or(0) + 1;
            cur.push(sub.min(del).min(ins));
        }
        prev = cur;
    }
    prev.last().copied().unwrap_or(0)
}

/// `*` matches any run of characters (including `.`).
pub(crate) fn wild_match(pat: &str, text: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == text;
    }
    let first = parts.first().copied().unwrap_or("");
    let last = parts.last().copied().unwrap_or("");
    if !text.starts_with(first) {
        return false;
    }
    let mut rest = text.get(first.len()..).unwrap_or("");
    let mid_end = parts.len().saturating_sub(1);
    for p in parts.get(1..mid_end).unwrap_or(&[]) {
        match rest.find(p) {
            Some(i) => rest = rest.get(i + p.len()..).unwrap_or(""),
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// P4-3: `rules.x` in a pattern or target means `rule.x`.
pub(crate) fn normalise_rules_alias(s: &str) -> String {
    match s.strip_prefix("rules.") {
        Some(r) => format!("rule.{r}"),
        None => s.to_string(),
    }
}

// --- kinds ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub(crate) struct KindTable {
    pub(crate) own: BTreeMap<ScopeId, BTreeMap<String, KindDecl>>,
}

impl KindTable {
    pub(crate) fn build(table: &ScopeTable, diags: &mut Vec<Diagnostic>) -> KindTable {
        let mut kt = KindTable::default();
        for scope in table.in_depth_order() {
            let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else {
                continue;
            };
            let inherited: BTreeSet<String> = table
                .chain_ids(&scope.id)
                .into_iter()
                .filter(|s| **s != scope.id)
                .filter_map(|s| kt.own.get(s))
                .flat_map(|m| m.keys().cloned())
                .collect();
            let mut mine = BTreeMap::new();
            for (name, decl) in &cf.kinds {
                if name == "exc" {
                    continue; // rejected at load by P3 (C-6)
                }
                if inherited.contains(name) {
                    diags.push(
                        Dg::new(codes::HN_C003, Severity::Error, &cf.path, 1, 1)
                            .scope(&scope.id)
                            .msg(format!("kind \"{name}\" is inherited and sealed; a child scope cannot redefine it"))
                            .hint("pick a new kind name, or escalate with `fairlead new escalation`")
                            .text(name.as_str())
                            .done(),
                    );
                    continue;
                }
                mine.insert(name.clone(), decl.clone());
            }
            kt.own.insert(scope.id.clone(), mine);
        }
        kt
    }

    fn visible<'a>(
        &'a self,
        table: &'a ScopeTable,
        scope: &ScopeId,
    ) -> impl Iterator<Item = (&'a String, &'a KindDecl)> {
        table
            .chain_ids(scope)
            .into_iter()
            .filter_map(|s| self.own.get(s))
            .flat_map(|m| m.iter())
    }

    pub(crate) fn declared_names(&self, table: &ScopeTable, scope: &ScopeId) -> Vec<String> {
        let mut v: BTreeSet<String> = BUILTIN_KINDS.iter().map(|s| s.to_string()).collect();
        v.extend(self.visible(table, scope).map(|(k, _)| k.clone()));
        v.into_iter().collect()
    }

    pub(crate) fn decl<'a>(
        &'a self,
        table: &'a ScopeTable,
        scope: &ScopeId,
        kind: &str,
    ) -> Option<&'a KindDecl> {
        self.visible(table, scope)
            .find(|(k, _)| k.as_str() == kind)
            .map(|(_, d)| d)
    }

    /// Kinds are visible along the chain of the referencing scope, and of the named scope for `q:kind.n`.
    pub(crate) fn is_declared(&self, table: &ScopeTable, from: &ScopeId, id: &Id) -> bool {
        let k = id.kind.as_str();
        if BUILTIN_KINDS.contains(&k) {
            return true;
        }
        let mut scopes = vec![from.clone()];
        if let IdQualifier::Scope(q) = &id.qualifier {
            scopes.push(ScopeId(q.clone()));
        }
        scopes.iter().any(|s| self.decl(table, s, k).is_some())
    }
}

// --- declarations -----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Cand {
    /// 0.3 (D-40): declared by a `def` in code; a doc or control declaration of the ID wins.
    pub(crate) from_code: bool,
    pub(crate) kind: Kind,
    pub(crate) name: Name,
    pub(crate) origin: Origin,
    pub(crate) policy: Policy,
    pub(crate) sum: Option<String>,
    pub(crate) fields: BTreeMap<String, FieldValue>,
    pub(crate) loc: Location,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Decls {
    pub(crate) items: BTreeMap<ItemId, Item>,
    pub(crate) locs: BTreeMap<ItemId, Location>,
}

fn sev_text(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn stage_text(s: Stage) -> &'static str {
    match s {
        Stage::Observe => "observe",
        Stage::Warn => "warn",
        Stage::Enforce => "enforce",
    }
}

/// Exact root pattern, then `waivable`, then wildcard pattern, then the kind default (R-SCOPE-030, R-CHK-006).
pub(crate) fn policy_for(
    root: Option<&ControlFile>,
    kind: &str,
    name: &str,
    explicit: Option<Policy>,
    waivable: bool,
) -> Policy {
    if let Some(p) = explicit {
        return p;
    }
    let key = format!("{kind}.{name}");
    let mut hits: Vec<(bool, usize, String, Policy)> = Vec::new();
    if let Some(rc) = root {
        for (pat, pol) in &rc.policies.by_pattern {
            let p = normalise_rules_alias(pat);
            if wild_match(&p, &key) {
                let spec = p.chars().filter(|c| *c != '*').count();
                hits.push((!p.contains('*'), spec, p, *pol));
            }
        }
    }
    hits.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)).then_with(|| a.2.cmp(&b.2)));
    match hits.first() {
        Some((true, _, _, p)) => *p,
        top => {
            if waivable {
                Policy::NeedsException
            } else if let Some((_, _, _, p)) = top {
                *p
            } else if kind == "rule" {
                Policy::Sealed
            } else {
                Policy::NeedsException
            }
        }
    }
}

fn control_cands(
    scope: &Scope,
    cf: &ControlFile,
    root: Option<&ControlFile>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Cand> {
    let loc = Location {
        path: cf.path.clone(),
        line: 0,
        col: 0,
    };
    let mut out = Vec::new();
    for (name, r) in &cf.rules {
        let Ok(id) = Id::parse(&format!("rule.{name}")) else {
            diags.push(bad_decl(
                scope,
                cf,
                &format!("rule name \"{name}\" is not a valid ID name"),
            ));
            continue;
        };
        let mut fields = BTreeMap::new();
        fields.insert(
            "sev".to_string(),
            FieldValue::Scalar(r.sev.map(sev_text).unwrap_or("error").to_string()),
        );
        fields.insert(
            "stage".to_string(),
            FieldValue::Scalar(r.stage.map(stage_text).unwrap_or("observe").to_string()),
        );
        fields.insert("glob".to_string(), FieldValue::List(r.glob.clone()));
        fields.insert("exclude".to_string(), FieldValue::List(r.exclude.clone()));
        // What `check` needs to evaluate the rule (P5a-T11); `ovr` may touch only sev/stage/glob/exclude.
        match &r.check {
            RuleCheckDecl::Leak { tokens } => {
                fields.insert("check".to_string(), FieldValue::Scalar("leak".to_string()));
                fields.insert("tokens".to_string(), FieldValue::List(tokens.clone()));
            }
            RuleCheckDecl::Pattern { forbid, msg } => {
                fields.insert(
                    "check".to_string(),
                    FieldValue::Scalar("pattern".to_string()),
                );
                fields.insert("forbid".to_string(), FieldValue::Scalar(forbid.clone()));
                fields.insert("msg".to_string(), FieldValue::Scalar(msg.clone()));
            }
            RuleCheckDecl::Builtin => {
                fields.insert(
                    "check".to_string(),
                    FieldValue::Scalar("builtin".to_string()),
                );
            }
        }
        fields.insert(
            "waivable".to_string(),
            FieldValue::Scalar(r.waivable.to_string()),
        );
        if let Some(why) = &r.why {
            fields.insert("why".to_string(), FieldValue::Scalar(why.clone()));
        }
        if let Some(since) = r.since {
            fields.insert("since".to_string(), FieldValue::Scalar(since.to_string()));
        }
        out.push(Cand {
            from_code: false,
            policy: policy_for(root, "rule", id.name.as_str(), None, r.waivable),
            kind: id.kind,
            name: id.name,
            origin: Origin::ControlTable {
                control: cf.path.clone(),
            },
            sum: r.why.clone(),
            fields,
            loc: loc.clone(),
        });
    }
    for f in &cf.files {
        let id = match Id::parse(&f.id) {
            Ok(id) if matches!(id.qualifier, IdQualifier::Bare) => id,
            _ => {
                diags.push(bad_decl(
                    scope,
                    cf,
                    &format!("file-item id \"{}\" must be a bare kind.name", f.id),
                ));
                continue;
            }
        };
        let mut fields = BTreeMap::new();
        fields.insert("kind".to_string(), FieldValue::Scalar(f.kind.clone()));
        fields.insert("path".to_string(), FieldValue::Scalar(f.path.clone()));
        if let Some(e) = &f.extends {
            fields.insert("extends".to_string(), FieldValue::Scalar(e.clone()));
        }
        out.push(Cand {
            from_code: false,
            policy: policy_for(root, id.kind.as_str(), id.name.as_str(), None, false),
            kind: id.kind,
            name: id.name,
            origin: Origin::ControlTable {
                control: cf.path.clone(),
            },
            sum: None,
            fields,
            loc: loc.clone(),
        });
    }
    out
}

fn bad_decl(scope: &Scope, cf: &ControlFile, msg: &str) -> Diagnostic {
    Dg::new(codes::HN_C001, Severity::Error, &cf.path, 1, 1)
        .scope(&scope.id)
        .msg(msg)
        .hint("fix the declaration in this control file")
        .done()
}

fn insert_cands(
    decls: &mut Decls,
    table: &ScopeTable,
    scope: &Scope,
    mut cands: Vec<Cand>,
    diags: &mut Vec<Diagnostic>,
) {
    cands.sort_by(|a, b| {
        (
            a.kind.as_str(),
            a.name.as_str(),
            a.loc.path.as_str(),
            a.loc.line,
        )
            .cmp(&(
                b.kind.as_str(),
                b.name.as_str(),
                b.loc.path.as_str(),
                b.loc.line,
            ))
    });
    for c in cands {
        let id = ItemId {
            scope: scope.id.clone(),
            kind: c.kind.clone(),
            name: c.name.clone(),
        };
        let text = format!("{}.{}", c.kind.as_str(), c.name.as_str());
        if decls.items.contains_key(&id) {
            diags.push(
                Dg::new(
                    codes::HN_D001,
                    Severity::Error,
                    &c.loc.path,
                    c.loc.line,
                    c.loc.col,
                )
                .scope(&scope.id)
                .msg(format!(
                    "`{text}` is declared twice in scope \"{}\"",
                    scope.id.0.as_str()
                ))
                .hint("keep one declaration or rename the other")
                .text(text)
                .done(),
            );
            continue;
        }
        let shadowed = table
            .chain_ids(&scope.id)
            .into_iter()
            .filter(|s| **s != scope.id)
            .map(|s| ItemId {
                scope: s.clone(),
                kind: c.kind.clone(),
                name: c.name.clone(),
            })
            .find(|anc| decls.items.contains_key(anc));
        if let Some(anc) = shadowed {
            diags.push(
                Dg::new(
                    codes::HN_R005,
                    Severity::Error,
                    &c.loc.path,
                    c.loc.line,
                    c.loc.col,
                )
                .scope(&scope.id)
                .msg(format!(
                    "`{text}` redefines an ID inherited from scope \"{}\"",
                    anc.scope.0.as_str()
                ))
                .related(anc.to_id().to_string())
                .hint("convert it to an `ovr` of the inherited item, or rename it")
                .text(text)
                .done(),
            );
            continue;
        }
        decls.locs.insert(id.clone(), c.loc.clone());
        decls.items.insert(
            id.clone(),
            Item {
                id,
                origin: c.origin,
                policy: c.policy,
                sum: c.sum,
                fields: c.fields,
            },
        );
    }
}

pub(crate) fn collect_items(
    table: &ScopeTable,
    mut docs: BTreeMap<ScopeId, Vec<Cand>>,
    diags: &mut Vec<Diagnostic>,
) -> Decls {
    let mut decls = Decls::default();
    let root = table.root_control();
    for scope in table.in_depth_order() {
        let (Some(cf), ScopeState::Resolved) = (scope.control.as_ref(), &scope.state) else {
            continue;
        };
        let mut cands = control_cands(scope, cf, root, diags);
        cands.extend(docs.remove(&scope.id).unwrap_or_default());
        insert_cands(&mut decls, table, scope, cands, diags);
    }
    decls
}

pub(crate) fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Def => "def",
        Verb::Use => "use",
        Verb::Ovr => "ovr",
        Verb::Flag => "flag",
        Verb::Env => "env",
        Verb::End => "end",
    }
}

pub(crate) fn arg_scalar(v: &ArgValue) -> String {
    match v {
        ArgValue::Word(s) | ArgValue::Quoted(s) => s.clone(),
        ArgValue::List(l) => l.join(","),
    }
}

pub(crate) fn arg_items(v: &ArgValue) -> Vec<String> {
    match v {
        ArgValue::Word(s) | ArgValue::Quoted(s) => vec![s.clone()],
        ArgValue::List(l) => l.clone(),
    }
}

/// Keys whose value is a list even when one word is written (R-SYN-015: `kw`).
const LIST_KEYS: [&str; 1] = ["kw"];

fn arg_field(a: &Arg) -> FieldValue {
    match (&a.op, &a.value) {
        (ArgOp::Append, v) => FieldValue::List(arg_items(v)),
        (ArgOp::Set, ArgValue::List(l)) => FieldValue::List(l.clone()),
        (ArgOp::Set, v) if LIST_KEYS.contains(&a.key.as_str()) => FieldValue::List(arg_items(v)),
        (ArgOp::Set, v) => FieldValue::Scalar(arg_scalar(v)),
    }
}

pub(crate) fn undeclared_kind(
    kinds: &KindTable,
    table: &ScopeTable,
    from: &ScopeId,
    id: &Id,
    path: &RepoPath,
    span: Span,
    text: &str,
) -> Diagnostic {
    let names = kinds.declared_names(table, from);
    let written = id.kind.as_str();
    let dym = names
        .iter()
        .min_by_key(|n| (levenshtein(n, written), n.as_str()))
        .map(|n| format!("{n}.{}", id.name.as_str()));
    let mut dg = Dg::new(codes::HN_S002, Severity::Error, path, span.line, span.col)
        .scope(from)
        .msg(format!("unknown kind \"{written}\" in \"{id}\""))
        .expected(format!("declared kinds: {}", names.join(" ")))
        .hint("declare the kind under [kinds.<k>] in a control file, or fix the ID; directive dropped")
        .text(text);
    if let Some(d) = dym {
        dg = dg.dym(d);
    }
    dg.done()
}

/// HN-S011 (C-5): keys known to the verb, `x-` extension keys and the kind's own keys are fine.
/// Keys a built-in kind accepts besides the verb's own: `handoff` carries the once-only `created`
/// stamp that `new` writes (A-41, C-33).
pub(crate) fn builtin_kind_keys(kind: &str) -> &'static [&'static str] {
    match kind {
        "handoff" => &["created"],
        _ => &[],
    }
}

pub(crate) fn check_keys(
    d: &Directive,
    scope: &ScopeId,
    table: &ScopeTable,
    kinds: &KindTable,
    path: &RepoPath,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    let kind_keys = d
        .ids
        .first()
        .filter(|_| d.verb == Verb::Def)
        .and_then(|id| kinds.decl(table, scope, id.kind.as_str()));
    let mut keep = true;
    for a in &d.args {
        let builtin = d
            .ids
            .first()
            .filter(|_| d.verb == Verb::Def)
            .is_some_and(|id| builtin_kind_keys(id.kind.as_str()).contains(&a.key.as_str()));
        let known = known_keys(d.verb).contains(&a.key.as_str())
            || builtin
            || a.key.starts_with("x-")
            || kind_keys.is_some_and(|k| k.keys.contains_key(&a.key));
        if known {
            continue;
        }
        let sev = if d.must_understand {
            Severity::Error
        } else {
            Severity::Warn
        };
        keep &= !d.must_understand;
        diags.push(
            Dg::new(codes::HN_S011, sev, path, a.span.line, a.span.col)
                .scope(scope)
                .msg(format!(
                    "unknown key \"{}\" on `{}`",
                    a.key,
                    verb_name(d.verb)
                ))
                .hint(
                    "remove it, rename it with an x- prefix, or declare it under [kinds.<k>.keys]",
                )
                .text(a.key.as_str())
                .done(),
        );
    }
    keep
}

/// P4-1: Markdown, frontmatter and managerial-folder files declare; everything else is a site.
pub(crate) fn is_decl_site(path: &RepoPath, scope: &Scope, folder: &str, d: &Directive) -> bool {
    if matches!(d.carrier, Carrier::Frontmatter) || path.extension() == "md" {
        return true;
    }
    let base = scope.dir.as_str().trim_matches('/');
    let mdir = if base.is_empty() {
        folder.to_string()
    } else {
        format!("{base}/{folder}")
    };
    is_within(path.as_str(), &mdir)
}

fn parse_policy(s: &str) -> Option<Policy> {
    match s {
        "open" => Some(Policy::Open),
        "needs-exception" => Some(Policy::NeedsException),
        "sealed" => Some(Policy::Sealed),
        _ => None,
    }
}

/// 0.3 (D-40, 30-hierarchy "Declarations"): a `def` in code declares its item, rather than only
/// marking an implementation site, when its kind is code-shaped or lists `layers` (a unit kind), or
/// when it carries a containment key (`layer`, `container`, `parent`, `path`). Otherwise a code
/// `def` stays an implementation site of an item declared in a doc (R-SYN, 0.1).
fn code_declares(kinds: &KindTable, table: &ScopeTable, scope: &ScopeId, d: &Directive) -> bool {
    let Some(id) = d.ids.first() else {
        return false;
    };
    if matches!(id.kind.as_str(), "foreign" | "rule" | "file" | "handoff") {
        return false;
    }
    // A bridge end marker (`def brg.x side=ts|rs`, R-HIER-062) marks a site of the bridge.
    if d.args.iter().any(|a| a.key == "side") {
        return false;
    }
    let by_kind = kinds
        .decl(table, scope, id.kind.as_str())
        .is_some_and(|k| matches!(k.shape, crate::config::KindShape::Code) || !k.layers.is_empty());
    by_kind
        || d.args
            .iter()
            .any(|a| matches!(a.key.as_str(), "layer" | "container" | "parent" | "path"))
}

/// A doc declaration visible from the scope (its own or an ancestor's) wins over a code one, and
/// the first code `def` of an ID (by path and line) declares it; the others stay its sites.
fn prefer_doc_decls(cands: &mut Vec<Cand>, docs: &BTreeSet<(String, String)>) {
    cands.sort_by(|a, b| (a.loc.path.as_str(), a.loc.line).cmp(&(b.loc.path.as_str(), b.loc.line)));
    let mut seen = BTreeSet::new();
    cands.retain(|c| {
        let key = (c.kind.as_str().to_owned(), c.name.as_str().to_owned());
        !c.from_code || (!docs.contains(&key) && seen.insert(key))
    });
}

pub(crate) fn doc_cands(
    table: &ScopeTable,
    kinds: &KindTable,
    folder: &str,
    corpus: &Corpus,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<ScopeId, Vec<Cand>> {
    let root = table.root_control();
    let mut out: BTreeMap<ScopeId, Vec<Cand>> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        let Some(sid) = table.scope_of(path) else {
            continue;
        };
        let Some(scope) = table.get(sid).filter(|s| s.state == ScopeState::Resolved) else {
            continue;
        };
        let Some(ex) = rec.extraction.as_ref() else {
            continue;
        };
        // R-VER-060: a `def` that exists only in a change doc that is not sealed (draft, ready
        // or abandoned) declares nothing yet; references to it dangle until the doc is sealed.
        let unsealed_change_doc = ex
            .frontmatter
            .as_ref()
            .is_some_and(|fm| crate::docs::is_unsealed_change_doc(&fm.table));
        if unsealed_change_doc {
            continue;
        }
        for d in &ex.directives {
            if d.verb != Verb::Def {
                continue;
            }
            let from_code = !is_decl_site(path, scope, folder, d);
            if from_code && !code_declares(kinds, table, sid, d) {
                continue;
            }
            let Some(id) = d.ids.first() else { continue };
            if id.kind.as_str() == "foreign" {
                continue; // region marker (P4-1)
            }
            let text = fairlead_syntax::render_directive_body(d);
            let qualifier_ok = match &id.qualifier {
                IdQualifier::Bare => true,
                IdQualifier::Scope(q) => *q == scope.id.0,
                IdQualifier::Root => scope.id == table.root,
            };
            if !qualifier_ok {
                diags.push(
                    Dg::new(codes::HN_C013, Severity::Error, path, d.span.line, d.span.col)
                        .scope(&scope.id)
                        .msg(format!("`def {id}` names another scope; an item is declared in the scope that holds the doc"))
                        .hint("drop the qualifier or move the declaration to that scope's docs")
                        .text(text)
                        .done(),
                );
                continue;
            }
            if !kinds.is_declared(table, sid, id) {
                diags.push(undeclared_kind(kinds, table, sid, id, path, d.span, &text));
                continue;
            }
            if !check_keys(d, sid, table, kinds, path, diags) {
                continue;
            }
            let mut explicit = None;
            let mut sum = None;
            let mut fields = BTreeMap::new();
            for a in &d.args {
                match a.key.as_str() {
                    "policy" => match parse_policy(&arg_scalar(&a.value)) {
                        Some(p) => explicit = Some(p),
                        None => diags.push(
                            Dg::new(
                                codes::HN_S011,
                                Severity::Warn,
                                path,
                                a.span.line,
                                a.span.col,
                            )
                            .scope(sid)
                            .msg("policy must be open, needs-exception or sealed")
                            .hint("fix the value; the kind default applies meanwhile")
                            .text(arg_scalar(&a.value))
                            .done(),
                        ),
                    },
                    "sum" => sum = Some(arg_scalar(&a.value)),
                    k if k.starts_with("x-") => {}
                    k => {
                        fields.insert(k.to_string(), arg_field(a));
                    }
                }
            }
            out.entry(scope.id.clone()).or_default().push(Cand {
                from_code,
                policy: policy_for(root, id.kind.as_str(), id.name.as_str(), explicit, false),
                kind: id.kind.clone(),
                name: id.name.clone(),
                origin: Origin::DocDef {
                    path: path.clone(),
                    span: d.span,
                },
                sum,
                fields,
                loc: Location {
                    path: path.clone(),
                    line: d.span.line,
                    col: d.span.col,
                },
            });
        }
    }
    let own_docs: BTreeMap<ScopeId, BTreeSet<(String, String)>> = out
        .iter()
        .map(|(sid, cands)| {
            let set = cands
                .iter()
                .filter(|c| !c.from_code)
                .map(|c| (c.kind.as_str().to_owned(), c.name.as_str().to_owned()))
                .collect();
            (sid.clone(), set)
        })
        .collect();
    for (sid, cands) in out.iter_mut() {
        let visible: BTreeSet<(String, String)> = table
            .chain_ids(sid)
            .into_iter()
            .filter_map(|s| own_docs.get(s))
            .flatten()
            .cloned()
            .collect();
        prefer_doc_decls(cands, &visible);
    }
    out
}

use crate::registry::ovr::FLOOR_RULES;

/// Borrowed read view used by `resolve` (before the `Registry` exists) and by `Registry` accessors.
pub(crate) struct Index<'a> {
    pub(crate) table: &'a ScopeTable,
    pub(crate) kinds: &'a KindTable,
    pub(crate) items: &'a BTreeMap<ItemId, Item>,
}

impl Index<'_> {
    fn item_in(&self, scope: &ScopeId, r: &Id) -> Option<ItemId> {
        let id = ItemId {
            scope: scope.clone(),
            kind: r.kind.clone(),
            name: r.name.clone(),
        };
        self.items.contains_key(&id).then_some(id)
    }

    fn floor_item(&self, r: &Id) -> Option<ItemId> {
        (r.kind.as_str() == "rule" && FLOOR_RULES.contains(&r.name.as_str())).then(|| ItemId {
            scope: self.table.root.clone(),
            kind: r.kind.clone(),
            name: r.name.clone(),
        })
    }

    /// R-SYN-025, R-ARCH-062.
    pub(crate) fn lookup(&self, from: &ScopeId, r: &Id) -> Lookup {
        let direct = |s: &ScopeId| match self.item_in(s, r).or_else(|| self.floor_item(r)) {
            Some(i) => Lookup::Found(i),
            None => Lookup::Missing {
                nearest: self.nearest(from, r),
            },
        };
        match &r.qualifier {
            IdQualifier::Root => direct(&self.table.root),
            IdQualifier::Scope(q) => direct(&ScopeId(q.clone())),
            IdQualifier::Bare => {
                for s in self.table.chain_ids(from).into_iter().rev() {
                    if let Some(i) = self.item_in(s, r) {
                        return Lookup::Found(i);
                    }
                }
                if let Some(i) = self.floor_item(r) {
                    return Lookup::Found(i);
                }
                let others: Vec<ItemId> = self
                    .items
                    .keys()
                    .filter(|k| k.kind == r.kind && k.name == r.name)
                    .cloned()
                    .collect();
                if others.len() >= 2 {
                    return Lookup::Ambiguous(others);
                }
                Lookup::Missing {
                    nearest: self.nearest(from, r),
                }
            }
        }
    }

    /// Up to three IDs: the same ID declared elsewhere (qualified), the same name under another kind,
    /// then the smallest edit distances.
    fn nearest(&self, from: &ScopeId, r: &Id) -> Vec<Id> {
        let want = format!("{}.{}", r.kind.as_str(), r.name.as_str());
        let chain = self.table.chain_ids(from);
        let mut c: Vec<(u8, usize, String, Id)> = Vec::new();
        for item in self.items.keys() {
            let on_chain = chain.contains(&&item.scope);
            let text = format!("{}.{}", item.kind.as_str(), item.name.as_str());
            let id = if on_chain {
                Id {
                    qualifier: IdQualifier::Bare,
                    kind: item.kind.clone(),
                    name: item.name.clone(),
                }
            } else {
                item.to_id()
            };
            if text == want {
                c.push((0, 0, id.to_string(), id));
            } else if on_chain && item.name == r.name {
                c.push((1, 0, id.to_string(), id));
            } else if on_chain {
                let d = levenshtein(&text, &want);
                if d <= (want.len() / 2).max(3) {
                    c.push((2, d, id.to_string(), id));
                }
            }
        }
        c.sort_by(|a, b| (a.0, a.1, &a.2).cmp(&(b.0, b.1, &b.2)));
        c.truncate(3);
        c.into_iter().map(|x| x.3).collect()
    }

    /// `kind.name` or `kind.name.field` (R-SCOPE-040); the longest item name wins.
    pub(crate) fn resolve_target(
        &self,
        from: &ScopeId,
        text: &str,
    ) -> Option<(ItemId, Option<String>)> {
        let text = normalise_rules_alias(text);
        let id = Id::parse(&text).ok()?;
        if let Lookup::Found(i) = self.lookup(from, &id) {
            return Some((i, None));
        }
        let (head, field) = text.rsplit_once('.')?;
        let id2 = Id::parse(head).ok()?;
        match self.lookup(from, &id2) {
            Lookup::Found(i) => Some((i, Some(field.to_string()))),
            _ => None,
        }
    }
}

// --- edges and sites ----------------------------------------------------------------------

pub(crate) struct Refs {
    pub(crate) edges: Vec<Edge>,
    pub(crate) sites: BTreeMap<ItemId, Vec<Site>>,
}

pub(crate) fn collect_refs(
    idx: &Index<'_>,
    folder: &str,
    corpus: &Corpus,
    diags: &mut Vec<Diagnostic>,
) -> Refs {
    let mut refs = Refs {
        edges: Vec::new(),
        sites: BTreeMap::new(),
    };
    for (path, rec) in &corpus.records {
        let Some(sid) = idx.table.scope_of(path) else {
            continue;
        };
        let Some(scope) = idx
            .table
            .get(sid)
            .filter(|s| s.state == ScopeState::Resolved)
        else {
            continue;
        };
        let Some(ex) = rec.extraction.as_ref() else {
            continue;
        };
        for d in &ex.directives {
            let is_use = d.verb == Verb::Use;
            let is_site = d.verb == Verb::Def && !is_decl_site(path, scope, folder, d);
            if !(is_use || is_site) || !check_keys(d, sid, idx.table, idx.kinds, path, diags) {
                continue;
            }
            let site = Site {
                path: path.clone(),
                span: d.span,
                extent: d.extent,
                scope: sid.clone(),
            };
            let sym = d.arg("sym").map(|a| arg_scalar(&a.value));
            for id in &d.ids {
                if id.kind.as_str() == "foreign" {
                    continue;
                }
                let text = fairlead_syntax::render_directive_body(d);
                if !idx.kinds.is_declared(idx.table, sid, id) {
                    diags.push(undeclared_kind(
                        idx.kinds, idx.table, sid, id, path, d.span, &text,
                    ));
                    continue;
                }
                let found = match idx.lookup(sid, id) {
                    Lookup::Found(i) => Some(i),
                    Lookup::Ambiguous(c) => {
                        let names: Vec<String> = c.iter().map(|i| i.to_id().to_string()).collect();
                        diags.push(
                            Dg::new(
                                codes::HN_R007,
                                Severity::Error,
                                path,
                                d.span.line,
                                d.span.col,
                            )
                            .scope(sid)
                            .msg(format!(
                                "reference `{id}` is ambiguous: declared in {}",
                                names.join(", ")
                            ))
                            .related(names.join(" "))
                            .hint("qualify it as scope:kind.name")
                            .text(text)
                            .done(),
                        );
                        None
                    }
                    Lookup::Missing { .. } => None, // dangling: HN-R001 is raised by P5 from `edges`
                };
                if is_use {
                    refs.edges.push(Edge {
                        from: site.clone(),
                        to: id.clone(),
                        sym: sym.clone(),
                        resolved: found,
                    });
                } else if let Some(item) = found {
                    refs.sites.entry(item).or_default().push(site.clone());
                }
            }
        }
    }
    refs
}

use fairlead_syntax::ids::ScopeSlug;

pub fn home_mismatches(reg: &Registry) -> Vec<(ItemId, Site, ScopeId)> {
    let mut out = Vec::new();
    for (id, item) in &reg.items {
        let Some(FieldValue::Scalar(h)) = item.fields.get("home") else {
            continue;
        };
        let Ok(slug) = ScopeSlug::parse(h) else {
            continue;
        };
        let home = ScopeId(slug);
        for site in reg.sites_of(id) {
            if !reg.table.is_ancestor_or_self(&home, &site.scope) {
                out.push((id.clone(), site.clone(), home.clone()));
            }
        }
    }
    out
}
