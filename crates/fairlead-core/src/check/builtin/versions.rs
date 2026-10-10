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
const CODES: [DiagCode; 4] = [HN_G003, HN_V006, HN_V007, HN_V009];

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
        ledger_continuity(cx, scope, out);
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
