//! Check engine (P5a). Pure: no filesystem, no git, no clock (R-ARCH-050, R-CHK-004).
pub(crate) mod apply;
pub mod baseline;
pub mod builtin;
pub mod codes;
pub mod finding;
pub mod fingerprint;
pub mod floor;
pub mod options;
pub mod rules;
pub mod summary;

pub use baseline::{AcceptRefusal, Baseline, BaselineEntry, BaselineIssue};
pub use finding::{Finding, FindingState, RuleId};
pub use options::{CheckOptions, MaxFindings};
pub use rules::{Check, DeclaredRule, RuleCheck, RuleSet};
pub use summary::{Summary, WaiverCount};

use crate::diag::Diagnostic;
use crate::discover::Edition;
use crate::extract::Corpus;
use crate::facts::Facts;
use crate::generate::{GeneratedSet, generate};
use crate::hash::Fingerprint;
use crate::path::RepoPath;
use crate::registry::{Registry, ScopeId, ScopeState};
use crate::time::AsOf;
use options::Deadline;
use rayon::prelude::*;
use std::collections::BTreeSet;

pub const FAIRLEAD_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct ReportHeader {
    pub fairlead_version: String,
    pub edition: Edition,
    pub as_of: AsOf,
    pub scopes: u32,
    pub files: u32,
    pub rules: u32,
}

pub struct CheckReport {
    pub header: ReportHeader,
    /// ALL findings, totally ordered; truncation is a render concern (R-ARCH-051).
    pub findings: Vec<Finding>,
    pub summary: Summary,
    /// `--budget` deadline hit (R-CLI-023).
    pub partial: bool,
    /// Unresolved or sparse scopes: the verdict depends on them (R-CHK-057).
    pub unknown_scopes: Vec<crate::registry::ScopeId>,
    /// The subset of `unknown_scopes` absent from the working tree: exit 5, not 2 (A-45).
    pub absent_scopes: Vec<crate::registry::ScopeId>,
    /// Zero declarations notice (R-CHK-003).
    pub zero_declarations: bool,
}

/// Everything `run` reads (contract 4.9). `load_diags` are the walk and control-file diagnostics.
pub struct CheckInput<'a> {
    pub registry: &'a Registry,
    pub corpus: &'a Corpus,
    pub facts: &'a Facts,
    pub load_diags: &'a [crate::diag::Diagnostic],
    pub baseline: Option<&'a Baseline>,
    pub options: &'a CheckOptions,
}

/// R-ARCH-102: call before `run`; `Err` maps to exit 5, never to a pass (A-17).
pub fn preflight(input: &CheckInput<'_>) -> Result<(), crate::error::Error> {
    let g = &input.facts.git;
    if input.options.changed {
        if let Some(why) = &g.unreadable {
            return Err(crate::error::Error::CannotDetermine(format!(
                "git state is unreadable: {why}"
            )));
        }
        if g.changed.is_none() {
            return Err(crate::error::Error::CannotDetermine(
                "--changed needs a git repository".to_string(),
            ));
        }
    }
    Ok(())
}

/// Read-only bundle handed to each `Check` (everything pure).
pub struct CheckCx<'a> {
    pub registry: &'a Registry,
    pub corpus: &'a Corpus,
    pub facts: &'a Facts,
    pub generated: &'a GeneratedSet,
    pub rules: &'a RuleSet,
    pub options: &'a CheckOptions,
}

static CHECKS: [&dyn Check; 13] = [
    &builtin::syntax::SyntaxCheck,
    &builtin::refs::RefsCheck,
    &builtin::tokens::TokensCheck,
    &builtin::leak::LeakCheck,
    &rules::PatternRules,
    &builtin::generated::GeneratedCheck,
    &builtin::exceptions::ExceptionsCheck,
    &builtin::controls::ControlsCheck,
    &builtin::scopes::ScopesCheck,
    &builtin::agents::AgentsCheck,
    &builtin::vendor::VendorCheck,
    &builtin::paths::PathsCheck,
    &builtin::notices::NoticesCheck,
];

/// Every built-in check, in a fixed order (the report order comes from sorting, not from this).
pub fn builtin_checks() -> &'static [&'static dyn Check] {
    &CHECKS
}

fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Pure (R-ARCH-050). Stage x severity ramp, waivers, baseline and filters applied; findings
/// totally ordered. Unresolved scopes get no checks (fail closed) and are listed as unknown.
pub fn run(input: &CheckInput<'_>) -> CheckReport {
    let (reg, opts) = (input.registry, input.options);
    let baseline = input.baseline.filter(|_| !opts.no_baseline);
    let edition = builtin::edition_of(reg);
    let generated = generate(reg, input.corpus, input.facts, baseline, edition);
    let rules = RuleSet::collect(reg);
    let cx = CheckCx {
        registry: reg,
        corpus: input.corpus,
        facts: input.facts,
        generated: &generated,
        rules: &rules,
        options: opts,
    };
    let deadline = Deadline::start(opts.budget);
    let scopes: Vec<&ScopeId> = reg
        .scopes()
        .filter(|s| matches!(s.state, ScopeState::Resolved))
        .map(|s| &s.id)
        .collect();
    let per_scope: Vec<(Vec<Diagnostic>, bool)> = scopes
        .par_iter()
        .map(|sid| {
            if deadline.expired() {
                return (Vec::new(), true);
            }
            let mut out = Vec::new();
            for c in builtin_checks() {
                c.run(&cx, sid, &mut out);
            }
            (out, false)
        })
        .collect();
    let partial = per_scope.iter().any(|(_, skipped)| *skipped);
    let mut diags: Vec<Diagnostic> = input.load_diags.to_vec();
    diags.extend(reg.diagnostics().iter().cloned());
    diags.extend(per_scope.into_iter().flat_map(|(d, _)| d));

    let mut findings = apply::lift(diags, reg, &rules);
    let days = builtin::exceptions::horizon_days(reg);
    let wcx = apply::WaiverCx {
        reg,
        corpus: input.corpus,
        rules: &rules,
        as_of: opts.as_of,
        days,
    };
    let hits = apply::apply_waivers(&mut findings, &wcx);
    let whole_run = !opts.is_partial_view() && !partial;
    if whole_run {
        let extra = apply::lift(apply::post_exceptions(&findings, &hits, &wcx), reg, &rules);
        findings.extend(extra);
    }
    if let Some(b) = baseline {
        let seen: BTreeSet<Fingerprint> = findings.iter().map(|f| f.fingerprint.clone()).collect();
        apply::apply_baseline(&mut findings, b);
        if let Ok(bp) = RepoPath::new(&builtin::baseline_rel(reg)) {
            let extra = apply::lift(apply::baseline_diags(&seen, b, &bp, whole_run), reg, &rules);
            findings.extend(extra);
        }
    }
    let keep = apply::ReportScope::new(opts, input.facts);
    findings.retain(|f| keep.keeps(f));
    findings.sort();
    let summary = summary::summarize(&findings, reg, &rules, opts, input.facts, baseline, days);
    let unknown = |absent_only: bool| {
        reg.scopes()
            .filter(|s| match s.state {
                ScopeState::Resolved => false,
                ScopeState::AbsentFromTree => true,
                ScopeState::Unresolved { .. } => !absent_only,
            })
            .map(|s| s.id.clone())
            .collect::<Vec<_>>()
    };
    CheckReport {
        header: ReportHeader {
            fairlead_version: FAIRLEAD_VERSION.to_string(),
            edition,
            as_of: opts.as_of,
            scopes: n32(reg.scopes().count()),
            files: n32(input.corpus.records.len()),
            rules: rules.total(),
        },
        findings,
        summary,
        partial,
        unknown_scopes: unknown(false),
        absent_scopes: unknown(true),
        zero_declarations: reg.view(reg.root_scope()).items.is_empty(),
    }
}

#[cfg(test)]
pub(crate) mod testkit;

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::{Built, World};

    fn input<'a>(b: &'a Built, o: &'a CheckOptions) -> CheckInput<'a> {
        CheckInput {
            registry: &b.registry,
            corpus: &b.corpus,
            facts: &b.facts,
            load_diags: &[],
            baseline: None,
            options: o,
        }
    }

    #[test]
    fn r_arch_102__unreadable_git_under_changed_is_cannot_determine() {
        use crate::error::Error::CannotDetermine;
        let mut b = World::new().build();
        let mut o = b.options();
        assert!(preflight(&input(&b, &o)).is_ok()); // not asked for --changed
        o.changed = true;
        assert!(matches!(preflight(&input(&b, &o)), Err(CannotDetermine(_)))); // no git facts
        b.facts.git.changed = Some(crate::facts::ChangedSet(Default::default()));
        assert!(preflight(&input(&b, &o)).is_ok());
        b.facts.git.unreadable = Some("index locked".into());
        assert!(
            matches!(preflight(&input(&b, &o)), Err(CannotDetermine(m)) if m.contains("index locked"))
        );
    }

    use crate::{
        check::baseline::{Baseline, BaselineEntry},
        diag::codes as c,
        outcome::ExitCode,
    };
    use fairlead_syntax::diag::Severity;
    use std::time::Duration;

    const KINDS: &str = "[kinds.cmp]\ntitle = \"c\"\n";
    fn report(b: &Built, load: &[Diagnostic], f: impl FnOnce(&mut CheckOptions)) -> CheckReport {
        let mut o = b.options();
        f(&mut o);
        run(&CheckInput {
            registry: &b.registry,
            corpus: &b.corpus,
            facts: &b.facts,
            load_diags: load,
            baseline: b.baseline.as_ref(),
            options: &o,
        })
    }
    const A: &str = "// fairlead: use cmp.nope\nexport const a = 1;\n";
    const B: &str = "// fairlead: use cmp.ghost\nexport const b = 1;\n";
    fn dangling() -> World {
        World::new()
            .root(KINDS)
            .file("src/b.tsx", B)
            .file("src/a.tsx", A)
    }

    #[test]
    fn r_chk_003__an_empty_declaration_set_is_nothing_verified_and_exit_one_only_when_required() {
        let b = World::new().build().synced();
        let r = report(&b, &[], |_| {});
        assert!(r.zero_declarations && r.findings.iter().any(|f| f.code == c::HN_N001));
        assert_eq!(r.outcome(&b.options()).exit_code(), ExitCode::Ok);
        let mut o = b.options();
        o.require_declarations = true;
        assert_eq!(r.outcome(&o).exit_code(), ExitCode::Findings);
    }

    #[test]
    fn r_chk_001__r_chk_040__r_arch_090__a_dangling_reference_fails_the_run_with_ordered_findings()
    {
        let b = dangling().build().synced();
        let r = report(&b, &[], |_| {});
        let at: Vec<_> = r
            .findings
            .iter()
            .filter(|f| f.code == c::HN_R001)
            .map(|f| f.location.path.as_str())
            .collect();
        assert_eq!(at, ["src/a.tsx", "src/b.tsx"], "{:#?}", r.findings);
        assert!(r.findings.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(r.outcome(&b.options()).exit_code(), ExitCode::Findings);
        assert_eq!(
            (r.header.scopes, r.header.files, r.header.as_of),
            (1, 2, b.as_of)
        );
    }

    #[test]
    fn r_chk_011__input_order_and_thread_count_do_not_change_the_bytes() {
        let a = dangling().build().synced();
        let b = World::new()
            .root(KINDS)
            .file("src/a.tsx", A)
            .file("src/b.tsx", B)
            .build()
            .synced();
        let (ra, rb) = (report(&a, &[], |_| {}), report(&b, &[], |_| {}));
        assert_eq!(ra.findings, rb.findings);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        // R-ARCH-080
        assert_eq!(
            pool.install(|| report(&a, &[], |_| {})).findings,
            ra.findings
        );
    }

    #[test]
    fn r_chk_002__a_path_scoped_run_equals_the_full_run_filtered() {
        let b = dangling().build().synced();
        let full = report(&b, &[], |_| {});
        let one = report(&b, &[], |o| {
            o.paths = vec![RepoPath::new("src/a.tsx").unwrap()];
        });
        let want: Vec<_> = full
            .findings
            .iter()
            .filter(|f| f.location.path.as_str() == "src/a.tsx")
            .cloned()
            .collect();
        assert_eq!(one.findings, want);
    }

    #[test]
    fn r_cli_023__an_expired_budget_is_partial_and_never_exit_zero() {
        let b = dangling().build().synced();
        let r = report(&b, &[], |o| o.budget = Some(Duration::ZERO));
        assert!(r.partial);
        assert_eq!(r.outcome(&b.options()).exit_code(), ExitCode::Usage);
    }

    #[test]
    fn r_chk_030__a_baselined_finding_no_longer_fails_and_a_fixed_entry_is_b001() {
        let first = report(&dangling().build(), &[], |_| {});
        let target = first
            .findings
            .iter()
            .find(|f| f.code == c::HN_R001)
            .unwrap();
        let mut base = Baseline::default();
        base.entries.insert(
            target.fingerprint.clone(),
            BaselineEntry {
                code: target.code,
                path: target.location.path.clone(),
                accepted: None,
            },
        );
        let b = World::new()
            .root(KINDS)
            .file("src/a.tsx", A)
            .baseline(base)
            .build()
            .synced();
        let r = report(&b, &[], |_| {});
        assert_eq!(
            (r.summary.baselined, r.outcome(&b.options()).exit_code()),
            (1, ExitCode::Ok),
            "{:#?}",
            r.findings
        );
        let fixed = World::new()
            .root(KINDS)
            .file("src/a.tsx", "export const a = 1;\n")
            .baseline(b.baseline.clone().unwrap())
            .build()
            .synced();
        let has_b001 = |r: &CheckReport| r.findings.iter().any(|f| f.code == c::HN_B001);
        assert!(has_b001(&report(&fixed, &[], |_| {})));
        assert!(!has_b001(&report(&fixed, &[], |o| o.no_baseline = true)));
    }

    #[test]
    fn r_arch_055__load_diagnostics_pass_through_floor_stays_enforce_and_inactive_codes_vanish() {
        use crate::check::finding::DiagB;
        let b = World::new().root(KINDS).build().synced();
        let p = RepoPath::new("CON.txt").unwrap();
        let load = vec![
            DiagB::new(c::HN_P001, &p, 0, 0, "reserved name")
                .severity(Severity::Info)
                .build(),
            DiagB::new(c::HN_V001, &p, 0, 0, "later milestone").build(),
        ];
        let r = report(&b, &load, |_| {});
        let floor = r.findings.iter().find(|f| f.code == c::HN_P001).unwrap();
        assert_eq!(
            (floor.severity, floor.stage),
            (Severity::Error, crate::config::Stage::Enforce)
        );
        assert!(r.findings.iter().all(|f| f.code != c::HN_V001));
    }

    macro_rules! sources { ($($p:literal),* $(,)?) => { [$(($p, include_str!($p))),*] }; }
    const FILES: [(&str, &str); 23] = sources!(
        "mod.rs",
        "codes.rs",
        "options.rs",
        "finding.rs",
        "fingerprint.rs",
        "floor.rs",
        "rules.rs",
        "summary.rs",
        "apply.rs",
        "testkit.rs",
        "builtin/mod.rs",
        "builtin/agents.rs",
        "builtin/controls.rs",
        "builtin/exceptions.rs",
        "builtin/generated.rs",
        "builtin/leak.rs",
        "builtin/notices.rs",
        "builtin/paths.rs",
        "builtin/refs.rs",
        "builtin/scopes.rs",
        "builtin/syntax.rs",
        "builtin/tokens.rs",
        "builtin/vendor.rs",
    );
    const LITERALS: &str = include_str!("../extract/literals.rs");
    const SECRETS: &str = include_str!("../secrets.rs");
    /// Needles are assembled at run time so this file does not trip itself.
    fn n(a: &str, b: &str) -> String {
        [a, b].concat()
    }

    #[test]
    fn r_arch_055__no_code_string_literals_outside_the_diag_tables() {
        let needle = n("\"HN", "-");
        for (name, src) in FILES.iter().chain([("literals.rs", LITERALS)].iter()) {
            assert!(!src.contains(&needle), "{name} contains a code literal");
        }
    }

    #[test]
    fn r_chk_004__r_cli_024__r_arch_050__check_is_pure_has_one_clock_and_no_unordered_maps() {
        let banned = [
            n("std", "::fs"),
            n("std", "::env"),
            n("std", "::process"),
            n("std", "::net"),
            n("System", "Time"),
            n("Hash", "Map"),
            n("Hash", "Set"),
            n("uns", "afe"),
        ];
        for (name, src) in FILES.iter().chain([("literals.rs", LITERALS)].iter()) {
            for b in &banned {
                assert!(!src.contains(b.as_str()), "{name} uses {b}");
            }
            if *name != "options.rs" {
                assert!(
                    !src.contains(&n("Instant", "::now")),
                    "{name} reads the clock"
                );
            }
        }
    }

    /// A-14: no extraction or secret stub may ship.
    #[test]
    fn r_agt_022__no_p5_stub_is_left_in_the_scanners() {
        let needle = n("STUB", "(P5)");
        for (name, src) in [("literals.rs", LITERALS), ("secrets.rs", SECRETS)] {
            assert!(!src.contains(&needle), "{name} still holds a stub");
        }
    }
}
