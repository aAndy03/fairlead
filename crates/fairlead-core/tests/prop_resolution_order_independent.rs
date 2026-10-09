#![allow(non_snake_case)] // reason: test names start with the rule id
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod support;

use fairlead_core::registry::{ApprovalStatus, OvrState};
use proptest::prelude::*;
use support::*;

const FIELDS: [(&str, [&str; 3]); 2] = [
    ("sev", ["info", "warn", "error"]),
    ("stage", ["observe", "warn", "enforce"]),
];

fn permute<T: Clone>(v: &[T], seed: u64) -> Vec<T> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by_key(|i| {
        seed.wrapping_mul(0x9E37_79B9)
            .wrapping_add((*i as u64).wrapping_mul(40_503))
            % 1_000_003
    });
    idx.into_iter().filter_map(|i| v.get(i).cloned()).collect()
}

fn build(ovrs: &[String], allows: &[String], search: bool) -> fairlead_core::registry::Registry {
    let rules = format!(
        "{}{}{}{}",
        rule("r1", "warn", "observe", false),
        rule("r2", "error", "enforce", false),
        rule("r3", "warn", "warn", true),
        allows.concat()
    );
    let kit = Kit::new(&root_toml(&format!("\n{rules}")))
        .scope("apps/web", "web", &child_toml("web", &ovrs.concat()))
        .scope("apps/admin", "admin", &child_toml("admin", ""));
    if search {
        kit.search_source().resolve()
    } else {
        kit.resolve()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn prop_resolution_order_independent(
        specs in proptest::collection::vec((0..3usize, 0..2usize, 0..3usize, any::<bool>()), 1..6),
        grants in proptest::collection::vec((0..3usize, 0..2usize), 0..4),
        seed in any::<u64>(),
    ) {
        let ovrs: Vec<String> = specs.iter().map(|(r, f, v, complete)| {
            let (field, values) = FIELDS[*f];
            let meta = if *complete { "\nwhy = \"w\"\nowner = \"@andy\"\nexp = \"2026-12-31\"" } else { "" };
            ovr_table(&format!("rule.r{}", r + 1), &format!("{field} = \"{}\"{meta}", values[*v]))
        }).collect();
        let allows: Vec<String> = grants.iter().map(|(r, f)| format!(
            "[[allow]]\ntarget = \"rules.r{}\"\nfield = \"{}\"\nfor = [\"web\"]\nwhy = \"g\"\nexp = \"2027-03-31\"\n\n", r + 1, FIELDS[*f].0
        )).collect();
        let a = build(&ovrs, &allows, false);
        let b = build(&permute(&ovrs, seed), &permute(&allows, seed ^ 7), true); // also another root selection
        for s in ["root", "web", "admin"] {
            prop_assert_eq!(&a.view(&sid(s)).items, &b.view(&sid(s)).items);
            prop_assert_eq!(a.ledger(&sid(s)).entries, b.ledger(&sid(s)).entries);
        }
        let states = |r: &fairlead_core::registry::Registry| -> Vec<(String, OvrState, ApprovalStatus)> {
            let mut v: Vec<_> = r.overrides().iter().map(|o| (format!("{:?}", o.id), o.state, o.approval)).collect();
            v.sort_by(|x, y| (&x.0, format!("{:?}", (x.1, x.2))).cmp(&(&y.0, format!("{:?}", (y.1, y.2)))));
            v
        };
        prop_assert_eq!(states(&a), states(&b));
        prop_assert_eq!(a.diagnostics(), b.diagnostics());
    }
}
