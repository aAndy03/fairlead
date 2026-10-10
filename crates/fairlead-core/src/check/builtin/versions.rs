//! Change docs and the version line (20-versions): draft-ID collisions, duplicate ordinals and
//! incomplete `ready` docs (R-VER-031, R-VER-042).
use crate::{
    check::{CheckCx, finding::DiagB, rules::Check},
    diag::{Diagnostic, codes::*},
    docs::{ChangeDoc, collect_change_docs, id_collisions, ordinal_duplicates, readiness_problems},
    registry::ScopeId,
};
use fairlead_syntax::diag::DiagCode;

pub struct VersionsCheck;
const CODES: [DiagCode; 6] = [HN_G003, HN_V003, HN_V004, HN_V006, HN_V007, HN_V009];

fn in_scope(cx: &CheckCx<'_>, d: &ChangeDoc, scope: &ScopeId) -> bool {
    cx.registry.scope_of(&d.path) == Some(scope)
}

/// R-RET-036, R-GIT-015: every ledger year file of this scope keeps its checkpoint chain, and the
/// years of one ledger folder chain through `# end` and `prev=`. A break is never auto-fixed.
fn ledger_continuity(cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    use crate::hist::{YearFile, verify_chain};
    use std::collections::BTreeMap;
    let mut by_folder: BTreeMap<String, Vec<(crate::path::RepoPath, YearFile)>> = BTreeMap::new();
    for (path, text) in &cx.facts.ledgers {
        if cx.registry.scope_of(path) != Some(scope) {
            continue;
        }
        let mut push = |line: u32, msg: String| {
            out.push(
                DiagB::new(HN_G003, path, line, 1, msg.clone())
                    .hint("a ledger is never edited by hand; reseal the later seal (fairlead seal --reseal) or restore the file from git")
                    .text(msg)
                    .build(),
            );
        };
        match YearFile::parse(text) {
            Err(e) => push(1, format!("ledger file is unreadable: {e}")),
            Ok(f) => {
                for m in f.verify() {
                    push(1, format!("ledger chain broken: {m}"));
                }
                let folder = path
                    .parent()
                    .map(|p| p.as_str().to_owned())
                    .unwrap_or_default();
                by_folder.entry(folder).or_default().push((path.clone(), f));
            }
        }
    }
    for (_, mut years) in by_folder {
        years.sort_by_key(|(_, f)| f.year);
        let refs: Vec<&YearFile> = years.iter().map(|(_, f)| f).collect();
        if let Some((last, _)) = years.last() {
            for m in verify_chain(&refs) {
                out.push(
                    DiagB::new(
                        HN_G003,
                        last,
                        1,
                        1,
                        format!("ledger chain broken across years: {m}"),
                    )
                    .hint("restore the year files from git; a finalised year is never rewritten")
                    .text(m)
                    .build(),
                );
            }
        }
    }
}

/// Seals a scope may lag behind its ancestors before `H-SYNC-ANCESTOR` turns from info to warn
/// (`version.ancestor-grace`, default 3, R-VER-020; the key is read once `[version]` is parsed).
const ANCESTOR_GRACE: usize = 3;

/// R-VER-020: the ancestor digest in this scope's `scope.lock` against the one `sync` would write.
fn ancestor_drift(cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
    use crate::diag::Severity;
    use crate::generate::GenKind;
    let Some(want) = cx
        .generated
        .files
        .values()
        .find(|f| f.kind == GenKind::ScopeLock && &f.scope == scope)
    else {
        return; // nothing sealed yet: no lock is expected
    };
    let Some((_, want_anc)) = crate::merkle::parse_scope_lock(&want.body) else {
        return;
    };
    let have = cx
        .facts
        .generated_on_disk
        .get(&want.path)
        .and_then(|b| crate::merkle::parse_scope_lock(&String::from_utf8_lossy(b)));
    let (msg, lag) = match have {
        None => (
            "scope.lock is missing; the scope was never reconciled".to_owned(),
            usize::MAX,
        ),
        Some((_, anc)) if anc == want_anc => return,
        Some((reconciled, _)) => {
            let folder = crate::check::builtin::root_control(cx.registry)
                .and_then(|c| c.settings.folder.clone())
                .unwrap_or_else(|| "fairlead".into());
            let lag = crate::docs::version_files(cx.corpus, &folder)
                .into_iter()
                .filter(|v| *v > reconciled)
                .count();
            (
                format!("inherited view changed since {reconciled} ({lag} seals ago)"),
                lag,
            )
        }
    };
    let severity = if lag < ANCESTOR_GRACE {
        Severity::Info
    } else {
        Severity::Warn
    };
    out.push(
        DiagB::new(HN_V004, &want.path, 1, 1, msg.clone())
            .severity(severity)
            .hint("run fairlead sync in this scope to reconcile its lock")
            .text(msg)
            .build(),
    );
}

impl Check for VersionsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let (docs, bad) = collect_change_docs(cx.corpus);
        for (path, problem) in bad
            .iter()
            .filter(|(p, _)| cx.registry.scope_of(p) == Some(scope))
        {
            out.push(
                DiagB::new(
                    HN_V009,
                    path,
                    1,
                    1,
                    format!("change doc is not usable: {problem}"),
                )
                .hint("fix the frontmatter key named above")
                .text(problem)
                .build(),
            );
        }
        for d in docs.iter().filter(|d| in_scope(cx, d, scope)) {
            let problems = readiness_problems(d);
            if !problems.is_empty() {
                out.push(
                    DiagB::new(
                        HN_V009,
                        &d.path,
                        1,
                        1,
                        format!(
                            "ready change doc {} is incomplete: {}",
                            d.id,
                            problems.join(", ")
                        ),
                    )
                    .hint("fill `covers` and `impact`, or set `status = \"draft\"`")
                    .text(d.id.as_str())
                    .build(),
                );
            }
        }
        // R-VER-054: a sealed doc whose body no longer matches its recorded body-hash.
        for d in docs.iter().filter(|d| in_scope(cx, d, scope)) {
            let (Some(recorded), Some(now)) = (&d.body_hash, cx.facts.doc_body_hashes.get(&d.path))
            else {
                continue;
            };
            if d.status == crate::docs::Status::Sealed && recorded != now {
                out.push(
                    DiagB::new(
                        HN_V003,
                        &d.path,
                        1,
                        1,
                        format!("sealed doc {} was edited after its seal (body-hash {recorded}, now {now})", d.id),
                    )
                    .hint("restore it from git; supersede it with a new doc (supersedes = \"<id>\") instead")
                    .text(d.id.as_str())
                    .build(),
                );
            }
        }
        ledger_continuity(cx, scope, out);
        ancestor_drift(cx, scope, out);
        let hash_of = |d: &ChangeDoc| cx.corpus.records.get(&d.path).map(|r| r.hash);
        for group in id_collisions(&docs, hash_of) {
            // Reported once, on the later path, in the scope that holds it.
            let (Some(first), Some(later)) = (group.first(), group.last()) else {
                continue;
            };
            if in_scope(cx, later, scope) {
                out.push(
                    DiagB::new(
                        HN_V006,
                        &later.path,
                        1,
                        1,
                        format!(
                            "draft ID {} is also used by {} with a different body",
                            later.id,
                            first.path.as_str()
                        ),
                    )
                    .hint("give the later doc a new draft ID (fairlead new --reid), or remove the copy")
                    .related(first.path.as_str())
                    .text(later.id.as_str())
                    .build(),
                );
            }
        }
        for group in ordinal_duplicates(&docs) {
            let (Some(first), Some(later)) = (group.first(), group.last()) else {
                continue;
            };
            if in_scope(cx, later, scope) {
                let ord = later.ordinal.unwrap_or_default();
                out.push(
                    DiagB::new(
                        HN_V007,
                        &later.path,
                        1,
                        1,
                        format!(
                            "ordinal {ord:04} of kind {} is also used by {}",
                            later.kind,
                            first.path.as_str()
                        ),
                    )
                    .hint(
                        "reseal the later doc so it gets the next ordinal (fairlead seal --reseal)",
                    )
                    .related(first.path.as_str())
                    .text(format!("{}:{ord}", later.kind))
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    fn change(id: &str, status: &str, extra: &str) -> String {
        format!(
            "+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"{status}\"\nscope = \"root\"\ncreated = \"0.1.0\"\n{extra}+++\n\n# x\n"
        )
    }

    #[test]
    fn r_ver_031__incomplete_ready_doc_is_h_doc_empty() {
        let d = World::new()
            .file(
                "fairlead/docs/changes/d-00000001-a.md",
                &change("d-00000001", "ready", ""),
            )
            .file(
                "fairlead/docs/changes/d-00000002-b.md",
                &change(
                    "d-00000002",
                    "ready",
                    "impact = \"add\"\ncovers = [\"src/**\"]\n",
                ),
            )
            .file(
                "fairlead/docs/changes/d-00000003-c.md",
                &change("d-00000003", "draft", ""),
            )
            .build()
            .run_check(&VersionsCheck);
        let v = of(&d, c::HN_V009);
        assert_eq!(v.len(), 1, "{d:?}");
        assert!(v[0].message.contains("d-00000001") && v[0].message.contains("`covers` is empty"));
    }

    #[test]
    fn r_ver_042__draft_id_collision_and_duplicate_ordinal() {
        let d = World::new()
            .file(
                "fairlead/docs/changes/d-00000001-a.md",
                &change("d-00000001", "draft", ""),
            )
            .file(
                "fairlead/docs/changes/d-00000001-b.md",
                &change("d-00000001", "draft", "impact = \"fix\"\n"),
            )
            .file(
                "fairlead/docs/changes/0007-x.md",
                &change("d-00000005", "sealed", "ordinal = 7\n"),
            )
            .file(
                "fairlead/docs/changes/0007-y.md",
                &change("d-00000006", "sealed", "ordinal = 7\n"),
            )
            .build()
            .run_check(&VersionsCheck);
        assert_eq!(of(&d, c::HN_V006).len(), 1, "{d:?}");
        assert_eq!(of(&d, c::HN_V007).len(), 1, "{d:?}");
    }

    #[test]
    fn r_git_015__union_merge_loss_in_a_ledger_is_h_g003() {
        use crate::hist::YearFile;
        use crate::version::Version;
        let mut f = YearFile::new("root", 2026, None, 1);
        f.append_version(
            Version::new(0, 2, 0),
            &[
                "d-00000001 kind=change".into(),
                "d-00000002 kind=change".into(),
            ],
        )
        .unwrap();
        let good = f.render();
        let broken = good.replace("d-00000002 kind=change", "d-00000003 kind=change");
        let broken = broken
            .lines()
            .filter(|l| !l.starts_with("d-00000001"))
            .map(|l| format!("{l}\n"))
            .collect::<String>();
        let path =
            |y: &str| crate::path::RepoPath::new(&format!("fairlead/ledger/hist/{y}.hl")).unwrap();
        let ok = World::new()
            .facts(|f| {
                f.ledgers.insert(path("2026"), good.clone().into());
            })
            .build()
            .run_check(&VersionsCheck);
        assert!(of(&ok, c::HN_G003).is_empty(), "{ok:?}");
        let bad = World::new()
            .facts(|f| {
                f.ledgers.insert(path("2026"), broken.into());
            })
            .build()
            .run_check(&VersionsCheck);
        assert!(!of(&bad, c::HN_G003).is_empty());
    }

    fn sealed_world(lock_on_disk: Option<&str>, versions: &[&str]) -> Vec<Diagnostic> {
        use crate::generate::{GenKind, generate};
        let mut w = World::new().scope("apps/web", "[fairlead]\nscope = \"web\"\n");
        for v in versions {
            w = w.file(&format!("fairlead/versions/{v}.md"), "+++\n+++\n");
        }
        let lock = lock_on_disk.map(str::to_owned);
        let b = w
            .facts(move |f| {
                if let Some(l) = lock {
                    f.generated_on_disk.insert(
                        crate::path::RepoPath::new("apps/web/fairlead/gen/scope.lock").unwrap(),
                        l.into_bytes().into(),
                    );
                }
            })
            .build();
        let generated = generate(
            &b.registry,
            &b.corpus,
            &b.facts,
            None,
            crate::discover::Edition(1),
        );
        assert!(
            generated
                .files
                .values()
                .any(|f| f.kind == GenKind::ScopeLock),
            "a sealed tree generates scope locks"
        );
        b.run_check_with(&VersionsCheck, &generated)
            .into_iter()
            .filter(|d| d.location.path.as_str().starts_with("apps/web/"))
            .collect()
    }

    #[test]
    fn r_ver_015__no_scope_lock_before_the_first_seal() {
        use crate::generate::{GenKind, generate};
        let b = World::new()
            .scope("apps/web", "[fairlead]\nscope = \"web\"\n")
            .build();
        let g = generate(
            &b.registry,
            &b.corpus,
            &b.facts,
            None,
            crate::discover::Edition(1),
        );
        assert!(g.files.values().all(|f| f.kind != GenKind::ScopeLock));
    }

    #[test]
    fn r_ver_020__ancestor_drift_ramps_info_then_warn() {
        use crate::diag::Severity;
        let current = {
            use crate::generate::generate;
            let b = World::new()
                .scope("apps/web", "[fairlead]\nscope = \"web\"\n")
                .file("fairlead/versions/0.2.0.md", "+++\n+++\n")
                .build();
            let g = generate(
                &b.registry,
                &b.corpus,
                &b.facts,
                None,
                crate::discover::Edition(1),
            );
            g.files
                .values()
                .find(|f| f.path.as_str() == "apps/web/fairlead/gen/scope.lock")
                .unwrap()
                .body
                .clone()
        };
        let same = sealed_world(Some(&current), &["0.2.0"]);
        assert!(of(&same, c::HN_V004).is_empty(), "{same:?}");
        let stale = "ancestor-digest=old\nreconciled=0.2.0\n";
        let fresh = sealed_world(Some(stale), &["0.2.0", "0.3.0"]);
        assert_eq!(
            of(&fresh, c::HN_V004)[0].severity,
            Severity::Info,
            "1 seal behind"
        );
        let old = sealed_world(Some(stale), &["0.2.0", "0.3.0", "0.4.0", "0.5.0"]);
        assert_eq!(
            of(&old, c::HN_V004)[0].severity,
            Severity::Warn,
            "3 seals behind"
        );
        let missing = sealed_world(None, &["0.2.0"]);
        assert!(of(&missing, c::HN_V004)[0].message.contains("missing"));
    }

    #[test]
    fn r_ver_054__edited_sealed_doc_is_tamper() {
        let path = "fairlead/docs/changes/0001-x.md";
        let sealed = |body: &str| {
            format!(
                "+++\nid = \"d-00000001\"\nkind = \"change\"\nstatus = \"sealed\"\nscope = \"root\"\nimpact = \"add\"\ncovers = [\"src/**\"]\nbody-hash = \"{}\"\nordinal = 1\n+++\n{body}",
                crate::docs::body_hash(&format!("+++\nx = 1\n+++\n\n# x\n"))
            )
        };
        let run = |text: String| {
            let t = text.clone();
            World::new()
                .file(path, &text)
                .facts(move |f| {
                    f.doc_body_hashes.insert(
                        crate::path::RepoPath::new(path).unwrap(),
                        crate::docs::body_hash(&t),
                    );
                })
                .build()
                .run_check(&VersionsCheck)
        };
        assert!(
            of(&run(sealed("\n# x\n")), c::HN_V003).is_empty(),
            "untouched body"
        );
        assert_eq!(of(&run(sealed("\n# x, edited\n")), c::HN_V003).len(), 1);
    }

    #[test]
    fn r_ver_040__unusable_change_doc_names_the_key() {
        let d = World::new()
            .file(
                "fairlead/docs/changes/bad.md",
                &change("chg-1", "draft", ""),
            )
            .build()
            .run_check(&VersionsCheck);
        let v = of(&d, c::HN_V009);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("`id`"), "{}", v[0].message);
    }
}
