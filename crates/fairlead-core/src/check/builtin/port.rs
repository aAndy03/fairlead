//! Portability (32-portability-and-restructure): `requires` (R-PORT-020, R-PORT-021), the lock
//! (R-PORT-022, `H-REQ-CHANGED`), sparse checkouts (R-PORT-052, `NOT-CHECKED`) and package
//! manifests (R-PORT-053, R-PORT-054, `PKG-HINT`).
use std::collections::BTreeSet;

use fairlead_syntax::diag::{DiagCode, Severity};

use crate::check::builtin::root_control;
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::generate::GenKind;
use crate::path::RepoPath;
use crate::registry::{ScopeId, ScopeState};

pub struct PortCheck;
const CODES: [DiagCode; 13] = [
    HN_V012, HN_C012, HN_R001, HN_M006, HN_M009, HN_M004, HN_V014, HN_C007, HN_M010, HN_C001,
    HN_M005, HN_M012, HN_M011,
];

impl Check for PortCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(sc) = reg.scope(scope) else { return };
        let control = sc.control.as_ref().map(|c| c.path.clone());
        if crate::port::opted_in(reg, scope)
            && let Some(cp) = &control
        {
            for raw in crate::port::unresolved_authored(reg, scope) {
                out.push(
                    DiagB::new(
                        HN_R001,
                        cp,
                        0,
                        0,
                        format!("[scope] requires names {raw}, which is not visible here"),
                    )
                    .hint("fix the ID, or add it to the target scope's provides (R-PORT-020)")
                    .text(raw)
                    .build(),
                );
            }
            req_changed(cx, scope, out);
        }
        unsatisfied(cx, scope, out);
        if *scope == *reg.root_scope() {
            for cycle in crate::port::requires_cycles(reg, cx.corpus) {
                let names: Vec<&str> = cycle.iter().map(|s| s.0.as_str()).collect();
                let Some(first) = cycle.first().and_then(|s| reg.scope(s)) else {
                    continue;
                };
                let at = first
                    .control
                    .as_ref()
                    .map_or_else(|| first.dir.clone(), |c| c.path.clone());
                out.push(
                    DiagB::new(
                        HN_C012,
                        &at,
                        0,
                        0,
                        format!("requires cycle: {} -> {}", names.join(" -> "), names.first().unwrap_or(&"")),
                    )
                    .hint("move the shared items to a common ancestor or drop one direction (R-PORT-021)")
                    .severity(Severity::Warn)
                    .text(names.join(","))
                    .build(),
                );
            }
            not_checked(cx, out);
            pkg_hints(cx, out);
            mounts(cx, out);
            pins(cx, out);
        }
        let _ = sc;
    }
}

/// `H-REQ-CHANGED` (HN-V012): a required item whose digest differs from the lock on disk.
fn req_changed(cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let Some(want) = cx
        .generated
        .files
        .values()
        .find(|f| f.kind == GenKind::ScopeLock && &f.scope == scope)
    else {
        return;
    };
    let Some(have) = cx.facts.generated_on_disk.get(&want.path) else {
        return;
    };
    let have = crate::port::parse_lock_reqs(&String::from_utf8_lossy(have));
    for now in crate::port::parse_lock_reqs(&want.body) {
        if let Some(old) = have.iter().find(|h| h.id == now.id && h.from == now.from)
            && old.dg != now.dg
        {
            out.push(
                DiagB::new(
                    HN_V012,
                    &want.path,
                    0,
                    0,
                    format!(
                        "required {}:{} changed since the lock ({} -> {})",
                        now.from, now.id, old.dg, now.dg
                    ),
                )
                .hint("review the change, then run fairlead sync to re-pin it (R-PORT-022)")
                .related(format!("{}:{}", now.from, now.id))
                .text(format!("{}:{}", now.from, now.id))
                .build(),
            );
        }
    }
}

/// R-PORT-041: a required item that the scope's ancestors no longer provide is `UNSATISFIED`
/// (HN-M005); a portable scope serves it from its snapshot as a `detached` layer (HN-M011, info).
fn unsatisfied(cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    let Some(lock) = cx
        .facts
        .generated_on_disk
        .iter()
        .find(|(p, _)| p.file_name() == "scope.lock" && cx.registry.scope_of(p) == Some(scope))
    else {
        return;
    };
    let text = String::from_utf8_lossy(lock.1);
    for (r, served) in crate::port::unsatisfied(cx.registry, scope, &text) {
        let id = format!("{}:{}", r.from, r.id);
        if served {
            out.push(
                DiagB::new(
                    HN_M011,
                    lock.0,
                    0,
                    0,
                    format!("{id} is served from the snapshot of the lock (detached layer)"),
                )
                .hint("adopt it into the new ancestors, or re-point the requirement (R-PORT-041)")
                .text(id)
                .build(),
            );
        } else {
            out.push(
                DiagB::new(HN_M005, lock.0, 0, 0, format!("required {id} is not provided here (UNSATISFIED)"))
                    .hint("declare it in an ancestor, mark the scope portable before moving it, or drop the requirement (R-PORT-041)")
                    .text(id)
                    .build(),
            );
        }
    }
}

/// R-PORT-051: a mount whose commit or rollup moved since the root's lock is `LOCK-STALE`.
fn pins(cx: &CheckCx<'_>, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let Some(want) = cx
        .generated
        .files
        .values()
        .find(|f| f.kind == GenKind::ScopeLock && f.scope == *reg.root_scope())
    else {
        return;
    };
    let Some(have) = cx.facts.generated_on_disk.get(&want.path) else {
        return;
    };
    let have = crate::port::parse_pins(&String::from_utf8_lossy(have));
    for (scope, now) in crate::port::parse_pins(&want.body) {
        if let Some(old) = have.get(&scope)
            && *old != now
        {
            out.push(
                DiagB::new(HN_M012, &want.path, 0, 0, format!("mount {scope} moved since the lock ({old} -> {now}): LOCK-STALE"))
                    .hint("review the sub-root's change, then run fairlead sync to re-pin it (R-PORT-051)")
                    .text(scope.clone())
                    .build(),
            );
        }
    }
}

/// R-PORT-030 to R-PORT-036: the boundary of each registered mount, judged from its rollup.
fn mounts(cx: &CheckCx<'_>, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let Some(rc) = root_control(reg) else { return };
    let outer_edition = crate::check::builtin::edition_of(reg).0;
    let outer_ids: BTreeSet<&str> = reg.scopes().map(|s| s.id.0.as_str()).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for m in &rc.mounts {
        let at = &rc.path;
        let Some(f) = cx.facts.mounts.get(&m.scope) else {
            continue;
        };
        if !f.present {
            // R-PORT-052: a submodule that is not checked out runs from the lock: offline pin.
            out.push(
                DiagB::new(
                    HN_M006,
                    at,
                    0,
                    0,
                    format!(
                        "mount {} ({}) is not checked out: offline pin",
                        m.scope, m.path
                    ),
                )
                .hint("check out the submodule, or let CI check it (R-PORT-052)")
                .text(m.scope.clone())
                .build(),
            );
            continue;
        }
        if !f.is_root || f.scope.as_deref() != Some(m.scope.as_str()) {
            out.push(
                DiagB::new(HN_C001, at, 0, 0, format!("[[mount]] {} names {}, which holds no root control file of scope {}", m.scope, m.path, m.scope))
                    .hint("point path at a directory whose control file has root = true and that scope (R-PORT-030)")
                    .text(m.scope.clone())
                    .build(),
            );
            continue;
        }
        // R-PORT-034, R-EDN-045: an edition mismatch is reported once, at the boundary.
        if let Some(e) = f.edition
            && e != outer_edition
        {
            out.push(
                DiagB::new(HN_V014, at, 0, 0, format!("mount {} is on edition {e}, the outer root on {outer_edition}: each side is checked by its own edition", m.scope))
                    .hint("upgrade one side when convenient; nothing fails at the boundary (R-EDN-045)")
                    .text(m.scope.clone())
                    .build(),
            );
        }
        // R-PORT-035: scope ids are unique across the tree including mounts (as= prefixes them).
        for id in f.scopes.iter().chain(std::iter::once(&m.scope)) {
            let shown = m
                .as_prefix
                .as_ref()
                .map_or_else(|| id.clone(), |p| format!("{p}-{id}"));
            if outer_ids.contains(shown.as_str()) || !seen.insert(shown.clone()) {
                out.push(
                    DiagB::new(
                        HN_C007,
                        at,
                        0,
                        0,
                        format!(
                            "scope id {shown} of mount {} is already a scope id of the tree",
                            m.scope
                        ),
                    )
                    .hint("give the mount an as= prefix (R-PORT-035)")
                    .text(shown.clone())
                    .build(),
                );
            }
        }
        // R-PORT-036: the outer root declares nothing inside a sealed sub-root.
        let inside = |p: &str| p == m.path || p.starts_with(&format!("{}/", m.path));
        for s in reg.scopes() {
            let Some(c) = s.control.as_ref() else {
                continue;
            };
            for fd in c.files.iter().filter(|fd| {
                let full = if s.dir.is_root() {
                    fd.path.clone()
                } else {
                    format!("{}/{}", s.dir.as_str(), fd.path)
                };
                inside(&full)
            }) {
                out.push(
                    DiagB::new(HN_M010, &c.path, 0, 0, format!("{} declares {} inside the mounted sub-root {}", s.id.0.as_str(), fd.path, m.scope))
                        .hint("a root writes and declares only inside its own subtree; declare it in the sub-root (R-PORT-036)")
                        .text(fd.id.clone())
                        .build(),
                );
            }
        }
    }
}

/// R-PORT-052: scopes outside a sparse checkout are `NOT-CHECKED`, never a silent pass.
fn not_checked(cx: &CheckCx<'_>, out: &mut Vec<Diagnostic>) {
    for s in cx
        .registry
        .scopes()
        .filter(|s| matches!(s.state, ScopeState::AbsentFromTree))
    {
        out.push(
            DiagB::new(
                HN_M006,
                &s.dir,
                0,
                0,
                format!(
                    "scope {} was not checked: it is outside the checkout",
                    s.id.0.as_str()
                ),
            )
            .hint("widen the sparse checkout, or let CI check the full tree (R-PORT-052)")
            .text(s.id.0.as_str())
            .build(),
        );
    }
}

/// R-PORT-054: a publishable crate that would ship the managerial folder, or whose `path` plus
/// `version` dependency disagrees with the dependency's own version.
fn pkg_hints(cx: &CheckCx<'_>, out: &mut Vec<Diagnostic>) {
    let reg = cx.registry;
    let folder = root_control(reg)
        .and_then(|c| c.settings.folder.clone())
        .unwrap_or_else(|| "fairlead".into());
    let hints = &cx.facts.manifest_hints;
    let managerial: BTreeSet<RepoPath> = reg
        .scopes()
        .filter_map(|s| {
            s.control
                .as_ref()
                .and_then(|c| c.path.parent())
                .and_then(|p| p.parent())
        })
        .collect();
    for (dir, p) in &hints.packages {
        let manifest = if dir.is_root() {
            RepoPath::new("Cargo.toml").ok()
        } else {
            RepoPath::new(&format!("{}/Cargo.toml", dir.as_str())).ok()
        };
        let Some(manifest) = manifest else { continue };
        if p.publishable && managerial.contains(dir) && p.include.is_empty() {
            let excluded = p.exclude.iter().any(|e| {
                let e = e
                    .trim_start_matches('/')
                    .trim_end_matches("/**")
                    .trim_end_matches('/');
                e == folder
            });
            if !excluded {
                out.push(
                    DiagB::new(
                        HN_M009,
                        &manifest,
                        0,
                        0,
                        format!("crate {} may be published with the managerial folder {folder}/", p.name),
                    )
                    .hint(format!("add exclude = [\"{folder}/\"] to [package], or publish = false (R-PORT-054)"))
                    .text(format!("{}:exclude", p.name))
                    .build(),
                );
            }
        }
        for (name, dep_dir, req) in &p.path_version_deps {
            let Some(dep) = hints.packages.get(dep_dir) else {
                continue;
            };
            if !dep.version.is_empty() && !crate::manifest::version_agrees(req, &dep.version) {
                out.push(
                    DiagB::new(
                        HN_M009,
                        &manifest,
                        0,
                        0,
                        format!(
                            "crate {} depends on {name} by path and version {req}, but {name} is {}",
                            p.name, dep.version
                        ),
                    )
                    .hint("make the version requirement match the path dependency's version (R-PORT-054)")
                    .text(format!("{}:{name}", p.name))
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    #[test]
    fn r_port_052__a_scope_outside_the_sparse_checkout_is_not_checked() {
        let b = World::new()
            .facts(|f| {
                f.generated_on_disk.insert(
                    RepoPath::new("fairlead/gen/members").unwrap(),
                    std::sync::Arc::from(b"scope.web dir=web\n".to_vec()),
                );
                f.git
                    .sparse_absent
                    .push(RepoPath::new("web/fairlead/control.toml").unwrap());
            })
            .build();
        let ds = b.run_check(&PortCheck);
        let m = of(&ds, c::HN_M006);
        assert_eq!(m.len(), 1, "{ds:#?}");
        assert!(m[0].message.contains("scope web was not checked"));
    }

    fn pkg(dir: &str, p: crate::manifest::Package) -> impl FnOnce(&mut crate::facts::Facts) {
        let dir = dir.to_owned();
        move |f| {
            f.manifest_hints.packages.insert(
                if dir.is_empty() {
                    RepoPath::root()
                } else {
                    RepoPath::new(&dir).unwrap()
                },
                p,
            );
        }
    }

    #[test]
    fn r_port_054__a_publishable_crate_excludes_the_managerial_folder() {
        let p = |exclude: &[&str], publishable: bool| crate::manifest::Package {
            name: "app".into(),
            version: "0.1.0".into(),
            publishable,
            exclude: exclude.iter().map(|s| (*s).to_owned()).collect(),
            ..Default::default()
        };
        let run = |x: crate::manifest::Package| {
            World::new().facts(pkg("", x)).build().run_check(&PortCheck)
        };
        let ds = run(p(&[], true));
        let m = of(&ds, c::HN_M009);
        assert_eq!(m.len(), 1, "{ds:#?}");
        assert_eq!(m[0].severity, Severity::Info);
        assert!(m[0].hint.contains("exclude = [\"fairlead/\"]"));
        assert!(of(&run(p(&["fairlead/"], true)), c::HN_M009).is_empty());
        assert!(
            of(&run(p(&[], false)), c::HN_M009).is_empty(),
            "publish = false"
        );
    }

    #[test]
    fn r_port_054__path_and_version_must_agree() {
        let app = crate::manifest::Package {
            name: "app".into(),
            version: "0.1.0".into(),
            publishable: false,
            path_version_deps: vec![("core".into(), RepoPath::new("core").unwrap(), "0.2".into())],
            ..Default::default()
        };
        let core = |v: &str| crate::manifest::Package {
            name: "core".into(),
            version: v.into(),
            publishable: false,
            ..Default::default()
        };
        let run = |v: &str| {
            World::new()
                .facts(pkg("", app.clone()))
                .facts(pkg("core", core(v)))
                .build()
                .run_check(&PortCheck)
        };
        let ds = run("0.3.0");
        assert_eq!(of(&ds, c::HN_M009).len(), 1, "{ds:#?}");
        assert!(of(&run("0.2.4"), c::HN_M009).is_empty());
        assert!(crate::manifest::version_agrees("1", "1.4.0"));
        assert!(!crate::manifest::version_agrees("=1.2.0", "1.2.1"));
        assert!(crate::manifest::version_agrees("~1.2", "1.2.9"));
    }

    #[test]
    fn r_port_053__manifests_are_read_never_edited() {
        // The manifest reader takes paths and returns facts; there is no write path for
        // manifests: no `Cargo.toml` is ever in a write set (R-PORT-053, D-50).
        let src = include_str!("../../manifest.rs");
        assert!(!src.contains("std::fs::write") && !src.contains("WriteSet"));
    }

    const KINDS: &str = "[kinds.cmp]\nshape = \"doc\"\n[kinds.tok]\nshape = \"doc\"\n[kinds.claim]\nshape = \"doc\"\n";
    fn ported(web_scope: &str, value: &str) -> World {
        World::new()
            .root(KINDS)
            .file(
                "fairlead/contract/a.md",
                &format!("+++\ndirectives = ['def cmp.button sum=\"Button\"', 'def tok.brand value={value}', 'def tok.space value=4px']\n+++\n# Contract\n"),
            )
            .file("fairlead/versions/0.1.0.md", "+++\n+++\n")
            .scope("web", &format!("[fairlead]\nscope = \"web\"\n{web_scope}"))
            .file("web/src/a.ts", "// fairlead: use root:cmp.button\nexport const a = 1;\n")
            .file(
                "web/fairlead/contract/w.md",
                "+++\ndirectives = ['def cmp.side extends=root:cmp.button']\n+++\n# Web\n",
            )
    }
    fn gen_of(b: &crate::check::testkit::Built, kind: GenKind, scope: &str) -> Option<String> {
        let set = crate::generate::generate(
            &b.registry,
            &b.corpus,
            &b.facts,
            None,
            crate::check::builtin::edition_of(&b.registry),
        );
        set.files
            .values()
            .find(|f| f.kind == kind && f.scope.0.as_str() == scope)
            .map(|f| f.body.clone())
    }

    #[test]
    fn r_port_020__requires_generated_from_use_and_extends() {
        let b = ported("[scope]\nrequires = [\"tok.brand\"]\n", "#2563eb").build();
        assert_eq!(
            gen_of(&b, GenKind::Requires, "web").as_deref(),
            Some("root:cmp.button via=use\nroot:tok.brand via=authored\n")
        );
        let plain = ported("", "#2563eb").build();
        assert_eq!(
            gen_of(&plain, GenKind::Requires, "web"),
            None,
            "a scope that does not opt in keeps its 0.2 files"
        );
        let bad = ported("[scope]\nrequires = [\"tok.nope\"]\n", "#2563eb").build();
        let ds = bad.run_check(&PortCheck);
        assert!(
            of(&ds, c::HN_R001)
                .iter()
                .any(|d| d.message.contains("requires names tok.nope")),
            "{ds:#?}"
        );
    }

    #[test]
    fn r_port_022__lock_is_digest_only_unless_portable() {
        let b = ported("[scope]\nrequires = [\"tok.brand\"]\n", "#2563eb").build();
        let lock = gen_of(&b, GenKind::ScopeLock, "web").unwrap();
        assert!(
            lock.contains("edition=1\n") && lock.contains("chain dg="),
            "{lock}"
        );
        let req: Vec<&str> = lock.lines().filter(|l| l.starts_with("req=")).collect();
        assert_eq!(req.len(), 2, "{lock}");
        assert!(
            req[0].starts_with("req=cmp.button from=root dg=") && req[0].ends_with(" ver=0.1.0"),
            "{lock}"
        );
        assert!(!lock.contains("snap "), "digest-only by default");
        let p = ported("[scope]\nportable = true\n", "#2563eb").build();
        let lock = gen_of(&p, GenKind::ScopeLock, "web").unwrap();
        assert!(
            lock.contains("snap root:tok.brand") || lock.contains("snap root:cmp.button"),
            "{lock}"
        );
        let root_lock = gen_of(&p, GenKind::ScopeLock, "root").unwrap();
        assert!(
            !root_lock.contains("req="),
            "the root does not opt in: 0.2 bytes"
        );
    }

    #[test]
    fn r_port_022__a_changed_required_item_is_req_changed() {
        let web = "[scope]\nrequires = [\"tok.brand\"]\n";
        let before = ported(web, "#2563eb").build().synced();
        let disk = before.facts.generated_on_disk.clone();
        let after = ported(web, "#1d4ed8")
            .facts(move |f| f.generated_on_disk = disk)
            .build();
        let set = crate::generate::generate(
            &after.registry,
            &after.corpus,
            &after.facts,
            None,
            crate::check::builtin::edition_of(&after.registry),
        );
        let ds = after.run_check_with(&PortCheck, &set);
        let v = of(&ds, c::HN_V012);
        assert_eq!(v.len(), 1, "{ds:#?}");
        assert!(
            v[0].message
                .contains("required root:tok.brand changed since the lock")
        );
        assert_eq!(v[0].severity, Severity::Info);
    }

    #[test]
    fn r_port_021__requires_cycle_warns_extends_cycle_errors() {
        let w = World::new()
            .root(KINDS)
            .scope(
                "a",
                "[fairlead]\nscope = \"a\"\n[scope]\nprovides = [\"cmp.*\"]\nrequires = []\n",
            )
            .scope(
                "b",
                "[fairlead]\nscope = \"b\"\n[scope]\nprovides = [\"cmp.*\"]\nrequires = []\n",
            )
            .file(
                "a/fairlead/contract/a.md",
                "+++\ndirectives = ['def cmp.x']\n+++\n# A\n",
            )
            .file(
                "b/fairlead/contract/b.md",
                "+++\ndirectives = ['def cmp.y']\n+++\n# B\n",
            )
            .file(
                "a/src/a.ts",
                "// fairlead: use b:cmp.y\nexport const a = 1;\n",
            )
            .file(
                "b/src/b.ts",
                "// fairlead: use a:cmp.x\nexport const b = 1;\n",
            );
        let ds = w.build().run_check(&PortCheck);
        let cy = of(&ds, c::HN_C012);
        assert_eq!(cy.len(), 1, "{ds:#?}");
        assert_eq!(
            cy[0].severity,
            Severity::Warn,
            "a requires cycle is a dependency warning"
        );
        assert!(cy[0].message.contains("requires cycle"));
        // an `extends` cycle stays the 0.2 error (R-FILE-051): `r_file_051` in controls.rs
    }

    #[test]
    fn r_port_025__a_restated_claim_without_contradicts_is_dup_claim() {
        let root =
            "+++\ndirectives = ['def claim.api-stable sum=\"The API is stable\"']\n+++\n# Claims\n";
        let restated = |extra: &str| {
            format!(
                "+++\ndirectives = ['def claim.api-stable sum=\"Stable\" {extra}']\n+++\n# Web\n"
            )
        };
        let b = World::new()
            .root(KINDS)
            .file("fairlead/contract/c.md", root)
            .scope("web", "[fairlead]\nscope = \"web\"\n")
            .file("web/fairlead/contract/c.md", &restated(""))
            .build();
        let codes: Vec<_> = b.registry.diagnostics().iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&c::HN_M004) && !codes.contains(&c::HN_R005),
            "{codes:?}"
        );
        let b = World::new()
            .root(KINDS)
            .file("fairlead/contract/c.md", root)
            .scope("web", "[fairlead]\nscope = \"web\"\n")
            .file(
                "web/fairlead/contract/c.md",
                &restated("contradicts=root:claim.api-stable"),
            )
            .build();
        assert!(
            b.registry
                .diagnostics()
                .iter()
                .all(|d| d.code != c::HN_M004 && d.code != c::HN_R005)
        );
    }

    #[test]
    fn r_port_024__doc_ids_are_qualified_outside_their_scope() {
        let id = crate::docs::DraftId::parse("d-7k3m9qxa").unwrap();
        assert!(crate::docs::doc_ref_is("web:d-7k3m9qxa", &id, "web"));
        assert!(
            !crate::docs::doc_ref_is("api:d-7k3m9qxa", &id, "web"),
            "a qualifier names the scope"
        );
        assert!(
            crate::docs::doc_ref_is("d-7k3m9qxa", &id, "web"),
            "the bare form resolves"
        );
        // an answer that qualifies the request with the wrong scope names no request
        let req = "+++\nkind = \"request\"\nid = \"d-7k3m9qxa\"\nneed = \"root:cmp.button\"\nwant = \"w\"\nbecause = \"b\"\nsum = \"s\"\nout = \"\"\n+++\n";
        let ans = |r: &str| {
            format!(
                "+++\nkind = \"change\"\nid = \"d-91cc04ab\"\nstatus = \"draft\"\ntype = \"variant\"\nanswers = [\"{r}\"]\n+++\n"
            )
        };
        let run = |r: &str| {
            World::new()
                .root(KINDS)
                .file(
                    "fairlead/contract/a.md",
                    "+++\ndirectives = ['def cmp.button']\n+++\n",
                )
                .scope("web", "[fairlead]\nscope = \"web\"\n")
                .file("web/fairlead/docs/requests/d-7k3m9qxa-b.md", req)
                .file("fairlead/docs/changes/d-91cc04ab-a.md", &ans(r))
                .build()
                .run_check(&crate::check::builtin::variation::VariationCheck)
        };
        assert!(of(&run("web:d-7k3m9qxa"), c::HN_R001).is_empty());
        assert_eq!(of(&run("api:d-7k3m9qxa"), c::HN_R001).len(), 1);
    }

    #[test]
    fn r_port_046__lock_stale_when_source_changed() {
        let mount = "[[mount]]\nscope = \"billing\"\npath = \"vendor/billing\"\n";
        let w = |commit: &str| {
            let c = commit.to_owned();
            World::new()
                .root(mount)
                .file("fairlead/versions/0.1.0.md", "+++\n+++\n")
                .facts(move |f| {
                    f.mounts.insert(
                        "billing".into(),
                        crate::facts::MountFacts {
                            present: true,
                            is_root: true,
                            scope: Some("billing".into()),
                            edition: Some(1),
                            scopes: vec![],
                            commit: Some(c),
                            digest: "dddddddddddd".into(),
                        },
                    );
                })
        };
        let before = w("1111111").build().synced();
        let disk = before.facts.generated_on_disk.clone();
        let after = w("2222222")
            .facts(move |f| f.generated_on_disk = disk)
            .build();
        let set = crate::generate::generate(
            &after.registry,
            &after.corpus,
            &after.facts,
            None,
            crate::check::builtin::edition_of(&after.registry),
        );
        let ds = after.run_check_with(&PortCheck, &set);
        let m = of(&ds, c::HN_M012);
        assert_eq!(m.len(), 1, "{ds:#?}");
        assert!(m[0].message.contains("mount billing moved since the lock"));
        let same = before.run_check_with(
            &PortCheck,
            &crate::generate::generate(
                &before.registry,
                &before.corpus,
                &before.facts,
                None,
                crate::check::builtin::edition_of(&before.registry),
            ),
        );
        assert!(of(&same, c::HN_M012).is_empty());
    }

    #[test]
    fn r_port_046__offline_pin_when_unreachable() {
        let ds = World::new()
            .root("[[mount]]\nscope = \"billing\"\npath = \"vendor/billing\"\n")
            .facts(|f| {
                f.mounts
                    .insert("billing".into(), crate::facts::MountFacts::default());
            })
            .build()
            .run_check(&PortCheck);
        let m = of(&ds, c::HN_M006);
        assert_eq!(m.len(), 1, "{ds:#?}");
        assert!(m[0].message.contains("not checked out: offline pin"));
    }

    #[test]
    fn r_port_041__a_missing_require_is_unsatisfied_unless_snapshotted() {
        let lock = |snap: bool| {
            let mut t = "# scope web edition=1\nancestor-digest=x\nreconciled=0.1.0\nreq=tok.gone from=root dg=bbbbbbbbbbbb ver=0.1.0\n".to_owned();
            if snap {
                t.push_str("snap root:tok.gone value=1px\n");
            }
            t
        };
        let run = |snap: bool| {
            let l = lock(snap);
            World::new()
                .scope("web", "[fairlead]\nscope = \"web\"\n")
                .facts(move |f| {
                    f.generated_on_disk.insert(
                        RepoPath::new("web/fairlead/gen/scope.lock").unwrap(),
                        std::sync::Arc::from(l.into_bytes()),
                    );
                })
                .build()
                .run_check(&PortCheck)
        };
        let ds = run(false);
        assert_eq!(of(&ds, c::HN_M005).len(), 1, "{ds:#?}");
        let ds = run(true);
        assert!(of(&ds, c::HN_M005).is_empty());
        assert_eq!(
            of(&ds, c::HN_M011).len(),
            1,
            "served from the snapshot: {ds:#?}"
        );
    }
}
