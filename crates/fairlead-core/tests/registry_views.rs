#![allow(non_snake_case)] // reason: test names start with the rule id
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
mod support;

use fairlead_core::registry::{FieldValue, OvrState, ovr_fingerprint};
use support::*;

fn scal(s: &str) -> FieldValue {
    FieldValue::Scalar(s.into())
}

fn kit(root_extra: &str, web: &str) -> Kit {
    Kit::new(&root_toml(&format!(
        "\n{}{root_extra}",
        rule("r1", "warn", "observe", false)
    )))
    .file(
        "fairlead/docs/c.md",
        &doc(&[
            "def cmp.button size=md kw=ui",
            "def tok.primary applies=color value=#111",
        ]),
    )
    .scope("apps/web", "web", &child_toml("web", web))
    .scope("apps/admin", "admin", &child_toml("admin", ""))
}

#[test]
fn r_scope_021__child_view_folds_the_chain_root_first_and_siblings_stay_apart() {
    let tighten = ovr_table("rule.r1", "sev = \"error\"");
    let reg = kit("", &tighten).resolve();
    let sev = |s: &str| {
        reg.view(&sid(s)).items[&iid("root", "rule.r1")].fields["sev"]
            .value
            .clone()
    };
    assert_eq!(
        (sev("root"), sev("web"), sev("admin")),
        (scal("warn"), scal("error"), scal("warn"))
    );
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!(f.provenance.home, sid("root"));
    assert_eq!(
        f.provenance.declared_at.path.as_str(),
        "fairlead/control.toml"
    );
    assert_eq!(f.provenance.applied, vec![reg.overrides()[0].id.clone()]);
    assert_eq!(reg.view(&sid("web")).chain, vec![sid("root"), sid("web")]);
}

#[test]
fn r_scope_022__merge_by_field_type_and_provenance_lists_grants() {
    let tables = format!(
        "{}{}",
        ovr_table("cmp.button", "op = \"add\"\nkw = [\"a\", \"ui\"]"),
        ovr_table("cmp.button", "size = \"lg\"")
    );
    let reg = kit("", &tables).resolve();
    let it = &reg.view(&sid("web")).items[&iid("root", "cmp.button")];
    assert_eq!(it.fields["size"].value, scal("lg")); // scalar replaced
    assert_eq!(
        it.fields["kw"].value,
        FieldValue::List(vec!["ui".into(), "a".into()])
    ); // appended, no duplicate
    let remove = ovr_table("cmp.button", "op = \"remove\"\nkw = [\"ui\"]");
    let reg = kit("", &remove).resolve();
    assert_eq!(
        reg.view(&sid("web")).items[&iid("root", "cmp.button")].fields["kw"].value,
        FieldValue::List(vec![])
    );
    // sealed + allow + approval: the grant id is recorded as provenance
    let allow = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"m\"\nexp = \"2027-03-31\"\n";
    let line = ovr_table(
        "rule.r1",
        "sev = \"info\"\nwhy = \"m\"\nowner = \"@andy\"\nexp = \"2026-12-31\"",
    );
    let fp = ovr_fingerprint(
        &iid("root", "rule.r1"),
        &sid("web"),
        "m",
        "@andy",
        Some(&date("2026-12-31")),
    );
    let reg = kit(allow, &line)
        .approve(fp, &iid("root", "rule.r1"), 0)
        .resolve();
    let f = &reg.view(&sid("web")).items[&iid("root", "rule.r1")].fields["sev"];
    assert_eq!(
        (f.value.clone(), f.provenance.grants.clone()),
        (scal("info"), vec!["root:allow.1".to_string()])
    );
}

#[test]
fn r_arch_041__unapproved_or_beyond_horizon_overrides_do_not_change_values() {
    let tok = |exp: &str| {
        ovr_table(
            "tok.primary",
            &format!("value = \"#fff\"\nwhy = \"b\"\nowner = \"@andy\"\nexp = \"{exp}\""),
        )
    };
    let value = |r: &fairlead_core::registry::Registry| {
        r.view(&sid("web")).items[&iid("root", "tok.primary")].fields["value"]
            .value
            .clone()
    };
    let t = iid("root", "tok.primary");
    let fp = |exp: &str| ovr_fingerprint(&t, &sid("web"), "b", "@andy", Some(&date(exp)));
    assert_eq!(value(&kit("", &tok("2026-12-31")).resolve()), scal("#111")); // no approval
    assert_eq!(
        value(
            &kit("", &tok("2026-12-31"))
                .approve(fp("2026-12-31"), &t, 0)
                .resolve()
        ),
        scal("#fff")
    );
    let far = kit("", &tok("2027-06-01"))
        .approve(fp("2027-06-01"), &t, 0)
        .resolve();
    assert_eq!(far.overrides()[0].state, OvrState::Effective);
    assert_eq!(value(&far), scal("#111")); // beyond the 90-day horizon (R-AGT-012)
}

#[test]
fn r_scope_021__file_local_ovr_is_overlaid_last_by_view_at_only() {
    let reg = kit("", "")
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n",
        )
        .file("apps/web/src/b.ts", "export const b = 1;\n")
        .resolve();
    let size = |v: fairlead_core::registry::EffectiveView| {
        v.items[&iid("root", "cmp.button")].fields["size"]
            .value
            .clone()
    };
    assert_eq!(size(reg.view(&sid("web")).clone()), scal("md"));
    assert_eq!(size(reg.view_at(&path("apps/web/src/a.ts"))), scal("sm"));
    assert_eq!(size(reg.view_at(&path("apps/web/src/b.ts"))), scal("md"));
}

#[test]
fn r_scope_018__transparent_convention_hides_route_group_segments_for_applies_matching() {
    let conv = "[[convention]]\nglob = \"app/(*)\"\ntransparent = true\n";
    let plain = Kit::new(&root_toml("")).resolve();
    let with = Kit::new(&root_toml(&format!("\n{conv}"))).resolve();
    let p = path("app/(app)/page.tsx");
    assert!(!plain.glob_matches(&sid("root"), "app/page.tsx", &p));
    assert!(with.glob_matches(&sid("root"), "app/page.tsx", &p));
    assert!(with.glob_matches(&sid("root"), "app/**/*.tsx", &p));
}

#[test]
fn r_scope_027__digest_changes_only_inside_the_edited_subtree() {
    let a = kit("", "")
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .resolve();
    let b = kit("", "")
        .file(
            "apps/web/fairlead/docs/c.md",
            &doc(&["def cmp.card sum=\"x\""]),
        )
        .resolve();
    let d = |r: &fairlead_core::registry::Registry, s: &str| r.view(&sid(s)).authored_digest;
    assert_ne!(d(&a, "web"), d(&b, "web"));
    assert_eq!(d(&a, "admin"), d(&b, "admin"));
    assert_eq!(d(&a, "root"), d(&b, "root"));
}

use fairlead_core::registry::ledger::{LineClass, propagation};

fn deep() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps", "apps", &child_toml("apps", ""))
        .file("apps/fairlead/docs/c.md", &doc(&["def cmp.card size=md"]))
        .scope(
            "apps/web",
            "web",
            &child_toml("web", &ovr_table("cmp.button", "size = \"lg\"")),
        )
        .scope(
            "apps/web/inner",
            "inner",
            &child_toml("inner", &ovr_table("cmp.button", "size = \"sm\"")),
        )
        .scope("apps/admin", "admin", &child_toml("admin", ""))
}

#[test]
fn r_scope_043__ledger_lists_an_ovr_from_the_parent_of_its_scope_up_to_the_home() {
    let reg = deep().resolve();
    let l = |s: &str| reg.ledger(&sid(s)).entries;
    let (root, apps, web, admin) = (l("root"), l("apps"), l("web"), l("admin"));
    assert_eq!(
        root.iter()
            .map(|e| (e.varied_in.0.as_str().to_string(), e.depth))
            .collect::<Vec<_>>(),
        vec![("web".to_string(), 2), ("inner".to_string(), 3)]
    );
    assert_eq!(apps.iter().map(|e| e.depth).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(web.len(), 1); // only the deeper `inner` record: nothing is listed inside its own scope
    assert!(admin.is_empty()); // never sideways
    assert!(
        root.iter()
            .all(|e| e.home == sid("root") && e.target == iid("root", "cmp.button"))
    );
}

#[test]
fn r_scope_043__nothing_is_listed_above_the_home_scope() {
    let reg = deep()
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr cmp.card size=lg\nexport const a = 1;\n",
        )
        .resolve();
    assert!(
        reg.ledger(&sid("root"))
            .entries
            .iter()
            .all(|e| e.target != iid("apps", "cmp.card"))
    );
    assert_eq!(
        reg.ledger(&sid("apps"))
            .entries
            .iter()
            .filter(|e| e.target == iid("apps", "cmp.card"))
            .count(),
        1
    );
}

#[test]
fn r_scope_042__one_record_two_readings() {
    let reg = deep().resolve();
    let size = |s: &str| {
        reg.view(&sid(s)).items[&iid("root", "cmp.button")].fields["size"]
            .value
            .clone()
    };
    assert_eq!(size("web"), scal("lg")); // inside: the patched value is the effective principle
    assert_eq!(size("root"), scal("md")); // outside: unchanged, and the ledger says who varied
    assert!(
        reg.ledger(&sid("root"))
            .entries
            .iter()
            .any(|e| e.varied_in == sid("web") && e.state == OvrState::Effective)
    );
}

#[test]
fn r_scope_060__direction_is_a_property_of_the_line_class() {
    let p = |c| {
        let x = propagation(c);
        (x.down, x.up)
    };
    assert_eq!(p(LineClass::Def), (true, true));
    assert_eq!(p(LineClass::Ovr), (true, true));
    assert_eq!(p(LineClass::Flag), (true, false));
    assert_eq!(p(LineClass::Allow), (true, false));
    assert_eq!(p(LineClass::Req), (false, true));
    assert_eq!(p(LineClass::NoteTail), (false, false));
}

#[test]
fn r_file_052__extends_target_is_the_nearest_file_item_and_rebinds_when_one_is_declared() {
    let file = |id: &str, extends: &str| {
        format!("[[file]]\nid = \"{id}\"\nkind = \"stylesheet\"\npath = \"app/x.css\"\n{extends}\n")
    };
    let root = file("file.styles", "");
    let before = Kit::new(&root_toml(&format!("\n{root}")))
        .scope("apps/web", "web", &child_toml("web", ""))
        .resolve();
    assert_eq!(
        before.nearest_file_item(&sid("web"), "stylesheet"),
        Some(&iid("root", "file.styles"))
    );
    let own = file("file.web-styles", "extends = \"root:file.styles\"");
    let after = Kit::new(&root_toml(&format!("\n{root}")))
        .scope("apps/web", "web", &child_toml("web", &own))
        .resolve();
    assert_eq!(
        after.nearest_file_item(&sid("web"), "stylesheet"),
        Some(&iid("web", "file.web-styles"))
    );
    assert_eq!(
        after.nearest_file_item(&sid("root"), "stylesheet"),
        Some(&iid("root", "file.styles"))
    );
    let item = after.item(&iid("web", "file.web-styles")).unwrap();
    assert_eq!(item.fields["extends"], scal("root:file.styles"));
    // R-FILE-043: the relationship resolves through the managerial declaration
    assert!(
        after
            .resolve_target(&sid("web"), "root:file.styles")
            .is_some()
    );
}

#[test]
fn r_file_043__an_in_file_anchor_never_creates_a_file_item() {
    let reg = Kit::new(&root_toml(""))
        .file(
            "app/globals.css",
            "/* fairlead: def file.fake */\n:root{}\n",
        )
        .resolve();
    assert!(reg.nearest_file_item(&sid("root"), "stylesheet").is_none());
    assert!(reg.items().all(|i| i.id.kind.as_str() != "file"));
}
