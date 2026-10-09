#![allow(non_snake_case)] // reason: test names start with the rule id
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod support;

use fairlead_core::config::Policy;
use fairlead_core::registry::{ApprovalStatus, FieldValue, OvrState, ovr_fingerprint};
use support::*;

#[test]
fn r_scope_042__worked_example_of_chapter_10_reads_both_ways() {
    let line = "// fairlead: ovr rule.no-inline-style op=loosen sev=info why=\"chart lib positions via style attr\" owner=@andy exp=2026-12-31\nexport const L = 1;\n";
    let target = iid("root", "rule.no-inline-style");
    let fp = ovr_fingerprint(
        &target,
        &sid("apps-console"),
        "chart lib positions via style attr",
        "@andy",
        Some(&date("2026-12-31")),
    );
    let build = |k: Kit| {
        k.scope(
            "apps/console",
            "apps-console",
            &child_toml("apps-console", ""),
        )
        .scope("apps/admin", "apps-admin", &child_toml("apps-admin", ""))
        .file("apps/console/src/charts/line.tsx", line)
        .approve(fp.clone(), &target, 0)
        .resolve()
    };
    let reg = build(Kit::new(&root_toml(&format!(
        "\n{}",
        rule("no-inline-style", "error", "enforce", true)
    ))));
    assert!(reg.diagnostics().is_empty(), "{:?}", reg.diagnostics());

    let inside = reg.view_at(&path("apps/console/src/charts/line.tsx"));
    let f = &inside.items[&target].fields["sev"];
    assert_eq!(f.value, FieldValue::Scalar("info".into()));
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(inside.items[&target].policy, Policy::NeedsException);
    assert_eq!(f.provenance.applied, vec![reg.overrides()[0].id.clone()]);

    let outside = reg.view_at(&path("apps/admin/src/a.tsx"));
    assert_eq!(
        outside.items[&target].fields["sev"].value,
        FieldValue::Scalar("error".into())
    );

    let l = reg.ledger(&sid("root")).entries;
    assert_eq!(l.len(), 1);
    assert_eq!(
        (l[0].varied_in.clone(), l[0].depth, l[0].state),
        (sid("apps-console"), 1, OvrState::Effective)
    );
    assert_eq!(
        (l[0].owner.as_deref(), l[0].exp),
        (Some("@andy"), Some(date("2026-12-31")))
    );
    assert_eq!(
        reg.overrides()[0].approval,
        ApprovalStatus::Approved { renewal: 0 }
    );
}

fn world(search: bool) -> fairlead_core::registry::Registry {
    let k = Kit::new(&root_toml(&format!(
        "
{}",
        rule("r1", "warn", "observe", false)
    )))
    .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
    .scope(
        "apps/web",
        "web",
        &child_toml("web", &ovr_table("rule.r1", "sev = \"error\"")),
    )
    .file(
        "apps/web/src/a.ts",
        "// fairlead: ovr cmp.button size=sm
export const a = 1;
",
    );
    if search {
        k.search_source().resolve()
    } else {
        k.resolve()
    }
}

#[test]
fn r_scope_020__resolution_is_a_pure_function_of_the_authored_files() {
    let (a, b) = (world(false), world(true));
    for s in ["root", "web"] {
        assert_eq!(a.view(&sid(s)).items, b.view(&sid(s)).items);
    }
    assert_eq!(a.diagnostics(), b.diagnostics());
}

#[test]
fn r_arch_043__invocation_directory_only_selects_a_cone_never_the_result() {
    let (a, b) = (world(false), world(true));
    let p = path("apps/web/src/a.ts");
    assert_eq!(a.view_at(&p).items, b.view_at(&p).items);
    assert_eq!(
        a.ledger(&sid("root")).entries,
        b.ledger(&sid("root")).entries
    );
}

#[test]
fn r_arch_042__provenance_names_home_declaration_site_and_applied_overrides() {
    let reg = world(false);
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(
        f.provenance.declared_at.path.as_str(),
        "fairlead/control.toml"
    );
    assert_eq!(f.provenance.applied.len(), 1);
    assert!(
        reg.view(&sid("root")).items[&iid("root", "rule.r1")].fields["sev"]
            .provenance
            .applied
            .is_empty()
    );
}

#[test]
fn r_arch_040__base_chained_overrides_fold_base_first_in_any_textual_order() {
    let target = iid("root", "cmp.button");
    let first = ovr_fingerprint(&target, &sid("web"), "a", "", None);
    let one = ovr_table(
        "cmp.button",
        "size = \"sm\"
why = \"a\"",
    );
    let two = ovr_table(
        "cmp.button",
        &format!(
            "size = \"lg\"
why = \"b\"
base = \"{first}\""
        ),
    );
    let size = |tables: String| {
        let reg = Kit::new(&root_toml(""))
            .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
            .scope("apps/web", "web", &child_toml("web", &tables))
            .resolve();
        reg.view(&sid("web")).items[&target].fields["size"]
            .value
            .clone()
    };
    assert_eq!(size(format!("{one}{two}")), FieldValue::Scalar("lg".into()));
    assert_eq!(size(format!("{two}{one}")), FieldValue::Scalar("lg".into()));
}
