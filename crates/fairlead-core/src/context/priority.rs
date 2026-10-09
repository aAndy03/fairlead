//! Priority classes, exact scoring and candidate generation (R-CTX-040..045, C-27).
use std::cmp::Ordering;

use fairlead_syntax::diag::Severity;

use crate::config::SevWeight;
use crate::context::brief::{Block, Entry};

/// The packing table of 14. `P0Header` is the fixed frame; `P5Recent` is 0.2 data (inactive).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    P0Header,
    P1ErrorRules,
    P2Drift,
    P3Items,
    P4WarnRules,
    P5Recent,
    P6SiblingsChildren,
    P7AncestorL1,
}

impl Priority {
    pub const fn is_mandatory(self) -> bool {
        matches!(self, Priority::P0Header | Priority::P1ErrorRules)
    }
    pub const fn is_active(self) -> bool {
        !matches!(self, Priority::P5Recent)
    }
}

/// Compiled constants tied to the edition (C-27): 1.0, 0.7, 0.4 as hundredths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relevance {
    Direct,
    Glob,
    ScopeWide,
}

impl Relevance {
    pub const fn pct(self) -> u64 {
        match self {
            Relevance::Direct => 100,
            Relevance::Glob => 70,
            Relevance::ScopeWide => 40,
        }
    }
}

/// `sev_weight x relevance / (1 + distance)` kept as an exact ratio: no floats, so byte-stable everywhere.
#[derive(Clone, Copy, Debug)]
pub struct Score {
    num: u64,
    den: u64,
}

impl Score {
    pub fn new(sev_weight: u32, rel: Relevance, distance: u32) -> Score {
        Score {
            num: u64::from(sev_weight) * rel.pct(),
            den: 1 + u64::from(distance),
        }
    }
    pub fn ratio(&self) -> (u64, u64) {
        (self.num, self.den)
    }
}

impl Ord for Score {
    fn cmp(&self, o: &Self) -> Ordering {
        (u128::from(self.num) * u128::from(o.den)).cmp(&(u128::from(o.num) * u128::from(self.den)))
    }
}
impl PartialOrd for Score {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl PartialEq for Score {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Score {}

pub fn sev_weight(sev: Severity, w: &SevWeight) -> u32 {
    match sev {
        Severity::Error => w.error,
        Severity::Warn => w.warn,
        Severity::Info => w.info,
    }
}

pub fn sev_name(sev: Severity) -> &'static str {
    match sev {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

/// One packable object: exactly one `Entry`. `hint` is the use count (R-CTX-012); `explain` is a valid
/// `fairlead explain` argument naming this object (DD-7).
#[derive(Clone, Debug)]
pub struct Candidate {
    pub priority: Priority,
    pub score: Score,
    pub hint: u32,
    pub key: String,
    pub explain: String,
    pub block: Block,
    pub entry: Entry,
}

/// Priority ascending, score descending, use count descending, then ID bytewise (R-CTX-042).
pub fn rank_cmp(a: &Candidate, b: &Candidate) -> Ordering {
    a.priority
        .cmp(&b.priority)
        .then_with(|| b.score.cmp(&a.score))
        .then_with(|| b.hint.cmp(&a.hint))
        .then_with(|| a.key.as_bytes().cmp(b.key.as_bytes()))
}

use std::collections::BTreeMap;

use crate::context::brief::{BriefRule, ChainLink, InhLine};
use crate::context::n32;

#[derive(Clone, Debug)]
pub struct OvrView {
    pub scope: String,
    pub note: Option<String>,
}

/// A rule as the scope sees it after resolution (effective severity, glob, why, effective override).
#[derive(Clone, Debug)]
pub struct RuleView {
    pub id: String,
    pub home: String,
    pub distance: u32,
    pub sev: Severity,
    pub stage: String,
    pub glob: Vec<String>,
    pub why: Option<String>,
    pub ovr: Option<OvrView>,
}

pub struct RuleSplit {
    pub inh: Vec<Candidate>,
    pub rules: Vec<Candidate>,
}

#[derive(Default)]
struct Slot {
    err: u32,
    warn: u32,
    rules: Vec<BriefRule>,
}

fn brief_rule(v: &RuleView, scope: &str, l0: bool) -> BriefRule {
    BriefRule {
        id: v.id.clone(),
        sev: sev_name(v.sev).to_string(),
        stage: v.stage.clone(),
        home: v.home.clone(),
        inherited: v.home != scope,
        glob: v.glob.clone(),
        why: v.why.clone(),
        ovr: v.ovr.as_ref().map(|o| match &o.note {
            Some(n) => format!("(ovr {}: {n})", o.scope),
            None => format!("(ovr {})", o.scope),
        }),
        l0,
    }
}

/// R-CTX-020..022. `ancestors` is the chain without the scope itself, root first.
pub fn split_rules(
    views: &[RuleView],
    scope: &str,
    ancestors: &[ChainLink],
    full: bool,
    w: &SevWeight,
) -> RuleSplit {
    let mut per: BTreeMap<String, Slot> = ancestors
        .iter()
        .map(|a| (a.scope.clone(), Slot::default()))
        .collect();
    let mut rules: Vec<Candidate> = Vec::new();
    for v in views {
        let anchor = v
            .ovr
            .as_ref()
            .map(|o| o.scope.as_str())
            .filter(|s| per.contains_key(*s))
            .unwrap_or(v.home.as_str());
        let local = v.home == scope
            || v.ovr.as_ref().is_some_and(|o| o.scope == scope)
            || !per.contains_key(anchor);
        if local {
            // error rules and any rule with an effective ovr are never dropped from context (40 trust list, item 5)
            let priority = if v.sev == Severity::Error || v.ovr.is_some() {
                Priority::P1ErrorRules
            } else {
                Priority::P4WarnRules
            };
            let rel = if v.glob.is_empty() {
                Relevance::ScopeWide
            } else {
                Relevance::Glob
            };
            rules.push(Candidate {
                priority,
                score: Score::new(sev_weight(v.sev, w), rel, v.distance),
                hint: 0,
                key: v.id.clone(),
                explain: v.id.clone(),
                block: Block::Rules,
                entry: Entry::Rule(brief_rule(v, scope, false)),
            });
            continue;
        }
        let Some(slot) = per.get_mut(anchor) else {
            continue;
        };
        if v.sev == Severity::Error {
            slot.err += 1;
        } else {
            slot.warn += 1;
        }
        if v.sev == Severity::Error || v.ovr.is_some() || full {
            slot.rules.push(brief_rule(v, scope, !full));
        }
    }
    let n = ancestors.len();
    let inh = ancestors
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let slot = per.remove(&a.scope).unwrap_or_default();
            Candidate {
                priority: Priority::P1ErrorRules,
                score: Score::new(w.error, Relevance::ScopeWide, n32(n - i)),
                hint: 0,
                key: format!("inh/{i:04}"),
                explain: a.scope.clone(),
                block: Block::Rules,
                entry: Entry::Inh(InhLine {
                    scope: a.scope.clone(),
                    digest: a.digest.clone(),
                    err: slot.err,
                    warn: slot.warn,
                    rules: slot.rules,
                }),
            }
        })
        .collect();
    RuleSplit { inh, rules }
}

use fairlead_syntax::directive::Verb;

use crate::context::brief::Omission;
use crate::extract::Corpus;
use crate::path::RepoPath;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdUsage {
    pub declared: bool,
    pub uses: u32,
}

/// R-CTX-012, R-CTX-070: a range scan from the target, not a tree walk. Every key that starts with the
/// target string is contiguous in the sorted map; `is_within` removes `src-old` next to `src`.
pub fn collect_id_usage(corpus: &Corpus, target: &RepoPath) -> BTreeMap<String, IdUsage> {
    let mut out: BTreeMap<String, IdUsage> = BTreeMap::new();
    let prefix = target.as_str();
    for (path, rec) in corpus.records.range(target.clone()..) {
        if !path.as_str().starts_with(prefix) {
            break;
        }
        if path != target && !path.is_within(target) {
            continue;
        }
        let Some(ex) = &rec.extraction else { continue };
        for d in &ex.directives {
            match d.verb {
                Verb::Def => {
                    for id in &d.ids {
                        out.entry(id.to_string()).or_default().declared = true;
                    }
                }
                Verb::Use => {
                    for id in &d.ids {
                        out.entry(id.to_string()).or_default().uses += 1;
                    }
                }
                _ => {}
            }
        }
    }
    out
}

pub const USES_TOP_K: usize = 12;

/// P3 candidates ranked by score (info weight x proximity of the home scope) then use count then ID;
/// everything past the top K becomes one `+N more` omission (R-CTX-012).
pub fn id_candidates(
    usage: &BTreeMap<String, IdUsage>,
    distance_of: &dyn Fn(&str) -> u32,
    w: &SevWeight,
) -> (Vec<Candidate>, Vec<Omission>) {
    let mut all: Vec<Candidate> = usage
        .iter()
        .map(|(id, u)| Candidate {
            priority: Priority::P3Items,
            score: Score::new(w.info, Relevance::Direct, distance_of(id)),
            hint: u.uses,
            key: id.clone(),
            explain: id.clone(),
            block: Block::Items,
            entry: if u.declared {
                Entry::Declares { id: id.clone() }
            } else {
                Entry::Uses { id: id.clone() }
            },
        })
        .collect();
    all.sort_by(rank_cmp);
    let rest = all.split_off(USES_TOP_K.min(all.len()));
    let omitted = match rest.first() {
        Some(first) => vec![Omission {
            block: Block::Items,
            count: n32(rest.len()),
            explain: first.explain.clone(),
        }],
        None => vec![],
    };
    (all, omitted)
}

use crate::check::{CheckReport, Finding, FindingState};
use crate::context::brief::DriftLine;
use crate::render::sanitize::sanitize_data;

pub struct DriftSet {
    pub candidates: Vec<Candidate>,
    pub waivers: u32,
}

/// R-CTX-013: findings of the check report whose file is the target or lies under it.
pub fn findings_under<'a>(report: &'a CheckReport, target: &RepoPath) -> Vec<&'a Finding> {
    report
        .findings
        .iter()
        .filter(|f| f.location.path == *target || f.location.path.is_within(target))
        .collect()
}

type GroupKey = (String, Option<String>, String);

fn drift_candidate(
    key: String,
    explain: &str,
    sev: Severity,
    line: DriftLine,
    w: &SevWeight,
) -> Candidate {
    Candidate {
        priority: Priority::P2Drift,
        score: Score::new(sev_weight(sev, w), Relevance::Direct, 0),
        hint: 0,
        key,
        explain: explain.to_string(),
        block: Block::Drift,
        entry: Entry::Drift(line),
    }
}

pub fn drift_candidates(
    found: &[&Finding],
    partial: bool,
    unknown: &[String],
    include_groups: bool,
    fallback_explain: &str,
    w: &SevWeight,
) -> DriftSet {
    let (mut new, mut baselined, mut waived) = (0u32, 0u32, 0u32);
    let mut groups: BTreeMap<GroupKey, (Vec<u32>, Severity, u32)> = BTreeMap::new();
    for f in found {
        match f.state {
            FindingState::Baselined => baselined += 1,
            FindingState::Waived => waived += 1,
            FindingState::New | FindingState::WaiverIneffective => {
                new += 1;
                let key = (
                    f.code.0.to_string(),
                    f.rule.as_ref().map(|r| format!("rule.{}", r.0)),
                    f.location.path.as_str().to_string(),
                );
                let g = groups
                    .entry(key)
                    .or_insert_with(|| (Vec::new(), f.severity, 0));
                if f.location.line > 0 {
                    g.0.push(f.location.line);
                }
                g.1 = g.1.max(f.severity);
                g.2 += 1;
            }
        }
    }
    let first_code = groups.keys().next().map(|k| k.0.clone());
    let explain = first_code.as_deref().unwrap_or(fallback_explain);
    let mut candidates = vec![drift_candidate(
        "0summary".to_string(),
        explain,
        Severity::Error,
        DriftLine::Summary {
            new,
            baselined,
            waived,
        },
        w,
    )];
    if partial {
        candidates.push(drift_candidate(
            "0partial".to_string(),
            explain,
            Severity::Error,
            DriftLine::Partial,
            w,
        ));
    }
    for s in unknown {
        candidates.push(drift_candidate(
            format!("0unknown/{s}"),
            fallback_explain,
            Severity::Error,
            DriftLine::Unknown { scope: s.clone() },
            w,
        ));
    }
    if include_groups {
        for ((code, rule, path), (mut lines, sev, n)) in groups {
            lines.sort_unstable();
            lines.dedup();
            let path = sanitize_data(&path, 120);
            let key = format!("1/{path}/{code}/{}", rule.clone().unwrap_or_default());
            candidates.push(drift_candidate(
                key,
                &code,
                sev,
                DriftLine::Group {
                    code: code.clone(),
                    rule,
                    path,
                    lines,
                    sev: sev_name(sev).to_string(),
                    n,
                },
                w,
            ));
        }
    }
    DriftSet {
        candidates,
        waivers: waived,
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    fn w() -> SevWeight {
        SevWeight {
            error: 4,
            warn: 2,
            info: 1,
        }
    }

    fn cand(prio: Priority, score: Score, hint: u32, key: &str) -> Candidate {
        Candidate {
            priority: prio,
            score,
            hint,
            key: key.into(),
            explain: key.into(),
            block: Block::Items,
            entry: Entry::Note { text: key.into() },
        }
    }

    #[test]
    fn r_ctx_042__score_is_sev_weight_times_proximity_times_relevance() {
        assert_eq!(Score::new(4, Relevance::Direct, 0).ratio(), (400, 1));
        assert_eq!(Score::new(2, Relevance::Glob, 1).ratio(), (140, 2));
        assert_eq!(Score::new(1, Relevance::ScopeWide, 3).ratio(), (40, 4));
    }

    #[test]
    fn r_ctx_042__proximity_is_one_over_one_plus_distance() {
        assert_eq!(
            Score::new(4, Relevance::Direct, 1),
            Score::new(2, Relevance::Direct, 0)
        );
        assert!(Score::new(4, Relevance::Direct, 0) > Score::new(4, Relevance::Direct, 1));
    }

    #[test]
    fn r_ctx_042__relevance_is_1_0_then_0_7_then_0_4() {
        assert_eq!(
            [
                Relevance::Direct.pct(),
                Relevance::Glob.pct(),
                Relevance::ScopeWide.pct()
            ],
            [100, 70, 40]
        );
    }

    #[test]
    fn r_ctx_042__a_nearby_warn_outranks_a_distant_scope_wide_error() {
        let err = Score::new(sev_weight(Severity::Error, &w()), Relevance::ScopeWide, 3);
        let warn = Score::new(sev_weight(Severity::Warn, &w()), Relevance::Direct, 0);
        assert!(warn > err);
    }

    #[test]
    fn r_ctx_045__severity_weights_come_from_the_control_file() {
        let custom = SevWeight {
            error: 1,
            warn: 9,
            info: 1,
        };
        let e = Score::new(sev_weight(Severity::Error, &custom), Relevance::Direct, 0);
        let wn = Score::new(sev_weight(Severity::Warn, &custom), Relevance::Direct, 0);
        assert!(wn > e);
        assert_eq!(
            (
                sev_weight(Severity::Error, &w()),
                sev_weight(Severity::Warn, &w()),
                sev_weight(Severity::Info, &w())
            ),
            (4, 2, 1)
        );
    }

    #[test]
    fn r_ctx_042__ties_break_by_id_bytewise() {
        let s = Score::new(4, Relevance::Direct, 0);
        let mut v = vec![
            cand(Priority::P3Items, s, 0, "tok.b"),
            cand(Priority::P3Items, s, 0, "tok.a"),
            cand(Priority::P3Items, s, 0, "Tok.z"),
        ];
        v.sort_by(rank_cmp);
        let keys: Vec<&str> = v.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["Tok.z", "tok.a", "tok.b"]); // 'T' (0x54) sorts before 't' (0x74)
    }

    #[test]
    fn r_ctx_012__use_count_breaks_score_ties_before_the_id() {
        let s = Score::new(1, Relevance::Direct, 0);
        let mut v = vec![
            cand(Priority::P3Items, s, 1, "tok.a"),
            cand(Priority::P3Items, s, 9, "tok.z"),
        ];
        v.sort_by(rank_cmp);
        assert_eq!(v.first().map(|c| c.key.as_str()), Some("tok.z"));
    }

    #[test]
    fn r_ctx_040__classes_are_ordered_p0_to_p7_and_only_p0_p1_are_mandatory() {
        use Priority::*;
        assert!(P0Header < P1ErrorRules && P1ErrorRules < P2Drift && P2Drift < P3Items);
        assert!(P3Items < P4WarnRules && P4WarnRules < P5Recent && P5Recent < P6SiblingsChildren);
        assert!(P6SiblingsChildren < P7AncestorL1);
        assert!(P0Header.is_mandatory() && P1ErrorRules.is_mandatory() && !P2Drift.is_mandatory());
        assert!(!P5Recent.is_active() && P4WarnRules.is_active());
    }

    #[test]
    fn r_ctx_042__priority_orders_before_score() {
        let hi = Score::new(4, Relevance::Direct, 0);
        let lo = Score::new(1, Relevance::ScopeWide, 9);
        let mut v = vec![
            cand(Priority::P6SiblingsChildren, hi, 0, "a"),
            cand(Priority::P2Drift, lo, 0, "b"),
        ];
        v.sort_by(rank_cmp);
        assert_eq!(v.first().map(|c| c.key.as_str()), Some("b"));
    }

    use crate::context::brief::{ChainLink, InhLine};

    fn view(id: &str, home: &str, sev: Severity) -> RuleView {
        RuleView {
            id: id.into(),
            home: home.into(),
            distance: 1,
            sev,
            stage: "enforce".into(),
            glob: vec![],
            why: Some(format!("data fairlead/control.toml@000000 \"{id}\"")),
            ovr: None,
        }
    }

    fn anc() -> Vec<ChainLink> {
        vec![
            ChainLink {
                scope: "root".into(),
                digest: "dq3f2xbr".into(),
            },
            ChainLink {
                scope: "web".into(),
                digest: "7bq4x2ec".into(),
            },
        ]
    }

    fn inh_of(s: &RuleSplit, scope: &str) -> InhLine {
        s.inh
            .iter()
            .find_map(|c| match &c.entry {
                Entry::Inh(i) if i.scope == scope => Some(i.clone()),
                _ => None,
            })
            .unwrap()
    }

    fn ids_in_rules(s: &RuleSplit) -> Vec<String> {
        s.rules
            .iter()
            .filter_map(|c| match &c.entry {
                Entry::Rule(r) => Some(r.id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn r_ctx_020__inherited_error_rules_are_inlined_under_their_ancestor_in_l0_form() {
        let v = vec![
            view("rule.a", "root", Severity::Error),
            view("rule.b", "root", Severity::Error),
            view("rule.c", "web", Severity::Warn),
            view("rule.d", "web", Severity::Error),
        ];
        let s = split_rules(&v, "settings", &anc(), false, &w());
        let root = inh_of(&s, "root");
        assert_eq!((root.err, root.warn), (2, 0));
        assert_eq!(
            root.rules.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["rule.a", "rule.b"]
        );
        assert!(root.rules.iter().all(|r| r.l0 && r.inherited));
        let web = inh_of(&s, "web");
        assert_eq!((web.err, web.warn), (1, 1));
        assert_eq!(web.rules.len(), 1);
        assert!(s.rules.is_empty());
    }

    #[test]
    fn r_ctx_020__every_ancestor_gets_one_mandatory_inh_line_in_chain_order() {
        let s = split_rules(&[], "settings", &anc(), false, &w());
        assert_eq!(s.inh.len(), 2);
        assert!(
            s.inh
                .iter()
                .all(|c| c.priority == Priority::P1ErrorRules && c.priority.is_mandatory())
        );
        let mut sorted = s.inh.clone();
        sorted.sort_by(|a, b| a.key.cmp(&b.key));
        let order: Vec<String> = sorted
            .iter()
            .filter_map(|c| {
                if let Entry::Inh(i) = &c.entry {
                    Some(i.scope.clone())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(order, ["root", "web"]);
    }

    #[test]
    fn r_ctx_020__rules_introduced_at_the_scope_go_to_the_rules_block() {
        let v = vec![
            view("rule.local", "settings", Severity::Error),
            view("rule.soft", "settings", Severity::Warn),
        ];
        let s = split_rules(&v, "settings", &anc(), false, &w());
        assert_eq!(ids_in_rules(&s), ["rule.local", "rule.soft"]);
        assert_eq!(
            s.rules.first().map(|c| c.priority),
            Some(Priority::P1ErrorRules)
        );
        assert_eq!(
            s.rules.get(1).map(|c| c.priority),
            Some(Priority::P4WarnRules)
        );
        assert!(matches!(&s.rules.first().map(|c| &c.entry), Some(Entry::Rule(r)) if !r.inherited));
    }

    #[test]
    fn r_ctx_021__a_rule_overridden_in_the_scope_is_printed_once_with_the_ovr_note() {
        let mut v = view("rule.a", "root", Severity::Warn);
        v.ovr = Some(OvrView {
            scope: "settings".into(),
            note: Some("data a.md@000000 \"legacy\"".into()),
        });
        let s = split_rules(&[v], "settings", &anc(), false, &w());
        assert_eq!(ids_in_rules(&s), ["rule.a"]);
        let Some(Entry::Rule(r)) = s.rules.first().map(|c| c.entry.clone()) else {
            panic!("no rule")
        };
        assert!(r.inherited);
        assert_eq!(
            r.ovr.as_deref(),
            Some("(ovr settings: data a.md@000000 \"legacy\")")
        );
        assert!(inh_of(&s, "root").rules.is_empty());
        assert_eq!(inh_of(&s, "root").err + inh_of(&s, "root").warn, 0);
    }

    #[test]
    fn r_ctx_021__a_rule_overridden_in_an_intermediate_ancestor_prints_once_under_that_ancestor() {
        let mut v = view("rule.a", "root", Severity::Error);
        v.ovr = Some(OvrView {
            scope: "web".into(),
            note: None,
        });
        let s = split_rules(&[v], "settings", &anc(), false, &w());
        assert!(s.rules.is_empty());
        assert!(inh_of(&s, "root").rules.is_empty());
        let web = inh_of(&s, "web");
        assert_eq!(web.err, 1);
        assert_eq!(
            web.rules.first().and_then(|r| r.ovr.clone()).as_deref(),
            Some("(ovr web)")
        );
    }

    #[test]
    fn r_ctx_022__full_inlines_every_inherited_rule_in_full_form() {
        let v = vec![
            view("rule.e", "root", Severity::Error),
            view("rule.w", "root", Severity::Warn),
            view("rule.i", "root", Severity::Info),
        ];
        let s = split_rules(&v, "settings", &anc(), true, &w());
        let root = inh_of(&s, "root");
        assert_eq!((root.err, root.warn), (1, 2));
        assert_eq!(root.rules.len(), 3);
        assert!(root.rules.iter().all(|r| !r.l0));
    }

    #[test]
    fn r_ctx_041__a_rule_with_an_effective_ovr_is_mandatory_even_when_it_is_only_a_warn() {
        let mut v = view("rule.a", "root", Severity::Warn);
        v.ovr = Some(OvrView {
            scope: "settings".into(),
            note: None,
        });
        let s = split_rules(&[v], "settings", &anc(), false, &w());
        assert_eq!(
            s.rules.first().map(|c| c.priority),
            Some(Priority::P1ErrorRules)
        );
        let mut v2 = view("rule.b", "root", Severity::Warn);
        v2.ovr = Some(OvrView {
            scope: "web".into(),
            note: None,
        });
        let s2 = split_rules(&[v2], "settings", &anc(), false, &w());
        assert_eq!(
            inh_of(&s2, "web").rules.len(),
            1,
            "an ovr'd warn is inlined under its ancestor, never only counted"
        );
    }

    #[test]
    fn r_ctx_041__a_rule_with_an_unknown_anchor_falls_back_to_rules_and_is_never_dropped() {
        let s = split_rules(
            &[view("rule.lost", "elsewhere", Severity::Error)],
            "settings",
            &anc(),
            false,
            &w(),
        );
        assert_eq!(ids_in_rules(&s), ["rule.lost"]);
    }

    use crate::context::testutil::{corpus, directive, rec};
    use crate::path::RepoPath;
    use fairlead_syntax::directive::Verb;

    fn corp() -> crate::extract::Corpus {
        corpus(vec![
            rec(
                "apps/web/src/Panel.tsx",
                vec![
                    directive(Verb::Use, &["tok.primary", "cmp.button"]),
                    directive(Verb::Def, &["cmp.panel"]),
                ],
            ),
            rec(
                "apps/web/src/Form.tsx",
                vec![directive(Verb::Use, &["tok.primary"])],
            ),
            rec(
                "apps/web/src-old/x.ts",
                vec![directive(Verb::Use, &["tok.never"])],
            ),
            rec("apps/api/main.rs", vec![directive(Verb::Use, &["cmp.api"])]),
        ])
    }

    #[test]
    fn r_ctx_012__declared_and_used_ids_come_from_directives_under_the_path() {
        let u = collect_id_usage(&corp(), &RepoPath::new("apps/web/src").unwrap());
        assert_eq!(
            u.get("tok.primary"),
            Some(&IdUsage {
                declared: false,
                uses: 2
            })
        );
        assert_eq!(
            u.get("cmp.button"),
            Some(&IdUsage {
                declared: false,
                uses: 1
            })
        );
        assert_eq!(
            u.get("cmp.panel"),
            Some(&IdUsage {
                declared: true,
                uses: 0
            })
        );
        assert!(
            !u.contains_key("tok.never"),
            "sibling dir with a shared string prefix must not leak in"
        );
        assert!(!u.contains_key("cmp.api"));
    }

    #[test]
    fn r_ctx_012__a_file_target_sees_only_that_file() {
        let u = collect_id_usage(&corp(), &RepoPath::new("apps/web/src/Form.tsx").unwrap());
        assert_eq!(
            u.keys().map(String::as_str).collect::<Vec<_>>(),
            ["tok.primary"]
        );
        assert_eq!(u.get("tok.primary").map(|x| x.uses), Some(1));
    }

    #[test]
    fn r_ctx_012__directory_keeps_top_k_by_use_count_and_reports_the_rest() {
        let usage: BTreeMap<String, IdUsage> = (1..=15u32)
            .map(|n| {
                (
                    format!("tok.t{n:02}"),
                    IdUsage {
                        declared: false,
                        uses: n,
                    },
                )
            })
            .collect();
        let (kept, omitted) = id_candidates(&usage, &|_| 0, &w());
        assert_eq!(kept.len(), USES_TOP_K);
        assert_eq!(kept.first().map(|c| c.key.as_str()), Some("tok.t15"));
        assert_eq!(omitted.len(), 1);
        let o = omitted.first().unwrap();
        assert_eq!(
            (o.block, o.count, o.explain.as_str()),
            (Block::Items, 3, "tok.t03")
        );
    }

    #[test]
    fn r_ctx_012__declared_ids_become_declares_and_used_ids_become_uses() {
        let usage: BTreeMap<String, IdUsage> = [
            (
                "cmp.panel".to_string(),
                IdUsage {
                    declared: true,
                    uses: 4,
                },
            ),
            (
                "tok.primary".to_string(),
                IdUsage {
                    declared: false,
                    uses: 2,
                },
            ),
        ]
        .into_iter()
        .collect();
        let (kept, omitted) = id_candidates(&usage, &|_| 0, &w());
        assert!(omitted.is_empty());
        assert!(
            kept.iter()
                .any(|c| matches!(&c.entry, Entry::Declares { id } if id == "cmp.panel"))
        );
        assert!(
            kept.iter()
                .any(|c| matches!(&c.entry, Entry::Uses { id } if id == "tok.primary"))
        );
        assert!(kept.iter().all(|c| c.priority == Priority::P3Items));
    }

    #[test]
    fn r_ctx_042__a_nearer_home_scope_outranks_a_farther_one_for_items() {
        let usage: BTreeMap<String, IdUsage> = [
            (
                "cmp.far".to_string(),
                IdUsage {
                    declared: false,
                    uses: 9,
                },
            ),
            (
                "cmp.near".to_string(),
                IdUsage {
                    declared: false,
                    uses: 1,
                },
            ),
        ]
        .into_iter()
        .collect();
        let (kept, _) = id_candidates(&usage, &|id| if id == "cmp.far" { 2 } else { 0 }, &w());
        assert_eq!(kept.first().map(|c| c.key.as_str()), Some("cmp.near"));
    }

    use crate::check::{Finding, FindingState};
    use crate::context::testutil::finding;
    use crate::diag::codes::{HN_R001, HN_T001};

    fn f(
        code: fairlead_syntax::diag::DiagCode,
        sev: Severity,
        path: &str,
        line: u32,
        st: FindingState,
    ) -> Finding {
        finding(code, sev, path, line, st, Some("no-raw-hex"))
    }

    fn drift_lines(d: &DriftSet) -> Vec<crate::context::brief::DriftLine> {
        let mut v: Vec<&Candidate> = d.candidates.iter().collect();
        v.sort_by(|a, b| rank_cmp(a, b));
        v.into_iter()
            .filter_map(|c| {
                if let Entry::Drift(l) = &c.entry {
                    Some(l.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn r_ctx_013__new_findings_group_by_code_rule_and_file_and_lines_collapse() {
        let p = "apps/web/src/Panel.tsx";
        let v = vec![
            f(HN_T001, Severity::Error, p, 44, FindingState::New),
            f(HN_T001, Severity::Error, p, 81, FindingState::New),
            f(HN_T001, Severity::Error, p, 90, FindingState::New),
            f(HN_T001, Severity::Warn, p, 44, FindingState::New),
        ];
        let refs: Vec<&Finding> = v.iter().collect();
        let d = drift_candidates(&refs, false, &[], true, "kinds", &w());
        let lines = drift_lines(&d);
        assert_eq!(
            lines.first(),
            Some(&crate::context::brief::DriftLine::Summary {
                new: 4,
                baselined: 0,
                waived: 0
            })
        );
        assert_eq!(
            lines.get(1),
            Some(&crate::context::brief::DriftLine::Group {
                code: "HN-T001".into(),
                rule: Some("rule.no-raw-hex".into()),
                path: p.into(),
                lines: vec![44, 81, 90],
                sev: "error".into(),
                n: 4,
            })
        );
    }

    #[test]
    fn r_ctx_013__baselined_findings_are_counted_separately_and_never_listed() {
        let v = vec![
            f(HN_T001, Severity::Error, "a.tsx", 1, FindingState::New),
            f(
                HN_T001,
                Severity::Error,
                "a.tsx",
                2,
                FindingState::Baselined,
            ),
            f(
                HN_T001,
                Severity::Error,
                "b.tsx",
                3,
                FindingState::Baselined,
            ),
        ];
        let refs: Vec<&Finding> = v.iter().collect();
        let lines = drift_lines(&drift_candidates(&refs, false, &[], true, "kinds", &w()));
        assert_eq!(
            lines.first(),
            Some(&crate::context::brief::DriftLine::Summary {
                new: 1,
                baselined: 2,
                waived: 0
            })
        );
        assert_eq!(lines.len(), 2); // summary + the one new group
    }

    #[test]
    fn r_ctx_013__only_findings_under_the_path_are_considered() {
        let report = crate::context::testutil::report(
            vec![
                f(
                    HN_T001,
                    Severity::Error,
                    "apps/web/src/Panel.tsx",
                    1,
                    FindingState::New,
                ),
                f(
                    HN_R001,
                    Severity::Error,
                    "apps/api/main.rs",
                    1,
                    FindingState::New,
                ),
                f(
                    HN_T001,
                    Severity::Error,
                    "apps/web/src-old/x.ts",
                    1,
                    FindingState::New,
                ),
            ],
            false,
        );
        let under_dir = findings_under(&report, &RepoPath::new("apps/web/src").unwrap());
        assert_eq!(under_dir.len(), 1);
        let under_file = findings_under(&report, &RepoPath::new("apps/api/main.rs").unwrap());
        assert_eq!(under_file.len(), 1);
    }

    #[test]
    fn r_ctx_013__a_partial_check_is_flagged_and_never_silent() {
        let d = drift_candidates(&[], true, &[], true, "kinds", &w());
        let lines = drift_lines(&d);
        assert!(lines.contains(&crate::context::brief::DriftLine::Partial));
    }

    #[test]
    fn r_ctx_013__unknown_scopes_are_listed() {
        let d = drift_candidates(&[], false, &["web".to_string()], true, "kinds", &w());
        assert!(
            drift_lines(&d).contains(&crate::context::brief::DriftLine::Unknown {
                scope: "web".into()
            })
        );
    }

    #[test]
    fn r_ctx_010__waived_findings_are_the_active_waivers() {
        let v = vec![
            f(HN_T001, Severity::Error, "a.tsx", 1, FindingState::Waived),
            f(HN_T001, Severity::Error, "a.tsx", 2, FindingState::Waived),
            f(HN_T001, Severity::Error, "a.tsx", 3, FindingState::New),
        ];
        let refs: Vec<&Finding> = v.iter().collect();
        let d = drift_candidates(&refs, false, &[], true, "kinds", &w());
        assert_eq!(d.waivers, 2);
        assert_eq!(
            drift_lines(&d).first(),
            Some(&crate::context::brief::DriftLine::Summary {
                new: 1,
                baselined: 0,
                waived: 2
            })
        );
    }

    #[test]
    fn r_ctx_042__error_drift_outranks_warn_drift() {
        let v = vec![
            f(HN_R001, Severity::Warn, "a.tsx", 1, FindingState::New),
            f(HN_T001, Severity::Error, "z.tsx", 1, FindingState::New),
        ];
        let refs: Vec<&Finding> = v.iter().collect();
        let lines = drift_lines(&drift_candidates(&refs, false, &[], true, "kinds", &w()));
        let first_group = lines.iter().find_map(|l| match l {
            crate::context::brief::DriftLine::Group { code, .. } => Some(code.clone()),
            _ => None,
        });
        assert_eq!(first_group.as_deref(), Some("HN-T001"));
    }

    #[test]
    fn r_ctx_001__session_start_drift_is_counts_only() {
        let v = vec![f(HN_T001, Severity::Error, "a.tsx", 1, FindingState::New)];
        let refs: Vec<&Finding> = v.iter().collect();
        let lines = drift_lines(&drift_candidates(&refs, false, &[], false, "kinds", &w()));
        assert_eq!(lines.len(), 1);
    }
}
