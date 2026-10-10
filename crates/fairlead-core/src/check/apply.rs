//! Diagnostics to findings, waivers, baseline and report filters (R-CHK-025..033, R-ARCH-075).
use crate::{
    check::{
        baseline::Baseline,
        builtin::{bare_id, control_path, exceptions::is_effective},
        codes::{is_active, spec},
        finding::{DiagB, Finding, FindingState, RuleId, clip, rule_id},
        fingerprint::{FingerprintInput, collapse_whitespace, fingerprint},
        floor::{clamp, is_floor},
        options::CheckOptions,
        rules::RuleSet,
    },
    diag::{Diagnostic, codes::*},
    extract::{Corpus, literals::rel_to},
    facts::Facts,
    hash::Fingerprint,
    path::RepoPath,
    registry::{Override, OvrId, Registry, ScopeId},
    time::AsOf,
};
use fairlead_syntax::{
    diag::DiagCode,
    directive::{Extent, Verb},
};
use globset::Glob;
use std::collections::{BTreeMap, BTreeSet};

/// A-16: the first line of the diagnostic's text, at most 240 chars.
fn excerpt_of(text: &str) -> Option<String> {
    let first = text.lines().next().unwrap_or("");
    (!first.is_empty()).then(|| clip(first, 240))
}

/// Deduplicate, apply the effective rule's (or tuning rule's) severity and stage, clamp floor codes,
/// assign occurrence and fingerprint; sorted.
pub(crate) fn lift(diags: Vec<Diagnostic>, reg: &Registry, rules: &RuleSet) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    let mut pairs: Vec<(Finding, String)> = Vec::new();
    for d in diags {
        let key = (
            d.code,
            d.location.path.clone(),
            d.location.line,
            d.location.col,
            d.text.clone(),
            d.message.clone(),
        );
        if !seen.insert(key) {
            continue;
        }
        let Some(sp) = spec(d.code).filter(|s| is_active(s.code)) else {
            continue;
        };
        let scope = d
            .scope
            .clone()
            .map(ScopeId)
            .or_else(|| reg.scope_of(&d.location.path).cloned())
            .unwrap_or_else(|| reg.root_scope().clone());
        let rule: Option<RuleId> = d.rule.as_deref().and_then(rule_id);
        let declared = rule
            .as_ref()
            .and_then(|r| rules.for_scope(&scope).iter().find(|x| x.id == *r));
        let tune = rules.tuning(d.code);
        let severity = declared
            .map(|r| r.sev)
            .or(tune.and_then(|t| t.sev))
            .unwrap_or(d.severity);
        let stage = declared
            .map(|r| r.stage)
            .or(tune.and_then(|t| t.stage))
            .unwrap_or(sp.default_stage);
        let (severity, stage) = clamp(d.code, severity, stage, sp.default_severity);
        let fix = if d.hint.is_empty() {
            d.did_you_mean
                .as_ref()
                .map(|x| format!("did you mean {x}"))
                .unwrap_or_default()
        } else {
            d.hint.clone()
        };
        let message = match &d.expected {
            Some(e) if !e.is_empty() => format!("{} (expected {e})", d.message),
            _ => d.message.clone(),
        };
        let text = if d.text.is_empty() {
            d.message.clone()
        } else {
            d.text.clone()
        };
        pairs.push((
            Finding {
                code: d.code,
                severity,
                stage,
                location: d.location.clone(),
                scope,
                rule,
                related: d.related.clone(),
                message,
                excerpt: excerpt_of(&d.text),
                intent: declared.and_then(|r| r.why.as_deref()).map(|w| clip(w, 80)),
                fix: clip(&fix, 100),
                occurrence: 0,
                fingerprint: Fingerprint::from_digest(&[0; 32]),
                state: FindingState::New,
                waived_by: None,
            },
            text,
        ));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut counts: BTreeMap<(DiagCode, Option<RuleId>, RepoPath, String), u32> = BTreeMap::new();
    for (f, text) in &mut pairs {
        let n = counts
            .entry((
                f.code,
                f.rule.clone(),
                f.location.path.clone(),
                collapse_whitespace(text),
            ))
            .or_insert(0);
        f.occurrence = *n;
        *n += 1;
        f.fingerprint = fingerprint(&FingerprintInput {
            code: f.code,
            rule: f.rule.as_ref(),
            scope: &f.scope,
            path: &f.location.path,
            text: text.as_str(),
            occurrence: f.occurrence,
        });
    }
    let mut out: Vec<Finding> = pairs.into_iter().map(|(f, _)| f).collect();
    out.sort();
    out
}

pub(crate) struct WaiverCx<'a> {
    pub reg: &'a Registry,
    pub corpus: &'a Corpus,
    pub rules: &'a RuleSet,
    pub as_of: AsOf,
    pub days: u32,
}
/// CI-12: below this many findings a rule's waive rate is not judged.
const RATE_MIN_SAMPLE: u32 = 5;

fn waivable(f: &Finding, cx: &WaiverCx<'_>) -> bool {
    if is_floor(f.code) {
        return false;
    }
    match &f.rule {
        Some(r) => cx
            .rules
            .for_scope(&f.scope)
            .iter()
            .any(|x| x.id == *r && x.waivable),
        None => {
            cx.rules.tuning(f.code).is_some_and(|t| t.waivable)
                && spec(f.code).is_some_and(|s| s.waivable)
        }
    }
}

/// Target (declared rule, or the built-in code named like the tuning rule) and extent (R-CHK-007 style).
fn covers(o: &Override, f: &Finding, cx: &WaiverCx<'_>) -> bool {
    let named = match &f.rule {
        Some(r) => o.target.name == r.0,
        None => o.target.name.as_str() == f.code.0.to_ascii_lowercase(),
    };
    if !named
        || o.target.kind.as_str() != "rule"
        || !cx.reg.chain(&f.scope).iter().any(|s| **s == o.target.scope)
    {
        return false;
    }
    let Some(sc) = cx.reg.scope(&o.declared_in) else {
        return false;
    };
    if sc.control.as_ref().is_some_and(|c| c.path == o.at.path) {
        let Some(rel) = rel_to(f.location.path.as_str(), sc.dir.as_str()) else {
            return false;
        };
        return o
            .applies
            .as_deref()
            .is_none_or(|g| Glob::new(g).is_ok_and(|g| g.compile_matcher().is_match(rel)));
    }
    if f.location.path != o.at.path {
        return false;
    }
    let extent = cx
        .corpus
        .records
        .get(&o.at.path)
        .and_then(|r| r.extraction.as_ref())
        .and_then(|e| {
            e.directives
                .iter()
                .find(|d| d.verb == Verb::Ovr && d.span.line == o.at.line)
        })
        .map(|d| &d.extent);
    match extent {
        Some(Extent::File | Extent::Doc) => true,
        Some(Extent::Lines { first, last }) => (*first..=*last).contains(&f.location.line),
        None => (o.at.line..=o.at.line.saturating_add(1)).contains(&f.location.line),
    }
}

/// Marks waived and waiver-ineffective findings; returns how many findings each ovr covered.
pub(crate) fn apply_waivers(fs: &mut [Finding], cx: &WaiverCx<'_>) -> BTreeMap<OvrId, u32> {
    let mut hits: BTreeMap<OvrId, u32> = BTreeMap::new();
    for f in fs.iter_mut() {
        if !waivable(f, cx) {
            continue;
        }
        let mut best: Option<(&Override, bool)> = None;
        for o in cx.reg.overrides().iter().filter(|o| covers(o, f, cx)) {
            *hits.entry(o.id.clone()).or_insert(0) += 1;
            let eff = is_effective(o, cx.as_of, cx.days);
            if best.is_none_or(|(_, e)| eff && !e) {
                best = Some((o, eff));
            }
        }
        match best {
            Some((o, true)) => {
                f.state = FindingState::Waived;
                f.waived_by = Some(o.id.clone());
            }
            Some((_, false)) => f.state = FindingState::WaiverIneffective,
            None => {}
        }
    }
    hits
}

/// X004 (effective ovr that waived nothing) and X008 (more than 20% of a rule's findings waived). Whole runs only.
pub(crate) fn post_exceptions(
    fs: &[Finding],
    hits: &BTreeMap<OvrId, u32>,
    cx: &WaiverCx<'_>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for o in cx
        .reg
        .overrides()
        .iter()
        // Only a waiver (an ovr of a rule) can be left without a violation; an ovr of any other
        // item changes a value and waives nothing.
        .filter(|o| {
            o.target.kind.as_str() == "rule"
                && is_effective(o, cx.as_of, cx.days)
                && !hits.contains_key(&o.id)
        })
    {
        out.push(
            DiagB::new(
                HN_X004,
                &o.at.path,
                o.at.line,
                o.at.col,
                format!("ovr of {} has no violation left", bare_id(&o.target)),
            )
            .hint("delete the ovr line")
            .text(o.id.0.as_str().to_string())
            .build(),
        );
    }
    let mut per: BTreeMap<RuleId, (ScopeId, u32, u32)> = BTreeMap::new();
    for f in fs
        .iter()
        .filter(|f| !matches!(f.state, FindingState::Baselined))
    {
        let Some(r) = &f.rule else { continue };
        let Some(home) = cx
            .rules
            .for_scope(&f.scope)
            .iter()
            .find(|x| x.id == *r)
            .map(|x| x.home.clone())
        else {
            continue;
        };
        let e = per.entry(r.clone()).or_insert((home, 0, 0));
        e.2 += 1;
        if f.state == FindingState::Waived {
            e.1 += 1;
        }
    }
    for (r, (home, waived, total)) in per {
        let Some(cp) = control_path(cx.reg, &home) else {
            continue;
        };
        if total >= RATE_MIN_SAMPLE && waived * 5 > total {
            let name = r.0.as_str();
            out.push(
                DiagB::new(
                    HN_X008,
                    &cp,
                    0,
                    0,
                    format!("{waived} of {total} findings of rule.{name} are waived"),
                )
                .rule(name)
                .hint("review the rule or fix the code; waivers are not revoked")
                .text(name)
                .build(),
            );
        }
    }
    out
}

/// Floor findings are never baselined (R-CHK-008); everything else known to the baseline stops failing.
pub(crate) fn apply_baseline(fs: &mut [Finding], b: &Baseline) {
    for f in fs.iter_mut().filter(|f| {
        !is_floor(f.code) && matches!(f.state, FindingState::New | FindingState::WaiverIneffective)
    }) {
        if b.contains(&f.fingerprint) {
            f.state = FindingState::Baselined;
        }
    }
}

/// B002 for a floor entry; B001 for an entry whose finding is gone (whole runs only).
pub(crate) fn baseline_diags(
    fps: &BTreeSet<Fingerprint>,
    b: &Baseline,
    path: &RepoPath,
    whole_run: bool,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (fp, e) in &b.entries {
        if is_floor(e.code) {
            out.push(
                DiagB::new(
                    HN_B002,
                    path,
                    0,
                    0,
                    format!("baseline holds floor finding {}", e.code.0),
                )
                .hint("git checkout the file; floor findings are never baselined")
                .text(fp.as_str())
                .build(),
            );
        } else if whole_run && !fps.contains(fp) {
            out.push(
                DiagB::new(
                    HN_B001,
                    path,
                    0,
                    0,
                    format!(
                        "baseline entry {} ({}) is fixed",
                        fp.as_str(),
                        e.path.as_str()
                    ),
                )
                .hint("prune with fairlead sync --baseline")
                .text(fp.as_str())
                .build(),
            );
        }
    }
    out
}

/// What the report keeps (R-CHK-002): `--changed` and `--files-from` intersect (C-36), `[PATH...]`
/// match files or directories, `--rule` narrows by code. A filter never hides a finding because
/// git facts are missing: `preflight` refuses that case first (R-ARCH-102).
pub(crate) struct ReportScope {
    allowed: Option<BTreeSet<RepoPath>>,
    prefixes: Vec<RepoPath>,
    rule: Option<DiagCode>,
}
impl ReportScope {
    pub(crate) fn new(opts: &CheckOptions, facts: &Facts) -> ReportScope {
        let changed = if opts.changed {
            facts.git.changed.as_ref().map(|c| c.0.clone())
        } else {
            None
        };
        let allowed = match (&opts.files_from, changed) {
            (Some(a), Some(b)) => Some(a.intersection(&b).cloned().collect()),
            (Some(a), None) => Some(a.clone()),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        ReportScope {
            allowed,
            prefixes: opts.paths.clone(),
            rule: opts.rule_filter,
        }
    }
    pub(crate) fn keeps(&self, f: &Finding) -> bool {
        let p = &f.location.path;
        self.allowed.as_ref().is_none_or(|a| a.contains(p))
            && (self.prefixes.is_empty() || self.prefixes.iter().any(|x| p == x || p.is_within(x)))
            && self.rule.is_none_or(|c| f.code == c)
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::{
        check::{
            baseline::BaselineEntry,
            rules::PatternRules,
            testkit::{Built, World},
        },
        config::Stage,
        diag::codes as c,
    };
    use fairlead_syntax::diag::Severity;

    fn lifted(b: &Built, ds: Vec<Diagnostic>) -> Vec<Finding> {
        lift(ds, &b.registry, &RuleSet::collect(&b.registry))
    }
    fn d(code: DiagCode, line: u32, text: &str) -> Diagnostic {
        DiagB::new(code, &RepoPath::new("src/a.ts").unwrap(), line, 1, "m")
            .text(text)
            .build()
    }
    const RULE: &str = "[rules.r]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"no x\"\nsev = \"error\"\nstage = \"warn\"\nwhy = \"a reason that is quite long, ........................................................ 100 chars\"\n[rules.hn-x002]\ncheck = \"builtin\"\nstage = \"enforce\"\n";

    #[test]
    fn r_chk_033__stage_and_severity_come_from_the_registry_the_rule_or_the_tuning_rule() {
        let b = World::new().root(RULE).build();
        let rule = DiagB::new(c::HN_U001, &RepoPath::new("src/a.ts").unwrap(), 1, 1, "m")
            .rule("r")
            .severity(Severity::Warn)
            .build();
        let f = lifted(
            &b,
            vec![
                d(c::HN_R001, 1, "a"),
                d(c::HN_T001, 2, "b"),
                d(c::HN_X002, 3, "c"),
                rule,
            ],
        );
        let by = |code| {
            f.iter()
                .find(|x| x.code == code)
                .map(|x| (x.severity, x.stage))
                .unwrap()
        };
        assert_eq!(by(c::HN_R001), (Severity::Error, Stage::Enforce));
        assert_eq!(by(c::HN_T001), (Severity::Error, Stage::Observe));
        assert_eq!(by(c::HN_X002), (Severity::Error, Stage::Enforce)); // R-CHK-055: a root may raise it
        assert_eq!(by(c::HN_U001), (Severity::Error, Stage::Warn)); // the rule's sev and stage win
        let u = f.iter().find(|x| x.code == c::HN_U001).unwrap();
        assert_eq!(u.intent.as_deref().map(|s| s.chars().count()), Some(80));
    }

    #[test]
    fn r_agt_009__a_floor_finding_cannot_be_lowered_and_inactive_codes_are_dropped() {
        let b = World::new().build();
        let low = DiagB::new(c::HN_F001, &RepoPath::new("a").unwrap(), 1, 1, "m")
            .severity(Severity::Info)
            .build();
        let f = lifted(&b, vec![low, d(c::HN_H001, 1, "x")]);
        assert_eq!(f.len(), 1);
        assert_eq!(
            (f[0].severity, f[0].stage),
            (Severity::Error, Stage::Enforce)
        );
    }

    #[test]
    fn r_chk_031__occurrence_counts_repeats_and_the_fingerprint_ignores_the_line() {
        let b = World::new().build();
        let two = lifted(
            &b,
            vec![
                d(c::HN_R001, 3, "same  text"),
                d(c::HN_R001, 9, "same text"),
            ],
        );
        assert_eq!((two[0].occurrence, two[1].occurrence), (0, 1));
        assert_ne!(two[0].fingerprint, two[1].fingerprint);
        let moved = lifted(&b, vec![d(c::HN_R001, 40, "same text")]);
        assert_eq!(moved[0].fingerprint, two[0].fingerprint);
        // an exact repeat is one finding
        assert_eq!(
            lifted(&b, vec![d(c::HN_R001, 3, "t"), d(c::HN_R001, 3, "t")]).len(),
            1
        );
    }

    #[test]
    fn r_arch_103__hint_is_capped_and_did_you_mean_and_expected_are_folded_in() {
        let b = World::new().build();
        let mut long = d(c::HN_R001, 1, "t");
        long.hint = "h".repeat(150);
        let mut dym = d(c::HN_R001, 2, "u");
        dym.did_you_mean = Some("cmp.x".into());
        dym.expected = Some("kind.name".into());
        let f = lifted(&b, vec![long, dym]);
        assert_eq!(f[0].fix.chars().count(), 100);
        assert_eq!(f[1].fix, "did you mean cmp.x");
        assert_eq!(f[1].message, "m (expected kind.name)");
        assert_eq!(f[1].excerpt.as_deref(), Some("u")); // A-16
    }

    const WAIVE: &str = "[policies]\n\"rules.no-console\" = \"open\"\n[rules.no-console]\ncheck = \"pattern\"\nforbid = \"console\\\\.log\"\nmsg = \"no console\"\nwaivable = true\nsev = \"error\"\nstage = \"enforce\"\n";
    fn ovr(exp: &str) -> String {
        format!("// fairlead: ovr rule.no-console why=\"legacy\" owner=@platform exp={exp}\n")
    }
    fn wcx<'a>(b: &'a Built, rules: &'a RuleSet) -> WaiverCx<'a> {
        WaiverCx {
            reg: &b.registry,
            corpus: &b.corpus,
            rules,
            as_of: b.as_of,
            days: 90,
        }
    }
    fn waived(b: &Built) -> (Vec<Finding>, Vec<Diagnostic>) {
        let rules = RuleSet::collect(&b.registry);
        let mut fs = lift(b.run_check(&PatternRules), &b.registry, &rules);
        let cx = wcx(b, &rules);
        let hits = apply_waivers(&mut fs, &cx);
        let post = post_exceptions(&fs, &hits, &cx);
        (fs, post)
    }

    #[test]
    fn r_chk_025__an_effective_ovr_waives_its_extent_and_an_expired_one_is_ineffective() {
        let src = |exp: &str| format!("{}\nconsole.log(1);\n", ovr(exp));
        let world = |path: &str, text: &str| World::new().root(WAIVE).file(path, text).build();
        let (fs, _) = waived(&world("src/a.ts", &src("2026-12-31")));
        assert_eq!(
            (fs.len(), fs[0].state),
            (1, FindingState::Waived),
            "{fs:#?}"
        );
        assert!(fs[0].waived_by.is_some());
        let (fs, _) = waived(&world("src/a.ts", &src("2026-01-01")));
        assert_eq!(fs[0].state, FindingState::WaiverIneffective);
        let (fs, _) = waived(&world("src/b.ts", "const x = 1;\n\nconsole.log(1);\n"));
        assert_eq!(fs[0].state, FindingState::New);
    }

    #[test]
    fn r_chk_008__a_floor_finding_is_never_waived() {
        let b = World::new().build();
        let rules = RuleSet::collect(&b.registry);
        let floor = DiagB::new(c::HN_F001, &RepoPath::new("a").unwrap(), 1, 1, "m").build();
        let mut fs = lift(vec![floor], &b.registry, &rules);
        assert!(apply_waivers(&mut fs, &wcx(&b, &rules)).is_empty());
        assert_eq!(fs[0].state, FindingState::New);
    }

    #[test]
    fn r_chk_029__r_chk_010__a_waiver_without_a_violation_is_x004_and_a_high_waive_rate_is_x008() {
        let clean = format!("{}\nconst x = 1;\n", ovr("2026-12-31"));
        let (_, post) = waived(&World::new().root(WAIVE).file("src/a.ts", &clean).build());
        assert_eq!(
            post.iter().filter(|d| d.code == c::HN_X004).count(),
            1,
            "{post:#?}"
        );
        let five = format!("{}\n{}", ovr("2026-12-31"), "console.log(1);\n".repeat(5));
        let (fs, post) = waived(&World::new().root(WAIVE).file("src/a.ts", &five).build());
        assert!(
            fs.iter().all(|f| f.state == FindingState::Waived),
            "{fs:#?}"
        );
        assert_eq!(post.iter().filter(|d| d.code == c::HN_X008).count(), 1);
        assert!(post.iter().all(|d| d.code != c::HN_X004));
    }

    #[test]
    fn r_chk_030__baselined_findings_stop_failing_and_fixed_or_floor_entries_are_reported() {
        let b = World::new().build();
        let rules = RuleSet::collect(&b.registry);
        let floor = DiagB::new(c::HN_F001, &RepoPath::new("a").unwrap(), 1, 1, "m").build();
        let mut fs = lift(vec![d(c::HN_R001, 1, "x"), floor], &b.registry, &rules);
        let mut base = Baseline::default();
        for f in &fs {
            base.entries.insert(
                f.fingerprint.clone(),
                BaselineEntry {
                    code: f.code,
                    path: f.location.path.clone(),
                    accepted: None,
                },
            );
        }
        base.entries.insert(
            Fingerprint::from_digest(&[9; 32]),
            BaselineEntry {
                code: c::HN_R001,
                path: RepoPath::new("old.ts").unwrap(),
                accepted: None,
            },
        );
        apply_baseline(&mut fs, &base);
        let by = |code| fs.iter().find(|f| f.code == code).unwrap().state;
        assert_eq!(
            (by(c::HN_R001), by(c::HN_F001)),
            (FindingState::Baselined, FindingState::New)
        );
        let fps: BTreeSet<Fingerprint> = fs.iter().map(|f| f.fingerprint.clone()).collect();
        let bp = RepoPath::new("fairlead/gen/baseline.txt").unwrap();
        let mut codes: Vec<_> = baseline_diags(&fps, &base, &bp, true)
            .iter()
            .map(|d| d.code)
            .collect();
        codes.sort();
        assert_eq!(codes, [c::HN_B001, c::HN_B002]);
        // narrowed runs cannot know which entries are fixed
        assert!(
            baseline_diags(&fps, &base, &bp, false)
                .iter()
                .all(|d| d.code != c::HN_B001)
        );
    }

    fn f(path: &str, code: DiagCode) -> Finding {
        crate::check::testkit::finding(
            path,
            1,
            code,
            Severity::Error,
            Stage::Enforce,
            FindingState::New,
        )
    }
    fn rp(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }
    fn opts() -> CheckOptions {
        CheckOptions::new(AsOf::parse("2026-10-09").unwrap())
    }

    #[test]
    fn r_arch_075__r_cli_022__changed_and_files_from_intersect() {
        let mut facts = crate::check::testkit::empty_facts();
        facts.git.changed = Some(crate::facts::ChangedSet(
            [rp("a.ts"), rp("b.ts")].into_iter().collect(),
        ));
        let mut o = opts();
        o.changed = true;
        o.files_from = Some([rp("b.ts"), rp("c.ts")].into_iter().collect());
        let s = ReportScope::new(&o, &facts);
        let kept: Vec<_> = ["a.ts", "b.ts", "c.ts"]
            .iter()
            .filter(|p| s.keeps(&f(p, c::HN_R001)))
            .collect();
        assert_eq!(kept, [&"b.ts"]);
    }

    #[test]
    fn r_chk_002__path_arguments_match_files_and_whole_directories_and_rule_narrows_by_code() {
        let mut o = opts();
        o.paths = vec![rp("src"), rp("lib/x.ts")];
        o.rule_filter = Some(c::HN_R001);
        let s = ReportScope::new(&o, &crate::check::testkit::empty_facts());
        assert!(s.keeps(&f("src/a/b.ts", c::HN_R001)));
        assert!(s.keeps(&f("lib/x.ts", c::HN_R001)));
        assert!(!s.keeps(&f("srcx/a.ts", c::HN_R001)));
        assert!(!s.keeps(&f("src/a.ts", c::HN_T001)));
    }

    #[test]
    fn r_arch_075__changed_without_git_changes_reports_everything_rather_than_nothing() {
        let mut o = opts();
        o.changed = true; // facts.git.changed is None: preflight refuses; the filter must not hide findings
        let s = ReportScope::new(&o, &crate::check::testkit::empty_facts());
        assert!(s.keeps(&f("a.ts", c::HN_R001)));
    }

    #[test]
    fn r_chk_029__an_ovr_of_a_value_waives_nothing_so_it_is_never_x004() {
        let root = "[kinds.tok]
title = \"t\"
shape = \"doc\"
[policies]
\"tok.*\" = \"open\"
";
        let doc = "+++
directives = ['def tok.a size=sm']
+++
# a
";
        let b = World::new()
            .root(root)
            .file("fairlead/docs/t.md", doc)
            .file(
                "src/a.ts",
                "// fairlead: ovr tok.a size=lg
export const a = 1;
",
            )
            .build();
        assert!(
            !b.registry.overrides().is_empty(),
            "the fixture needs an ovr"
        );
        let (fs, post) = waived(&b);
        assert!(
            fs.is_empty() && post.iter().all(|d| d.code != c::HN_X004),
            "{post:#?}"
        );
    }
}
