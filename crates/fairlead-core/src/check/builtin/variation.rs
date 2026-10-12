//! The variation protocol (31-variation-protocol): variants and axes, interim wrappers,
//! adapters and external libraries, requests and their answers, and fork suspects.
//!
//! Own codes: `HN-W001` (`var.request-stale`), `HN-W002` (`var.adapter-bypass`), `HN-W003`
//! (`var.fork-suspect`), `HN-W004` (`var.axis-suggest`). The protocol's other violations reuse
//! the table's codes (errata E3-26): a dangling ID is `HN-R001`, a malformed declaration
//! `HN-S016`, a malformed request or answer doc `HN-S004`, `variant-of` outside the base's scope
//! `HN-C013`, an unapproved answer or an unrequested variant `HN-X002`, an expiring or invalid
//! interim wrapper `HN-X001` (ramped), a request matching the graveyard `HN-L002`.
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::directive::{ArgValue, Verb};
use fairlead_syntax::ids::Id;

use crate::check::builtin::{bare_id, field_str, origin_loc, root_control};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::classify::FileClass;
use crate::config::ForkDetect;
use crate::diag::{Diagnostic, codes::*};
use crate::path::RepoPath;
use crate::registry::{Item, ItemId, Lookup, Registry, ScopeId};
use crate::variation::{self as var, Answer, ReqStatus};
use crate::version::Version;

pub struct VariationCheck;
const CODES: [DiagCode; 12] = [
    HN_W001, HN_W002, HN_W003, HN_W004, HN_R001, HN_S016, HN_S004, HN_C013, HN_X001, HN_X002,
    HN_L002, HN_X011,
];

/// R-RET-050: the ledger summary line of a compacted record a doc reference names.
fn compacted_ref(cx: &CheckCx<'_>, raw: &str) -> Option<String> {
    let (scope, id) = match raw.split_once(':') {
        Some((s, d)) => (Some(s), d),
        None => (None, raw),
    };
    crate::retention::compacted(&cx.facts.ledgers)
        .remove(id)
        .filter(|c| scope.is_none_or(|s| s == c.scope))
        .map(|c| c.line)
}

/// R-PORT-024: whether a doc reference names this request.
fn req_named(reg: &Registry, r: &var::Request, raw: &str) -> bool {
    r.id.as_ref()
        .is_some_and(|i| crate::docs::doc_ref_is(raw, i, var::scope_of(reg, &r.path).0.as_str()))
}

fn resolve(reg: &Registry, from: &ScopeId, raw: &str) -> Option<ItemId> {
    let id = Id::parse(raw).ok()?;
    match reg.lookup(from, &id) {
        Lookup::Found(i) => Some(i),
        _ => None,
    }
}

fn truthy(it: &Item, k: &str) -> bool {
    matches!(field_str(it, k), Some("true" | "1" | "yes"))
}

/// The files an item stands for: its declaration file and its sites.
fn files_of(reg: &Registry, i: &ItemId) -> BTreeSet<RepoPath> {
    let mut out: BTreeSet<RepoPath> = reg.sites_of(i).iter().map(|s| s.path.clone()).collect();
    if let Some(it) = reg.item(i) {
        out.insert(origin_loc(it).0);
    }
    out
}

/// Whether a `use` of `target` is written in one of `files`.
fn uses_from(reg: &Registry, files: &BTreeSet<RepoPath>, target: &ItemId) -> bool {
    reg.edges()
        .iter()
        .any(|e| e.resolved.as_ref() == Some(target) && files.contains(&e.from.path))
}

fn dangling(p: &RepoPath, l: u32, c: u32, what: String, raw: &str) -> Diagnostic {
    DiagB::new(HN_R001, p, l, c, what)
        .hint("fix the ID or declare it (31-variation-protocol)")
        .text(raw.to_owned())
        .build()
}

fn malformed(p: &RepoPath, l: u32, c: u32, what: String, text: String, hint: &str) -> Diagnostic {
    DiagB::new(HN_S016, p, l, c, what)
        .hint(hint)
        .text(text)
        .build()
}

/// R-VAR-061: info two minors ahead of `target`, warn one minor ahead (due at the next minor),
/// error once the open version is at or past it. `None` when not yet near.
fn ramp(open: Version, target: Version) -> Option<Severity> {
    if open >= target {
        return Some(Severity::Error);
    }
    if target.major != open.major {
        return None;
    }
    match target.minor.saturating_sub(open.minor) {
        0 | 1 => Some(Severity::Warn),
        2 => Some(Severity::Info),
        _ => None,
    }
}

impl Check for VariationCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(root) = root_control(reg) else {
            return;
        };
        let vcfg = &root.variation;
        let hcfg = &root.hier;
        let mine: Vec<&Item> = reg.items().filter(|it| it.id.scope == *scope).collect();
        let (docs, _) = crate::docs::collect_change_docs(cx.corpus);
        let answers = var::answers(&docs, &cx.facts.approvals);
        let requests = var::collect_requests(cx.corpus);
        let awaiting = var::awaiting(reg, cx.corpus);

        variants(cx, &mine, vcfg, hcfg, &answers, out);
        wrappers_and_adapters(cx, &mine, &docs, out);
        bypass(cx, scope, out);
        requests_of(cx, scope, &requests, &answers, &awaiting, vcfg, out);
        answers_of(cx, scope, &requests, &answers, out);
        awaiting_of(cx, scope, &requests, &answers, &awaiting, out);
        sprawl(cx, scope, &awaiting, root.exceptions.max_live_per_rule, out);
        forks(cx, scope, vcfg.fork_detect, out);
    }
}

/// R-VAR-012, R-VAR-020, R-VAR-050, R-VAR-051, R-VAR-053.
fn variants(
    cx: &CheckCx<'_>,
    mine: &[&Item],
    vcfg: &crate::config::VariationConfig,
    hcfg: &crate::config::HierConfig,
    answers: &[Answer<'_>],
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    // every variant of the tree by base, for the axis suggestion and `native=` uniqueness
    let mut by_base: BTreeMap<ItemId, Vec<&Item>> = BTreeMap::new();
    for it in reg.items() {
        if let Some(b) = field_str(it, "variant-of").and_then(|raw| resolve(reg, &it.id.scope, raw))
        {
            by_base.entry(b).or_default().push(it);
        }
    }
    for it in mine {
        let Some(raw) = field_str(it, "variant-of") else {
            continue;
        };
        let (p, l, c) = origin_loc(it);
        let Some(base) = resolve(reg, &it.id.scope, raw) else {
            out.push(dangling(
                &p,
                l,
                c,
                format!(
                    "{} is variant-of {raw}, which is not declared",
                    bare_id(&it.id)
                ),
                raw,
            ));
            continue;
        };
        if base.scope != it.id.scope {
            out.push(
                DiagB::new(
                    HN_C013,
                    &p,
                    l,
                    c,
                    format!(
                        "{} is variant-of {}, whose home is scope {}: a variant is declared only in the base's own scope",
                        bare_id(&it.id),
                        bare_id(&base),
                        base.scope.0.as_str()
                    ),
                )
                .hint("request the variant from the home scope (fairlead new request), or wrap it locally with wraps= (R-VAR-012)")
                .text(bare_id(&it.id))
                .build(),
            );
            continue;
        }
        let Some(b) = reg.item(&base) else { continue };
        let name = bare_id(&it.id);
        if it.id.kind != base.kind {
            out.push(malformed(
                &p,
                l,
                c,
                format!(
                    "variant {name} is a {}, its base {} a {}",
                    it.id.kind.as_str(),
                    bare_id(&base),
                    base.kind.as_str()
                ),
                name.clone(),
                "a variant inherits its base's kind (R-VAR-050)",
            ));
        }
        if field_str(it, "layer").is_some() && field_str(it, "layer") != field_str(b, "layer") {
            out.push(malformed(
                &p,
                l,
                c,
                format!(
                    "variant {name} is in layer {}, its base in {}",
                    field_str(it, "layer").unwrap_or("-"),
                    field_str(b, "layer").unwrap_or("-")
                ),
                name.clone(),
                "a variant inherits its base's layer (R-VAR-050)",
            ));
        }
        let (vc, bc) = (field_str(it, "container"), field_str(b, "container"));
        if let (Some(vc), Some(bc)) = (vc, bc)
            && vc != bc
        {
            let parent = |c: &str| {
                resolve(reg, &it.id.scope, c)
                    .and_then(|i| reg.item(&i))
                    .and_then(|i| field_str(i, "parent").map(str::to_owned))
            };
            if parent(vc).is_none() || parent(vc) != parent(bc) {
                out.push(malformed(&p, l, c, format!("variant {name} is in container {vc}, neither its base's {bc} nor a sibling of it"), name.clone(), "share the base's container or declare a sibling container (R-VAR-050)"));
            }
        }
        if !uses_from(reg, &files_of(reg, &it.id), &base) {
            out.push(malformed(
                &p,
                l,
                c,
                format!("variant {name} has no `use {}`", bare_id(&base)),
                name.clone(),
                "add `use <base>` where the variant is written (R-VAR-050)",
            ));
        }
        // R-VAR-020: a variation of a request-layer unit, or of one marked request=required, is
        // answered by a change doc.
        let gated = field_str(b, "request") == Some("required")
            || field_str(b, "layer")
                .is_some_and(|ly| vcfg.request_layers.iter().any(|pat| hcfg.matches(pat, ly)));
        if gated {
            let named = |a: &&Answer<'_>| {
                ["result", "adds", "touches"].iter().any(|k| {
                    a.doc.extra.get(*k).is_some_and(|v| {
                        crate::lifecycle::doc_ids(v)
                            .iter()
                            .any(|x| *x == name || *x == bare_id(&base))
                    })
                })
            };
            if !answers.iter().any(|a| named(&a) && !a.rejected()) {
                out.push(
                    DiagB::new(HN_X002, &p, l, c, format!("variant {name} of {} goes through a request: no change doc answers one for it", bare_id(&base)))
                        .hint("the consumer files `fairlead new request`; the home scope answers it with a change doc listing the variant in result, adds or touches (R-VAR-020)")
                        .text(name.clone())
                        .build(),
                );
            }
        }
    }
    for (base, vs) in &by_base {
        // R-VAR-053: native spellings are unique among one base's variants
        let mut seen: BTreeMap<&str, &Item> = BTreeMap::new();
        for v in vs.iter().filter(|v| v.id.scope == base.scope) {
            if let Some(n) = field_str(v, "native")
                && let Some(first) = seen.insert(n, v)
                && mine.iter().any(|m| m.id == v.id)
            {
                let (p, l, c) = origin_loc(v);
                out.push(malformed(
                    &p,
                    l,
                    c,
                    format!(
                        "native={n} of {} is already the spelling of {}",
                        bare_id(&v.id),
                        bare_id(&first.id)
                    ),
                    bare_id(&v.id),
                    "native spellings are unique among one unit's variants (R-VAR-053)",
                ));
            }
        }
        // R-VAR-051: more than V variants suggests an axis
        if mine.iter().any(|m| m.id == *base)
            && vs.len() > vcfg.axis_after as usize
            && let Some(b) = reg.item(base)
        {
            let (p, l, c) = origin_loc(b);
            out.push(
                DiagB::new(
                    HN_W004,
                    &p,
                    l,
                    c,
                    format!(
                        "{} has {} variants: consider an axis",
                        bare_id(base),
                        vs.len()
                    ),
                )
                .hint(format!(
                    "declare an axis of {} with the variants as its values (R-VAR-051)",
                    bare_id(base)
                ))
                .text(bare_id(base))
                .build(),
            );
        }
    }
}

/// R-VAR-021, R-VAR-070.
fn wrappers_and_adapters(
    cx: &CheckCx<'_>,
    mine: &[&Item],
    docs: &[crate::docs::ChangeDoc],
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    for it in mine {
        let (p, l, c) = origin_loc(it);
        let name = bare_id(&it.id);
        if let Some(raw) = field_str(it, "wraps") {
            match resolve(reg, &it.id.scope, raw) {
                None => out.push(dangling(
                    &p,
                    l,
                    c,
                    format!("{name} wraps {raw}, which is not declared"),
                    raw,
                )),
                Some(t) => {
                    if t.kind.as_str() != "ext" && !uses_from(reg, &files_of(reg, &it.id), &t) {
                        out.push(malformed(&p, l, c, format!("{name} wraps {} but has no `use {}`", bare_id(&t), bare_id(&t)), name.clone(), "a wrapper composes the unit's public surface: add `use <id>` (R-VAR-021)"));
                    }
                }
            }
        }
        if truthy(it, "adapter") {
            let rejected = field_str(it, "rejected-by");
            let wraps = field_str(it, "wraps");
            if rejected.is_none() && wraps.is_none() {
                out.push(malformed(&p, l, c, format!("adapter {name} declares neither rejected-by= nor wraps="), name.clone(), "an adapter links to the rejecting change doc (rejected-by=) or wraps an ext (R-VAR-070)"));
            }
            if let Some(r) = rejected {
                match docs.iter().find(|d| {
                    crate::docs::doc_ref_is(r, &d.id, var::scope_of(reg, &d.path).0.as_str())
                }) {
                    // R-RET-050: a compacted rejection resolves to its ledger summary line
                    None if compacted_ref(cx, r)
                        .is_some_and(|line| line.contains("out=rejected")) => {}
                    None => out.push(dangling(
                        &p,
                        l,
                        c,
                        format!("adapter {name} is rejected-by {r}, which is no change doc"),
                        r,
                    )),
                    Some(d) => {
                        let rej = d.extra.get("result").map(String::as_str) == Some("rejected")
                            || d.extra.get("type").map(String::as_str) == Some("rejected");
                        if !rej {
                            out.push(malformed(&p, l, c, format!("adapter {name} is rejected-by {r}, which is not a rejection"), name.clone(), "rejected-by names a change doc with result = \"rejected\" (R-VAR-070)"));
                        }
                    }
                }
            }
        }
    }
}

/// R-VAR-072, R-VAR-074: an `ext` import specifier outside the adapter is `var.adapter-bypass`.
fn bypass(cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let exts: Vec<(&Item, &str)> = reg
        .items()
        .filter(|it| it.id.kind.as_str() == "ext")
        .filter_map(|it| Some((it, field_str(it, "imports")?)))
        .collect();
    if exts.is_empty() {
        return;
    }
    for (ext, spec) in exts {
        let adapters: Vec<&Item> = reg
            .items()
            .filter(|a| {
                field_str(a, "wraps")
                    .and_then(|w| resolve(reg, &a.id.scope, w))
                    .is_some_and(|w| w == ext.id)
            })
            .collect();
        let allowed: BTreeSet<RepoPath> =
            adapters.iter().flat_map(|a| files_of(reg, &a.id)).collect();
        let import_as = adapters.iter().find_map(|a| field_str(a, "import-as"));
        for (path, rec) in cx
            .corpus
            .records
            .iter()
            .filter(|(p, r)| reg.scope_of(p) == Some(scope) && r.class == FileClass::Authored)
        {
            if allowed.contains(path) {
                continue;
            }
            for imp in rec.imports.iter().filter(|i| i.spec == spec) {
                out.push(
                    DiagB::new(
                        HN_W002,
                        path,
                        imp.line,
                        1,
                        format!(
                            "import of {spec} bypasses the adapter of {}",
                            bare_id(&ext.id)
                        ),
                    )
                    .hint(match import_as {
                        Some(a) => format!("import {a} instead (R-VAR-072)"),
                        None => {
                            "import the adapter's import-as specifier instead (R-VAR-072)".into()
                        }
                    })
                    .related(bare_id(&ext.id))
                    .text(spec.to_owned())
                    .build(),
                );
            }
        }
    }
}

/// R-VAR-030, R-VAR-032, R-VAR-042 (a request matching the graveyard).
fn requests_of(
    cx: &CheckCx<'_>,
    scope: &ScopeId,
    requests: &[var::Request],
    answers: &[Answer<'_>],
    awaiting: &[var::Awaiting],
    vcfg: &crate::config::VariationConfig,
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    let folder = root_control(reg)
        .and_then(|r| r.settings.folder.clone())
        .unwrap_or_else(|| "fairlead".into());
    let sealed = crate::docs::version_files(cx.corpus, &folder);
    let (graves, _) = crate::graveyard::collect(cx.corpus);
    for r in requests
        .iter()
        .filter(|r| var::scope_of(reg, &r.path) == scope)
    {
        let p = &r.path;
        let Some(id) = &r.id else {
            out.push(
                DiagB::new(
                    HN_S004,
                    p,
                    1,
                    1,
                    "a request needs `id`, a draft ID".to_string(),
                )
                .hint("create requests with fairlead new request (R-VAR-030)")
                .text(p.as_str().to_owned())
                .build(),
            );
            continue;
        };
        for k in r.missing() {
            out.push(
                DiagB::new(
                    HN_S004,
                    p,
                    1,
                    1,
                    format!("request {} has no `{k}`", id.as_str()),
                )
                .hint("a request needs need, want, because, sum and out (R-VAR-030)")
                .text(format!("{}:{k}", id.as_str()))
                .build(),
            );
        }
        if let Some(need) = r.field("need")
            && resolve(reg, scope, need).is_none()
        {
            out.push(dangling(
                p,
                1,
                1,
                format!(
                    "request {} needs {need}, which is not declared",
                    id.as_str()
                ),
                need,
            ));
        }
        let st = var::status(
            id,
            var::scope_of(reg, &r.path).0.as_str(),
            answers,
            awaiting,
        );
        let since = var::seals_since(r.created, &sealed);
        if st == ReqStatus::Open && since > vcfg.stale_seals as usize {
            out.push(
                DiagB::new(HN_W001, p, 1, 1, format!("request {} is open after {since} seals", id.as_str()))
                    .hint(format!("the home scope answers it, or the consumer withdraws it ([variation] stale-seals = {}, R-VAR-032)", vcfg.stale_seals))
                    .text(id.as_str().to_owned())
                    .build(),
            );
        }
        if let Some(want) = r.field("want") {
            for g in &graves {
                if !crate::graveyard::text_hits(g, want).is_empty() {
                    out.push(
                        DiagB::new(HN_L002, p, 1, 1, format!("request {} asks for what graveyard entry {} rejected", id.as_str(), g.id))
                            .hint(format!("read {} first; a rejected want is filed again only with new reasons (R-VAR-042)", g.path.as_str()))
                            .related(g.id.clone())
                            .text(id.as_str().to_owned())
                            .build(),
                    );
                }
            }
        }
    }
}

/// R-VAR-040 to R-VAR-043.
fn answers_of(
    cx: &CheckCx<'_>,
    scope: &ScopeId,
    requests: &[var::Request],
    answers: &[Answer<'_>],
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    let (graves, _) = crate::graveyard::collect(cx.corpus);
    for a in answers
        .iter()
        .filter(|a| var::scope_of(reg, &a.doc.path) == scope)
    {
        let d = a.doc;
        let p = &d.path;
        let did = d.id.as_str();
        let done = matches!(
            d.status,
            crate::docs::Status::Ready | crate::docs::Status::Sealed
        );
        for x in &a.answers {
            if !requests.iter().any(|r| req_named(reg, r, x)) && compacted_ref(cx, x).is_none() {
                out.push(dangling(
                    p,
                    1,
                    1,
                    format!("change doc {did} answers {x}, which is no request"),
                    x,
                ));
            }
        }
        match a.kind.as_deref() {
            Some(t) if var::ANSWER_TYPES.contains(&t) => {}
            other => out.push(
                DiagB::new(
                    HN_S004,
                    p,
                    1,
                    1,
                    format!(
                        "change doc {did} answers requests but its type is {}",
                        other.unwrap_or("missing")
                    ),
                )
                .hint("type is variant, axis or rejected (R-VAR-040)")
                .text(format!("{did}:type"))
                .build(),
            ),
        }
        // R-VAR-041: every scope that uses a touched unit is listed, or affects = "all".
        let touched: Vec<ItemId> = d
            .extra
            .get("touches")
            .map(|t| crate::lifecycle::doc_ids(t))
            .unwrap_or_default()
            .iter()
            .filter_map(|t| resolve(reg, scope, t))
            .collect();
        let users: BTreeSet<String> = reg
            .edges()
            .iter()
            .filter(|e| e.resolved.as_ref().is_some_and(|r| touched.contains(r)))
            .filter_map(|e| reg.scope_of(&e.from.path))
            .filter(|s| *s != scope)
            .map(|s| s.0.as_str().to_owned())
            .collect();
        let affects = d.extra.get("affects").map(String::as_str);
        if !users.is_empty() && affects != Some("all") {
            let listed: Vec<String> = affects.map(crate::lifecycle::doc_ids).unwrap_or_default();
            let missing: Vec<&String> = users.iter().filter(|u| !listed.contains(u)).collect();
            if !missing.is_empty() {
                out.push(
                    DiagB::new(HN_S004, p, 1, 1, format!("change doc {did} touches units used from {}, which affects does not list", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")))
                        .hint(format!("affects = [{}] or \"all\" (R-VAR-041)", users.iter().map(|u| format!("\"{u}\"")).collect::<Vec<_>>().join(", ")))
                        .text(format!("{did}:affects"))
                        .build(),
                );
            }
        }
        if !done {
            continue;
        }
        // R-VAR-042: a rejection leaves a graveyard entry whose terms cover the want text.
        if a.rejected() {
            for x in &a.answers {
                let Some(want) = requests
                    .iter()
                    .find(|r| req_named(reg, r, x))
                    .and_then(|r| r.field("want"))
                else {
                    continue;
                };
                if !graves
                    .iter()
                    .any(|g| !crate::graveyard::text_hits(g, want).is_empty())
                {
                    out.push(DiagB::new(HN_S004, p, 1, 1, format!("change doc {did} rejects {x}, but no graveyard entry covers its want text")).hint("add a graveyard entry whose terms match the request's want, so a repeat is caught (R-VAR-042)").text(format!("{did}:{x}")).build());
                }
            }
        }
        // R-VAR-043: the home scope's owner accepts through `fairlead approve`.
        if !a.approved {
            let fp = var::answer_fp(did, &a.answers, a.kind.as_deref().unwrap_or(""));
            out.push(
                DiagB::new(HN_X002, p, 1, 1, format!("answer {did} is not approved, so its requests stay open"))
                    .hint(format!("the home scope's owner runs `fairlead approve {did}` (fingerprint {}, R-VAR-043)", fp.as_str()))
                    .text(did.to_owned())
                    .build(),
            );
        }
    }
}

/// R-VAR-060 to R-VAR-062.
fn awaiting_of(
    cx: &CheckCx<'_>,
    scope: &ScopeId,
    requests: &[var::Request],
    answers: &[Answer<'_>],
    awaiting: &[var::Awaiting],
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    let open = crate::check::builtin::plans::due_cx(cx).map(|d| d.line.open);
    for w in awaiting
        .iter()
        .filter(|w| reg.scope_of(&w.path) == Some(scope))
    {
        let (p, l, c) = (&w.path, w.line, w.col);
        let (Some(to), Some(target)) = (&w.to, &w.target) else {
            out.push(malformed(
                p,
                l,
                c,
                "flag awaiting needs to=<request> and target=<version>".into(),
                "awaiting".into(),
                "flag awaiting to=<request id> target=<version> (R-VAR-060)",
            ));
            continue;
        };
        let Some(req) = requests.iter().find(|r| req_named(reg, r, to)) else {
            // R-RET-050, R-VAR-062: a compacted request is closed
            if compacted_ref(cx, to).is_some() {
                out.push(
                    DiagB::new(
                        HN_X001,
                        p,
                        l,
                        c,
                        format!("flag awaiting names {to}, which is closed and compacted"),
                    )
                    .hint("migrate to the answered variant and drop the flag (R-VAR-062)")
                    .severity(Severity::Error)
                    .text(to.clone())
                    .build(),
                );
                continue;
            }
            out.push(dangling(
                p,
                l,
                c,
                format!("flag awaiting names {to}, which is no request"),
                to,
            ));
            continue;
        };
        let wrapper = w
            .wrapper
            .as_ref()
            .map_or_else(|| "the wrapper".to_owned(), bare_id);
        if let Some(id) = &req.id
            && var::status(
                id,
                var::scope_of(reg, &req.path).0.as_str(),
                answers,
                awaiting,
            ) == ReqStatus::Rejected
        {
            out.push(
                DiagB::new(HN_X001, p, l, c, format!("{wrapper} awaits {to}, which was rejected"))
                    .hint("convert the wrapper to an adapter (adapter=true rejected-by=<doc>) or remove it (R-VAR-062)")
                    .severity(Severity::Error)
                    .text(to.clone())
                    .build(),
            );
            continue;
        }
        let Some(t) = Version::parse(target) else {
            out.push(malformed(
                p,
                l,
                c,
                format!("target={target} is not a version"),
                target.clone(),
                "target is MAJOR.MINOR.PATCH (R-VAR-061, D-42)",
            ));
            continue;
        };
        if let Some(sev) = open.and_then(|o| ramp(o, t)) {
            let msg = match sev {
                Severity::Error => format!("{wrapper} awaits {to} past its target {t}"),
                Severity::Warn => format!("{wrapper} awaits {to}; its target {t} is due next"),
                _ => format!("{wrapper} awaits {to}; its target {t} is near"),
            };
            out.push(
                DiagB::new(HN_X001, p, l, c, msg)
                    .hint("migrate to the answered variant and drop the flag, or retarget it in a change doc (R-VAR-061)")
                    .severity(sev)
                    .text(format!("{to}@{t}"))
                    .build(),
            );
        }
    }
}

/// R-VAR-063: more interim wrappers of one unit than `[exceptions] max-live-per-rule` is sprawl.
fn sprawl(
    cx: &CheckCx<'_>,
    scope: &ScopeId,
    awaiting: &[var::Awaiting],
    limit: u32,
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    let mut by_base: BTreeMap<ItemId, BTreeSet<ItemId>> = BTreeMap::new();
    for w in awaiting {
        let Some(wr) = &w.wrapper else { continue };
        let Some(base) = reg
            .item(wr)
            .and_then(|it| field_str(it, "wraps"))
            .and_then(|raw| resolve(reg, &wr.scope, raw))
        else {
            continue;
        };
        by_base.entry(base).or_default().insert(wr.clone());
    }
    for (base, wrappers) in by_base.iter().filter(|(b, _)| b.scope == *scope) {
        if wrappers.len() <= limit as usize {
            continue;
        }
        let Some(b) = reg.item(base) else { continue };
        let (p, l, c) = origin_loc(b);
        out.push(
            DiagB::new(
                HN_X011,
                &p,
                l,
                c,
                format!(
                    "{} has {} interim wrappers awaiting requests",
                    bare_id(base),
                    wrappers.len()
                ),
            )
            .hint("answer the requests in the home scope: a variant or an axis (R-VAR-063)")
            .text(bare_id(base))
            .build(),
        );
    }
}

/// One `ovr rule.var-fork` directive (R-VAR-080, R-VAR-082).
struct ForkOvr {
    of: String,
    dismiss: bool,
    pair: Option<String>,
}

fn fork_ovrs(cx: &CheckCx<'_>) -> Vec<ForkOvr> {
    let mut out = Vec::new();
    for rec in cx.corpus.records.values() {
        let Some(ex) = rec.extraction.as_ref() else {
            continue;
        };
        for d in ex.directives.iter().filter(|d| d.verb == Verb::Ovr) {
            if d.ids
                .first()
                .is_none_or(|i| i.kind.as_str() != "rule" || i.name.as_str() != "var-fork")
            {
                continue;
            }
            let arg = |k: &str| match &d.arg(k)?.value {
                ArgValue::Word(w) | ArgValue::Quoted(w) => Some(w.clone()),
                ArgValue::List(l) => Some(l.join(",")),
            };
            if let Some(of) = arg("of") {
                out.push(ForkOvr {
                    of,
                    dismiss: arg("dismiss").as_deref() == Some("true"),
                    pair: arg("pair"),
                });
            }
        }
    }
    out
}

/// R-VAR-082: the pair hash over the declaring locations (scope plus `sym` or basename).
pub fn pair_hash(locations: &BTreeSet<String>) -> String {
    let joined: Vec<&str> = locations.iter().map(String::as_str).collect();
    crate::hash::Fingerprint::from_hash(&crate::hash::hash_raw(joined.join("\n").as_bytes()))
        .as_str()
        .to_owned()
}

/// R-VAR-081 to R-VAR-084: the same `sym` or the same file basename declaring a unit in two
/// scopes; off unless `[variation] fork-detect` is set.
fn forks(cx: &CheckCx<'_>, scope: &ScopeId, mode: ForkDetect, out: &mut Vec<Diagnostic>) {
    if mode == ForkDetect::Off {
        return;
    }
    let reg = cx.registry;
    let authored = |p: &RepoPath| {
        cx.corpus
            .records
            .get(p)
            .is_none_or(|r| r.class == FileClass::Authored)
    };
    // key ("sym:<s>" or "file:<basename>") -> items declaring it
    let mut keyed: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for it in reg.items() {
        let (p, _, _) = origin_loc(it);
        if !authored(&p) {
            continue; // R-VAR-083
        }
        if let Some(s) = field_str(it, "sym").filter(|s| !s.starts_with("--")) {
            keyed.entry(format!("sym:{s}")).or_default().push(it);
        }
        if field_str(it, "layer").is_some() && !p.as_str().ends_with(".md") {
            keyed
                .entry(format!("file:{}", p.file_name()))
                .or_default()
                .push(it);
        }
    }
    let ovrs = fork_ovrs(cx);
    let stage = match mode {
        ForkDetect::Enforce => crate::config::Stage::Enforce,
        ForkDetect::Warn => crate::config::Stage::Warn,
        _ => crate::config::Stage::Observe,
    };
    for (key, items) in &keyed {
        let scopes: BTreeSet<&ScopeId> = items.iter().map(|i| &i.id.scope).collect();
        if scopes.len() < 2 {
            continue;
        }
        let (what, val) = key.split_once(':').unwrap_or(("", key));
        let locations: BTreeSet<String> = items
            .iter()
            .map(|i| format!("{}:{val}", i.id.scope.0.as_str()))
            .collect();
        let hash = pair_hash(&locations);
        for it in items.iter().filter(|i| i.id.scope == *scope) {
            let name = bare_id(&it.id);
            let others: Vec<String> = items
                .iter()
                .filter(|o| o.id != it.id)
                .map(|o| format!("{}:{}", o.id.scope.0.as_str(), bare_id(&o.id)))
                .collect();
            let covered = ovrs.iter().any(|o| {
                let names = |x: &Item| {
                    o.of == bare_id(&x.id)
                        || o.of == format!("{}:{}", x.id.scope.0.as_str(), bare_id(&x.id))
                };
                items.iter().any(|x| names(x))
                    && (!o.dismiss || o.pair.as_deref() == Some(hash.as_str()))
            });
            if covered {
                continue;
            }
            let (p, l, c) = origin_loc(it);
            out.push(
                DiagB::new(
                    HN_W003,
                    &p,
                    l,
                    c,
                    format!(
                        "{name} shares its {} {val} with {}: a possible fork",
                        if what == "sym" { "sym" } else { "file name" },
                        others.join(", ")
                    ),
                )
                .hint(format!("declare it (ovr rule.var-fork of={name} why= owner= until=), dismiss the pair (dismiss=true pair={hash}), or use the original (R-VAR-080, R-VAR-082)"))
                .stage(stage)
                .text(format!("{name}:{key}"))
                .build(),
            );
        }
    }
}

#[cfg(test)]
mod tests;
