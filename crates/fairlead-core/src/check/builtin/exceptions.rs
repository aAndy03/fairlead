//! Exceptions: states, horizon, budget, sprawl, ramp (R-CHK-025, R-SCOPE-046..048, R-CHK-033).
use crate::{
    check::{
        CheckCx,
        builtin::{bare_id, control_path, root_control},
        finding::DiagB,
        rules::Check,
    },
    config::Stage,
    diag::{Diagnostic, codes::*},
    registry::{ApprovalStatus, InertReason, Override, OvrState, Registry, ScopeId},
    time::AsOf,
};
use fairlead_syntax::diag::DiagCode;

pub struct ExceptionsCheck;
const CODES: [DiagCode; 9] = [
    HN_X001, HN_X002, HN_X003, HN_X005, HN_X006, HN_X007, HN_X009, HN_X010, HN_X011,
];

/// C-18: the smaller of `[policies.waiver] max-exp` and `[exceptions] max-days` (default 90).
pub(crate) fn horizon_days(reg: &Registry) -> u32 {
    root_control(reg).map_or(90, |c| {
        let w = c.policies.waiver.max_exp_days;
        c.exceptions.max_days.map_or(w, |m| m.min(w))
    })
}

pub(crate) fn beyond_horizon(o: &Override, as_of: AsOf, days: u32) -> bool {
    let Some(exp) = o.exp else { return false };
    as_of
        .0
        .checked_add(jiff::Span::new().days(i64::from(days)))
        .is_ok_and(|limit| exp > limit)
}

/// R-CHK-025: well formed and permitted (state), approved when required, inside the horizon.
pub(crate) fn is_effective(o: &Override, as_of: AsOf, days: u32) -> bool {
    matches!(o.state, OvrState::Effective)
        && !matches!(o.approval, ApprovalStatus::Missing)
        && !beyond_horizon(o, as_of, days)
}

/// R-CHK-033: the latest nudge point reached by a rule `days` old, if any.
pub(crate) fn nudge_point(days: i32, nudge: [u32; 2]) -> Option<u32> {
    let d = i64::from(days);
    if d >= i64::from(nudge[1]) {
        Some(nudge[1])
    } else if d >= i64::from(nudge[0]) {
        Some(nudge[0])
    } else {
        None
    }
}

pub(crate) fn override_diags(o: &Override, as_of: AsOf, days: u32) -> Vec<Diagnostic> {
    let (p, l, c) = (&o.at.path, o.at.line, o.at.col);
    let (tgt, id) = (bare_id(&o.target), o.id.0.as_str().to_string());
    let exp = o.exp.map(|e| e.to_string()).unwrap_or_default();
    let d = |code, msg: String, hint: String| {
        vec![
            DiagB::new(code, p, l, c, msg)
                .hint(hint)
                .text(id.clone())
                .related(tgt.clone())
                .build(),
        ]
    };
    match &o.state {
        OvrState::Expired => d(
            HN_X001,
            format!("ovr of {tgt} expired on {exp}; the finding it waived is back"),
            "renew with fairlead approve, or delete the ovr".into(),
        ),
        OvrState::Inert(InertReason::SealedNoAllow) => d(
            HN_X005,
            format!("ovr of sealed {tgt} has no allow and is inert"),
            "fairlead new escalation".into(),
        ),
        OvrState::Inert(InertReason::FloorRule) => d(
            HN_X003,
            format!("ovr targets floor rule {tgt}; it is inert"),
            "floor rules cannot be overridden; remove the ovr".into(),
        ),
        OvrState::Inert(InertReason::OutsideSubtree) => d(
            HN_C001,
            format!("ovr of {tgt} applies outside its subtree"),
            "keep applies inside the declaring scope".into(),
        ),
        OvrState::Stale => d(
            HN_X010,
            format!("the target {tgt} changed or vanished"),
            "run fairlead sync to keep, localise or drop the ovr".into(),
        ),
        OvrState::Inert(_) => Vec::new(), // reported by their own emitters (syntax, registry)
        OvrState::Effective => {
            let mut v = Vec::new();
            if matches!(o.approval, ApprovalStatus::Missing) {
                v.extend(d(
                    HN_X002,
                    format!("ovr of {tgt} has no approval and is ineffective"),
                    format!("owner runs fairlead approve {id}"),
                ));
            }
            if beyond_horizon(o, as_of, days) {
                v.extend(d(
                    HN_X006,
                    format!("exp {exp} is beyond the {days}-day horizon"),
                    "shorten exp".into(),
                ));
            }
            v
        }
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Check for ExceptionsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let (reg, as_of) = (cx.registry, cx.options.as_of);
        let days = horizon_days(reg);
        let mine = || reg.overrides().iter().filter(|o| o.declared_in == *scope);
        for o in mine() {
            out.extend(override_diags(o, as_of, days));
        }
        let Some(cp) = control_path(reg, scope) else {
            return;
        };
        let Some(root) = root_control(reg) else {
            return;
        };
        let active = count(mine().filter(|o| is_effective(o, as_of, days)).count());
        let budget = root.policies.waiver.budget;
        if active > budget {
            out.push(
                DiagB::new(
                    HN_X007,
                    &cp,
                    0,
                    0,
                    format!("{active} active waivers exceed the budget of {budget}"),
                )
                .hint("retire waivers or fix the findings they hide")
                .text(scope.0.as_str())
                .build(),
            );
        }
        for r in cx
            .rules
            .for_scope(scope)
            .iter()
            .filter(|r| r.home == *scope)
        {
            let name = r.id.0.as_str();
            let live = count(
                reg.overrides()
                    .iter()
                    .filter(|o| {
                        o.target.scope == r.home
                            && o.target.kind.as_str() == "rule"
                            && o.target.name == r.id.0
                            && matches!(o.state, OvrState::Effective)
                    })
                    .count(),
            );
            // R-SCOPE-057: a delegated, tighter limit of the rule's home scope wins.
            let limit = reg
                .scope(scope)
                .and_then(|s| s.control.as_ref())
                .and_then(|c| crate::config::delegated(root, c, "exceptions.max-live-per-rule"))
                .and_then(|v| v.as_integer())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(root.exceptions.max_live_per_rule);
            if live > limit {
                out.push(
                    DiagB::new(
                        HN_X011,
                        &cp,
                        0,
                        0,
                        format!("rule.{name} has {live} live exceptions"),
                    )
                    .rule(name)
                    .hint("re-declare the rule or retire it")
                    .text(name)
                    .build(),
                );
            }
            let Some(since) = r.since.filter(|_| r.stage != Stage::Enforce) else {
                continue;
            };
            let age = since.until(as_of.0).map(|s| s.get_days()).unwrap_or(0);
            if let Some(n) = nudge_point(age, cx.options.ramp.nudge_days) {
                out.push(
                    DiagB::new(
                        HN_X009,
                        &cp,
                        0,
                        0,
                        format!("rule.{name} has sat below enforce for {age} days (nudge at {n})"),
                    )
                    .rule(name)
                    .hint("raise the stage, or record why it stays")
                    .text(format!("{name}@{n}"))
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
    use crate::{
        check::testkit::{World, of},
        diag::{Location, codes as c},
        hash::Fingerprint,
        path::RepoPath,
        registry::{FieldChange, ItemId, OvrId, OvrOp},
    };
    use fairlead_syntax::ids::{Id, ScopeSlug};
    use std::collections::BTreeMap;

    fn until_world(until: &str) -> crate::check::testkit::Built {
        World::new()
            .root("[rules.r]\ncheck = \"pattern\"\nforbid = \"ZZZ\"\nmsg = \"no\"\nwaivable = true\n[exceptions]\nmax-minors = 2\n")
            .file(
                "src/a.ts",
                &format!("// fairlead: ovr rule.r why=\"legacy\" owner=@platform until={until}\nexport const A = 1;\n"),
            )
            .file("fairlead/versions/0.4.0.md", "+++\nversion = \"0.4.0\"\n+++\n")
            .build()
    }

    #[test]
    fn r_ver_070__until_is_effective_while_open_is_below_it() {
        // S = 0.4.0, nothing pending: O = 0.4.1.
        let live = until_world("0.5.0");
        assert_eq!(live.registry.overrides()[0].state, OvrState::Effective);
        let gone = until_world("0.4.1");
        assert_eq!(
            gone.registry.overrides()[0].state,
            OvrState::Expired,
            "O >= until"
        );
    }

    #[test]
    fn r_ver_075__until_beyond_the_cap_is_exc_cap() {
        // cap = 0.(4 + 2).0 = 0.6.0
        assert_eq!(
            until_world("0.6.0").registry.overrides()[0].state,
            OvrState::Effective
        );
        let far = until_world("0.7.0");
        assert!(
            far.registry
                .diagnostics()
                .iter()
                .any(|d| d.code == c::HN_X016),
            "{:?}",
            far.registry.diagnostics()
        );
    }

    fn ovr(state: OvrState, approval: ApprovalStatus, exp: Option<&str>) -> Override {
        let id = Id::parse("rule.no-raw-hex").unwrap();
        Override {
            id: OvrId(Fingerprint::from_digest(&[7; 32])),
            target: ItemId {
                scope: ScopeId(ScopeSlug::parse("root").unwrap()),
                kind: id.kind,
                name: id.name,
            },
            field: None,
            op: OvrOp::Loosen,
            set: BTreeMap::<String, FieldChange>::new(),
            why: Some("legacy".into()),
            owner: Some("@platform".into()),
            exp: exp.map(|e| e.parse().unwrap()),
            until: None,
            base: None,
            declared_in: ScopeId(ScopeSlug::parse("web").unwrap()),
            applies: None,
            at: Location {
                path: RepoPath::new("src/a.tsx").unwrap(),
                line: 3,
                col: 1,
            },
            state,
            approval,
            target_hash: None,
        }
    }
    fn codes_of(o: &Override) -> Vec<&'static str> {
        override_diags(o, AsOf::parse("2026-10-09").unwrap(), 90)
            .iter()
            .map(|d| d.code.0)
            .collect()
    }

    #[test]
    fn r_scope_046__each_override_state_maps_to_its_code() {
        let ok = ApprovalStatus::Approved { renewal: 0 };
        let inert = |r| codes_of(&ovr(OvrState::Inert(r), ok, None));
        assert_eq!(
            codes_of(&ovr(OvrState::Expired, ok, Some("2026-01-01"))),
            [c::HN_X001.0]
        );
        assert_eq!(inert(InertReason::SealedNoAllow), [c::HN_X005.0]);
        assert_eq!(inert(InertReason::FloorRule), [c::HN_X003.0]);
        assert_eq!(inert(InertReason::OutsideSubtree), [c::HN_C001.0]);
        assert_eq!(
            codes_of(&ovr(OvrState::Stale, ok, Some("2026-12-31"))),
            [c::HN_X010.0]
        );
        // HN-S016 for an incomplete ovr is the registry's.
        assert!(inert(InertReason::Incomplete).is_empty());
    }

    #[test]
    fn r_chk_025__unapproved_effective_ovr_is_x002_and_a_long_horizon_is_x006() {
        let eff = |a, e| codes_of(&ovr(OvrState::Effective, a, Some(e)));
        assert_eq!(eff(ApprovalStatus::Missing, "2026-12-31"), [c::HN_X002.0]);
        assert_eq!(
            eff(ApprovalStatus::Missing, "2027-06-01"),
            [c::HN_X002.0, c::HN_X006.0]
        );
        assert!(eff(ApprovalStatus::NotRequired, "2026-12-31").is_empty());
    }

    #[test]
    fn r_chk_033__ramp_nudges_at_the_configured_points() {
        assert_eq!(nudge_point(59, [60, 120]), None);
        assert_eq!(nudge_point(60, [60, 120]), Some(60));
        assert_eq!(nudge_point(121, [60, 120]), Some(120));
    }

    #[test]
    fn r_scope_048__budget_and_sprawl_are_reported_per_scope() {
        let root = "[policies]\n\"rules.no-raw-hex\" = \"open\"\n[policies.waiver]\nbudget = 0\n[exceptions]\nmax-live-per-rule = 0\n[rules.no-raw-hex]\ncheck = \"pattern\"\nforbid = \"zzz\"\nmsg = \"no zzz\"\nwaivable = true\nsince = \"2026-01-01\"\nstage = \"warn\"\n";
        let src = "// fairlead: ovr rule.no-raw-hex why=\"legacy\" owner=@platform exp=2026-12-31\nexport const a = 1;\n";
        let b = World::new().root(root).file("src/a.ts", src).build();
        let ds = b.run_check(&ExceptionsCheck);
        assert_eq!(of(&ds, c::HN_X007).len(), 1, "{ds:#?}");
        assert_eq!(of(&ds, c::HN_X011).len(), 1, "{ds:#?}");
        // since 2026-01-01 is 281 days before as-of
        assert_eq!(of(&ds, c::HN_X009).len(), 1, "{ds:#?}");
    }
}
