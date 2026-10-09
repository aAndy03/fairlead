//! References: dangling (R001), unused (R002), too many sites (D002).
use crate::{
    check::{
        CheckCx,
        builtin::{bare_id, kind_decl, origin_loc},
        finding::DiagB,
        rules::Check,
    },
    config::ImplLimit,
    diag::{Diagnostic, codes::*},
    path::RepoPath,
    registry::{ItemId, Lookup, Registry, ScopeId, ScopeState, Site},
};
use fairlead_syntax::{
    diag::DiagCode,
    directive::{ArgValue, Directive, Verb},
    ids::{Id, IdQualifier},
};
use std::collections::BTreeSet;

pub struct RefsCheck;
const CODES: [DiagCode; 3] = [HN_R001, HN_R002, HN_D002];
/// Kinds that need no declaration in a doc (control tables, foreign regions, built-ins).
const RESERVED: [&str; 5] = ["rule", "file", "exc", "foreign", "ext"];

fn referenced(d: &Directive) -> Vec<Id> {
    let mut v: Vec<Id> = match d.verb {
        Verb::Use | Verb::Ovr | Verb::Def | Verb::Flag => d.ids.clone(),
        _ => Vec::new(),
    };
    if d.verb == Verb::Flag
        && let Some(a) = d.arg("to")
        && let ArgValue::Word(w) | ArgValue::Quoted(w) = &a.value
        && let Ok(id) = Id::parse(w)
    {
        v.push(id);
    }
    v
}

fn in_absent_scope(reg: &Registry, id: &Id) -> bool {
    match &id.qualifier {
        IdQualifier::Scope(q) => reg
            .scope(&ScopeId(q.clone()))
            .is_some_and(|s| matches!(s.state, ScopeState::AbsentFromTree)),
        _ => false,
    }
}

fn dangling(reg: &Registry, path: &RepoPath, d: &Directive, id: &Id, nearest: &[Id]) -> Diagnostic {
    let mut sugg: Vec<String> = reg
        .items()
        .filter(|it| it.id.name == id.name && it.id.kind != id.kind)
        .map(|it| format!("{}.{}", it.id.kind.as_str(), it.id.name.as_str()))
        .collect();
    sugg.extend(nearest.iter().map(|n| n.to_string()));
    let mut seen = BTreeSet::new();
    sugg.retain(|s| seen.insert(s.clone()));
    sugg.truncate(3);
    let b = DiagB::new(
        HN_R001,
        path,
        d.span.line,
        d.span.col,
        format!("unknown id {id}"),
    )
    .text(id.to_string());
    if sugg.is_empty() {
        b.hint(format!("declare {id} in a doc or control file"))
            .build()
    } else {
        b.hint(format!("did you mean {}", sugg.join(" | ")))
            .dym(sugg.join(" | "))
            .build()
    }
}

impl Check for RefsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        for (path, rec) in &cx.corpus.records {
            if reg.scope_of(path) != Some(scope) {
                continue;
            }
            let Some(ex) = rec.extraction.as_ref() else {
                continue;
            };
            for d in &ex.directives {
                for id in referenced(d) {
                    let kind = id.kind.as_str();
                    if RESERVED.contains(&kind) || kind_decl(reg, scope, kind).is_none() {
                        continue;
                    } // unknown kind is S002's
                    if let Lookup::Missing { nearest } = reg.lookup(scope, &id) {
                        // R-CHK-057: an ID qualified with a scope absent from the tree is unknown,
                        // not dangling; the absent scope itself is reported (HN-V013, exit 5).
                        if in_absent_scope(reg, &id) {
                            continue;
                        }
                        out.push(dangling(reg, path, d, &id, &nearest));
                    }
                }
            }
        }
        let used: BTreeSet<&ItemId> = reg
            .edges()
            .iter()
            .filter_map(|e| e.resolved.as_ref())
            .collect();
        for it in reg.items().filter(|i| i.id.scope == *scope) {
            let Some(kd) = kind_decl(reg, scope, it.id.kind.as_str()) else {
                continue;
            };
            let sites = reg.sites_of(&it.id);
            if let Some(sev) = kd.unused
                && !used.contains(&it.id)
                && sites.is_empty()
            {
                let (p, l, c) = origin_loc(it);
                out.push(
                    DiagB::new(
                        HN_R002,
                        &p,
                        l,
                        c,
                        format!("{} is declared but never used", bare_id(&it.id)),
                    )
                    .severity(sev)
                    .hint("add a use or a code def, or remove the declaration")
                    .text(bare_id(&it.id))
                    .build(),
                );
            }
            let limit = match kd.impl_limit {
                ImplLimit::Any => continue,
                ImplLimit::None => 0usize,
                ImplLimit::N(n) => n as usize,
            };
            let mut sorted: Vec<&Site> = sites.iter().collect();
            sorted.sort_by(|a, b| (&a.path, a.span.start).cmp(&(&b.path, b.span.start)));
            for s in sorted.into_iter().skip(limit) {
                out.push(
                    DiagB::new(
                        HN_D002,
                        &s.path,
                        s.span.line,
                        s.span.col,
                        format!(
                            "{} has more code def sites than its kind allows (limit {limit})",
                            bare_id(&it.id)
                        ),
                    )
                    .hint("remove the extra def; reference the item with use")
                    .text(bare_id(&it.id))
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
    };
    use fairlead_syntax::diag::Severity;

    const DOC: &str = "+++\ndirectives = ['def cmp.sync-button sym=SyncButton', 'def tok.a']\n+++\n# Components\n";
    fn world() -> World {
        World::new()
            .root("[kinds.cmp]\nimpl = 1\n[kinds.tok]\nunused = \"warn\"\n")
            .file("fairlead/docs/cmp.md", DOC)
    }

    #[test]
    fn r_syn_010__dangling_use_names_the_nearest_declared_id() {
        let b = world()
            .file(
                "src/a.tsx",
                "// fairlead: use cmp.sync-buton\nexport const A = 1;\n",
            )
            .build();
        let ds = b.run_check(&RefsCheck);
        let r = of(&ds, c::HN_R001);
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].location.line, r[0].did_you_mean.as_deref()),
            (1, Some("cmp.sync-button"))
        );
    }

    #[test]
    fn r_syn_010__right_name_wrong_kind_prefix_is_suggested_first() {
        let b = world()
            .file(
                "src/a.tsx",
                "// fairlead: use tok.sync-button\nexport const A = 1;\n",
            )
            .build();
        let ds = b.run_check(&RefsCheck);
        let r = of(&ds, c::HN_R001);
        assert!(
            r[0].did_you_mean
                .as_deref()
                .unwrap()
                .starts_with("cmp.sync-button")
        );
    }

    #[test]
    fn r_syn_010__code_def_ovr_and_flag_target_of_an_undeclared_id_are_dangling() {
        let src =
            "// fairlead: def cmp.ghost\n// fairlead: flag dep to=cmp.nope\nexport const A = 1;\n";
        let b = world().file("src/a.tsx", src).build();
        assert_eq!(of(&b.run_check(&RefsCheck), c::HN_R001).len(), 2);
    }

    #[test]
    fn r_chk_051__declared_foreign_and_unknown_kind_references_are_not_dangling() {
        let src = "// fairlead: use cmp.sync-button\n// fairlead: def foreign.shadcn tool=shadcn +end\nx\n// fairlead: end\n// fairlead: use zzz.q\nexport const A = 1;\n";
        let b = world().file("src/a.tsx", src).build();
        assert!(of(&b.run_check(&RefsCheck), c::HN_R001).is_empty());
    }

    #[test]
    fn r_chk_005__unused_is_opt_in_per_kind() {
        let b = world()
            .file(
                "src/a.tsx",
                "// fairlead: use cmp.sync-button\nexport const A = 1;\n",
            )
            .build();
        let ds = b.run_check(&RefsCheck);
        let u = of(&ds, c::HN_R002);
        assert_eq!(u.len(), 1); // tok.a has no use; cmp has no `unused` key
        assert_eq!(u[0].severity, Severity::Warn);
    }

    #[test]
    fn r_syn_010__more_sites_than_impl_allows_is_d002_on_the_extra_sites() {
        let b = world()
            .file(
                "src/a.tsx",
                "// fairlead: def cmp.sync-button\nexport const A = 1;\n",
            )
            .file(
                "src/b.tsx",
                "// fairlead: def cmp.sync-button\nexport const B = 1;\n",
            )
            .build();
        let ds = b.run_check(&RefsCheck);
        let d = of(&ds, c::HN_D002);
        assert_eq!((d.len(), d[0].location.path.as_str()), (1, "src/b.tsx"));
    }
}
