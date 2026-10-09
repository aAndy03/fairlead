#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
//! Property tests: the baseline only shrinks, pruning is a fixed point, the file round-trips.

use std::collections::BTreeSet;

use fairlead_core::check::{Baseline, BaselineEntry, Finding, FindingState};
use fairlead_core::config::Stage;
use fairlead_core::diag::codes::HN_T001;
use fairlead_core::diag::{Location, Severity};
use fairlead_core::discover::Edition;
use fairlead_core::hash::Fingerprint;
use fairlead_core::path::RepoPath;
use fairlead_core::registry::ScopeId;
use proptest::prelude::*;

const FP: &str = "[0-9a-hjkmnp-tv-z]{12}";

fn baseline(fps: &BTreeSet<String>) -> Baseline {
    let mut b = Baseline::default();
    for f in fps {
        b.entries.insert(
            Fingerprint::parse(f).unwrap(),
            BaselineEntry {
                code: HN_T001,
                path: RepoPath::new("a.rs").unwrap(),
                accepted: None,
            },
        );
    }
    b
}

fn finding(fp: &str) -> Finding {
    Finding {
        code: HN_T001,
        severity: Severity::Error,
        stage: Stage::Enforce,
        location: Location {
            path: RepoPath::new("a.rs").unwrap(),
            line: 1,
            col: 1,
        },
        scope: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("web").unwrap()),
        rule: None,
        related: None,
        message: "m".to_string(),
        intent: None,
        fix: "f".to_string(),
        occurrence: 0,
        fingerprint: Fingerprint::parse(fp).unwrap(),
        state: FindingState::New,
        waived_by: None,
    }
}

proptest! {
    #[test]
    fn r_chk_032__prune_is_a_shrinking_fixed_point(
        base in prop::collection::btree_set(FP, 0..24),
        live in prop::collection::btree_set(FP, 0..24),
    ) {
        let b = baseline(&base);
        let current: Vec<Finding> = live.iter().map(|f| finding(f)).collect();
        let once = b.prune(&current);
        prop_assert_eq!(once.prune(&current), once.clone());
        prop_assert!(once.entries.keys().all(|k| b.entries.contains_key(k)), "prune added an entry");
        prop_assert!(once.entries.keys().all(|k| live.contains(k.as_str())), "prune kept a fixed entry");
        prop_assert!(once.len() <= b.len());
    }

    #[test]
    fn r_chk_030__render_then_parse_is_the_identity(base in prop::collection::btree_set(FP, 0..24)) {
        let b = baseline(&base);
        let text = b.render(Edition(1));
        prop_assert_eq!(Baseline::parse(text.as_bytes()), Ok(b));
    }
}
