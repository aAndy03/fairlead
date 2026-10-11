//! References: dangling (R001), unused (R002), too many sites (D002), retired IDs (R004, D003).
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
const CODES: [DiagCode; 7] = [
    HN_R001, HN_R002, HN_D002, HN_R004, HN_D003, HN_M007, HN_R008,
];

/// R-SCOPE-029: an item a sibling may name: listed in its scope's `[scope] provides`
/// (`kind.name` or `kind.*`) or declared `vis=public`.
pub(crate) fn provided(reg: &Registry, item: &ItemId) -> bool {
    let name = format!("{}.{}", item.kind.as_str(), item.name.as_str());
    let public = reg
        .item(item)
        .and_then(|it| crate::check::builtin::field_str(it, "vis"))
        == Some("public");
    public
        || reg
            .scope(&item.scope)
            .and_then(|s| s.control.as_ref())
            .is_some_and(|c| {
                c.scope_cfg.provides.iter().any(|p| {
                    p == &name
                        || p.strip_suffix(".*")
                            .is_some_and(|k| k == item.kind.as_str())
                })
            })
}
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

/// R-CHK-059: a `def` of a tombstoned ID is `HN-D003`; any other reference is `HN-R004`, a warning
/// naming the successor while a `by` alias exists and an error without one (R-LC-022).
fn retired(path: &RepoPath, d: &Directive, id: &Id, t: &crate::lifecycle::Tomb) -> Diagnostic {
    let why = if t.why.is_empty() {
        String::new()
    } else {
        format!(": {}", t.why)
    };
    if d.verb == Verb::Def {
        return reserved(path, d.span.line, d.span.col, id, t);
    }
    if t.deleted_by.as_deref() == Some("delete") {
        // R-PORT-010: a live reference to an item of a deleted scope is DANGLING.
        return DiagB::new(
            HN_M007,
            path,
            d.span.line,
            d.span.col,
            format!("{id} belonged to a deleted scope{why}"),
        )
        .hint("remove the reference; the item is tombstoned")
        .related(id.to_string())
        .text(id.to_string())
        .build();
    }
    let b = DiagB::new(
        HN_R004,
        path,
        d.span.line,
        d.span.col,
        format!("{id} was removed in {}{why}", t.removed),
    )
    .related(id.to_string())
    .text(id.to_string());
    match &t.by {
        Some(by) => b
            .severity(fairlead_syntax::diag::Severity::Warn)
            .hint(format!("use {by} (the replaced-by alias)"))
            .dym(by.clone())
            .build(),
        None => b
            .severity(fairlead_syntax::diag::Severity::Error)
            .hint("remove the reference; the ID is reserved forever")
            .build(),
    }
}

fn reserved(
    path: &RepoPath,
    line: u32,
    col: u32,
    id: &impl std::fmt::Display,
    t: &crate::lifecycle::Tomb,
) -> Diagnostic {
    DiagB::new(
        HN_D003,
        path,
        line,
        col,
        format!(
            "{id} is tombstoned (removed in {}) and reserved forever",
            t.removed
        ),
    )
    .hint("choose a new ID; a revive doc creates a versioned successor (R-LC-024)")
    .related(id.to_string())
    .text(id.to_string())
    .build()
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
            // R-VER-060: the `def`s of an unsealed change doc are proposals, not references.
            let unsealed_change_doc = ex
                .frontmatter
                .as_ref()
                .is_some_and(|fm| crate::docs::is_unsealed_change_doc(&fm.table));
            for d in &ex.directives {
                if unsealed_change_doc && d.verb == Verb::Def {
                    continue;
                }
                for id in referenced(d) {
                    let kind = id.kind.as_str();
                    if RESERVED.contains(&kind) || kind_decl(reg, scope, kind).is_none() {
                        continue;
                    } // unknown kind is S002's
                    if let Some(t) = reg.tomb_for(scope, &id) {
                        if d.verb == Verb::Def
                            || matches!(reg.lookup(scope, &id), Lookup::Missing { .. })
                        {
                            out.push(retired(path, d, &id, t));
                        }
                        continue;
                    }
                    if let IdQualifier::Scope(_) = &id.qualifier
                        && d.verb != Verb::Def
                        && let Lookup::Found(item) = reg.lookup(scope, &id)
                        && !reg.chain(scope).contains(&&item.scope)
                        && !reg.chain(&item.scope).contains(&scope)
                        && !provided(reg, &item)
                    {
                        out.push(
                            DiagB::new(
                                HN_R008,
                                path,
                                d.span.line,
                                d.span.col,
                                format!("{id} is not provided by scope {}", item.scope.0.as_str()),
                            )
                            .hint(format!(
                                "add it to [scope] provides in {}'s control file, or declare it vis=public",
                                item.scope.0.as_str()
                            ))
                            .related(id.to_string())
                            .text(id.to_string())
                            .build(),
                        );
                    }
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
        // 0.2.2-T3 (R-LC-021): a control table (`[rules.X]`, `[[file]]`) is a declaration too; the
        // field test re-created a tombstoned rule and nothing reported it.
        for it in reg.items().filter(|i| i.id.scope == *scope) {
            let crate::registry::Origin::ControlTable { control } = &it.origin else {
                continue;
            };
            let bare = format!("{}.{}", it.id.kind.as_str(), it.id.name.as_str());
            if let Some(t) = reg
                .retired()
                .iter()
                .find(|t| t.is_item() && t.id == bare && t.scope.as_ref() == Some(scope))
            {
                out.push(reserved(control, 0, 0, &bare, t));
            }
        }
        // R-LC-024: the only way back for a tombstoned ID is an approved revive doc that creates a
        // versioned successor.
        let (docs, _) = crate::docs::collect_change_docs(cx.corpus);
        for d in docs.iter().filter(|d| {
            d.kind == "revive" && d.status.is_pending() && reg.scope_of(&d.path) == Some(scope)
        }) {
            for p in crate::lifecycle::revive_problems(d, reg, &cx.facts.approvals) {
                out.push(
                    DiagB::new(
                        HN_D003,
                        &d.path,
                        1,
                        1,
                        format!("revive doc {}: {p}", d.id.as_str()),
                    )
                    .hint(
                        "a tombstoned ID returns only as <id>-v<N> through an approved revive doc",
                    )
                    .text(d.id.as_str().to_string())
                    .build(),
                );
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
    fn r_ver_060__a_def_in_an_unsealed_change_doc_is_not_a_declaration() {
        let doc = |status: &str| {
            format!(
                "+++\nid = \"d-00000001\"\nkind = \"change\"\nstatus = \"{status}\"\ndirectives = [\n  'def cmp.panel',\n]\n+++\n"
            )
        };
        let using = "// fairlead: use cmp.panel\nexport const A = 1;\n";
        for (status, dangling) in [("draft", 1), ("ready", 1), ("abandoned", 1), ("sealed", 0)] {
            let b = world()
                .file("fairlead/docs/changes/d-00000001-p.md", &doc(status))
                .file("src/a.tsx", using)
                .build();
            let ds = b.run_check(&RefsCheck);
            assert_eq!(of(&ds, c::HN_R001).len(), dangling, "{status}: {ds:?}");
        }
    }

    const TOMBS: &str = "[[tomb]]\nid = \"cmp.old-button\"\nremoved = \"0.5.0\"\nby = \"cmp.sync-button\"\nwhy = \"merged\"\n\n[[tomb]]\nid = \"cmp.gone\"\nremoved = \"0.5.0\"\nwhy = \"unused\"\n";

    fn tombed(src: &str) -> Vec<Diagnostic> {
        world()
            .facts(|f| {
                f.tombstones.insert(
                    RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                    TOMBS.into(),
                );
            })
            .file("src/a.tsx", src)
            .build()
            .run_check(&RefsCheck)
    }

    #[test]
    fn r_chk_059__three_codes_are_distinct() {
        let ds = tombed(
            "// fairlead: use cmp.old-button\n// fairlead: use cmp.gone\n// fairlead: def cmp.gone\nexport const A = 1;\n",
        );
        assert!(
            of(&ds, c::HN_R001).is_empty(),
            "never a generic unknown id: {ds:?}"
        );
        let r4 = of(&ds, c::HN_R004);
        assert_eq!(r4.len(), 2);
        assert_eq!(
            (r4[0].severity, r4[0].did_you_mean.as_deref()),
            (Severity::Warn, Some("cmp.sync-button"))
        );
        assert_eq!(r4[1].severity, Severity::Error, "no alias");
        assert_eq!(of(&ds, c::HN_D003).len(), 1);
    }

    #[test]
    fn r_scope_029__sideways_ref_needs_provides() {
        let w = |lib_ctl: &str, def: &str| {
            World::new()
                .root("[kinds.cmp]\nimpl = 1\n")
                .scope("lib", lib_ctl)
                .scope("app", "[fairlead]\nscope = \"app\"\n")
                .file(
                    "lib/fairlead/docs/cmp.md",
                    &format!("+++\ndirectives = ['{def}']\n+++\n# C\n"),
                )
                .file(
                    "app/main.ts",
                    "// fairlead: use lib:cmp.x\nexport const M = 1;\n",
                )
                .build()
                .run_check(&RefsCheck)
        };
        let plain = "[fairlead]\nscope = \"lib\"\n";
        let hidden = w(plain, "def cmp.x");
        assert_eq!(of(&hidden, c::HN_R008).len(), 1, "{hidden:?}");
        let listed = w(
            &format!("{plain}[scope]\nprovides = [\"cmp.*\"]\n"),
            "def cmp.x",
        );
        assert!(of(&listed, c::HN_R008).is_empty());
        let public = w(plain, "def cmp.x vis=public");
        assert!(of(&public, c::HN_R008).is_empty());
    }

    #[test]
    fn r_lc_024__revive_needs_approval_and_new_id() {
        let revive = |as_id: &str| {
            format!(
                "+++\nid = \"d-00000001\"\nkind = \"revive\"\nstatus = \"ready\"\nimpact = \"add\"\ncovers = [\"src/**\"]\nrevives = \"cmp.gone\"\nas = \"{as_id}\"\n+++\n"
            )
        };
        let run = |as_id: &str, approved: bool| {
            world()
                .facts(|f| {
                    f.tombstones.insert(
                        RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                        TOMBS.into(),
                    );
                    if approved {
                        f.approvals.approvals.push(crate::config::Approval {
                            fp: crate::lifecycle::revive_fp("d-00000001", "cmp.gone", as_id),
                            target: Id::parse("cmp.gone").unwrap(),
                            scope: fairlead_syntax::ids::ScopeSlug::parse("root").unwrap(),
                            approver: "@platform".into(),
                            date: "2026-10-09".parse().unwrap(),
                            renewal: 0,
                        });
                    }
                })
                .file("fairlead/docs/changes/d-00000001-back.md", &revive(as_id))
                .build()
                .run_check(&RefsCheck)
        };
        let unapproved = run("cmp.gone-v2", false);
        let d = of(&unapproved, c::HN_D003);
        assert_eq!(d.len(), 1);
        assert!(
            d[0].message
                .contains("not approved: a human runs `fairlead approve d-00000001`")
        );
        assert!(of(&run("cmp.gone-v2", true), c::HN_D003).is_empty());
        let same = run("cmp.gone", true);
        assert!(
            of(&same, c::HN_D003)[0]
                .message
                .contains("must be a new ID cmp.gone-v<N>")
        );
    }

    #[test]
    fn r_lc_022__alias_reference_warns_names_successor() {
        let ds = tombed("// fairlead: use cmp.old-button\nexport const A = 1;\n");
        let r = of(&ds, c::HN_R004);
        assert!(
            r[0].message.contains("removed in 0.5.0: merged"),
            "{}",
            r[0].message
        );
        assert!(r[0].hint.contains("cmp.sync-button"));
    }

    #[test]
    fn r_lc_021__a_doc_def_of_a_removed_id_is_reserved() {
        let b = World::new()
            .root("[kinds.cmp]\nimpl = 1\n")
            .file(
                "fairlead/docs/cmp.md",
                "+++\ndirectives = ['def cmp.gone']\n+++\n# C\n",
            )
            .facts(|f| {
                f.tombstones.insert(
                    RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                    TOMBS.into(),
                );
            })
            .build();
        assert_eq!(of(&b.run_check(&RefsCheck), c::HN_D003).len(), 1);
        assert_eq!(b.registry.retired().len(), 2);
    }

    #[test]
    fn r_lc_021__a_control_table_that_reuses_a_removed_id_is_reserved() {
        let tomb = "[[tomb]]\nid = \"rule.no-todo\"\nremoved = \"2026-10-11\"\nwhy = \"TODO placeholder\"\ndeleted-by = \"sync\"\n";
        let run = |rule: &str| {
            World::new()
                .root(rule)
                .file("src/a.rs", "fn a() {}\n")
                .facts(|f| {
                    f.tombstones.insert(
                        RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                        tomb.into(),
                    );
                })
                .build()
                .run_check(&RefsCheck)
        };
        let reused = run("[rules.no-todo]\ncheck = \"pattern\"\nforbid = \"TODO\"\nmsg = \"m\"\n");
        let d = of(&reused, c::HN_D003);
        assert_eq!(d.len(), 1, "{reused:?}");
        assert_eq!(d[0].location.path.as_str(), "fairlead/control.toml");
        let other = run("[rules.no-fixme]\ncheck = \"pattern\"\nforbid = \"FIXME\"\nmsg = \"m\"\n");
        assert!(of(&other, c::HN_D003).is_empty());
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
