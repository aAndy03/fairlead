//! Docs-first path coverage (R-VER-032 to R-VER-034, R-CHK-034, R-CHK-045, D-15): every file
//! changed since the coverage base is covered by a `draft` or `ready` change doc. It proves docs
//! are present, not that they came first (R-VER-039).
use crate::{
    check::{CheckCx, finding::DiagB, rules::Check},
    classify::FileClass,
    diag::{Diagnostic, codes::*},
    docs::{Status, collect_change_docs, covers_path},
    facts::Coverage,
    path::RepoPath,
    registry::ScopeId,
};
use fairlead_syntax::diag::DiagCode;

pub struct CoverageCheck;
const CODES: [DiagCode; 2] = [HN_D006, HN_V010];

/// Files coverage never asks about: the seal's and the tool's own outputs, the change docs
/// themselves, and the cache (R-VER-033).
fn exempt(cx: &CheckCx<'_>, folder: &str, p: &RepoPath) -> bool {
    let s = p.as_str();
    if s.starts_with(".fairlead/") || s.starts_with(&format!("{folder}/versions/")) {
        return true;
    }
    if s.contains(&format!("{folder}/docs/changes/")) || s.contains(&format!("{folder}/ledger/")) {
        return true;
    }
    cx.corpus
        .records
        .get(p)
        .is_some_and(|r| matches!(r.class, FileClass::Generated | FileClass::VendorOwned))
}

impl Check for CoverageCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(coverage) = &cx.facts.git.coverage else {
            return; // nothing sealed yet: docs-first starts with the first seal
        };
        let folder = crate::check::builtin::root_control(reg)
            .and_then(|c| c.settings.folder.clone())
            .unwrap_or_else(|| "fairlead".into());
        let files = match coverage {
            Coverage::Unavailable(why) => {
                if scope == reg.root_scope()
                    && let Some(cp) = crate::check::builtin::control_path(reg, scope)
                {
                    let msg = format!("docs-first coverage is unavailable: {why}");
                    out.push(
                        DiagB::new(HN_V010, &cp, 0, 0, msg.clone())
                            .hint("fetch full history (fetch-depth: 0 in CI) so the coverage base is reachable")
                            .text(msg)
                            .build(),
                    );
                }
                return;
            }
            Coverage::Changed { files, .. } => files,
        };
        let (docs, _) = collect_change_docs(cx.corpus);
        let open: Vec<_> = docs
            .iter()
            .filter(|d| matches!(d.status, Status::Draft | Status::Ready))
            .map(|d| {
                let dir = reg
                    .scope_of(&d.path)
                    .and_then(|s| reg.scope(s))
                    .map(|s| s.dir.as_str().to_owned())
                    .unwrap_or_default();
                (d, dir)
            })
            .collect();
        for p in files {
            if reg.scope_of(p) != Some(scope) || exempt(cx, &folder, p) {
                continue;
            }
            if open.iter().any(|(d, dir)| covers_path(d, dir, p)) {
                continue;
            }
            out.push(
                DiagB::new(
                    HN_D006,
                    p,
                    0,
                    0,
                    "changed file is not covered by a draft or ready change doc",
                )
                .hint("fairlead new change \"<title>\" --field covers=<glob>, or add it to an open doc's covers")
                .text(p.as_str())
                .build(),
            );
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;
    use std::collections::BTreeSet;

    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    fn doc(id: &str, status: &str, covers: &str) -> String {
        format!(
            "+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"{status}\"\nscope = \"root\"\ncovers = [{covers}]\n+++\n"
        )
    }

    fn run(changed: &[&str], docs: &[(&str, String)]) -> Vec<Diagnostic> {
        let files: BTreeSet<RepoPath> = changed.iter().map(|p| rp(p)).collect();
        let mut w = World::new()
            .scope("apps/web", "[fairlead]\nscope = \"web\"\n")
            .facts(move |f| {
                f.git.coverage = Some(Coverage::Changed {
                    base: "b".into(),
                    files,
                });
            });
        for p in changed {
            w = w.file(p, "x\n");
        }
        for (p, t) in docs {
            w = w.file(p, t);
        }
        w.build().run_check(&CoverageCheck)
    }

    #[test]
    fn r_ver_032__one_doc_covers_a_shared_file_and_uncovered_files_warn() {
        let d = run(
            &["src/a.rs", "src/b.rs", "docs/x.md"],
            &[(
                "fairlead/docs/changes/d-00000001-a.md",
                doc("d-00000001", "draft", "\"src/**\""),
            )],
        );
        let u = of(&d, c::HN_D006);
        assert_eq!(u.len(), 1, "{d:?}");
        assert_eq!(u[0].location.path.as_str(), "docs/x.md");
    }

    #[test]
    fn r_ver_032__abandoned_and_sealed_docs_do_not_cover() {
        for status in ["abandoned", "sealed"] {
            let d = run(
                &["src/a.rs"],
                &[(
                    "fairlead/docs/changes/d-00000001-a.md",
                    doc("d-00000001", status, "\"src/**\""),
                )],
            );
            assert_eq!(of(&d, c::HN_D006).len(), 1, "{status}");
        }
    }

    #[test]
    fn r_ver_030__covers_globs_are_relative_to_the_home_scope() {
        let web_doc = (
            "apps/web/fairlead/docs/changes/d-00000002-w.md",
            doc("d-00000002", "ready", "\"src/**\""),
        );
        let d = run(&["apps/web/src/x.ts", "src/a.rs"], &[web_doc]);
        let u = of(&d, c::HN_D006);
        assert_eq!(u.len(), 1, "{d:?}");
        assert_eq!(
            u[0].location.path.as_str(),
            "src/a.rs",
            "a web doc never covers root files"
        );
    }

    #[test]
    fn r_ver_033__own_outputs_and_change_docs_are_exempt() {
        let d = run(
            &[
                "fairlead/docs/changes/d-00000001-a.md",
                "fairlead/versions/0.1.0.md",
                "fairlead/ledger/hist/2026.hl",
            ],
            &[],
        );
        assert!(of(&d, c::HN_D006).is_empty(), "{d:?}");
    }

    #[test]
    fn r_git_040__unavailable_coverage_is_reported_never_a_pass() {
        let d = World::new()
            .facts(|f| f.git.coverage = Some(Coverage::Unavailable("shallow".into())))
            .build()
            .run_check(&CoverageCheck);
        assert_eq!(of(&d, c::HN_V010).len(), 1);
    }
}
