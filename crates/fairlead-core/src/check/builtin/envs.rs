//! Environments and maturity (chapter 21): unknown vocabulary words (E004), missing maturity
//! (E005), per-env order (E006), regression against the committed shard (E007), and the edge
//! rules `rule.maturity-gap` (E006) and `rule.env-leakage` (E001) over declared `use` edges.
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::directive::{ArgValue, Directive, Extent, Verb};

use crate::check::builtin::{bare_id, field_list, origin_loc, root_control};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::extract::Corpus;
use crate::lifecycle::{Axes, UnknownWord, Vocab, order_violations, parse_in, parse_mat};
use crate::path::RepoPath;
use crate::registry::{ItemId, Lookup, Registry, ScopeId};
use crate::render::line::TerseLine;

pub struct EnvCheck;
const CODES: [DiagCode; 5] = [HN_E001, HN_E004, HN_E005, HN_E006, HN_E007];

/// The env and maturity words written for one item (doc `def` fields, then code `def` args).
#[derive(Default)]
struct Written {
    loc: Option<(RepoPath, u32, u32)>,
    ins: Option<Vec<String>>,
    mats: Option<Vec<String>>,
    bind: Vec<String>,
}

fn arg_words(v: &ArgValue) -> Vec<String> {
    match v {
        ArgValue::List(l) => l.clone(),
        ArgValue::Word(w) | ArgValue::Quoted(w) => w
            .split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

fn written(reg: &Registry, corpus: &Corpus) -> BTreeMap<ItemId, Written> {
    let mut out: BTreeMap<ItemId, Written> = BTreeMap::new();
    for it in reg.items() {
        let has = |k: &str| it.fields.contains_key(k);
        if !(has("in") || has("mat") || has("bind")) {
            continue;
        }
        let w = out.entry(it.id.clone()).or_default();
        w.loc = Some(origin_loc(it));
        w.ins = has("in").then(|| field_list(it, "in"));
        w.mats = has("mat").then(|| field_list(it, "mat"));
        w.bind = field_list(it, "bind");
    }
    for (path, rec) in &corpus.records {
        let Some(ex) = rec.extraction.as_ref() else {
            continue;
        };
        let Some(scope) = reg.scope_of(path) else {
            continue;
        };
        for d in ex.directives.iter().filter(|d| d.verb == Verb::Def) {
            let arg = |k: &str| d.arg(k).map(|a| arg_words(&a.value));
            let (ins, mats, bind) = (arg("in"), arg("mat"), arg("bind").unwrap_or_default());
            if ins.is_none() && mats.is_none() && bind.is_empty() {
                continue;
            }
            let Some(id) = d.ids.first() else { continue };
            let Lookup::Found(item) = reg.lookup(scope, id) else {
                continue;
            };
            let w = out.entry(item).or_default();
            if w.loc.is_none() {
                w.loc = Some((path.clone(), d.span.line, d.span.col));
            }
            w.ins = w.ins.take().or(ins);
            w.mats = w.mats.take().or(mats);
            if w.bind.is_empty() {
                w.bind = bind;
            }
        }
    }
    out
}

/// The scope defaults along the chain, narrowed only (R-ENV-005, R-ENV-040): the env set is the
/// intersection of every declared `[defaults] in`, the maturity the highest declared `mat`.
fn defaults(
    reg: &Registry,
    v: &Vocab<'_>,
    scope: &ScopeId,
) -> (Option<BTreeSet<usize>>, Option<i64>) {
    let (mut envs, mut mat): (Option<BTreeSet<usize>>, Option<i64>) = (None, None);
    for s in reg.chain(scope) {
        let Some(c) = reg.scope(s).and_then(|x| x.control.as_ref()) else {
            continue;
        };
        if let Some(words) = &c.defaults.envs {
            let set = parse_in(v, words, &mut Vec::new());
            envs = Some(match envs {
                Some(e) => e.intersection(&set).copied().collect(),
                None => set,
            });
        }
        if let Some(r) = c.defaults.mat.as_deref().and_then(|m| v.rank(m)) {
            mat = Some(mat.map_or(r, |m: i64| m.max(r)));
        }
    }
    (envs, mat)
}

/// Resolved axes of every item that writes an axis key, with the words that resolved to nothing.
pub(crate) struct Resolved {
    pub axes: BTreeMap<ItemId, Axes>,
    pub loc: BTreeMap<ItemId, (RepoPath, u32, u32)>,
    /// `bind=` values per item (R-ENV-030).
    pub binds: BTreeMap<ItemId, Vec<String>>,
    unknown: BTreeMap<ItemId, Vec<UnknownWord>>,
    missing: BTreeSet<ItemId>,
}

pub(crate) fn resolve_axes(reg: &Registry, corpus: &Corpus, v: &Vocab<'_>) -> Resolved {
    let mut r = Resolved {
        axes: BTreeMap::new(),
        loc: BTreeMap::new(),
        binds: BTreeMap::new(),
        unknown: BTreeMap::new(),
        missing: BTreeSet::new(),
    };
    for (id, w) in written(reg, corpus) {
        let (def_envs, def_mat) = defaults(reg, v, &id.scope);
        let mut bad = Vec::new();
        let envs = match &w.ins {
            Some(words) => parse_in(v, words, &mut bad),
            None => def_envs.unwrap_or_else(|| v.all_envs()),
        };
        let mut mat = match &w.mats {
            Some(words) => parse_mat(v, words, &envs, &mut bad),
            None => BTreeMap::new(),
        };
        if let Some(d) = def_mat {
            for e in &envs {
                mat.entry(*e).or_insert(d);
            }
        }
        mat.retain(|e, _| envs.contains(e));
        if v.has_maturity()
            && (w.ins.is_some() || !w.bind.is_empty())
            && envs.iter().any(|e| !mat.contains_key(e))
        {
            r.missing.insert(id.clone());
        }
        if let Some(l) = w.loc.clone() {
            r.loc.insert(id.clone(), l);
        }
        if !w.bind.is_empty() {
            r.binds.insert(id.clone(), w.bind.clone());
        }
        if !bad.is_empty() {
            r.unknown.insert(id.clone(), bad);
        }
        r.axes.insert(id, Axes { envs, mat });
    }
    r
}

fn covers(extent: &Extent, line: u32) -> bool {
    match extent {
        Extent::File | Extent::Doc => true,
        Extent::Lines { first, last } => (*first..=*last).contains(&line),
    }
}

/// Env words of every `env` directive whose region covers `line` (intersected).
fn gate(v: &Vocab<'_>, dirs: &[Directive], line: u32) -> Option<BTreeSet<usize>> {
    let mut out: Option<BTreeSet<usize>> = None;
    for d in dirs
        .iter()
        .filter(|d| d.verb == Verb::Env && covers(&d.extent, line))
    {
        let set = parse_in(v, &d.bare, &mut Vec::new());
        out = Some(match out {
            Some(o) => o.intersection(&set).copied().collect(),
            None => set,
        });
    }
    out
}

/// The position of each distinct declared rank (ranks may have gaps, R-ENV-002).
fn rank_steps(v: &Vocab<'_>) -> BTreeMap<i64, usize> {
    let ranks: BTreeSet<i64> = v.cfg.maturity.values().copied().collect();
    ranks.into_iter().enumerate().map(|(i, r)| (r, i)).collect()
}

/// Committed maturity per item from the on-disk registry shards of `scope` (R-ENV-012).
fn committed_mats(cx: &CheckCx<'_>, scope: &ScopeId) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (p, bytes) in &cx.facts.generated_on_disk {
        if !p.as_str().ends_with(".reg") || cx.registry.scope_of(p) != Some(scope) {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            if let Ok(t) = TerseLine::parse(line)
                && let Some(m) = t.get("mat")
            {
                out.insert(t.id.clone(), m.split(',').map(str::to_string).collect());
            }
        }
    }
    out
}

impl Check for EnvCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(root) = root_control(reg) else {
            return;
        };
        let v = Vocab::new(&root.vocab);
        if scope == reg.root_scope() {
            for w in v
                .cfg
                .env_order
                .iter()
                .filter(|w| crate::lifecycle::THEME_WORDS.contains(&w.as_str()))
            {
                out.push(
                    DiagB::new(
                        HN_E004,
                        &root.path,
                        0,
                        0,
                        format!("`{w}` in env.order is a theme variant, not an environment"),
                    )
                    .hint(
                        "remove it from [env] order; theme variants are values of a variation axis",
                    )
                    .text(w.clone())
                    .build(),
                );
            }
        }
        let res = resolve_axes(reg, cx.corpus, &v);
        let here = |id: &ItemId| id.scope == *scope;
        for (id, bad) in res.unknown.iter().filter(|(id, _)| here(id)) {
            let Some((p, l, c)) = res.loc.get(id) else {
                continue;
            };
            for b in bad {
                let msg = if b.theme {
                    format!(
                        "`{}` in {}= is a theme variant, not an environment",
                        b.word, b.key
                    )
                } else {
                    format!(
                        "`{}` in {}= is not a declared {} word",
                        b.word,
                        b.key,
                        if b.key == "in" { "env" } else { "maturity" }
                    )
                };
                out.push(
                    DiagB::new(HN_E004, p, *l, *c, msg)
                        .hint("use a word of the root [env] order or [maturity] table, or an alias")
                        .related(bare_id(id))
                        .text(b.word.clone())
                        .build(),
                );
            }
        }
        for id in res.missing.iter().filter(|id| here(id)) {
            let Some((p, l, c)) = res.loc.get(id) else {
                continue;
            };
            out.push(
                DiagB::new(
                    HN_E005,
                    p,
                    *l,
                    *c,
                    format!("{} has no maturity for some of its envs", bare_id(id)),
                )
                .hint("add mat=<word> or mat=<env>:<word>, or a scope [defaults] mat")
                .related(bare_id(id))
                .text(bare_id(id))
                .build(),
            );
        }
        let committed = committed_mats(cx, scope);
        for (id, axes) in res.axes.iter().filter(|(id, _)| here(id)) {
            let Some((p, l, c)) = res.loc.get(id) else {
                continue;
            };
            for (a, b) in order_violations(axes) {
                out.push(
                    DiagB::new(
                        HN_E006,
                        p,
                        *l,
                        *c,
                        format!(
                            "{} is more mature in {} than in {}",
                            bare_id(id),
                            v.env_name(b),
                            v.env_name(a)
                        ),
                    )
                    .hint("a later env may not be more mature than an earlier one")
                    .related(bare_id(id))
                    .text(bare_id(id))
                    .build(),
                );
            }
            if let Some(old) = committed.get(&bare_id(id)) {
                let before = parse_mat(&v, old, &axes.envs, &mut Vec::new());
                for (e, r) in &axes.mat {
                    if before.get(e).is_some_and(|o| r < o) {
                        out.push(
                            DiagB::new(
                                HN_E007,
                                p,
                                *l,
                                *c,
                                format!(
                                    "{} regressed in {} from {} to {}",
                                    bare_id(id),
                                    v.env_name(*e),
                                    v.rank_word(before[e]),
                                    v.rank_word(*r)
                                ),
                            )
                            .hint("a regression needs a change doc; seal records it in the ledger")
                            .related(bare_id(id))
                            .text(bare_id(id))
                            .build(),
                        );
                    }
                }
            }
        }
        edges(cx, scope, &v, &res, out);
    }
}

/// `rule.maturity-gap` and `rule.env-leakage` over the declared `use` edges of this scope.
fn edges(
    cx: &CheckCx<'_>,
    scope: &ScopeId,
    v: &Vocab<'_>,
    res: &Resolved,
    out: &mut Vec<Diagnostic>,
) {
    let reg = cx.registry;
    let steps = rank_steps(v);
    for e in reg.edges().iter().filter(|e| e.from.scope == *scope) {
        let Some(to) = e.resolved.as_ref() else {
            continue;
        };
        let Some(ta) = res.axes.get(to) else { continue };
        let line = e.from.span.line;
        let dirs = cx
            .corpus
            .records
            .get(&e.from.path)
            .and_then(|r| r.extraction.as_ref())
            .map_or(&[][..], |x| &x.directives[..]);
        let region = gate(v, dirs, line);
        let users: Vec<&ItemId> = res
            .axes
            .keys()
            .filter(|d| *d != to)
            .filter(|d| {
                reg.sites_of(d)
                    .iter()
                    .any(|s| s.path == e.from.path && covers(&s.extent, line))
            })
            .collect();
        for d in users {
            let da = &res.axes[d];
            if let (Some(dr), Some(tr)) = (da.mat.values().min(), ta.mat.values().min()) {
                let gap = steps
                    .get(dr)
                    .zip(steps.get(tr))
                    .map_or(0, |(a, b)| a.saturating_sub(*b));
                if gap > 0 {
                    let sev = if gap >= 2 {
                        Severity::Error
                    } else {
                        Severity::Warn
                    };
                    out.push(
                        DiagB::new(
                            HN_E006,
                            &e.from.path,
                            line,
                            e.from.span.col,
                            format!(
                                "{} ({}) depends on {} ({})",
                                bare_id(d),
                                v.rank_word(*dr),
                                bare_id(to),
                                v.rank_word(*tr)
                            ),
                        )
                        .rule("maturity-gap")
                        .severity(sev)
                        .hint("raise the dependency's maturity, lower the dependent's, or waive rule.maturity-gap")
                        .related(bare_id(to))
                        .text(bare_id(to))
                        .build(),
                    );
                }
            }
            let reach: BTreeSet<usize> = match &region {
                Some(g) => da.envs.intersection(g).copied().collect(),
                None => da.envs.clone(),
            };
            for env in reach.difference(&ta.envs) {
                out.push(
                    DiagB::new(
                        HN_E001,
                        &e.from.path,
                        line,
                        e.from.span.col,
                        format!(
                            "{} reaches {} but uses {}, which does not exist there",
                            bare_id(d),
                            v.env_name(*env),
                            bare_id(to)
                        ),
                    )
                    .rule("env-leakage")
                    .hint("narrow the dependent's in=, gate the use with `env`, or widen the dependency")
                    .related(bare_id(to))
                    .text(bare_id(to))
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;

    const VOCAB: &str = "[env]\norder = [\"dev\", \"staging\", \"prod\"]\n[env.alias]\nqa = \"staging\"\n[maturity]\nalpha = 0\nbeta = 1\nstable = 3\n[kinds.engine]\nimpl = 1\n";

    fn doc(defs: &[&str]) -> String {
        let lines: Vec<String> = defs.iter().map(|d| format!("  '{d}',")).collect();
        format!(
            "+++\ndirectives = [\n{}\n]\n+++\n# Engines\n",
            lines.join("\n")
        )
    }

    fn run(w: World) -> Vec<Diagnostic> {
        w.build().run_check(&EnvCheck)
    }

    #[test]
    fn r_env_003__unknown_env_and_maturity_words_are_voc_unknown() {
        let ds = run(World::new().root(VOCAB).file(
            "fairlead/docs/engine.md",
            &doc(&[
                "def engine.a in=qa,canary mat=beta",
                "def engine.b in=dev mat=gold",
            ]),
        ));
        let e = of(&ds, c::HN_E004);
        let words: Vec<&str> = e.iter().map(|d| d.text.as_str()).collect();
        assert_eq!(words, ["canary", "gold"], "qa is an alias: valid");
    }

    #[test]
    fn r_env_037__theme_word_in_in_or_order_is_voc_unknown() {
        let ds = run(World::new()
            .root("[env]\norder = [\"dev\", \"dark\"]\n[kinds.engine]\nimpl = 1\n")
            .file("fairlead/docs/engine.md", &doc(&["def engine.a in=light"])));
        let e = of(&ds, c::HN_E004);
        assert_eq!(e.len(), 2);
        assert!(e.iter().all(|d| d.message.contains("theme variant")));
    }

    #[test]
    fn r_env_005__missing_maturity_uses_scope_default_else_mat_missing() {
        let w = || {
            World::new().root(VOCAB).file(
                "fairlead/docs/engine.md",
                &doc(&["def engine.a in=dev,prod"]),
            )
        };
        assert_eq!(of(&run(w()), c::HN_E005).len(), 1);
        let with_default = run(w().root("[defaults]\nmat = \"beta\"\n"));
        assert!(of(&with_default, c::HN_E005).is_empty());
    }

    #[test]
    fn r_env_011__later_env_more_mature_is_mat_order() {
        let ds = run(World::new().root(VOCAB).file(
            "fairlead/docs/engine.md",
            &doc(&["def engine.a in=dev,prod mat=dev:alpha,prod:stable"]),
        ));
        let e = of(&ds, c::HN_E006);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("more mature in prod than in dev"));
    }

    #[test]
    fn r_env_012__regression_against_the_committed_shard_warns() {
        let w = World::new()
            .root(VOCAB)
            .file(
                "fairlead/docs/engine.md",
                &doc(&["def engine.a in=dev mat=beta"]),
            )
            .facts(|f| {
                f.generated_on_disk.insert(
                    RepoPath::new("fairlead/gen/reg/engine.reg").unwrap(),
                    b"engine.a pol=open in=dev mat=stable\n".to_vec().into(),
                );
            });
        let ds = run(w);
        let e = of(&ds, c::HN_E007);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("from stable to beta"));
    }

    #[test]
    fn r_env_040__a_child_default_only_narrows() {
        let ds = run(
            World::new()
                .root(VOCAB)
                .root("[defaults]\nin = [\"dev\"]\nmat = \"beta\"\n")
                .scope("web", "[fairlead]\nscope = \"web\"\n[defaults]\nin = [\"dev\", \"prod\"]\nmat = \"alpha\"\n")
                .file("web/fairlead/docs/engine.md", &doc(&["def engine.w bind=feature:x"])),
        );
        assert!(of(&ds, c::HN_E005).is_empty(), "{ds:?}");
        let b = World::new()
            .root(VOCAB)
            .root("[defaults]\nin = [\"dev\"]\nmat = \"beta\"\n")
            .scope("web", "[fairlead]\nscope = \"web\"\n[defaults]\nin = [\"dev\", \"prod\"]\nmat = \"alpha\"\n")
            .file("web/fairlead/docs/engine.md", &doc(&["def engine.w bind=feature:x"]))
            .build();
        let v = Vocab::new(&root_control(&b.registry).unwrap().vocab);
        let r = resolve_axes(&b.registry, &b.corpus, &v);
        let a = r.axes.values().next().unwrap();
        assert_eq!(
            a.envs,
            BTreeSet::from([0]),
            "prod is not added by the child"
        );
        assert_eq!(
            a.mat.get(&0),
            Some(&1),
            "alpha does not lower the beta floor"
        );
    }

    const USE_SRC: &str =
        "// fairlead: def engine.app\n\nfn main() {\n    // fairlead: use engine.lib\n}\n";

    fn gap_world(app: &str, lib: &str) -> World {
        World::new()
            .root(VOCAB)
            .file("fairlead/docs/engine.md", &doc(&[app, lib]))
            .file("src/main.rs", USE_SRC)
    }

    #[test]
    fn r_env_020__gap_two_is_error_gap_one_warn() {
        let two = run(gap_world(
            "def engine.app mat=stable",
            "def engine.lib mat=alpha",
        ));
        let g: Vec<_> = of(&two, c::HN_E006);
        assert_eq!((g.len(), g[0].severity), (1, Severity::Error));
        assert_eq!(g[0].rule.as_deref(), Some("rule.maturity-gap"));
        let one = run(gap_world(
            "def engine.app mat=beta",
            "def engine.lib mat=alpha",
        ));
        assert_eq!(of(&one, c::HN_E006)[0].severity, Severity::Warn);
        let up = run(gap_world(
            "def engine.app mat=alpha",
            "def engine.lib mat=stable",
        ));
        assert!(
            of(&up, c::HN_E006).is_empty(),
            "alpha may depend on anything"
        );
    }

    #[test]
    fn r_env_020__root_sev_replaces_gap_default() {
        let b = gap_world("def engine.app mat=beta", "def engine.lib mat=alpha")
            .root("[rules.maturity-gap]\ncheck = \"builtin\"\nsev = \"error\"\nwaivable = true\n")
            .build();
        assert!(
            b.registry.diagnostics().is_empty(),
            "{:?}",
            b.registry.diagnostics()
        );
        let rules = crate::check::RuleSet::collect(&b.registry);
        let ds = b.run_check(&EnvCheck);
        let f = crate::check::apply::lift(ds, &b.registry, &rules);
        let gap: Vec<_> = f.iter().filter(|f| f.code == c::HN_E006).collect();
        assert_eq!(gap.len(), 1);
        assert_eq!(gap[0].severity, Severity::Error, "a one-rank gap, root sev");
    }

    #[test]
    fn r_env_022__leak_over_declared_edge() {
        let ds = run(gap_world("def engine.app in=all", "def engine.lib in=dev"));
        let e = of(&ds, c::HN_E001);
        let envs: Vec<bool> = e
            .iter()
            .map(|d| d.message.contains("reaches staging") || d.message.contains("reaches prod"))
            .collect();
        assert_eq!(envs, [true, true]);
    }

    #[test]
    fn r_env_023__dev_only_unreachable_in_prod_is_by_design() {
        let ds = run(gap_world("def engine.app in=dev", "def engine.lib in=dev"));
        assert!(of(&ds, c::HN_E001).is_empty());
        let gated = World::new()
            .root(VOCAB)
            .file("fairlead/docs/engine.md", &doc(&["def engine.app in=all", "def engine.lib in=dev"]))
            .file(
                "src/main.rs",
                "// fairlead: def engine.app\n\nfn main() {\n    // fairlead: env dev +end\n    // fairlead: use engine.lib\n    // fairlead: end\n}\n",
            );
        assert!(
            of(&run(gated), c::HN_E001).is_empty(),
            "the use sits in a dev-gated region"
        );
    }
}
