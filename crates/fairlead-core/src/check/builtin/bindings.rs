//! Env bindings (R-ENV-030 to R-ENV-036): each `bind=` names a gate that exists (a Cargo feature
//! outside `default`, a token of the root `[bind]` table); a dev-only crate stays out of a prod
//! crate's `[dependencies]` (HN-E001); TS env tokens match the table (HN-E003); a file using a
//! binding declares its env (HN-E002). Everything is "declared + bound + syntactically present":
//! no `cfg` is evaluated and no feature unification is resolved (D-16).
use std::collections::{BTreeMap, BTreeSet};

use fairlead_syntax::diag::DiagCode;
use fairlead_syntax::directive::{Extent, Verb};

use crate::check::builtin::envs::resolve_axes;
use crate::check::builtin::{bare_id, root_control};
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::extract::{Corpus, EnvToken};
use crate::lifecycle::{EnvWord, Vocab, parse_in};
use crate::path::RepoPath;
use crate::registry::ScopeId;

pub struct BindingsCheck;
const CODES: [DiagCode; 3] = [HN_E001, HN_E002, HN_E003];

/// One spelling for a gate token: no whitespace, `feature="x"` as `feature:x`.
pub(crate) fn norm(tok: &str) -> String {
    let t: String = tok.chars().filter(|c| !c.is_whitespace()).collect();
    match t.strip_prefix("feature=") {
        Some(f) => format!("feature:{}", f.trim_matches('"')),
        None => t,
    }
}

/// Every token of the root `[bind]` table with the env slots it binds.
fn bind_table(v: &Vocab<'_>) -> BTreeMap<String, BTreeSet<usize>> {
    let mut out: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (env, b) in &v.cfg.bind {
        let EnvWord::Slot(slot) = v.env(env) else {
            continue;
        };
        for t in b.rust.iter().chain(&b.ts) {
            out.entry(norm(t)).or_default().insert(slot);
        }
    }
    out
}

fn is_test_path(p: &RepoPath) -> bool {
    let s = p.as_str();
    let name = p.file_name();
    s.starts_with("tests/")
        || s.contains("/tests/")
        || s.contains("__tests__/")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with("_test.rs")
}

/// The `test` env is declared (as a word or an alias) (R-ENV-036).
fn test_env(v: &Vocab<'_>) -> bool {
    matches!(v.env("test"), EnvWord::Slot(_))
}

fn manifests(corpus: &Corpus) -> BTreeMap<RepoPath, (RepoPath, &[EnvToken])> {
    corpus
        .records
        .iter()
        .filter(|(p, _)| p.file_name() == "Cargo.toml")
        .map(|(p, r)| {
            let dir = p.parent().unwrap_or_else(RepoPath::root);
            (dir, (p.clone(), &r.env_tokens[..]))
        })
        .collect()
}

fn nearest<'m>(
    m: &'m BTreeMap<RepoPath, (RepoPath, &'m [EnvToken])>,
    path: &RepoPath,
) -> Option<&'m (RepoPath, &'m [EnvToken])> {
    let mut dir = path.parent();
    while let Some(d) = dir {
        if let Some(x) = m.get(&d) {
            return Some(x);
        }
        dir = if d == RepoPath::root() {
            None
        } else {
            d.parent()
        };
    }
    m.get(&RepoPath::root())
}

fn has(tokens: &[EnvToken], t: &str) -> bool {
    tokens.iter().any(|x| x.token == t)
}

/// A crate is dev-only when its root source file carries a file-wide `env` gate that leaves out the
/// most exposed env (R-ENV-032).
fn dev_only_crate(cx: &CheckCx<'_>, v: &Vocab<'_>, dir: &RepoPath) -> bool {
    let Some(last) = v.cfg.env_order.len().checked_sub(1) else {
        return false;
    };
    ["src/lib.rs", "src/main.rs"].iter().any(|f| {
        let p = if *dir == RepoPath::root() {
            RepoPath::new(f).ok()
        } else {
            RepoPath::new(&format!("{}/{f}", dir.as_str())).ok()
        };
        p.and_then(|p| cx.corpus.records.get(&p))
            .and_then(|r| r.extraction.as_ref())
            .is_some_and(|x| {
                x.directives.iter().any(|d| {
                    d.verb == Verb::Env
                        && d.extent == Extent::File
                        && !parse_in(v, &d.bare, &mut Vec::new()).contains(&last)
                })
            })
    })
}

impl Check for BindingsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(root) = root_control(reg) else {
            return;
        };
        let v = Vocab::new(&root.vocab);
        if !v.has_envs() {
            return;
        }
        let table = bind_table(&v);
        let mans = manifests(cx.corpus);
        let res = resolve_axes(reg, cx.corpus, &v);
        for (id, binds) in res.binds.iter().filter(|(id, _)| id.scope == *scope) {
            let Some((p, l, c)) = res.loc.get(id) else {
                continue;
            };
            for b in binds {
                let b = norm(b);
                let at = |code, msg: String, hint: &str| {
                    DiagB::new(code, p, *l, *c, msg)
                        .hint(hint)
                        .related(bare_id(id))
                        .text(b.clone())
                        .build()
                };
                if let Some(flag) = b.strip_prefix("flag:") {
                    out.push(at(
                        HN_E003,
                        format!("{} is bound to the runtime flag {flag}: declared, its state is not known", bare_id(id)),
                        "fairlead never claims a runtime flag's state",
                    ));
                } else if let Some(feat) = b.strip_prefix("feature:") {
                    match nearest(&mans, p) {
                        None => out.push(at(
                            HN_E001,
                            format!("{} is bound to feature {feat} but no Cargo.toml is above it", bare_id(id)),
                            "bind to a feature of the crate that holds the item",
                        )),
                        Some((m, toks)) if !has(toks, &b) => out.push(at(
                            HN_E001,
                            format!("feature {feat} is not in [features] of {}", m.as_str()),
                            "add the feature to the manifest, or fix the binding",
                        )),
                        Some((m, toks)) if has(toks, &format!("default:{feat}")) => out.push(at(
                            HN_E001,
                            format!("feature {feat} is in the default features of {}, so every build has it", m.as_str()),
                            "remove it from `default`; a gate that is always on gates nothing",
                        )),
                        Some(_) => {}
                    }
                } else if !table.contains_key(&b) {
                    out.push(at(
                        HN_E003,
                        format!("binding {b} of {} is in no root [bind] table", bare_id(id)),
                        "extend the [bind.<env>] table, not the code",
                    ));
                }
            }
        }
        for (path, rec) in &cx.corpus.records {
            if reg.scope_of(path) != Some(scope) || rec.env_tokens.is_empty() {
                continue;
            }
            if path.file_name() == "Cargo.toml" {
                let dir = path.parent().unwrap_or_else(RepoPath::root);
                if dev_only_crate(cx, &v, &dir) {
                    continue;
                }
                for t in &rec.env_tokens {
                    let Some(dep) = t.token.strip_prefix("dep:") else {
                        continue;
                    };
                    let dev = mans.iter().find(|(d, (_, toks))| {
                        has(toks, &format!("package:{dep}")) && dev_only_crate(cx, &v, d)
                    });
                    if dev.is_some() {
                        out.push(
                            DiagB::new(HN_E001, path, t.line, 1, format!("the dev-only crate {dep} is a [dependencies] entry of a prod crate"))
                                .hint("move it to [dev-dependencies] or behind an optional feature")
                                .text(dep.to_string())
                                .build(),
                        );
                    }
                }
                continue;
            }
            for t in rec.env_tokens.iter().filter(|t| {
                (t.token.starts_with("import.meta.env.") || t.token == "process.env.NODE_ENV")
                    && !table.contains_key(&t.token)
            }) {
                out.push(
                    DiagB::new(
                        HN_E003,
                        path,
                        t.line,
                        1,
                        format!("{} is in no root [bind] table", t.token),
                    )
                    .hint("extend the [bind.<env>] ts table, not the code")
                    .text(t.token.clone())
                    .build(),
                );
            }
            let dirs = rec
                .extraction
                .as_ref()
                .map_or(&[][..], |x| &x.directives[..]);
            let declared = dirs
                .iter()
                .any(|d| d.verb == Verb::Env || (d.verb == Verb::Def && d.arg("in").is_some()));
            let test_code = test_env(&v)
                && (is_test_path(path) || rec.env_tokens.iter().any(|t| t.token == "cfg(test)"));
            if declared || test_code {
                continue;
            }
            if let Some(t) = rec.env_tokens.iter().find(|t| table.contains_key(&t.token)) {
                out.push(
                    DiagB::new(
                        HN_E002,
                        path,
                        t.line,
                        1,
                        format!("this file uses the binding {} but declares no env", t.token),
                    )
                    .hint("add `env <envs>` to the file or region, or in= on its def")
                    .text(t.token.clone())
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

    const ROOT: &str = "[env]\norder = [\"dev\", \"prod\"]\n[bind.dev]\nrust = ['feature=\"devtools\"', \"cfg(debug_assertions)\"]\nts = [\"import.meta.env.DEV\"]\n[bind.prod]\nts = [\"import.meta.env.PROD\"]\n[kinds.cmp]\nimpl = 1\n";
    const CARGO: &str =
        "[package]\nname = \"app\"\n\n[features]\ndefault = [\"std\"]\nstd = []\ndevtools = []\n";

    fn doc(def: &str) -> String {
        format!("+++\ndirectives = ['{def}']\n+++\n# Components\n")
    }
    fn run(w: World) -> Vec<Diagnostic> {
        w.build().run_check(&BindingsCheck)
    }

    #[test]
    fn r_env_032__bound_feature_must_exist_outside_default() {
        let w = |def: &str| {
            World::new()
                .root(ROOT)
                .file("Cargo.toml", CARGO)
                .file("fairlead/docs/cmp.md", &doc(def))
        };
        assert!(
            of(
                &run(w("def cmp.panel in=dev bind=feature:devtools")),
                c::HN_E001
            )
            .is_empty()
        );
        let missing = run(w("def cmp.panel in=dev bind=feature:nope"));
        assert!(
            of(&missing, c::HN_E001)[0]
                .message
                .contains("not in [features]")
        );
        let default = run(w("def cmp.panel in=dev bind=feature:std"));
        assert!(
            of(&default, c::HN_E001)[0]
                .message
                .contains("default features")
        );
    }

    #[test]
    fn r_env_032__dev_only_crate_in_prod_dependencies_is_env_gate() {
        let w = World::new()
            .root(ROOT)
            .file(
                "Cargo.toml",
                "[package]\nname = \"app\"\n\n[dependencies]\nprobe = { path = \"probe\" }\n",
            )
            .file("src/main.rs", "fn main() {}\n")
            .file("probe/Cargo.toml", "[package]\nname = \"probe\"\n")
            .file(
                "probe/src/lib.rs",
                "// fairlead: env dev\n\npub fn p() {}\n",
            );
        let ds = run(w);
        let e = of(&ds, c::HN_E001);
        assert_eq!(e.len(), 1, "{ds:?}");
        assert_eq!(
            (e[0].location.path.as_str(), e[0].location.line),
            ("Cargo.toml", 5)
        );
    }

    #[test]
    fn r_env_033__ts_token_outside_the_table_is_unbound_info() {
        let ds = run(World::new().root(ROOT).file(
            "src/a.ts",
            "// fairlead: env dev,prod\n\nif (import.meta.env.DEV || import.meta.env.VITE_BETA) {}\n",
        ));
        let e = of(&ds, c::HN_E003);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].text, "import.meta.env.VITE_BETA");
        assert_eq!(e[0].severity, fairlead_syntax::diag::Severity::Info);
    }

    #[test]
    fn r_env_034__flag_binding_is_info_only() {
        let ds = run(World::new().root(ROOT).file(
            "fairlead/docs/cmp.md",
            &doc("def cmp.panel in=dev bind=flag:new-panel"),
        ));
        assert_eq!(of(&ds, c::HN_E003).len(), 1);
        assert!(of(&ds, c::HN_E001).is_empty());
    }

    #[test]
    fn r_env_031__binding_without_env_declaration_is_undeclared() {
        let src = "#[cfg(debug_assertions)]\nfn probe() {}\n";
        let ds = run(World::new().root(ROOT).file("src/a.rs", src));
        assert_eq!(of(&ds, c::HN_E002).len(), 1);
        let gated = run(World::new()
            .root(ROOT)
            .file("src/a.rs", &format!("// fairlead: env dev\n\n{src}")));
        assert!(of(&gated, c::HN_E002).is_empty());
    }

    #[test]
    fn r_env_036__test_code_defaults_to_the_test_env_when_declared() {
        let root = ROOT.replace("[\"dev\", \"prod\"]", "[\"test\", \"dev\", \"prod\"]")
            + "[bind.test]\nrust = [\"cfg(test)\"]\n";
        let src = "fn f() {}\n#[cfg(test)]\nmod tests {}\n";
        assert!(
            of(
                &run(World::new().root(&root).file("src/a.rs", src)),
                c::HN_E002
            )
            .is_empty()
        );
        let undeclared_root = ROOT.replace(
            "rust = ['feature=\"devtools\"', \"cfg(debug_assertions)\"]",
            "rust = ['feature=\"devtools\"', \"cfg(debug_assertions)\", \"cfg(test)\"]",
        );
        let ds = run(World::new().root(&undeclared_root).file("src/a.rs", src));
        assert_eq!(
            of(&ds, c::HN_E002).len(),
            1,
            "no test env: the test binding is undeclared"
        );
    }
}
