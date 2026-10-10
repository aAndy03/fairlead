//! Lifecycle plans (R-LC-001 to R-LC-008): references to a deprecated or replaced ID (HN-R006,
//! warn until due, error from due), replacement cycles and repeated retargeting (HN-L001).
use std::collections::BTreeMap;

use fairlead_syntax::diag::{DiagCode, Severity};

use crate::check::builtin::{bare_id, field_str, root_control};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::docs::collect_change_docs;
use crate::lifecycle::{
    DueState, Plan, State, Vocab, collect_plans, due, effective_target, item_key, recorded_targets,
    resolve_chain, retargets,
};
use crate::registry::{ItemId, Registry, ScopeId};
use crate::version::{Version, VersionLine};

pub struct PlansCheck;
const CODES: [DiagCode; 2] = [HN_L001, HN_R006];

/// Everything the due computation needs, derived once per run.
pub(crate) struct DueCx {
    pub plans: Vec<Plan>,
    pub line: VersionLine,
    pub start: Version,
    pub recorded: BTreeMap<String, Version>,
    pub moved: BTreeMap<String, (Version, u32)>,
}

pub(crate) fn due_cx(cx: &CheckCx<'_>) -> Option<DueCx> {
    due_cx_of(cx.registry, cx.corpus, &cx.facts.ledgers)
}

pub(crate) fn due_cx_of(
    reg: &Registry,
    corpus: &crate::extract::Corpus,
    ledgers: &BTreeMap<crate::path::RepoPath, std::sync::Arc<str>>,
) -> Option<DueCx> {
    let root = root_control(reg)?;
    let start = Version::parse(&root.version.start).unwrap_or(Version::new(0, 1, 0));
    let folder = root
        .settings
        .folder
        .clone()
        .unwrap_or_else(|| "fairlead".into());
    let (docs, _) = collect_change_docs(corpus);
    Some(DueCx {
        plans: collect_plans(reg, corpus),
        line: crate::docs::line_of(corpus, &folder, start),
        start,
        recorded: recorded_targets(ledgers.values().map(|t| &**t)),
        moved: retargets(&docs, reg),
    })
}

/// The seal's view of every plan (R-LC-006) and the ledger records of the symbolic targets it
/// resolves (R-LC-003): a symbolic target not yet recorded resolves against the sealed version
/// before this seal.
pub(crate) fn seal_lifecycle(
    reg: &Registry,
    corpus: &crate::extract::Corpus,
    ledgers: &BTreeMap<crate::path::RepoPath, std::sync::Arc<str>>,
    as_of: jiff::civil::Date,
) -> (
    Vec<crate::seal::DueItem>,
    Vec<(crate::path::RepoPath, String)>,
) {
    let Some(dc) = due_cx_of(reg, corpus, ledgers) else {
        return (Vec::new(), Vec::new());
    };
    let window = root_control(reg).map_or(1, |r| r.lc.warn_window);
    let mut items = Vec::new();
    let mut records = Vec::new();
    for p in &dc.plans {
        let refs: Vec<String> = reg
            .edges()
            .iter()
            .filter(|e| e.resolved.as_ref() == Some(&p.item))
            .map(|e| format!("{}:{}", e.from.path.as_str(), e.from.span.line))
            .collect();
        items.push(crate::seal::DueItem {
            id: bare_id(&p.item),
            key: item_key(&p.item),
            word: p.word.as_str().to_string(),
            target: dc.target(p),
            state: dc.state(p, as_of, window),
            refs,
        });
        if let Some(t) = p.target
            && !matches!(t, crate::lifecycle::Target::Version(_))
            && !dc.recorded.contains_key(&item_key(&p.item))
        {
            let v = t.resolve(dc.line.sealed, dc.start);
            records.push((p.path.clone(), crate::lifecycle::plan_record(p, v)));
        }
    }
    (items, records)
}

impl DueCx {
    pub fn target(&self, p: &Plan) -> Option<Version> {
        effective_target(p, &self.recorded, &self.moved, self.line.sealed, self.start)
    }
    pub fn state(&self, p: &Plan, as_of: jiff::civil::Date, window: u32) -> DueState {
        due(self.target(p), p.by_date, self.line.open, as_of, window)
    }
}

/// The retiring plan of an item, if any (deprecate, replace or remove), else a `st=deprecated`.
fn retiring<'a>(plans: &'a [Plan], id: &ItemId) -> Option<&'a Plan> {
    plans.iter().find(|p| p.item == *id && p.word.retires())
}

fn declared_deprecated(reg: &Registry, v: &Vocab<'_>, id: &ItemId) -> bool {
    reg.item(id)
        .and_then(|it| field_str(it, "st"))
        .and_then(|s| v.state(s))
        .is_some_and(|s| s == State::Deprecated)
}

impl Check for PlansCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(root) = root_control(reg) else {
            return;
        };
        let v = Vocab::new(&root.vocab);
        let Some(dc) = due_cx(cx) else { return };
        let as_of = cx.options.as_of.0;
        let window = root.lc.warn_window;
        let next: BTreeMap<ItemId, ItemId> = dc
            .plans
            .iter()
            .filter_map(|p| Some((p.item.clone(), p.to.clone()?)))
            .collect();
        for p in dc
            .plans
            .iter()
            .filter(|p| reg.scope_of(&p.path) == Some(scope))
        {
            if let Err(cycle) = resolve_chain(&p.item, &next)
                && cycle.first() == Some(&p.item)
            {
                let names: Vec<String> = cycle.iter().map(bare_id).collect();
                out.push(
                    DiagB::new(
                        HN_L001,
                        &p.path,
                        p.line,
                        p.col,
                        format!("replacement cycle: {}", names.join(" -> ")),
                    )
                    .severity(Severity::Error)
                    .hint("a replace chain must end at an active ID; fix one to=")
                    .related(bare_id(&p.item))
                    .text(bare_id(&p.item))
                    .build(),
                );
            }
            if let Some((_, n)) = dc.moved.get(&item_key(&p.item))
                && *n >= root.lc.stale_after
            {
                out.push(
                    DiagB::new(
                        HN_L001,
                        &p.path,
                        p.line,
                        p.col,
                        format!("the plan for {} was retargeted {n} times", bare_id(&p.item)),
                    )
                    .hint("finish the plan or drop the flag; retargeting again only moves the date")
                    .related(bare_id(&p.item))
                    .text(bare_id(&p.item))
                    .build(),
                );
            }
        }
        for e in reg.edges().iter().filter(|e| e.from.scope == *scope) {
            let Some(to) = e.resolved.as_ref() else {
                continue;
            };
            let plan = retiring(&dc.plans, to);
            if plan.is_none() && !declared_deprecated(reg, &v, to) {
                continue;
            }
            let state = plan.map_or(DueState::NotDue, |p| dc.state(p, as_of, window));
            let successor = match resolve_chain(to, &next) {
                Ok(s) if s != *to => Some(s),
                _ => None,
            };
            let mut msg = format!("{} is deprecated", bare_id(to));
            if let Some(t) = plan.and_then(|p| dc.target(p)) {
                msg.push_str(&format!(
                    " ({} at {t})",
                    plan.map_or("deprecate", |p| p.word.as_str())
                ));
            }
            let hint = match &successor {
                Some(s) => format!("use {} instead", bare_id(s)),
                None => "remove the reference before the plan is due".to_string(),
            };
            let mut b = DiagB::new(
                HN_R006,
                &e.from.path,
                e.from.span.line,
                e.from.span.col,
                msg,
            )
            .hint(hint)
            .related(bare_id(to))
            .text(bare_id(to));
            if state == DueState::Due {
                b = b.severity(Severity::Error);
            }
            out.push(b.build());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    const ROOT: &str = "[kinds.engine]\nimpl = 1\n";
    fn doc(defs: &[&str]) -> String {
        let lines: Vec<String> = defs.iter().map(|d| format!("  '{d}',")).collect();
        format!(
            "+++\ndirectives = [\n{}\n]\n+++\n# Engines\n",
            lines.join("\n")
        )
    }
    fn using(id: &str) -> String {
        format!("// fairlead: use {id}\nexport const A = 1;\n")
    }
    fn version_file(v: &str) -> (String, String) {
        (
            format!("fairlead/versions/{v}.md"),
            "+++\n+++\n".to_string(),
        )
    }

    #[test]
    fn r_lc_008__deprecated_reference_warns_until_due_then_errors() {
        let w = |target: &str| {
            let (vp, vt) = version_file("1.4.0");
            World::new()
                .root(ROOT)
                .file(
                    "fairlead/docs/engine.md",
                    &doc(&[
                        "def engine.old",
                        "def engine.new",
                        &format!("flag replace engine.old to=engine.new target={target}"),
                    ]),
                )
                .file("src/a.ts", &using("engine.old"))
                .file(&vp, &vt)
        };
        let early = w("2.0.0").build().run_check(&PlansCheck);
        let r = of(&early, c::HN_R006);
        assert_eq!((r.len(), r[0].severity), (1, Severity::Warn));
        assert!(r[0].hint.contains("engine.new"), "{}", r[0].hint);
        let due = w("1.4.1").build().run_check(&PlansCheck);
        assert_eq!(
            of(&due, c::HN_R006)[0].severity,
            Severity::Error,
            "O = 1.4.1 reached"
        );
    }

    #[test]
    fn r_lc_008__st_deprecated_without_a_plan_warns() {
        let ds = World::new()
            .root(ROOT)
            .file(
                "fairlead/docs/engine.md",
                &doc(&["def engine.old st=deprecated"]),
            )
            .file("src/a.ts", &using("engine.old"))
            .build()
            .run_check(&PlansCheck);
        assert_eq!(of(&ds, c::HN_R006).len(), 1);
    }

    #[test]
    fn r_lc_007__replacement_cycle_is_printed() {
        let ds = World::new()
            .root(ROOT)
            .file(
                "fairlead/docs/engine.md",
                &doc(&[
                    "def engine.a",
                    "def engine.b",
                    "flag replace engine.a to=engine.b target=9.0.0",
                    "flag replace engine.b to=engine.a target=9.0.0",
                ]),
            )
            .build()
            .run_check(&PlansCheck);
        let l = of(&ds, c::HN_L001);
        assert_eq!(l.len(), 2, "one per flag in the cycle");
        assert!(l[0].message.contains("engine.a -> engine.b -> engine.a"));
        assert_eq!(l[0].severity, Severity::Error);
    }

    #[test]
    fn r_lc_006__repeated_retargeting_is_stale_plan() {
        let retarget = |n: u32| {
            format!(
                "+++\nid = \"d-0000000{n}\"\nkind = \"retarget\"\nstatus = \"sealed\"\nimpact = \"fix\"\ncovers = [\"src/**\"]\nsealed = \"1.{n}.0\"\nordinal = {n}\nretargets = \"engine.old\"\ntarget = \"3.{n}.0\"\n+++\n"
            )
        };
        let mut w = World::new().root(ROOT).file(
            "fairlead/docs/engine.md",
            &doc(&["def engine.old", "flag remove engine.old target=2.0.0"]),
        );
        for n in 1..=3 {
            w = w.file(&format!("fairlead/docs/changes/000{n}-r.md"), &retarget(n));
        }
        let ds = w.build().run_check(&PlansCheck);
        let l = of(&ds, c::HN_L001);
        assert_eq!(l.len(), 1, "{ds:?}");
        assert!(l[0].message.contains("retargeted 3 times"));
    }
}
