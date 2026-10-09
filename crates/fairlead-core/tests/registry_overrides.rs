#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::diag::codes::HN_S009;
use fairlead_core::registry::{FieldChange, FieldValue, InertReason, OvrOp, OvrState};
use support::*;

fn base() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button layer=atom size=md kw=ui", "def tok.primary applies=color"]))
        .scope("apps/web", "web", &child_toml("web", ""))
}

#[test]
fn r_scope_040__ovr_names_target_op_and_values() {
    let reg = base()
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button op=set layer=molecule\nexport const a = 1;\n")
        .resolve();
    let o = &reg.overrides()[0];
    assert_eq!(o.target, iid("root", "cmp.button"));
    assert_eq!(o.op, OvrOp::Set);
    assert_eq!(o.declared_in, sid("web"));
    assert_eq!(o.set["layer"], FieldChange::Replace(FieldValue::Scalar("molecule".into())));
    assert_eq!(o.state, OvrState::Effective); // open policy needs nothing (R-SCOPE-031)
}

#[test]
fn r_scope_041__ovr_is_file_local_or_a_scope_level_table() {
    let ctl = child_toml("web", &ovr_table("cmp.button", "op = \"set\"\napplies = \"src/**\"\nsize = \"lg\""));
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &ctl)
        .file("apps/web/src/a.ts", "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n")
        .resolve();
    let table: Vec<_> = reg.overrides().iter().filter(|o| o.applies.is_some()).collect();
    let local: Vec<_> = reg.overrides().iter().filter(|o| o.applies.is_none()).collect();
    assert_eq!((table.len(), local.len()), (1, 1));
    assert_eq!(table[0].applies.as_deref(), Some("src/**"));
    assert_eq!(local[0].at.path.as_str(), "apps/web/src/a.ts");
}

#[test]
fn r_syn_013__append_marker_remove_op_and_unknown_field() {
    let reg = base()
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr cmp.button kw+=a,b\n// fairlead: ovr cmp.button nosuch=1\nexport const a = 1;\n",
        )
        .resolve();
    let app = reg.overrides().iter().find(|o| o.set.contains_key("kw")).unwrap();
    assert_eq!(app.set["kw"], FieldChange::Append(vec!["a".into(), "b".into()]));
    let bad = reg.overrides().iter().find(|o| o.set.contains_key("nosuch")).unwrap();
    assert_eq!(bad.state, OvrState::Inert(InertReason::UnknownField));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S009));
}

#[test]
fn r_scope_040__target_may_name_a_field_and_a_dangling_target_makes_no_override() {
    let reg = base()
        .file(
            "apps/web/src/a.ts",
            "// fairlead: ovr tok.primary.value value=#ffffff\n// fairlead: ovr cmp.nothere size=sm\nexport const a = 1;\n",
        )
        .resolve();
    assert_eq!(reg.overrides().len(), 1);
    assert_eq!(reg.overrides()[0].field.as_deref(), Some("value"));
    assert_eq!(reg.resolve_target(&sid("web"), "tok.primary.value"), Some((iid("root", "tok.primary"), Some("value".into()))));
    assert_eq!(reg.resolve_target(&sid("web"), "cmp.nothere"), None);
}

#[test]
fn r_chk_026__ovr_id_binds_to_content_not_to_the_file_path() {
    let line = "// fairlead: ovr cmp.button size=sm why=\"dense\" owner=@andy exp=2026-12-31\nexport const a = 1;\n";
    let a = base().file("apps/web/src/a.ts", line).resolve();
    let b = base().file("apps/web/lib/moved/b.ts", line).resolve();
    assert_eq!(a.overrides()[0].id, b.overrides()[0].id);
    let c = base().file("apps/web/src/a.ts", &line.replace("2026-12-31", "2027-01-31")).resolve();
    assert_ne!(a.overrides()[0].id, c.overrides()[0].id); // re-dating changes the fingerprint
}
