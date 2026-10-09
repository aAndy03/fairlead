//! Declared rules (R-CHK-005..007, C-32).
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::ids::Id;

use crate::check::CheckCx;
use crate::check::codes::all_specs;
use crate::check::finding::{DiagB, RuleId};
use crate::config::Stage;
use crate::diag::Diagnostic;
use crate::diag::codes::HN_U001;
use crate::path::RepoPath;
use crate::registry::{EffectiveItem, FieldValue, ItemId, Registry, ScopeId};

#[derive(Clone, Debug)]
pub enum RuleCheck {
    Leak { tokens: Vec<Id> },
    Pattern { forbid: regex::Regex, msg: String },
}

#[derive(Clone, Debug)]
pub struct DeclaredRule {
    pub id: RuleId,
    pub home: ScopeId,
    pub check: RuleCheck,
    pub sev: Severity,
    pub stage: Stage,
    pub glob: Vec<String>,
    pub exclude: Vec<String>,
    pub waivable: bool,
    pub why: Option<String>,
    pub since: Option<jiff::civil::Date>,
}

/// Built-in structural checks, one impl per file in `check/builtin/`. Pure; run per scope.
pub trait Check: Sync {
    fn codes(&self) -> &'static [DiagCode];
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>);
}

/// Severity, stage and waivability set for a built-in code by a root `check = "builtin"` rule.
#[derive(Clone, Debug)]
pub(crate) struct Tuning {
    pub sev: Option<Severity>,
    pub stage: Option<Stage>,
    pub waivable: bool,
    #[allow(dead_code, reason = "explain shows where a built-in is tuned (P7-T17)")]
    pub home: ScopeId,
}

#[derive(Default)]
pub struct RuleSet {
    by_scope: BTreeMap<ScopeId, Vec<DeclaredRule>>,
    tuning: BTreeMap<DiagCode, Tuning>,
}

fn scalar<'a>(it: &'a EffectiveItem, k: &str) -> Option<&'a str> {
    match &it.fields.get(k)?.value {
        FieldValue::Scalar(s) => Some(s.as_str()),
        FieldValue::List(_) => None,
    }
}

fn list(it: &EffectiveItem, k: &str) -> Vec<String> {
    match it.fields.get(k).map(|f| &f.value) {
        Some(FieldValue::List(v)) => v.clone(),
        Some(FieldValue::Scalar(s)) => vec![s.clone()],
        None => Vec::new(),
    }
}

fn parse_sev(s: &str) -> Option<Severity> {
    match s {
        "error" => Some(Severity::Error),
        "warn" => Some(Severity::Warn),
        "info" => Some(Severity::Info),
        _ => None,
    }
}

fn parse_stage(s: &str) -> Option<Stage> {
    match s {
        "observe" => Some(Stage::Observe),
        "warn" => Some(Stage::Warn),
        "enforce" => Some(Stage::Enforce),
        _ => None,
    }
}

/// `tok.*` expands to every visible item of that kind; other entries must parse as IDs.
fn expand_tokens(reg: &Registry, from: &ScopeId, specs: &[String]) -> Vec<Id> {
    let mut out = BTreeSet::new();
    for s in specs {
        if let Some(kind) = s.strip_suffix(".*") {
            for iid in reg
                .view(from)
                .items
                .keys()
                .filter(|i| i.kind.as_str() == kind)
            {
                out.insert(iid.to_id());
            }
        } else if let Ok(id) = Id::parse(s) {
            out.insert(id);
        }
    }
    out.into_iter().collect()
}

fn declared(reg: &Registry, iid: &ItemId, item: &EffectiveItem) -> Option<DeclaredRule> {
    let check = match scalar(item, "check")? {
        "leak" => RuleCheck::Leak {
            tokens: expand_tokens(reg, &iid.scope, &list(item, "tokens")),
        },
        "pattern" => RuleCheck::Pattern {
            forbid: regex::Regex::new(scalar(item, "forbid")?).ok()?,
            msg: scalar(item, "msg").unwrap_or("").to_string(),
        },
        _ => return None,
    };
    Some(DeclaredRule {
        id: RuleId(iid.name.clone()),
        home: iid.scope.clone(),
        check,
        // R-CHK-009, A-52: an item without explicit values is `sev=error stage=observe`.
        sev: scalar(item, "sev")
            .and_then(parse_sev)
            .unwrap_or(Severity::Error),
        stage: scalar(item, "stage")
            .and_then(parse_stage)
            .unwrap_or(Stage::Observe),
        glob: list(item, "glob"),
        exclude: list(item, "exclude"),
        waivable: scalar(item, "waivable") == Some("true"),
        why: scalar(item, "why").map(str::to_string),
        since: scalar(item, "since").and_then(|s| s.parse().ok()),
    })
}

impl RuleSet {
    pub fn collect(reg: &Registry) -> RuleSet {
        let mut rs = RuleSet::default();
        for scope in reg.scopes() {
            let mut rules = Vec::new();
            for (iid, item) in &reg.view(&scope.id).items {
                if iid.kind.as_str() != "rule" {
                    continue;
                }
                if let Some(r) = declared(reg, iid, item) {
                    rules.push(r);
                }
            }
            rules.sort_by(|a, b| a.id.cmp(&b.id));
            rs.by_scope.insert(scope.id.clone(), rules);
        }
        for (iid, item) in &reg.view(reg.root_scope()).items {
            if iid.kind.as_str() != "rule" || scalar(item, "check") != Some("builtin") {
                continue;
            }
            if let Some(s) = all_specs()
                .iter()
                .find(|s| s.code.0.eq_ignore_ascii_case(iid.name.as_str()))
            {
                rs.tuning.insert(
                    s.code,
                    Tuning {
                        sev: scalar(item, "sev").and_then(parse_sev),
                        stage: scalar(item, "stage").and_then(parse_stage),
                        waivable: scalar(item, "waivable") == Some("true"),
                        home: iid.scope.clone(),
                    },
                );
            }
        }
        rs
    }

    /// Rules effective at `s` (own and inherited), sorted by id.
    pub fn for_scope(&self, s: &ScopeId) -> &[DeclaredRule] {
        self.by_scope.get(s).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Distinct declared rules over all scopes.
    pub fn total(&self) -> u32 {
        let n = self
            .by_scope
            .values()
            .flatten()
            .map(|r| (&r.home, &r.id))
            .collect::<BTreeSet<_>>()
            .len();
        u32::try_from(n).unwrap_or(u32::MAX)
    }

    pub(crate) fn tuning(&self, code: DiagCode) -> Option<&Tuning> {
        self.tuning.get(&code)
    }

    /// R-CHK-007: declared under or inherited by the file's scope, matching `glob`, not `exclude`.
    /// Globs are relative to the declaring scope and go through `Registry::glob_matches`, so
    /// `[[convention]] transparent` segments are hidden (A-27).
    pub fn applies(&self, r: &DeclaredRule, path: &RepoPath, reg: &Registry) -> bool {
        let Some(sid) = reg.scope_of(path) else {
            return false;
        };
        if !self
            .for_scope(sid)
            .iter()
            .any(|x| x.home == r.home && x.id == r.id)
        {
            return false;
        }
        let included =
            r.glob.is_empty() || r.glob.iter().any(|g| reg.glob_matches(&r.home, g, path));
        included && !r.exclude.iter().any(|g| reg.glob_matches(&r.home, g, path))
    }
}

pub struct PatternRules;
impl Check for PatternRules {
    fn codes(&self) -> &'static [DiagCode] {
        &[HN_U001]
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        for (path, rec) in cx
            .corpus
            .records
            .iter()
            .filter(|(p, _)| cx.registry.scope_of(p) == Some(scope))
        {
            for h in &rec.pattern_hits {
                let Some((home, name)) = h.rule_key.split_once(':') else {
                    continue;
                };
                let Some(rule) = cx
                    .rules
                    .for_scope(scope)
                    .iter()
                    .find(|r| r.home.0.as_str() == home && r.id.0.as_str() == name)
                else {
                    continue;
                };
                let RuleCheck::Pattern { msg, .. } = &rule.check else {
                    continue;
                };
                if !cx.rules.applies(rule, path, cx.registry) {
                    continue;
                }
                out.push(
                    DiagB::new(
                        HN_U001,
                        path,
                        h.span.line,
                        h.span.col,
                        format!("rule.{name}: {msg}"),
                    )
                    .rule(name)
                    .severity(rule.sev)
                    .text(h.text.clone())
                    .hint(format!("see fairlead explain rule.{name}"))
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::World;
    use crate::diag::codes as c;

    const RULES: &str = r#"
[kinds.tok]
shape = "doc"
[rules.no-raw-hex]
check = "leak"
tokens = ["tok.*"]
sev = "error"
stage = "warn"
glob = ["**/src/**/*.css"]
exclude = ["**/skip/**"]
waivable = true
why = "colours come from tokens"
[rules.no-console]
check = "pattern"
forbid = "console\\.log"
msg = "no console.log"
[rules.hn-x002]
check = "builtin"
stage = "enforce"
"#;

    #[test]
    fn r_chk_005__r_chk_009__rules_come_from_effective_views_with_defaults() {
        let b = World::new().root(RULES).build();
        let rs = RuleSet::collect(&b.registry);
        let root = b.registry.root_scope().clone();
        let rules = rs.for_scope(&root);
        assert_eq!(rules.len(), 2);
        let leak = rules
            .iter()
            .find(|r| matches!(r.check, RuleCheck::Leak { .. }))
            .unwrap();
        assert_eq!(
            (leak.sev, leak.stage, leak.waivable),
            (Severity::Error, Stage::Warn, true)
        );
        assert_eq!(leak.why.as_deref(), Some("colours come from tokens"));
        let pat = rules
            .iter()
            .find(|r| matches!(r.check, RuleCheck::Pattern { .. }))
            .unwrap();
        assert_eq!((pat.sev, pat.stage), (Severity::Error, Stage::Observe)); // R-CHK-009, A-52
        assert_eq!(rs.total(), 2);
    }

    #[test]
    fn r_chk_005__builtin_tuning_rule_is_named_after_the_code() {
        let b = World::new().root(RULES).build();
        let rs = RuleSet::collect(&b.registry);
        assert_eq!(
            rs.tuning(c::HN_X002).and_then(|t| t.stage),
            Some(Stage::Enforce)
        );
        assert!(rs.tuning(c::HN_X004).is_none());
    }

    #[test]
    fn r_chk_007__applies_uses_glob_exclude_inheritance_and_transparent_segments() {
        let web =
            "[fairlead]\nscope = \"web\"\n[[convention]]\nglob = \"app/(*)\"\ntransparent = true\n";
        let b = World::new().root(RULES).scope("web", web).build();
        let rs = RuleSet::collect(&b.registry);
        let root = b.registry.root_scope().clone();
        let rule = rs
            .for_scope(&root)
            .iter()
            .find(|r| matches!(r.check, RuleCheck::Leak { .. }))
            .unwrap()
            .clone();
        let yes = |p: &str| rs.applies(&rule, &RepoPath::new(p).unwrap(), &b.registry);
        assert!(yes("web/src/a.css")); // inherited by the child scope
        assert!(yes("lib/src/b.css"));
        assert!(!yes("web/src/skip/c.css")); // exclude
        assert!(!yes("web/src/d.ts")); // glob
        assert!(!yes("web/lib/e.css"));
    }

    #[test]
    fn r_chk_060__pattern_hits_become_u001_with_the_rules_severity_and_id() {
        let rule = "[rules.no-console]\ncheck = \"pattern\"\nforbid = \"console\\\\.log\"\nmsg = \"no console.log\"\nsev = \"error\"\nglob = [\"**/*.ts\"]\n";
        let b = World::new()
            .root(rule)
            .file("src/a.ts", "const x = 1;\n  console.log(x);\n")
            .file("src/a.md", "console.log(1)\n")
            .build();
        let ds = b.run_check(&PatternRules);
        assert_eq!(ds.len(), 1);
        let d = &ds[0];
        assert_eq!(
            (d.code, d.severity, d.location.line, d.location.col),
            (c::HN_U001, Severity::Error, 2, 3)
        );
        assert_eq!(
            (d.message.as_str(), d.rule.as_deref(), d.text.as_str()),
            (
                "rule.no-console: no console.log",
                Some("rule.no-console"),
                "console.log"
            )
        );
    }
}
