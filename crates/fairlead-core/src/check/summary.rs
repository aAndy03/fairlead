//! Summary and exit mapping (R-CHK-028/029, R-CHK-040, C-22).
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::{DiagCode, Severity};

use crate::check::baseline::Baseline;
use crate::check::builtin::{exceptions::is_effective, root_control};
use crate::check::options::CheckOptions;
use crate::check::rules::RuleSet;
use crate::check::{CheckReport, Finding, FindingState};
use crate::config::Stage;
use crate::diag::codes::{HN_A002, HN_G001};
use crate::facts::Facts;
use crate::outcome::{ExitCode, VerbOutcome};
use crate::registry::{ApprovalStatus, OvrState, Registry, ScopeId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaiverCount {
    pub active: u32,
    pub expiring_30d: u32,
    pub budget: u32,
    pub renewals: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub errors: u32,
    pub warns: u32,
    pub infos: u32,
    pub baselined: u32,
    pub waived: u32,
    pub observe: u32,
    pub accepted: u32,
    pub waivers: BTreeMap<ScopeId, WaiverCount>,
    pub relaxations: Vec<String>,
    pub exceptions_delta: i32,
    pub codeowners_uncovered: u32,
    pub requests_open: u32,
    pub oldest_request_days: Option<u32>,
}

#[allow(dead_code, reason = "wired by P5a-T29; remove once used")]
pub(crate) fn count(fs: &[Finding]) -> Summary {
    let mut s = Summary::default();
    for f in fs {
        match f.state {
            FindingState::Baselined => s.baselined += 1,
            FindingState::Waived => s.waived += 1,
            FindingState::New | FindingState::WaiverIneffective if f.stage == Stage::Observe => {
                s.observe += 1
            }
            FindingState::New | FindingState::WaiverIneffective => match f.severity {
                Severity::Error => s.errors += 1,
                Severity::Warn => s.warns += 1,
                Severity::Info => s.infos += 1,
            },
        }
    }
    s
}

fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::Observe => "observe",
        Stage::Warn => "warn",
        Stage::Enforce => "enforce",
    }
}
fn sev_name(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}
fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The full summary of a run: counts, waivers per scope, relaxations, exception delta, requests,
/// CODEOWNERS coverage (R-CHK-028/029, R-AGT-011/033/034, R-SCOPE-048).
#[allow(dead_code, reason = "wired by P5a-T29; remove once used")]
pub(crate) fn summarize(
    fs: &[Finding],
    reg: &Registry,
    rules: &RuleSet,
    opts: &CheckOptions,
    facts: &Facts,
    baseline: Option<&Baseline>,
    horizon: u32,
) -> Summary {
    let mut s = count(fs);
    s.accepted = baseline.map_or(0, |b| {
        n32(b.entries.values().filter(|e| e.accepted.is_some()).count())
    });
    let budget = root_control(reg).map_or(25, |c| c.policies.waiver.budget);
    let soon = opts.as_of.0.checked_add(jiff::Span::new().days(30)).ok();
    for o in reg
        .overrides()
        .iter()
        .filter(|o| is_effective(o, opts.as_of, horizon))
    {
        let w = s
            .waivers
            .entry(o.declared_in.clone())
            .or_insert_with(|| WaiverCount {
                budget,
                ..WaiverCount::default()
            });
        w.active += 1;
        if o.exp.zip(soon).is_some_and(|(e, lim)| e <= lim) {
            w.expiring_30d += 1;
        }
        if let ApprovalStatus::Approved { renewal } = o.approval {
            w.renewals += renewal;
        }
    }
    // E-14 (A-52): a relaxation is a scope whose effective rule sits below the rule as declared.
    let mut rel = BTreeSet::new();
    for sc in reg.scopes() {
        for r in rules.for_scope(&sc.id).iter().filter(|r| r.home != sc.id) {
            let Some(d) = rules
                .for_scope(&r.home)
                .iter()
                .find(|d| d.id == r.id && d.home == r.home)
            else {
                continue;
            };
            let (name, at) = (r.id.0.as_str(), sc.id.0.as_str());
            if r.stage < d.stage {
                rel.insert(format!(
                    "rules.{name} stage={} in {at} (declared {})",
                    stage_name(r.stage),
                    stage_name(d.stage)
                ));
            }
            if r.sev < d.sev {
                rel.insert(format!(
                    "rules.{name} sev={} in {at} (declared {})",
                    sev_name(r.sev),
                    sev_name(d.sev)
                ));
            }
        }
    }
    for a in reg.grants().iter().filter(|a| a.valid) {
        let scopes = a
            .for_scopes
            .iter()
            .map(|x| x.0.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let field = a
            .field
            .as_ref()
            .map(|f| format!(".{f}"))
            .unwrap_or_default();
        rel.insert(format!(
            "allow {scopes} {}s.{}{field} exp {}",
            a.target.kind.as_str(),
            a.target.name.as_str(),
            a.exp
        ));
    }
    s.relaxations = rel.into_iter().collect();
    let live = reg
        .overrides()
        .iter()
        .filter(|o| matches!(o.state, OvrState::Effective))
        .count();
    let on_disk: usize = facts
        .generated_on_disk
        .iter()
        .filter(|(p, _)| p.as_str().ends_with("gen/reg/exc.reg"))
        .map(|(_, b)| {
            String::from_utf8_lossy(b)
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .count()
        })
        .sum();
    s.exceptions_delta =
        i32::try_from(live).unwrap_or(i32::MAX) - i32::try_from(on_disk).unwrap_or(i32::MAX);
    s.codeowners_uncovered = n32(fs.iter().filter(|f| f.code == HN_A002).count());
    s.requests_open = n32(reg.requests().len());
    s
}

fn family(code: DiagCode) -> Option<u8> {
    code.0.as_bytes().get(3).copied()
}

impl CheckReport {
    /// R-CHK-040 mapping, exactly as C-22; precedence is `ExitCode::resolve`'s.
    pub fn outcome(&self, opts: &CheckOptions) -> VerbOutcome {
        let live = |f: &&Finding| {
            matches!(f.state, FindingState::New | FindingState::WaiverIneffective)
                && f.stage != Stage::Observe
        };
        // A-45: an unresolved scope is a usage error; one absent from the tree is cannot-determine.
        let unresolved = self
            .unknown_scopes
            .iter()
            .any(|s| !self.absent_scopes.contains(s));
        let (mut usage, mut findings, mut stale) = (unresolved, false, false);
        for f in self.findings.iter().filter(live) {
            usage |= f.severity == Severity::Error && matches!(family(f.code), Some(b'S' | b'C'));
            findings |= (f.severity == Severity::Error && f.stage == Stage::Enforce)
                || (opts.ramp.deny_warnings && f.severity >= Severity::Warn);
            stale |= f.code == HN_G001;
        }
        findings |= self.zero_declarations && opts.require_declarations;
        let mut o = VerbOutcome::ok();
        if usage {
            o.signal(ExitCode::Usage);
        }
        if findings {
            o.signal(ExitCode::Findings);
        }
        if stale {
            o.signal(ExitCode::StaleGenerated);
        }
        if !self.absent_scopes.is_empty() {
            o.signal(ExitCode::Internal);
        }
        if self.partial && !(usage || findings || stale) {
            o.signal(ExitCode::Usage);
        }
        o
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::{
        check::{
            FindingState::{Baselined as Ba, New as Nw, Waived as Wv, WaiverIneffective as Wi},
            ReportHeader, RuleSet,
            testkit::{World, finding},
        },
        config::Stage::{Enforce as En, Observe as Ob, Warn as Wn},
        diag::codes as c,
        time::AsOf,
    };
    use fairlead_syntax::{
        diag::Severity::{Error as E, Info as I, Warn as W},
        ids::ScopeSlug,
    };

    fn report(findings: Vec<Finding>) -> CheckReport {
        let as_of = AsOf::parse("2026-10-09").unwrap();
        CheckReport {
            header: ReportHeader {
                fairlead_version: "0.1.0".into(),
                edition: crate::discover::Edition(1),
                as_of,
                scopes: 1,
                files: 0,
                rules: 0,
            },
            findings,
            summary: Summary::default(),
            partial: false,
            unknown_scopes: Vec::new(),
            absent_scopes: Vec::new(),
            zero_declarations: false,
        }
    }
    fn code_of(r: &CheckReport, deny: bool) -> ExitCode {
        let mut o = CheckOptions::new(r.header.as_of);
        o.ramp.deny_warnings = deny;
        r.outcome(&o).exit_code()
    }
    fn sid(s: &str) -> ScopeId {
        ScopeId(ScopeSlug::parse(s).unwrap())
    }

    #[test]
    fn r_chk_028__counts_split_by_state_stage_and_severity() {
        let fs = vec![
            finding("a", 1, c::HN_R001, E, En, Nw),
            finding("a", 2, c::HN_X004, W, Wn, Nw),
            finding("a", 3, c::HN_X008, I, Wn, Nw),
            finding("a", 4, c::HN_T001, E, Ob, Nw),
            finding("a", 5, c::HN_R001, E, En, Ba),
            finding("a", 6, c::HN_R001, E, En, Wv),
            finding("a", 7, c::HN_R001, E, En, Wi),
        ];
        let s = count(&fs);
        assert_eq!(
            (s.errors, s.warns, s.infos, s.observe, s.baselined, s.waived),
            (2, 1, 1, 1, 1, 1)
        );
    }

    #[test]
    fn r_chk_040__exit_mapping_follows_c22() {
        let r = |fs| report(fs);
        assert_eq!(code_of(&r(vec![]), false), ExitCode::Ok);
        let err = finding("a", 1, c::HN_R001, E, En, Nw);
        assert_eq!(code_of(&r(vec![err.clone()]), false), ExitCode::Findings);
        let quiet = vec![
            finding("a", 1, c::HN_R001, E, En, Ba),
            finding("a", 2, c::HN_R001, E, En, Wv),
            finding("a", 3, c::HN_T001, E, Ob, Nw),
        ];
        assert_eq!(code_of(&r(quiet), false), ExitCode::Ok);
        let warn = r(vec![finding("a", 1, c::HN_X004, W, Wn, Nw)]);
        assert_eq!(
            (code_of(&warn, false), code_of(&warn, true)),
            (ExitCode::Ok, ExitCode::Findings)
        );
        let syntax = vec![
            finding("a", 1, c::HN_S001, E, En, Nw),
            finding("a", 2, c::HN_R001, E, En, Nw),
        ];
        assert_eq!(code_of(&r(syntax), false), ExitCode::Usage);
        let stale = finding("a", 1, c::HN_G001, E, Wn, Nw);
        assert_eq!(
            code_of(&r(vec![stale.clone()]), false),
            ExitCode::StaleGenerated
        );
        let both = vec![stale, finding("a", 2, c::HN_R001, E, En, Nw)];
        assert_eq!(code_of(&r(both), false), ExitCode::Findings);
    }

    #[test]
    fn r_cli_023__partial_unknown_scope_and_missing_declarations() {
        let mut r = report(vec![]);
        r.partial = true;
        assert_eq!(code_of(&r, false), ExitCode::Usage); // a partial green is never exit 0
        r.findings.push(finding("a", 1, c::HN_R001, E, En, Nw));
        assert_eq!(code_of(&r, false), ExitCode::Findings);
        let mut u = report(vec![]);
        u.unknown_scopes.push(sid("web"));
        assert_eq!(code_of(&u, false), ExitCode::Usage); // R-CHK-057
        let mut a = report(vec![]);
        a.unknown_scopes.push(sid("legacy"));
        a.absent_scopes.push(sid("legacy"));
        assert_eq!(code_of(&a, false), ExitCode::Internal); // A-45
        let mut z = report(vec![]);
        z.zero_declarations = true;
        let mut o = CheckOptions::new(z.header.as_of);
        assert_eq!(z.outcome(&o).exit_code(), ExitCode::Ok);
        o.require_declarations = true;
        assert_eq!(z.outcome(&o).exit_code(), ExitCode::Findings);
    }

    #[test]
    fn r_chk_028__r_agt_011__r_agt_033__r_agt_034__relaxations_requests_and_codeowners_lines() {
        let root = "[policies]\n\"rules.r\" = \"open\"\n[rules.r]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"no x\"\nstage = \"enforce\"\n[[allow]]\ntarget = \"rule.r\"\nfield = \"stage\"\nfor = [\"web\"]\nwhy = \"migrating\"\nexp = \"2027-03-31\"\n";
        let web = "[fairlead]\nscope = \"web\"\n[[ovr]]\ntarget = \"rule.r\"\nstage = \"observe\"\n[[req]]\nto = \"root\"\ntarget = \"layer-ranks.widget\"\nwhy = \"x\"\n";
        let b = World::new().root(root).scope("web", web).build();
        let rules = RuleSet::collect(&b.registry);
        let fs = [finding("a", 1, c::HN_A002, E, Wn, Nw)];
        let s = summarize(&fs, &b.registry, &rules, &b.options(), &b.facts, None, 90);
        let has = |l: &str| s.relaxations.iter().any(|x| x == l);
        assert!(
            has("rules.r stage=observe in web (declared enforce)"),
            "{:?} {:#?}",
            s.relaxations,
            b.registry.diagnostics()
        );
        assert!(
            has("allow web rules.r.stage exp 2027-03-31"),
            "{:?}",
            s.relaxations
        );
        assert_eq!(
            (
                s.requests_open,
                s.codeowners_uncovered,
                s.oldest_request_days
            ),
            (1, 1, None)
        );
    }
}
