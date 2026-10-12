#![allow(non_snake_case)] // reason: test names start with the rule id
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod support;

use fairlead_core::config::EligibilityConfig;
use fairlead_core::manifest::ManifestHints;
use fairlead_core::registry::ovr::hoist_candidates;
use fairlead_core::registry::scope::{
    DemoteRefusal, EligibilityReason, PromoteRefusal, demote_model, promote_model,
    scope_eligibility,
};
use fairlead_syntax::ids::ScopeSlug;
use support::*;

fn use_lines(n: usize) -> String {
    format!(
        "{}export const a = 1;\n",
        "// fairlead: use cmp.button\n".repeat(n)
    )
}

#[test]
fn r_scope_011__busy_folder_and_manifest_member_are_eligible_hints_only() {
    let mut kit = Kit::new(&root_toml("")).file("fairlead/docs/c.md", &doc(&["def cmp.button"]));
    for f in ["a", "b", "c"] {
        kit = kit.file(&format!("src/busy/{f}.ts"), &use_lines(5));
    }
    kit = kit
        .file("src/thin/a.ts", &use_lines(5))
        .file("src/thin/b.ts", &use_lines(5));
    let reg = kit.resolve();
    let mut hints = ManifestHints {
        workspace_members: Default::default(),
        default_excludes: vec![],
        crate_deps: Default::default(),
        packages: Default::default(),
    };
    hints.workspace_members.insert(path("crates/core"));
    let cfg = EligibilityConfig {
        min_lines: 5,
        min_files: 3,
    };
    let got: Vec<_> = scope_eligibility(&reg, &cfg, &hints)
        .into_iter()
        .map(|h| (h.dir.as_str().to_string(), h.reason))
        .collect();
    assert_eq!(
        got,
        vec![
            ("crates/core".to_string(), EligibilityReason::ManifestMember),
            (
                "src/busy".to_string(),
                EligibilityReason::FileLocalLines { files: 3 }
            ),
        ]
    );
}

#[test]
fn r_scope_013__identical_file_local_ovrs_in_several_files_are_hoist_candidates() {
    let line = "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n";
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/src/a.ts", line)
        .file("apps/web/src/b.ts", line)
        .file(
            "apps/web/src/c.ts",
            "// fairlead: ovr cmp.button size=lg\nexport const c = 1;\n",
        )
        .resolve();
    let g = hoist_candidates(&reg, &path("apps/web/src"), 2);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].applies, "src/**");
    assert_eq!(g[0].sites.len(), 2);
    assert!(g[0].body.contains("size=sm"));
}

fn promote_kit() -> Kit {
    let tables = format!(
        "{}{}",
        ovr_table("cmp.button", "applies = \"web/**\"\nsize = \"lg\""),
        ovr_table("cmp.card", "applies = \"docs/**\"\nsize = \"sm\"")
    );
    Kit::new(&root_toml(&format!("\n{tables}"))).file(
        "fairlead/docs/c.md",
        &doc(&["def cmp.button size=md", "def cmp.card size=md"]),
    )
}

#[test]
fn r_scope_012__promote_moves_matching_ovrs_demote_restores_them() {
    let reg = promote_kit().resolve();
    let slug = ScopeSlug::parse("web").unwrap();
    let plan = promote_model(&reg, &sid("root"), &path("web"), &slug).unwrap();
    assert_eq!(plan.moved.len(), 1);
    assert_eq!(plan.moved[0].applies, None); // `web/**` became the whole new scope
    assert_eq!(plan.kept.len(), 1);
    assert_eq!(plan.rebinds.len(), 1);
    assert!(matches!(
        promote_model(
            &reg,
            &sid("root"),
            &path("docs"),
            &ScopeSlug::parse("root").unwrap()
        ),
        Err(PromoteRefusal::IdInUse)
    ));
    // after the move the scope exists; demoting it re-prefixes the globs
    let after = promote_kit()
        .scope(
            "web",
            "web",
            &child_toml("web", &ovr_table("cmp.button", "size = \"lg\"")),
        )
        .resolve();
    let d = demote_model(&after, &sid("web"), after.as_of()).unwrap();
    assert_eq!(d.into, sid("root"));
    assert_eq!(d.moved[0].applies.as_deref(), Some("web/**"));
    assert_eq!(
        (d.tombstone.id.as_str(), d.tombstone.former_path.as_str()),
        ("web", "web")
    );
    assert_eq!(d.tombstone.written_in, sid("root"));
}

#[test]
fn r_scope_014__demotion_refuses_a_duplicate_id_and_never_the_root() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .resolve();
    assert_eq!(
        demote_model(&reg, &sid("web"), reg.as_of()).unwrap_err(),
        DemoteRefusal::DuplicateId(iid("web", "cmp.card"))
    );
    assert_eq!(
        demote_model(&reg, &sid("root"), reg.as_of()).unwrap_err(),
        DemoteRefusal::Root
    );
    assert!(matches!(
        promote_model(
            &reg,
            &sid("root"),
            &path("apps/web"),
            &ScopeSlug::parse("x").unwrap()
        ),
        Err(PromoteRefusal::ScopeExists)
    ));
}
