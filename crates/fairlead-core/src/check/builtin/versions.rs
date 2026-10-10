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
const CODES: [DiagCode; 3] = [HN_V006, HN_V007, HN_V009];

fn in_scope(cx: &CheckCx<'_>, d: &ChangeDoc, scope: &ScopeId) -> bool {
    cx.registry.scope_of(&d.path) == Some(scope)
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
