#![allow(non_snake_case)] // reason: test names start with the rule id
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod support;

use fairlead_core::config::OvrDecl;
use fairlead_core::registry::ovr::OvrOp;
use fairlead_core::registry::scope::{demote_model, promote_model};
use fairlead_core::registry::view::view_signature;
use fairlead_syntax::directive::ArgValue;
use fairlead_syntax::ids::ScopeSlug;
use proptest::prelude::*;
use support::*;

const TARGETS: [&str; 2] = ["cmp.button", "cmp.card"];
const FIELDS: [&str; 2] = ["size", "layer"];
const VALUES: [&str; 3] = ["a", "b", "c"];
const GLOBS: [&str; 2] = ["web/**", "web/src/**"];
const PATHS: [&str; 3] = ["web/src/a.rs", "web/lib/b.rs", "docs/c.rs"];

fn op_str(o: OvrOp) -> &'static str {
    match o {
        OvrOp::Loosen => "loosen",
        OvrOp::Tighten => "tighten",
        OvrOp::Set => "set",
        OvrOp::Replace => "replace",
        OvrOp::Add => "add",
        OvrOp::Remove => "remove",
    }
}

fn render(d: &OvrDecl) -> String {
    let mut s = format!(
        "[[ovr]]\ntarget = \"{}\"\nop = \"{}\"\n",
        d.target,
        op_str(d.op)
    );
    if let Some(a) = &d.applies {
        s.push_str(&format!("applies = \"{a}\"\n"));
    }
    for (k, v) in &d.set {
        let v = match v {
            ArgValue::Word(w) | ArgValue::Quoted(w) => format!("\"{w}\""),
            ArgValue::List(l) => format!(
                "[{}]",
                l.iter()
                    .map(|x| format!("\"{x}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        s.push_str(&format!("{k} = {v}\n"));
    }
    s.push('\n');
    s
}

fn docs() -> String {
    doc(&[
        "def cmp.button size=md layer=atom",
        "def cmp.card size=md layer=atom",
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn prop_promote_demote_roundtrip(specs in proptest::collection::vec((0..2usize, 0..2usize, 0..3usize, 0..2usize), 1..5)) {
        let tables: String = specs.iter().map(|(t, f, v, g)| {
            ovr_table(TARGETS[*t], &format!("applies = \"{}\"\n{} = \"{}\"", GLOBS[*g], FIELDS[*f], VALUES[*v]))
        }).collect();
        let a = Kit::new(&root_toml(&format!("\n{tables}"))).file("fairlead/docs/c.md", &docs()).resolve();
        let original: Vec<OvrDecl> = a.scope(&sid("root")).unwrap().control.as_ref().unwrap().ovrs.clone();

        let plan = promote_model(&a, &sid("root"), &path("web"), &ScopeSlug::parse("web").unwrap()).unwrap();
        prop_assert!(plan.kept.is_empty());
        let kept: String = plan.kept.iter().map(render).collect();
        let moved: String = plan.moved.iter().map(render).collect();
        let b = Kit::new(&root_toml(&format!("\n{kept}")))
            .file("fairlead/docs/c.md", &docs())
            .scope("web", "web", &child_toml("web", &moved))
            .resolve();
        for p in PATHS {
            prop_assert_eq!(view_signature(&a.view_at(&path(p))), view_signature(&b.view_at(&path(p))));
        }

        let back = demote_model(&b, &sid("web"), b.as_of()).unwrap();
        // Verify: an `[[ovr]]` without `op` parses to OvrOp::Set; values are compared as text.
        let key = |d: &OvrDecl| {
            let set: Vec<(String, String)> = d.set.iter().map(|(k, v)| (k.clone(), match v {
                ArgValue::Word(s) | ArgValue::Quoted(s) => s.clone(),
                ArgValue::List(l) => l.join(","),
            })).collect();
            format!("{}|{:?}|{:?}", d.target, d.applies, set)
        };
        let mut got: Vec<String> = back.moved.iter().map(key).collect();
        let mut want: Vec<String> = original.iter().map(key).collect();
        got.sort();
        want.sort();
        prop_assert_eq!(got, want);
    }
}
