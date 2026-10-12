//! Retention (33-retention-and-compaction): a closed record that could not compact is reported
//! before the seal refuses it (R-RET-004, `HN-K001` at warn), a child's `[retain]` that would
//! shorten retention is inert (R-RET-013, `HN-C003`), and a decision-grade record deleted by the
//! user is a notice with its digest (R-RET-045).
use fairlead_syntax::diag::{DiagCode, Severity};

use crate::check::builtin::root_control;
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::config::Stage;
use crate::diag::{Diagnostic, codes::*};
use crate::registry::ScopeId;

pub struct RetentionCheck;
const CODES: [DiagCode; 2] = [HN_K001, HN_C003];

impl Check for RetentionCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        if !crate::retention::enabled(reg) {
            return;
        }
        for r in crate::retention::collect(reg, cx.corpus, &cx.facts.approvals)
            .iter()
            .filter(|r| r.scope == *scope && r.closed.is_some())
        {
            let problems = r.problems();
            if problems.is_empty() {
                continue;
            }
            out.push(
                DiagB::new(
                    HN_K001,
                    &r.path,
                    1,
                    1,
                    format!("record {} cannot compact: {}", r.id.as_str(), problems.join("; ")),
                )
                .hint("fill `sum` (and `out`, `why`) now; a seal refuses it once it is due (R-RET-004, R-RET-022)")
                .severity(Severity::Warn)
                .stage(Stage::Warn)
                .text(r.id.as_str().to_owned())
                .build(),
            );
        }
        // R-RET-013: a child's shortening keys are inert
        if let Some(c) = reg
            .scope(scope)
            .and_then(|s| s.control.as_ref())
            .filter(|_| *scope != *reg.root_scope())
            .filter(|c| c.retain.declared)
        {
            let root = root_control(reg)
                .map(|r| r.retain.clone())
                .unwrap_or_default();
            for class in ["chg", "rq", "adr", "grv", "handoff", "brainstorm", "exc"] {
                let mut p = crate::config::retain::root_policy(&root, class);
                for k in crate::config::retain::tighten(&mut p, &c.retain, class) {
                    out.push(
                        DiagB::new(HN_C003, &c.path, 0, 0, format!("[retain] {k}: a child only keeps records longer, so it is inert"))
                            .hint("propose the change to the root's owner with fairlead new escalation (R-RET-013)")
                            .severity(Severity::Warn)
                            .text(k)
                            .build(),
                    );
                }
            }
        }
        // R-RET-045: a decision-grade record deleted by the user
        if *scope == *reg.root_scope() {
            let lines = crate::retention::compacted(&cx.facts.ledgers);
            for (path, text) in &cx.facts.tombstones {
                for block in text.split("[[tomb]]").skip(1) {
                    if !block.contains("deleted-by = \"user\"") {
                        continue;
                    }
                    let Some(id) = block
                        .lines()
                        .find_map(|l| l.trim().strip_prefix("id = \""))
                        .and_then(|r| r.strip_suffix('"'))
                    else {
                        continue;
                    };
                    let decision = lines.get(id).is_some_and(|c| {
                        c.line.contains("out=rejected")
                            || c.line.contains("out=removed")
                            || c.line.contains(" dnr=1")
                    }) || block.contains("kind = \"adr\"");
                    if decision {
                        out.push(
                            DiagB::new(HN_K001, path, 0, 0, format!("decision-grade record {id} was deleted by the user; its re-introduction protection weakens"))
                                .hint("restore it from git, or keep its graveyard entry (R-RET-045)")
                                .severity(Severity::Info)
                                .text(id.to_owned())
                                .build(),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    fn sealed(sum: &str) -> String {
        format!(
            "+++\nid = \"d-00000001\"\nkind = \"change\"\nstatus = \"sealed\"\nsealed = \"0.1.0\"\nordinal = 1\nimpact = \"fix\"\ncovers = [\"src/**\"]\nsum = \"{sum}\"\nout = \"\"\n+++\n# A\n"
        )
    }

    #[test]
    fn r_ret_004__closed_record_missing_sum_warns() {
        let run = |retain: &str, sum: &str| {
            World::new()
                .root(retain)
                .file("fairlead/docs/changes/0001-a.md", &sealed(sum))
                .build()
                .run_check(&RetentionCheck)
        };
        let ds = run("[retain]\n", "");
        let k = of(&ds, c::HN_K001);
        assert_eq!(k.len(), 1, "{ds:#?}");
        assert_eq!(k[0].severity, Severity::Warn);
        assert!(k[0].message.contains("`sum` is empty"));
        assert!(of(&run("[retain]\n", "done"), c::HN_K001).is_empty());
        assert!(
            of(&run("", ""), c::HN_K001).is_empty(),
            "retention off: nothing reported"
        );
    }

    #[test]
    fn r_ret_013__a_shortening_child_override_is_inert() {
        let ds = World::new()
            .root("[retain]\n")
            .scope(
                "web",
                "[fairlead]\nscope = \"web\"\n[retain.chg]\nttl = \"2v\"\nhot = 6\n",
            )
            .build()
            .run_check(&RetentionCheck);
        let x = of(&ds, c::HN_C003);
        assert_eq!(x.len(), 1, "only the shortening key: {ds:#?}");
        assert!(
            x[0].message
                .contains("retain.chg.ttl = 2v is shorter than 8v")
        );
    }

    #[test]
    fn r_ret_045__a_deleted_decision_grade_record_is_a_notice() {
        let line = "d-00000001 kind=change scope=root title=x closed=0.1.0 out=rejected sum=s why=w git=- body=b";
        let ds = World::new()
            .root("[retain]\n")
            .facts(move |f| {
                f.ledgers.insert(
                    crate::path::RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
                    std::sync::Arc::from(format!(
                        "# scope=root year=2026 prev=- edition=1\n{line} d={}\n",
                        crate::hist::record_digest(line)
                    )),
                );
                f.tombstones.insert(
                    crate::path::RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                    std::sync::Arc::from("[[tomb]]\nid = \"d-00000001\"\nremoved = \"2026-03-01\"\nwhy = \"x\"\ndeleted-by = \"user\"\n"),
                );
            })
            .build()
            .run_check(&RetentionCheck);
        let k = of(&ds, c::HN_K001);
        assert_eq!(k.len(), 1, "{ds:#?}");
        assert_eq!(k[0].severity, Severity::Info);
    }
}
