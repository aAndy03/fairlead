//! `fairlead new` and the demote planner (R-CLI-028..031).
use super::init::{toml_list, toml_str};
use super::writeset::{PlannedWrite, WriteKind, WriteSet, WriteSetKind};
use super::{Plan, PlanAction, PlanLine, plain_diff, refuse, repo_path};
use crate::classify::FileClass;
use crate::config::{ImplLimit, KindDecl, KindShape};
use crate::error::{Error, RefusalKind};
use crate::facts::Facts;
use crate::registry::{Lookup, Origin, OvrOp, Registry, ScopeId, ScopeState};
use crate::time::AsOf;
use fairlead_syntax::ids::{Id, IdQualifier, PLACEHOLDER};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NewRequest {
    pub kind: String,
    pub name: String,
    pub scope: Option<ScopeId>,
    pub fields: BTreeMap<String, String>,
    pub print: bool,
    pub at: Option<crate::path::RepoPath>,
    pub hoist: bool,
}

pub(crate) fn folder_of(reg: &Registry) -> String {
    reg.scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .and_then(|c| c.settings.folder.clone())
        .unwrap_or_else(|| "fairlead".into())
}
pub(crate) fn under(dir: &str, rest: &str) -> String {
    let d = dir.trim_matches('/');
    if d.is_empty() || d == "." {
        rest.to_owned()
    } else {
        format!("{d}/{rest}")
    }
}
pub(crate) fn op_word(o: OvrOp) -> &'static str {
    match o {
        OvrOp::Loosen => "loosen",
        OvrOp::Tighten => "tighten",
        OvrOp::Set => "set",
        OvrOp::Replace => "replace",
        OvrOp::Add => "add",
        OvrOp::Remove => "remove",
    }
}

pub(crate) struct Ctx {
    pub folder: String,
    pub scope: ScopeId,
    pub dir: String,
}

fn ctx(req: &NewRequest, reg: &Registry) -> Result<Ctx, Error> {
    let scope = req
        .scope
        .clone()
        .unwrap_or_else(|| reg.root_scope().clone());
    let sc = reg
        .scope(&scope)
        .ok_or_else(|| Error::Usage(format!("unknown scope {}", scope.0.as_str())))?;
    if !matches!(sc.state, ScopeState::Resolved) {
        return Err(Error::Usage(format!(
            "scope {} is unresolved; fix its control file first",
            scope.0.as_str()
        )));
    }
    Ok(Ctx {
        folder: folder_of(reg),
        dir: sc.dir.as_str().to_owned(),
        scope,
    })
}

pub(crate) fn check_fields(
    fields: &BTreeMap<String, String>,
    allowed: &[&str],
    what: &str,
) -> Result<(), Error> {
    for (k, v) in fields {
        if !allowed.contains(&k.as_str()) && !k.starts_with("x-") {
            return Err(Error::Usage(format!(
                "unknown field {k:?} for {what}; allowed: {}",
                allowed.join(", ")
            )));
        }
        if v.chars().any(|c| c.is_control() || c == '\'') {
            return Err(Error::Usage(format!(
                "field {k:?} must not contain control characters or single quotes"
            )));
        }
    }
    Ok(())
}

pub(crate) fn directive_value(v: &str) -> String {
    if !v.is_empty()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.#/@%:*".contains(c))
    {
        v.to_owned()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

fn builtin_handoff() -> KindDecl {
    KindDecl {
        title: Some("handoff".into()),
        shape: KindShape::Doc,
        impl_limit: ImplLimit::Any,
        unused: None,
        dir: Some("docs/handoff/".into()),
        source_generate: false,
        keys: BTreeMap::new(),
        layers: Vec::new(),
        container: false,
    }
}

fn kind_decl(reg: &Registry, scope: &ScopeId, kind: &str) -> Option<KindDecl> {
    for sid in reg.chain(scope).into_iter().rev() {
        if let Some(d) = reg
            .scope(sid)
            .and_then(|s| s.control.as_ref())
            .and_then(|c| c.kinds.get(kind))
        {
            return Some(d.clone());
        }
    }
    (kind == "handoff").then(builtin_handoff)
}

fn declared_kinds(reg: &Registry, scope: &ScopeId) -> BTreeSet<String> {
    let mut k: BTreeSet<String> = reg
        .chain(scope)
        .into_iter()
        .filter_map(|s| reg.scope(s).and_then(|x| x.control.as_ref()))
        .flat_map(|c| c.kinds.keys().cloned())
        .collect();
    k.insert("handoff".into());
    k
}

fn valid_id(kind: &str, name: &str) -> Result<Id, Error> {
    let id = Id::parse(&format!("{kind}.{name}"))
        .map_err(|e| Error::Usage(format!("invalid name {name:?}: {e:?}")))?;
    if matches!(id.qualifier, IdQualifier::Bare) {
        Ok(id)
    } else {
        Err(Error::Usage(format!("invalid name {name:?}")))
    }
}

pub fn new_plan(
    req: &NewRequest,
    reg: &Registry,
    _facts: &Facts,
    as_of: AsOf,
) -> Result<Plan, Error> {
    // `_facts` is part of the contract signature; scaffolds in 0.1 need nothing from disk facts.
    let cx = ctx(req, reg)?;
    if req.at.is_some() && req.kind != "scope" {
        return Err(Error::Usage("--at is only valid for `new scope`".into()));
    }
    match req.kind.as_str() {
        "exc" => Err(Error::Usage(
            "`exc` is a reserved kind and cannot be authored".into(),
        )),
        "approval" => Err(Error::Usage(
            "approvals are recorded with `fairlead approve`, not `fairlead new`".into(),
        )),
        "change" => Err(Error::Internal(
            "change docs need the version line; plan them through pipeline::plan_new".into(),
        )),
        "revive" => Err(Error::Reserved {
            verb: "new revive",
            milestone: "0.2",
        }),
        "request" => Err(Error::Reserved {
            verb: "new request",
            milestone: "0.3",
        }),
        "ladder" => plan_ladder(req, reg, &cx),
        "scope" => plan_scope(req, reg, &cx),
        "rule" => plan_rule(req, reg, as_of, &cx),
        "escalation" => plan_escalation(req, reg, &cx),
        kind => {
            let decl = kind_decl(reg, &cx.scope, kind).ok_or_else(|| {
                Error::Usage(format!(
                    "unknown kind {kind:?}; declared kinds: {}",
                    declared_kinds(reg, &cx.scope)
                        .into_iter()
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
            plan_doc(req, reg, as_of, &cx, &decl)
        }
    }
}

/// `new change "<title>"` (R-VER-012, R-VER-040): a draft change doc in the scope's
/// `<folder>/docs/changes/`, named `d-<draft>-<slug>.md`, with `created` set to the open version.
/// `bits` are 40 random bits drawn by the caller; a draft ID already in the tree is refused.
pub fn new_change_plan(
    req: &NewRequest,
    reg: &Registry,
    open: crate::version::Version,
    bits: [u8; 5],
    taken: &BTreeSet<crate::docs::DraftId>,
    as_of: AsOf,
) -> Result<Plan, Error> {
    use crate::docs::{DraftId, slug};
    let cx = ctx(req, reg)?;
    check_fields(
        &req.fields,
        &["impact", "covers", "hotfix", "revisits", "revisit-why"],
        "change",
    )?;
    let hotfix = req.fields.contains_key("hotfix");
    let hotfix_on = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .is_some_and(|c| c.docs_first.hotfix);
    if hotfix && !hotfix_on {
        return Err(Error::Usage(
            "the hotfix path is off: set docs-first.hotfix = true at the root (R-VER-047)".into(),
        ));
    }
    if let Some(i) = req.fields.get("impact")
        && crate::version::Impact::parse(i).is_none()
    {
        return Err(Error::Usage(format!(
            "impact must be break, add or fix, not {i:?}"
        )));
    }
    let id = DraftId::from_bits(bits);
    if taken.contains(&id) {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("draft ID {id} is already in the tree"),
            "run the command again; a new ID is drawn each time",
        ));
    }
    let title = req.name.trim();
    if title.is_empty() {
        return Err(Error::Usage("new change needs a title".into()));
    }
    let target = repo_path(&under(
        &cx.dir,
        &format!("{}/docs/changes/{id}-{}.md", cx.folder, slug(title)),
    ))?;
    let mut fm = format!("+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"draft\"\n");
    if let Some(i) = req.fields.get("impact") {
        fm.push_str(&format!("impact = \"{i}\"\n"));
    }
    let covers: Vec<String> = req.fields.get("covers").map(|c| csv(c)).unwrap_or_default();
    if let Some(r) = req.fields.get("revisits") {
        fm.push_str(&format!("revisits = {}\n", toml_str(r)));
    }
    if let Some(r) = req.fields.get("revisit-why") {
        fm.push_str(&format!("revisit-why = {}\n", toml_str(r)));
    }
    if hotfix {
        // R-VER-047: the debt window counts whole UTC days from this date.
        fm.push_str(&format!("hotfix = true\nopened = \"{}\"\n", as_of.0));
    }
    fm.push_str(&format!(
        "scope = {}\ntitle = {}\ncovers = {}\ncreated = \"{open}\"\n+++\n",
        toml_str(cx.scope.0.as_str()),
        toml_str(title),
        toml_list(&covers)
    ));
    let text = format!(
        "{fm}\n# {title}\n\n{PLACEHOLDER}: what changes and why. Fill `covers` (globs or IDs under this scope) and `impact` (break, add or fix) before setting `status = \"ready\"`.\n"
    );
    let mut set = WriteSet::new(WriteSetKind::New);
    set.add(PlannedWrite {
        path: target.clone(),
        kind: WriteKind::Create,
        bytes: text.clone().into_bytes(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    let lines = vec![PlanLine {
        action: PlanAction::Create,
        path: target,
        detail: format!("change {id}"),
        diff: Some(plain_diff("", &text, 60)),
    }];
    let mut notes = Vec::new();
    if covers.is_empty() || !req.fields.contains_key("impact") {
        notes.push("fill before ready: covers, impact".to_owned());
    }
    Ok(finish(req, set, lines, notes, &text))
}

pub(crate) fn finish(
    req: &NewRequest,
    set: WriteSet,
    lines: Vec<PlanLine>,
    notes: Vec<String>,
    scaffold: &str,
) -> Plan {
    if req.print {
        let shown = lines.first().map(|l| PlanLine {
            action: PlanAction::Create,
            path: l.path.clone(),
            detail: "print only".into(),
            diff: Some(scaffold.to_owned()),
        });
        return Plan::new(
            WriteSet::new(WriteSetKind::New),
            shown.into_iter().collect(),
            notes,
        );
    }
    Plan::new(set, lines, notes)
}

/// The text `--print` writes to stdout.
pub fn scaffold_text(plan: &Plan) -> String {
    plan.lines.iter().filter_map(|l| l.diff.clone()).collect()
}

const DEF_KEYS: [&str; 10] = [
    "st", "sum", "kw", "sym", "of", "policy", "value", "applies", "min-len", "home",
];

fn plan_doc(
    req: &NewRequest,
    reg: &Registry,
    as_of: AsOf,
    cx: &Ctx,
    decl: &KindDecl,
) -> Result<Plan, Error> {
    if matches!(decl.shape, KindShape::Code) {
        return Err(Error::Usage(format!(
            "kind {} is code-shaped; declare items with a def comment next to the symbol",
            req.kind
        )));
    }
    let id = valid_id(&req.kind, &req.name)?;
    if let Some(t) = reg.tomb_for(&cx.scope, &id) {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!(
                "{}.{} was removed in {} and is reserved forever",
                req.kind, req.name, t.removed
            ),
            "pick another name; a revive doc creates a versioned successor (R-LC-024)",
        ));
    }
    if !matches!(reg.lookup(&cx.scope, &id), Lookup::Missing { .. }) {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("{}.{} is already declared", req.kind, req.name),
            "pick another name",
        ));
    }
    let mut allowed: Vec<&str> = DEF_KEYS.to_vec();
    allowed.extend(decl.keys.keys().map(String::as_str));
    allowed.push("css");
    check_fields(&req.fields, &allowed, &req.kind)?;
    let mut keys: BTreeMap<String, String> = decl
        .keys
        .iter()
        .filter(|(_, k)| k.required)
        .map(|(n, _)| (n.clone(), PLACEHOLDER.to_owned()))
        .collect();
    if req.kind == "handoff" {
        keys.insert("sum".into(), PLACEHOLDER.to_owned());
    }
    for (k, v) in req.fields.iter().filter(|(k, _)| *k != "css") {
        keys.insert(k.clone(), v.clone());
    }
    let mut def = format!("def {}.{}", req.kind, req.name);
    for (k, v) in &keys {
        def.push_str(&format!(" {k}={}", directive_value(v)));
    }

    let docs = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .map(|c| c.layout.docs.clone())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "docs".into());
    let sub = decl
        .dir
        .clone()
        .unwrap_or_else(|| format!("{docs}/{}/", req.kind));
    let target = repo_path(&under(
        &cx.dir,
        &format!("{}/{}/{}.md", cx.folder, sub.trim_matches('/'), req.name),
    ))?;
    let fold = target.fold_key();
    if reg
        .items()
        .any(|i| matches!(&i.origin, Origin::DocDef { path, .. } if path.fold_key() == fold))
    {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("{} collides with an existing doc", target.as_str()),
            "names may not differ only by case",
        ));
    }
    let text = format!(
        "+++\ntitle = {}\ncreated = \"{}\"\ndirectives = [\n  '{def}',\n]\n+++\n\n# {}\n\n{PLACEHOLDER}: describe this {}.\n",
        toml_str(&req.name),
        as_of.0,
        req.name,
        req.kind
    );
    let mut notes = Vec::new();
    let slots: Vec<&str> = keys
        .iter()
        .filter(|(_, v)| v.as_str() == PLACEHOLDER)
        .map(|(k, _)| k.as_str())
        .collect();
    if !slots.is_empty() {
        notes.push(format!("fill before check passes: {}", slots.join(", ")));
    }
    if let Some(css) = req.fields.get("css") {
        notes.push(format!(
            "optional stub for {css} (paste at your configured marker): --{}: {PLACEHOLDER};",
            req.name
        ));
    }
    let mut set = WriteSet::new(WriteSetKind::New);
    set.add(PlannedWrite {
        path: target.clone(),
        kind: WriteKind::Create,
        bytes: text.clone().into_bytes(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    let lines = vec![PlanLine {
        action: PlanAction::Create,
        path: target,
        detail: format!("{} {}", req.kind, req.name),
        diff: Some(plain_diff("", &text, 60)),
    }];
    Ok(finish(req, set, lines, notes, &text))
}

use crate::registry::Override;
use fairlead_syntax::ids::{ScopeSlug, is_reserved_windows_name};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HoistGroup {
    pub sample: Override,
    pub sites: Vec<crate::diag::Location>,
}

/// Group identical file-local `ovr`s under `dir_prefix` that occur in at least `min_files` distinct files.
pub fn hoist_groups(overrides: &[Override], dir_prefix: &str, min_files: usize) -> Vec<HoistGroup> {
    let mut by: BTreeMap<String, Vec<&Override>> = BTreeMap::new();
    for o in overrides {
        let p = o.at.path.as_str();
        if o.applies.is_some() || !p.starts_with(dir_prefix) || p.ends_with("control.toml") {
            continue;
        }
        let key = format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            o.target.to_id(),
            o.field,
            o.op,
            o.set,
            o.why,
            o.owner,
            o.exp
        );
        by.entry(key).or_default().push(o);
    }
    by.into_values()
        .filter(|v| {
            v.iter()
                .map(|o| o.at.path.as_str())
                .collect::<BTreeSet<_>>()
                .len()
                >= min_files
        })
        .filter_map(|v| {
            let sample = (*v.first()?).clone();
            let mut sites: Vec<_> = v.iter().map(|o| o.at.clone()).collect();
            sites.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
            Some(HoistGroup { sample, sites })
        })
        .collect()
}

pub fn hoist_block(g: &HoistGroup) -> String {
    use crate::registry::{FieldChange, FieldValue};
    let o = &g.sample;
    let mut s = format!(
        "\n[[ovr]]\ntarget = {}\nop = {}\napplies = \"**\"\n",
        toml_str(&o.target.to_id().to_string()),
        toml_str(op_word(o.op))
    );
    if let Some(w) = &o.why {
        s.push_str(&format!("why = {}\n", toml_str(w)));
    }
    if let Some(w) = &o.owner {
        s.push_str(&format!("owner = {}\n", toml_str(w)));
    }
    if let Some(d) = &o.exp {
        s.push_str(&format!("exp = {}\n", toml_str(&d.to_string())));
    }
    if !o.set.is_empty() {
        s.push_str("[ovr.set]\n");
        for (k, c) in &o.set {
            match c {
                FieldChange::Replace(FieldValue::Scalar(v)) => {
                    s.push_str(&format!("{k} = {}\n", toml_str(v)))
                }
                FieldChange::Replace(FieldValue::List(v)) => {
                    s.push_str(&format!("{k} = {}\n", toml_list(v)))
                }
                FieldChange::Append(v) => s.push_str(&format!(
                    "# {k} appends {} (not hoisted automatically)\n",
                    v.join(",")
                )),
            }
        }
    }
    s
}

fn plan_scope(req: &NewRequest, reg: &Registry, cx: &Ctx) -> Result<Plan, Error> {
    let bad = |what: &str, e: &dyn std::fmt::Debug| Error::Usage(format!("invalid {what}: {e:?}"));
    check_fields(&req.fields, &["purpose"], "scope")?;
    let slug = ScopeSlug::parse(&req.name).map_err(|e| bad("scope id", &e))?;
    if is_reserved_windows_name(slug.as_str()) {
        return Err(bad("scope id", &"Windows-reserved name"));
    }
    if reg.scope(&ScopeId(slug.clone())).is_some() {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("scope id {} already exists", slug.as_str()),
            "a scope's identity is its id; pick another id",
        ));
    }
    let dir = req
        .at
        .as_ref()
        .map_or_else(|| under(&cx.dir, slug.as_str()), |p| p.as_str().to_owned());
    if let Some(seg) = dir.split('/').find(|s| is_reserved_windows_name(s)) {
        return Err(bad("directory", &seg));
    }
    let dir_path = repo_path(&dir)?;
    if reg
        .scopes()
        .any(|s| s.dir.fold_key() == dir_path.fold_key())
    {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("a scope already lives at {dir}"),
            "choose another directory",
        ));
    }
    let control = repo_path(&format!("{dir}/{}/control.toml", cx.folder))?;
    let parent = reg
        .scope_of(&control)
        .and_then(|id| reg.scope(id))
        .ok_or_else(|| Error::Usage(format!("{dir} is outside every scope")))?;
    if req.scope.is_some() && parent.id != cx.scope {
        return Err(Error::Usage(format!(
            "{dir} is not inside scope {}",
            cx.scope.0.as_str()
        )));
    }
    let mut text = format!("[fairlead]\nscope = {}\n", toml_str(slug.as_str()));
    if let Some(p) = req.fields.get("purpose") {
        if p.chars().count() > 160 {
            return Err(Error::Usage("purpose is at most 160 characters".into()));
        }
        text.push_str(&format!("\n[settings]\npurpose = {}\n", toml_str(p)));
    }
    let mut notes = vec![
        "run `fairlead sync` to register the scope in its parent's membership shard".to_owned(),
    ];
    if parent.depth + 1 > 3 {
        notes.push(format!(
            "warning: scope depth {} exceeds 3 (advisory, R-SCOPE-010)",
            parent.depth + 1
        ));
    }
    if req.hoist {
        let min = reg
            .scope(reg.root_scope())
            .and_then(|s| s.control.as_ref())
            .map_or(3, |c| c.eligibility.min_files.max(2)) as usize;
        for g in hoist_groups(reg.overrides(), &format!("{dir}/"), min) {
            text.push_str(&hoist_block(&g));
            for l in g.sites.iter().take(20) {
                notes.push(format!("hoisted: remove the file-local ovr at {}:{} (until then DUP-OVR makes both inert)", l.path.as_str(), l.line));
            }
        }
    }
    let mut set = WriteSet::new(WriteSetKind::New);
    set.add(PlannedWrite {
        path: control.clone(),
        kind: WriteKind::Create,
        bytes: text.clone().into_bytes(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    let lines = vec![PlanLine {
        action: PlanAction::Create,
        path: control,
        detail: format!("scope {}", slug.as_str()),
        diff: Some(plain_diff("", &text, 60)),
    }];
    Ok(finish(req, set, lines, notes, &text))
}

fn control_cas(
    reg: &Registry,
    scope: &ScopeId,
    folder: &str,
) -> Result<(crate::path::RepoPath, crate::hash::ContentHash), Error> {
    let sc = reg
        .scope(scope)
        .ok_or_else(|| Error::Usage("unknown scope".into()))?;
    let c = sc
        .control
        .as_ref()
        .ok_or_else(|| Error::Usage(format!("scope {} has no control file", scope.0.as_str())))?;
    Ok((
        repo_path(&under(sc.dir.as_str(), &format!("{folder}/control.toml")))?,
        c.content_hash,
    ))
}

fn append_control(
    path: crate::path::RepoPath,
    prior: crate::hash::ContentHash,
    text: String,
    detail: String,
    req: &NewRequest,
    notes: Vec<String>,
) -> Result<Plan, Error> {
    let mut set = WriteSet::new(WriteSetKind::New);
    set.add(PlannedWrite {
        path: path.clone(),
        kind: WriteKind::AppendBlock {
            marker: String::new(),
        },
        bytes: text.clone().into_bytes(),
        class: FileClass::Authored,
        expect_prior: Some(prior),
    })?;
    let lines = vec![PlanLine {
        action: PlanAction::Edit,
        path,
        detail,
        diff: Some(plain_diff("", &text, 40)),
    }];
    Ok(finish(req, set, lines, notes, &text))
}

fn csv(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn plan_rule(req: &NewRequest, reg: &Registry, as_of: AsOf, cx: &Ctx) -> Result<Plan, Error> {
    let id = valid_id("rule", &req.name)?;
    check_fields(
        &req.fields,
        &[
            "check", "sev", "glob", "exclude", "waivable", "why", "forbid", "msg", "tokens",
        ],
        "rule",
    )?;
    // 0.2.2-T3 (R-LC-021): as for doc kinds, a removed rule ID is reserved forever.
    if let Some(t) = reg.tomb_for(&cx.scope, &id) {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!(
                "rule.{} was removed ({}) and is reserved forever",
                req.name, t.removed
            ),
            "pick another name; a revive doc creates a versioned successor (R-LC-024)",
        ));
    }
    if !matches!(reg.lookup(&cx.scope, &id), Lookup::Missing { .. }) {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("rule.{} is already declared", req.name),
            "pick another name",
        ));
    }
    let get = |k: &str, d: &str| req.fields.get(k).cloned().unwrap_or_else(|| d.to_owned());
    let choice = |k: &str, d: &str, ok: &[&str]| -> Result<String, Error> {
        let v = get(k, d);
        if ok.contains(&v.as_str()) {
            Ok(v)
        } else {
            Err(Error::Usage(format!(
                "{k} must be one of {}",
                ok.join(", ")
            )))
        }
    };
    let check = choice("check", "pattern", &["pattern", "leak"])?;
    let mut s = format!("\n[rules.{}]\ncheck = {}\n", req.name, toml_str(&check));
    if check == "leak" {
        let toks = req
            .fields
            .get("tokens")
            .map_or_else(|| vec![PLACEHOLDER.to_owned()], |v| csv(v));
        s.push_str(&format!("tokens = {}\n", toml_list(&toks)));
    } else {
        s.push_str(&format!(
            "forbid = {}\nmsg = {}\n",
            toml_str(&get("forbid", PLACEHOLDER)),
            toml_str(&get("msg", PLACEHOLDER))
        ));
    }
    s.push_str(&format!(
        "sev = {}\nstage = \"observe\"\n",
        toml_str(&choice("sev", "warn", &["error", "warn", "info"])?)
    ));
    for k in ["glob", "exclude"] {
        if let Some(v) = req.fields.get(k) {
            s.push_str(&format!("{k} = {}\n", toml_list(&csv(v))));
        }
    }
    if let Some(v) = req.fields.get("waivable") {
        s.push_str(&format!(
            "waivable = {}\n",
            choice("waivable", "false", &["true", "false"]).map(|_| v.as_str())?
        ));
    }
    s.push_str(&format!(
        "why = {}\nsince = {}\n",
        toml_str(&get("why", PLACEHOLDER)),
        toml_str(&as_of.0.to_string())
    ));
    let mut notes =
        vec!["stage is observe (R-CHK-009); raise it when the baseline is clean".to_owned()];
    let slots: Vec<&str> = [
        ("why", get("why", PLACEHOLDER)),
        ("forbid", get("forbid", PLACEHOLDER)),
        ("msg", get("msg", PLACEHOLDER)),
    ]
    .iter()
    .filter(|(_, v)| v == PLACEHOLDER)
    .map(|(k, _)| *k)
    .collect();
    if !slots.is_empty() {
        notes.push(format!("fill before check passes: {}", slots.join(", ")));
    }
    let (path, prior) = control_cas(reg, &cx.scope, &cx.folder)?;
    append_control(path, prior, s, format!("rule {}", req.name), req, notes)
}

/// D-20: the presets of 30-hierarchy are content, scaffolded into the root control file.
const LADDER_UI: &str = "
[layers.ui]
atom = 10
molecule = 20
organism = 30
template = 40
page = 50

[free.chrome]
of = \"ui\"
may-use = [\"ui.atom\", \"ui.molecule\", \"ui.organism\"]
";
const LADDER_CORE: &str = "
[layers.core]
primitive = 10
adapter = 20
engine = 30
pipeline = 40
service = 50
shell = 60

[layer-policy.\"core.shell\"]
used-by = \"entry\"
";

/// `new ladder ui|core` (R-HIER-001, D-20): ladders are root vocabulary, written once.
fn plan_ladder(req: &NewRequest, reg: &Registry, cx: &Ctx) -> Result<Plan, Error> {
    check_fields(&req.fields, &[], "ladder")?;
    if cx.scope != *reg.root_scope() {
        return Err(Error::Usage(
            "ladders are declared in the root control file only (R-HIER-001)".into(),
        ));
    }
    let text = match req.name.as_str() {
        "ui" => LADDER_UI,
        "core" => LADDER_CORE,
        other => {
            return Err(Error::Usage(format!(
                "no preset ladder {other:?}: the presets are ui and core; declare another ladder as a [layers.<name>] table"
            )));
        }
    };
    let declared = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .is_some_and(|c| c.hier.ladders.contains_key(&req.name));
    if declared {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("ladder {} is already declared", req.name),
            "edit its [layers] table in the root control file",
        ));
    }
    let (path, prior) = control_cas(reg, &cx.scope, &cx.folder)?;
    append_control(
        path,
        prior,
        text.to_owned(),
        format!("ladder {}", req.name),
        req,
        vec!["presets are content (D-20): rename, add or remove layers freely; set [hier] level (0 to 3) to switch the checks on".to_owned()],
    )
}

fn plan_escalation(req: &NewRequest, reg: &Registry, cx: &Ctx) -> Result<Plan, Error> {
    valid_id("req", &req.name)?; // the target is a dotted slug path
    check_fields(&req.fields, &["to", "set", "why", "blocking"], "escalation")?;
    let sc = reg
        .scope(&cx.scope)
        .ok_or_else(|| Error::Usage("unknown scope".into()))?;
    // 0.2.2-T1: the name is the `[[req]]` target. The field test passed a label
    // (`allow-dbg-demo`), which wrote a request no grant can ever answer.
    let Some((head, _)) = req.name.split_once('.') else {
        return Err(Error::Usage(format!(
            "{:?} is not an escalation target: name the item or key to escalate, as kind.name (for example rule.no-dbg) or a vocabulary key (layer-ranks.widget)",
            req.name
        )));
    };
    let item_kind = crate::registry::item::BUILTIN_KINDS.contains(&head)
        || kind_decl(reg, &cx.scope, head).is_some();
    if item_kind && reg.resolve_target(&cx.scope, &req.name).is_none() {
        return Err(Error::Usage(format!(
            "no item {} is visible from scope {}: an escalation names an existing item (kind.name, or kind.name.field)",
            req.name,
            cx.scope.0.as_str()
        )));
    }
    let parent = sc
        .parent
        .clone()
        .ok_or_else(|| Error::Usage("the root scope has no ancestor to escalate to".into()))?;
    let to = req
        .fields
        .get("to")
        .cloned()
        .unwrap_or_else(|| parent.0.as_str().to_owned());
    if !reg
        .chain(&cx.scope)
        .iter()
        .any(|s| s.0.as_str() == to && **s != cx.scope)
    {
        return Err(Error::Usage(format!(
            "{to:?} is not an ancestor of scope {}",
            cx.scope.0.as_str()
        )));
    }
    if reg
        .requests()
        .iter()
        .any(|r| r.from == cx.scope && r.to.0.as_str() == to && r.target == req.name)
    {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!("an escalation of {} to {to} already exists", req.name),
            "wait for the ancestor to answer",
        ));
    }
    let mut s = format!(
        "\n[[req]]\nto = {}\ntarget = {}\n",
        toml_str(&to),
        toml_str(&req.name)
    );
    if let Some(v) = req.fields.get("set") {
        s.push_str(&format!("set = {}\n", toml_str(v)));
    }
    s.push_str(&format!(
        "why = {}\n",
        toml_str(
            &req.fields
                .get("why")
                .cloned()
                .unwrap_or_else(|| PLACEHOLDER.to_owned())
        )
    ));
    if let Some(b) = req.fields.get("blocking") {
        if !matches!(b.as_str(), "true" | "false") {
            return Err(Error::Usage("blocking must be true or false".into()));
        }
        s.push_str(&format!("blocking = {b}\n"));
    }
    let (path, prior) = control_cas(reg, &cx.scope, &cx.folder)?;
    append_control(
        path,
        prior,
        s,
        format!("escalation to {to}"),
        req,
        vec!["a [[req]] is inert until the ancestor answers (R-SCOPE-053)".to_owned()],
    )
}

/// C-9: library only in 0.1; the verb is `move --demote` (0.2).
pub fn plan_demote(reg: &Registry, scope: &ScopeId) -> Result<Plan, Error> {
    if scope == reg.root_scope() {
        return Err(Error::Usage("the root scope cannot be demoted".into()));
    }
    let sc = reg
        .scope(scope)
        .ok_or_else(|| Error::Usage(format!("unknown scope {}", scope.0.as_str())))?;
    let parent_id = sc
        .parent
        .clone()
        .ok_or_else(|| Error::Usage("the scope has no ancestor to demote into".into()))?;
    let parent = reg
        .scope(&parent_id)
        .ok_or_else(|| Error::Internal("parent scope vanished".into()))?;
    let ancestors = reg.chain(&parent_id);
    // The scope's own IDs: its items plus its control-file rule tables (a rule that redeclares an
    // ancestor's is dropped by the registry as shadowing, so it is not an item of its own).
    let mut mine: std::collections::BTreeSet<String> = reg
        .items()
        .filter(|i| &i.id.scope == scope)
        .map(|i| i.id.to_id().kind_name())
        .collect();
    if let Some(c) = &sc.control {
        mine.extend(c.rules.keys().map(|n| format!("rule.{n}")));
    }
    let mut clashes: Vec<String> = mine
        .into_iter()
        .filter(|kn| {
            reg.items()
                .any(|o| ancestors.contains(&&o.id.scope) && o.id.to_id().kind_name() == *kn)
        })
        .collect();
    clashes.sort();
    if !clashes.is_empty() {
        return Err(refuse(
            RefusalKind::NoClobber,
            format!(
                "demoting {} would duplicate ids in {}: {}",
                scope.0.as_str(),
                parent_id.0.as_str(),
                clashes.join(", ")
            ),
            "rename or remove the duplicates first",
        ));
    }
    let date = reg.as_of().0;
    let path = repo_path(&under(
        parent.dir.as_str(),
        &format!("{}/tombstones/{}.toml", folder_of(reg), date.year()),
    ))?;
    let former = sc.dir.as_str().to_owned();
    let text = format!(
        "\n[[tomb]]\nid = {}\nremoved = {}\nby = \"demote\"\nwhy = {}\npaths = {}\n",
        toml_str(scope.0.as_str()),
        toml_str(&date.to_string()),
        toml_str(&format!("scope demoted into {}", parent_id.0.as_str())),
        toml_list(std::slice::from_ref(&former))
    );
    let mut notes = vec![format!(
        "move the declarations of {} into {} with applies = \"{former}/**\", then delete {former}/{}/ (R-SCOPE-012)",
        scope.0.as_str(),
        parent_id.0.as_str(),
        folder_of(reg)
    )];
    let kids: Vec<&str> = reg
        .scopes()
        .filter(|s| s.parent.as_ref() == Some(scope))
        .map(|s| s.id.0.as_str())
        .collect();
    if !kids.is_empty() {
        notes.push(format!(
            "child scopes re-parent to {}: {}",
            parent_id.0.as_str(),
            kids.join(", ")
        ));
    }
    let mut set = WriteSet::new(WriteSetKind::Sync);
    set.add(PlannedWrite {
        path: path.clone(),
        kind: WriteKind::AppendBlock {
            marker: String::new(),
        },
        bytes: text.clone().into_bytes(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    let lines = vec![PlanLine {
        action: PlanAction::Edit,
        path,
        detail: format!("tombstone for {}", scope.0.as_str()),
        diff: Some(plain_diff("", &text, 20)),
    }];
    Ok(Plan::new(set, lines, notes))
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::error::{Error, RefusalKind};
    use crate::write::testutil::*;
    use crate::write::{PlanAction, WriteKind};

    pub(super) const TOK: &str = "\n[kinds.tok]\ntitle = \"token\"\nshape = \"doc\"\ndir = \"contract/tokens/\"\n[kinds.tok.keys]\nrole = { type = \"string\", required = true }\n";
    pub(super) fn env(extra: &str) -> (crate::discover::Root, Registry) {
        registry(
            &[
                ("fairlead/control.toml", &format!("{ROOT_CONTROL}{extra}")),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        )
    }
    pub(super) fn nr(kind: &str, name: &str) -> NewRequest {
        NewRequest {
            kind: kind.into(),
            name: name.into(),
            scope: None,
            fields: Default::default(),
            print: false,
            at: None,
            hoist: false,
        }
    }
    pub(super) fn plan(extra: &str, r: &NewRequest) -> Result<Plan, Error> {
        let (_root, reg) = env(extra);
        new_plan(
            r,
            &reg,
            &facts_empty(),
            crate::time::AsOf::parse("2026-10-09").unwrap(),
        )
    }
    pub(super) fn text(p: &Plan, path: &str) -> String {
        String::from_utf8(p.set.writes[&rp(path)].bytes.clone()).unwrap()
    }

    fn change(r: &NewRequest, taken: &[&str]) -> Result<Plan, Error> {
        let (_root, reg) = env("");
        let taken = taken
            .iter()
            .map(|t| crate::docs::DraftId::parse(t).unwrap())
            .collect();
        new_change_plan(
            r,
            &reg,
            crate::version::Version::new(1, 2, 0),
            [0, 0, 0, 0, 1],
            &taken,
            crate::time::AsOf::parse("2026-10-09").unwrap(),
        )
    }

    #[test]
    fn r_lc_021__new_refuses_reserved_id() {
        let mut facts = facts_empty();
        facts.tombstones.insert(
            rp("fairlead/tombstones/2026.toml"),
            "[[tomb]]\nid = \"tok.old\"\nremoved = \"0.3.0\"\nwhy = \"x\"\n\n[[tomb]]\nid = \"rule.no-todo\"\nremoved = \"2026-10-11\"\nwhy = \"TODO placeholder\"\ndeleted-by = \"sync\"\n".into(),
        );
        let (_root, reg) = registry_with(
            &[("fairlead/control.toml", &format!("{ROOT_CONTROL}{TOK}"))],
            "2026-10-09",
            facts,
        );
        let mut r = nr("tok", "old");
        r.fields.insert("role".into(), "brand".into());
        let mut rule = nr("rule", "no-todo");
        rule.fields.insert("forbid".into(), "TODO".into());
        // 0.2.2-T3: a rule is a control-table item; its removed ID is reserved too.
        for r in [r, rule] {
            let e = new_plan(
                &r,
                &reg,
                &facts_empty(),
                crate::time::AsOf::parse("2026-10-09").unwrap(),
            )
            .unwrap_err();
            assert!(
                matches!(&e, Error::Refused(f) if f.kind == RefusalKind::NoClobber && f.message.contains("reserved forever")),
                "{e:?}"
            );
        }
    }

    #[test]
    fn r_ver_012__new_change_is_a_draft_created_at_the_open_version() {
        let mut r = nr("change", "Adds diff v2");
        r.fields.insert("impact".into(), "add".into());
        let p = change(&r, &[]).unwrap();
        let path = "fairlead/docs/changes/d-00000001-adds-diff-v2.md";
        let t = text(&p, path);
        assert!(
            t.starts_with("+++\nid = \"d-00000001\"\nkind = \"change\"\nstatus = \"draft\"\nimpact = \"add\"\nscope = \"root\"\ntitle = \"Adds diff v2\"\ncovers = []\ncreated = \"1.2.0\"\n+++\n"),
            "{t}"
        );
        let toml_part = t
            .strip_prefix("+++\n")
            .and_then(|r| r.split_once("+++\n"))
            .map(|(fm, _)| fm)
            .unwrap();
        let table: toml::Table = toml_part.parse().unwrap();
        let d = crate::docs::parse_change_doc(&rp(path), &table)
            .unwrap()
            .unwrap();
        assert_eq!(d.created.as_deref(), Some("1.2.0"));
        assert_eq!(
            t.matches("created").count(),
            1,
            "the only stamp (R-VER-013)"
        );
    }

    #[test]
    fn r_ver_040__a_draft_id_already_in_the_tree_is_refused() {
        let e = change(&nr("change", "x"), &["d-00000001"]).unwrap_err();
        assert!(
            matches!(&e, Error::Refused(r) if r.kind == RefusalKind::NoClobber),
            "{e}"
        );
    }

    #[test]
    fn r_ver_031__impact_must_be_a_known_word() {
        let mut r = nr("change", "x");
        r.fields.insert("impact".into(), "huge".into());
        assert!(matches!(change(&r, &[]), Err(Error::Usage(_))));
    }

    #[test]
    fn r_cli_030__doc_kind_scaffold_has_frontmatter_def_line_and_created_stamp() {
        let p = plan("", &nr("adr", "use-css-vars")).unwrap();
        let t = text(&p, "fairlead/docs/adr/use-css-vars.md");
        assert!(t.starts_with("+++\ntitle = \"use-css-vars\"\ncreated = \"2026-10-09\"\ndirectives = [\n  'def adr.use-css-vars',\n]\n+++\n"), "{t}");
        assert_eq!(
            p.set.writes[&rp("fairlead/docs/adr/use-css-vars.md")].kind,
            WriteKind::Create
        );
        assert!(!t.contains('\r'));
    }

    #[test]
    fn r_cli_031__mandatory_slots_carry_the_placeholder_and_are_listed() {
        let p = plan(TOK, &nr("tok", "primary")).unwrap();
        let t = text(&p, "fairlead/contract/tokens/primary.md");
        assert!(
            t.contains("'def tok.primary role=\"TODO(fairlead)\"'"),
            "{t}"
        );
        assert!(p.notes.iter().any(|n| n.contains("role")));
        let mut r = nr("tok", "primary");
        r.fields.insert("role".into(), "brand".into());
        assert!(
            text(
                &plan(TOK, &r).unwrap(),
                "fairlead/contract/tokens/primary.md"
            )
            .contains("role=brand")
        );
        let h = plan("", &nr("handoff", "2026-10-09-sync")).unwrap();
        assert!(
            text(&h, "fairlead/docs/handoff/2026-10-09-sync.md")
                .contains("'def handoff.2026-10-09-sync sum=\"TODO(fairlead)\"'")
        );
    }

    #[test]
    fn r_file_010__no_per_file_version_header_only_the_created_stamp() {
        let t = text(
            &plan("", &nr("adr", "x")).unwrap(),
            "fairlead/docs/adr/x.md",
        );
        assert_eq!(t.matches("created").count(), 1);
        for banned in ["version", "sealed", "rev ="] {
            assert!(!t.contains(banned), "{banned}");
        }
    }

    #[test]
    fn r_cli_029__bad_names_are_rejected_before_any_write() {
        for bad in ["con", "Button", "..", "a/b", "nul", "a b", &"x".repeat(65)] {
            assert!(
                matches!(plan("", &nr("adr", bad)), Err(Error::Usage(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn r_cli_028__print_returns_the_scaffold_and_writes_nothing() {
        let mut r = nr("adr", "x");
        r.print = true;
        let p = plan("", &r).unwrap();
        assert!(p.set.is_empty() && scaffold_text(&p).starts_with("+++\ntitle = \"x\""));
    }

    #[test]
    fn r_cli_030__unknown_code_shaped_and_reserved_kinds_are_usage_errors() {
        match plan("", &nr("zzz", "x")) {
            Err(Error::Usage(m)) => assert!(m.contains("adr") && m.contains("handoff"), "{m}"),
            o => panic!("{:?}", o.map(|p| p.notes)),
        }
        assert!(
            matches!(plan("", &nr("cmp", "x")), Err(Error::Usage(m)) if m.contains("code-shaped"))
        );
        assert!(matches!(plan("", &nr("exc", "x")), Err(Error::Usage(_))));
        assert!(
            matches!(plan("", &nr("approval", "x")), Err(Error::Usage(m)) if m.contains("fairlead approve"))
        );
        assert!(matches!(
            plan("", &nr("revive", "x")),
            Err(Error::Reserved {
                milestone: "0.2",
                ..
            })
        ));
        assert!(
            matches!(plan("", &nr("change", "x")), Err(Error::Internal(m)) if m.contains("plan_new")),
            "change docs go through pipeline::plan_new"
        );
        assert!(matches!(
            plan("", &nr("request", "x")),
            Err(Error::Reserved {
                milestone: "0.3",
                ..
            })
        ));
    }

    #[test]
    fn r_cli_028__unknown_fields_are_rejected() {
        let mut r = nr("adr", "x");
        r.fields.insert("bogus".into(), "1".into());
        assert!(matches!(plan("", &r), Err(Error::Usage(m)) if m.contains("allowed")));
    }

    #[test]
    fn r_file_032__token_scaffold_may_print_an_optional_css_stub() {
        let mut r = nr("tok", "primary");
        r.fields.insert("role".into(), "brand".into());
        r.fields.insert("css".into(), "app/globals.css".into());
        let p = plan(TOK, &r).unwrap();
        assert!(
            p.notes
                .iter()
                .any(|n| n.contains("app/globals.css") && n.contains("--primary: TODO(fairlead);"))
        );
        assert!(!text(&p, "fairlead/contract/tokens/primary.md").contains("css="));
    }

    #[test]
    fn r_scope_010__a_doc_never_creates_a_scope_or_control_file_and_validates() {
        let (root, reg) = env("");
        let p = new_plan(
            &nr("adr", "x"),
            &reg,
            &facts_empty(),
            crate::time::AsOf::parse("2026-10-09").unwrap(),
        )
        .unwrap();
        assert!(
            p.set
                .writes
                .keys()
                .all(|k| !k.as_str().ends_with("control.toml"))
        );
        p.set.validate(&root).unwrap();
        assert_eq!(p.lines[0].action, PlanAction::Create);
        let mut inside = nr("adr", "y");
        inside.scope = Some(crate::registry::ScopeId(
            fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
        ));
        assert!(
            plan("", &inside)
                .unwrap()
                .set
                .writes
                .contains_key(&rp("apps/web/fairlead/docs/adr/y.md"))
        );
    }

    fn scope_req(id: &str, at: Option<&str>) -> NewRequest {
        let mut r = nr("scope", id);
        r.at = at.map(rp);
        r
    }

    #[test]
    fn r_scope_050__new_scope_writes_only_a_header_only_control_file() {
        let p = plan("", &scope_req("ui", Some("apps/ui"))).unwrap();
        assert_eq!(p.set.writes.len(), 1);
        assert_eq!(
            text(&p, "apps/ui/fairlead/control.toml"),
            "[fairlead]\nscope = \"ui\"\n"
        );
        assert!(
            p.notes.iter().any(|n| n.contains("fairlead sync")),
            "{:?}",
            p.notes
        );
        let mut with_purpose = scope_req("ui", Some("apps/ui"));
        with_purpose
            .fields
            .insert("purpose".into(), "UI kit".into());
        assert!(
            text(
                &plan("", &with_purpose).unwrap(),
                "apps/ui/fairlead/control.toml"
            )
            .ends_with("\n[settings]\npurpose = \"UI kit\"\n")
        );
    }

    #[test]
    fn r_scope_010__default_dir_is_under_the_parent_and_duplicates_refuse() {
        let p = plan("", &scope_req("ui", None)).unwrap();
        assert!(p.set.writes.contains_key(&rp("ui/fairlead/control.toml")));
        assert!(
            matches!(plan("", &scope_req("web", Some("elsewhere"))), Err(Error::Refused(f)) if f.kind == RefusalKind::NoClobber)
        );
        assert!(
            matches!(plan("", &scope_req("other", Some("apps/web"))), Err(Error::Refused(f)) if f.kind == RefusalKind::NoClobber)
        );
        assert!(
            matches!(
                plan("", &scope_req("other", Some("Apps/Web"))),
                Err(Error::Refused(_))
            ),
            "case-fold collision"
        );
    }

    #[test]
    fn r_cli_029__scope_ids_and_dirs_are_validated() {
        for (id, at) in [
            ("Con", None),
            ("con", None),
            ("a_b", None),
            ("ok", Some("aux/x")),
            ("ok", Some("a/prn")),
        ] {
            assert!(
                matches!(plan("", &scope_req(id, at)), Err(Error::Usage(_))),
                "{id} {at:?}"
            );
        }
    }

    #[test]
    fn r_cli_030__depth_beyond_three_warns_without_refusing() {
        let controls = [
            ("fairlead/control.toml", ROOT_CONTROL),
            ("a/fairlead/control.toml", "[fairlead]\nscope = \"a\"\n"),
            ("a/b/fairlead/control.toml", "[fairlead]\nscope = \"b\"\n"),
            ("a/b/c/fairlead/control.toml", "[fairlead]\nscope = \"c\"\n"),
        ];
        let (_r, reg) = registry(&controls, "2026-10-09");
        let p = new_plan(
            &scope_req("d", Some("a/b/c/d")),
            &reg,
            &facts_empty(),
            crate::time::AsOf::parse("2026-10-09").unwrap(),
        )
        .unwrap();
        assert!(
            p.notes
                .iter()
                .any(|n| n.starts_with("warning: scope depth 4 exceeds 3")),
            "{:?}",
            p.notes
        );
        let shallow = new_plan(
            &scope_req("d", Some("a/d")),
            &reg,
            &facts_empty(),
            crate::time::AsOf::parse("2026-10-09").unwrap(),
        )
        .unwrap();
        assert!(!shallow.notes.iter().any(|n| n.starts_with("warning")));
    }

    #[test]
    fn r_scope_013__hoist_groups_identical_file_local_ovrs_across_files() {
        let mut v = vec![
            ovr_fixture("apps/ui/a.tsx", 3, "chart lib"),
            ovr_fixture("apps/ui/b.tsx", 9, "chart lib"),
            ovr_fixture("apps/ui/c.tsx", 1, "chart lib"),
            ovr_fixture("apps/ui/d.tsx", 5, "different reason"),
            ovr_fixture("apps/other/e.tsx", 1, "chart lib"),
        ];
        let mut in_control = ovr_fixture("apps/ui/fairlead/control.toml", 4, "chart lib");
        in_control.applies = Some("**".into());
        v.push(in_control);
        let groups = hoist_groups(&v, "apps/ui/", 3);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].sites.len(), 3);
        let b = hoist_block(&groups[0]);
        let tgt = item_id("root", "rule.no-raw-hex").to_id().to_string();
        assert!(
            b.starts_with(&format!(
                "\n[[ovr]]\ntarget = \"{tgt}\"\nop = \"loosen\"\napplies = \"**\"\n"
            )),
            "{b}"
        );
        assert!(
            b.contains("why = \"chart lib\"")
                && b.contains("owner = \"@lead\"")
                && b.contains("exp = \"2026-12-31\"")
                && b.contains("[ovr.set]\nsev = \"warn\"\n")
        );
        assert!(
            hoist_groups(&v, "apps/ui/", 4).is_empty(),
            "needs min-files distinct files"
        );
    }

    #[test]
    fn r_chk_009__rule_stub_is_observe_with_placeholder_why_and_a_cas_plain_append() {
        let mut r = nr("rule", "no-raw-hex");
        r.fields.insert("forbid".into(), "#[0-9a-f]{6}".into());
        r.fields.insert("msg".into(), "use tokens".into());
        let p = plan("", &r).unwrap();
        let w = &p.set.writes[&rp("fairlead/control.toml")];
        assert_eq!(
            w.kind,
            WriteKind::AppendBlock {
                marker: String::new()
            }
        );
        assert!(
            w.expect_prior.is_some(),
            "compare-and-swap against the planned control file"
        );
        let appended = String::from_utf8(w.bytes.clone()).unwrap();
        let whole = format!("{ROOT_CONTROL}{appended}");
        let t: toml::Table = toml::from_str(&whole).unwrap();
        let rule = &t["rules"]["no-raw-hex"];
        assert_eq!(
            (
                rule["stage"].as_str(),
                rule["why"].as_str(),
                rule["check"].as_str()
            ),
            (Some("observe"), Some("TODO(fairlead)"), Some("pattern"))
        );
        assert_eq!(rule["forbid"].as_str(), Some("#[0-9a-f]{6}"));
        assert_eq!(rule["since"].as_str(), Some("2026-10-09"));
        assert!(
            crate::config::parse_control(&rp("fairlead/control.toml"), whole.as_bytes(), true)
                .is_ok()
        );
    }

    #[test]
    fn r_cli_028__rule_refuses_existing_names_and_validates_fields() {
        let dup = "\n[rules.dup]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"m\"\nwhy = \"w\"\n";
        assert!(
            matches!(plan(dup, &nr("rule", "dup")), Err(Error::Refused(f)) if f.kind == RefusalKind::NoClobber)
        );
        let mut r = nr("rule", "x");
        r.fields.insert("check".into(), "builtin".into());
        assert!(matches!(plan("", &r), Err(Error::Usage(_))));
        let mut r = nr("rule", "x");
        r.fields.insert("stage".into(), "enforce".into());
        assert!(
            matches!(plan("", &r), Err(Error::Usage(m)) if m.contains("allowed")),
            "stage is always observe"
        );
        let mut leak = nr("rule", "no-leak");
        leak.fields.insert("check".into(), "leak".into());
        leak.fields.insert("tokens".into(), "tok.*,tok.x".into());
        assert!(
            text(&plan("", &leak).unwrap(), "fairlead/control.toml")
                .contains("tokens = [\"tok.*\", \"tok.x\"]")
        );
    }

    #[test]
    fn r_scope_053__escalation_is_an_inert_req_addressed_to_an_ancestor() {
        let mut r = nr("escalation", "layer-ranks.widget");
        r.scope = Some(crate::registry::ScopeId(
            fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
        ));
        r.fields.insert("set".into(), "2".into());
        r.fields.insert("why".into(), "console needs a rank".into());
        let p = plan("", &r).unwrap();
        let t = text(&p, "apps/web/fairlead/control.toml");
        assert_eq!(
            t,
            "\n[[req]]\nto = \"root\"\ntarget = \"layer-ranks.widget\"\nset = \"2\"\nwhy = \"console needs a rank\"\n"
        );
        let whole = format!("{WEB_CONTROL}{t}");
        assert!(
            crate::config::parse_control(
                &rp("apps/web/fairlead/control.toml"),
                whole.as_bytes(),
                false
            )
            .is_ok()
        );
    }

    #[test]
    fn r_scope_053__escalation_needs_a_non_root_scope_and_an_ancestor_target() {
        assert!(
            matches!(plan("", &nr("escalation", "x.y")), Err(Error::Usage(m)) if m.contains("root"))
        );
        let web = Some(crate::registry::ScopeId(
            fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
        ));
        let mut r = nr("escalation", "x.y");
        r.scope = web.clone();
        r.fields.insert("to".into(), "web".into());
        assert!(matches!(plan("", &r), Err(Error::Usage(m)) if m.contains("ancestor")));
        let mut dflt = nr("escalation", "x.y");
        dflt.scope = web;
        assert!(
            text(&plan("", &dflt).unwrap(), "apps/web/fairlead/control.toml")
                .contains("why = \"TODO(fairlead)\"")
        );
    }

    #[test]
    fn d_20__presets_are_content() {
        let ui = plan("", &nr("ladder", "ui")).unwrap();
        let t = text(&ui, "fairlead/control.toml");
        assert!(
            t.contains(
                "[layers.ui]
atom = 10
"
            ) && t.contains("[free.chrome]"),
            "{t}"
        );
        let whole = format!("{ROOT_CONTROL}{t}{LADDER_CORE}");
        let c = crate::config::parse_control(&rp("fairlead/control.toml"), whole.as_bytes(), true)
            .unwrap();
        assert_eq!(c.hier.rank("core.service"), Some(("core", 50)));
        assert_eq!(c.hier.entry_only, ["core.shell"]);
        assert!(matches!(
            plan("[layers.ui]
atom = 10
", &nr("ladder", "ui")),
            Err(Error::Refused(f)) if f.kind == RefusalKind::NoClobber
        ));
        assert!(
            matches!(plan("", &nr("ladder", "web")), Err(Error::Usage(m)) if m.contains("presets"))
        );
        let mut child = nr("ladder", "ui");
        child.scope = Some(crate::registry::ScopeId(
            fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
        ));
        assert!(matches!(plan("", &child), Err(Error::Usage(m)) if m.contains("root")));
    }

    #[test]
    fn r_scope_053__escalation_names_an_existing_item_or_a_vocabulary_key() {
        let web = || {
            Some(crate::registry::ScopeId(
                fairlead_syntax::ids::ScopeSlug::parse("web").unwrap(),
            ))
        };
        let rule = "\n[rules.r]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"no x\"\n";
        let mut label = nr("escalation", "allow-dbg-demo");
        label.scope = web();
        assert!(
            matches!(plan(rule, &label), Err(Error::Usage(m)) if m.contains("kind.name")),
            "a label is not a target"
        );
        let mut missing = nr("escalation", "rule.nope");
        missing.scope = web();
        assert!(
            matches!(plan(rule, &missing), Err(Error::Usage(m)) if m.contains("no item rule.nope"))
        );
        let mut ok = nr("escalation", "rule.r");
        ok.scope = web();
        assert!(
            text(&plan(rule, &ok).unwrap(), "apps/web/fairlead/control.toml")
                .contains("target = \"rule.r\"")
        );
    }

    #[test]
    fn r_ctx_008__handoff_has_the_created_stamp_that_orders_it_and_a_mandatory_sum() {
        let p = plan("", &nr("handoff", "sync-engine")).unwrap();
        let t = text(&p, "fairlead/docs/handoff/sync-engine.md");
        assert!(t.contains("created = \"2026-10-09\"") && t.contains("sum=\"TODO(fairlead)\""));
        assert!(p.notes.iter().any(|n| n.contains("sum")));
    }

    use crate::registry::ScopeId;
    fn sid(s: &str) -> ScopeId {
        ScopeId(fairlead_syntax::ids::ScopeSlug::parse(s).unwrap())
    }

    #[test]
    fn r_scope_014__demote_appends_a_tombstone_with_alias_path_and_date() {
        let (root, reg) = env("");
        let p = plan_demote(&reg, &sid("web")).unwrap();
        assert_eq!(p.verb, WriteSetKind::Sync);
        let t = text(&p, "fairlead/tombstones/2026.toml");
        assert_eq!(
            t,
            "\n[[tomb]]\nid = \"web\"\nremoved = \"2026-10-09\"\nby = \"demote\"\nwhy = \"scope demoted into root\"\npaths = [\"apps/web\"]\n"
        );
        assert_eq!(
            p.set.writes[&rp("fairlead/tombstones/2026.toml")].kind,
            WriteKind::AppendBlock {
                marker: String::new()
            }
        );
        p.set.validate(&root).unwrap();
        assert!(
            p.notes
                .iter()
                .any(|n| n.contains("applies = \"apps/web/**\""))
        );
        assert_eq!(
            plan_demote(&reg, &sid("web")).unwrap().plan_hash,
            p.plan_hash,
            "deterministic"
        );
    }

    #[test]
    fn r_scope_014__root_and_unknown_scopes_cannot_be_demoted() {
        let (_r, reg) = env("");
        assert!(matches!(
            plan_demote(&reg, &sid("root")),
            Err(Error::Usage(_))
        ));
        assert!(matches!(
            plan_demote(&reg, &sid("nope")),
            Err(Error::Usage(_))
        ));
    }

    #[test]
    fn r_scope_014__demotion_that_duplicates_an_ancestor_id_is_refused() {
        // Verify: assumes P4 keeps both items (home root and home web); otherwise build the clash from HN-R005.
        let dup = "\n[rules.dup]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"m\"\nwhy = \"w\"\n";
        let (_r, reg) = registry(
            &[
                ("fairlead/control.toml", &format!("{ROOT_CONTROL}{dup}")),
                (
                    "apps/web/fairlead/control.toml",
                    &format!("{WEB_CONTROL}{dup}"),
                ),
            ],
            "2026-10-09",
        );
        match plan_demote(&reg, &sid("web")) {
            Err(Error::Refused(f)) => {
                assert_eq!(f.kind, RefusalKind::NoClobber);
                assert!(f.message.contains("rule.dup"));
            }
            other => panic!("{:?}", other.map(|p| p.notes)),
        }
    }
}
