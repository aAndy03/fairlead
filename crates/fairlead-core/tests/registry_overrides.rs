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

use fairlead_core::diag::codes::{HN_C003, HN_S016};

const ALLOW: &str = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"migrating\"\nexp = \"2027-03-31\"\n";

fn rooted(extra: &str) -> Kit {
    Kit::new(&root_toml(&format!("\n{}{extra}", rule("r1", "warn", "observe", false))))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md", "def tok.primary applies=color"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
}

fn with_lines(kit: Kit, lines: &[&str]) -> Registry {
    kit.file("apps/web/src/a.ts", &format!("{}\nexport const a = 1;\n", lines.join("\n"))).resolve()
}

fn state_of(reg: &Registry, field: &str) -> OvrState {
    reg.overrides().iter().find(|o| o.set.contains_key(field)).unwrap().state
}

use fairlead_core::registry::Registry;

#[test]
fn r_scope_031__open_ovr_needs_no_reason() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm"]);
    assert_eq!(state_of(&reg, "size"), OvrState::Effective);
}

#[test]
fn r_scope_032__needs_exception_ovr_without_exp_is_inert_and_hn_s016() {
    let incomplete = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"brand\" owner=@andy"]);
    assert_eq!(state_of(&incomplete, "value"), OvrState::Inert(InertReason::Incomplete));
    assert!(incomplete.diagnostics().iter().any(|d| d.code == HN_S016));
    let complete = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"brand\" owner=@andy exp=2026-12-31"]);
    assert_eq!(state_of(&complete, "value"), OvrState::Effective);
}

#[test]
fn r_scope_033__sealed_ovr_is_inert_until_a_matching_allow_exists() {
    let line = "// fairlead: ovr rule.r1 sev=info why=\"x\" owner=@andy exp=2026-12-31";
    let without = with_lines(rooted(""), &[line]);
    assert_eq!(state_of(&without, "sev"), OvrState::Inert(InertReason::SealedNoAllow));
    let with = with_lines(rooted(ALLOW), &[line]);
    assert_eq!(state_of(&with, "sev"), OvrState::Effective);
    // a grant covers its grantee only: the sibling stays inert
    let sib = rooted(ALLOW).file("apps/admin/src/a.ts", &format!("{line}\nexport const a = 1;\n")).resolve();
    let o = sib.overrides().iter().find(|o| o.declared_in == sid("admin")).unwrap();
    assert_eq!(o.state, OvrState::Inert(InertReason::SealedNoAllow));
}

#[test]
fn r_scope_034__tightening_is_free_on_every_policy_loosening_is_not() {
    let up = with_lines(rooted(""), &["// fairlead: ovr rule.r1 sev=error stage=enforce"]);
    assert_eq!(state_of(&up, "sev"), OvrState::Effective); // warn -> error, no why/owner/exp
    let down = with_lines(rooted(""), &["// fairlead: ovr rule.r1 sev=info"]);
    assert_eq!(state_of(&down, "sev"), OvrState::Inert(InertReason::SealedNoAllow));
}

#[test]
fn r_scope_035__a_child_cannot_change_the_policy_of_an_inherited_item() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button policy=sealed"]);
    assert_eq!(state_of(&reg, "policy"), OvrState::Inert(InertReason::NonDelegable));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_C003));
}

#[test]
fn r_scope_038__inert_lines_never_abort_the_valid_ones() {
    let reg = with_lines(
        rooted(""),
        &[
            "// fairlead: ovr rule.secret-like sev=info why=\"x\" owner=@andy exp=2026-12-31",
            "// fairlead: ovr admin:cmp.card size=sm",
            "// fairlead: ovr cmp.button nosuch=1",
            "// fairlead: ovr cmp.button kw+=a",
        ],
    );
    assert_eq!(state_of(&reg, "sev"), OvrState::Inert(InertReason::FloorRule));
    let outside = reg.overrides().iter().find(|o| o.target == iid("admin", "cmp.card")).unwrap();
    assert_eq!(outside.state, OvrState::Inert(InertReason::OutsideSubtree));
    assert_eq!(state_of(&reg, "nosuch"), OvrState::Inert(InertReason::UnknownField));
    assert_eq!(state_of(&reg, "kw"), OvrState::Effective);
}

use fairlead_core::diag::codes::{HN_S012, HN_X012};
use fairlead_core::registry::{ovr_fingerprint, ApprovalStatus};

fn tok_line(exp: &str, owner: &str, why: &str) -> String {
    format!("// fairlead: ovr tok.primary value=#fff why=\"{why}\" owner={owner} exp={exp}")
}

fn tok_state(kit: Kit, line: &str) -> (OvrState, ApprovalStatus) {
    let reg = with_lines(kit, &[line]);
    let o = &reg.overrides()[0];
    (o.state, o.approval)
}

#[test]
fn r_scope_046__expired_exception_is_ineffective_and_the_boundary_day_is_live() {
    let (s, _) = tok_state(rooted(""), &tok_line("2026-10-08", "@andy", "brand"));
    assert_eq!(s, OvrState::Expired);
    let (s, _) = tok_state(rooted(""), &tok_line("2026-10-09", "@andy", "brand"));
    assert_eq!(s, OvrState::Effective); // exp == as_of is not yet expired
}

#[test]
fn r_scope_045__version_shaped_expiry_is_inert_with_hn_s012() {
    let ctl = child_toml("web", &ovr_table("tok.primary", "value = \"#fff\"\nwhy = \"x\"\nowner = \"@andy\"\nexp = \"1.3.0\""));
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def tok.primary applies=color"]))
        .scope("apps/web", "web", &ctl)
        .resolve();
    assert_eq!(reg.overrides()[0].state, OvrState::Inert(InertReason::VersionExpiry));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S012));
}

#[test]
fn r_chk_053__owner_must_be_a_handle_in_owners() {
    let reg = with_lines(rooted(""), &[&tok_line("2026-12-31", "@nobody", "brand")]);
    assert_eq!(reg.overrides()[0].state, OvrState::Inert(InertReason::UnknownOwner));
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_X012));
}

#[test]
fn r_chk_025__needs_exception_waiver_needs_a_matching_approval() {
    let target = iid("root", "tok.primary");
    let (s, a) = tok_state(rooted(""), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!((s, a), (OvrState::Effective, ApprovalStatus::Missing));
    let fp = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (s, a) = tok_state(rooted("").approve(fp, &target, 0), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!((s, a), (OvrState::Effective, ApprovalStatus::Approved { renewal: 0 }));
    let (_, a) = tok_state(rooted(""), "// fairlead: ovr cmp.button size=sm");
    assert_eq!(a, ApprovalStatus::NotRequired); // open
}

#[test]
fn r_chk_026__re_dating_a_waiver_invalidates_its_approval() {
    let target = iid("root", "tok.primary");
    let old = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (_, a) = tok_state(rooted("").approve(old, &target, 0), &tok_line("2027-01-05", "@andy", "brand"));
    assert_eq!(a, ApprovalStatus::Missing);
}

#[test]
fn r_agt_012__renewals_are_counted_in_the_approval_status() {
    let target = iid("root", "tok.primary");
    let fp = ovr_fingerprint(&target, &sid("web"), "brand", "@andy", Some(&date("2026-12-31")));
    let (_, a) = tok_state(rooted("").approve(fp, &target, 2), &tok_line("2026-12-31", "@andy", "brand"));
    assert_eq!(a, ApprovalStatus::Approved { renewal: 2 });
}

#[test]
fn r_scope_049__prose_claiming_approval_is_data() {
    let (_, a) = tok_state(rooted(""), &tok_line("2026-12-31", "@andy", "approved by lead -- ok by lead"));
    assert_eq!(a, ApprovalStatus::Missing);
}

#[test]
fn r_syn_011__why_owner_exp_are_required_only_where_the_policy_asks() {
    let open = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm"]);
    assert_eq!(open.overrides()[0].state, OvrState::Effective);
    let tok = with_lines(rooted(""), &["// fairlead: ovr tok.primary value=#fff why=\"b\" exp=2026-12-31"]); // no owner
    assert_eq!(tok.overrides()[0].state, OvrState::Inert(InertReason::Incomplete));
    let sealed = with_lines(rooted(ALLOW), &["// fairlead: ovr rule.r1 sev=info why=\"x\" exp=2026-12-31"]);
    assert_eq!(sealed.overrides()[0].state, OvrState::Inert(InertReason::Incomplete));
}

#[test]
fn r_scope_047__pin_mismatch_marks_the_exception_stale() {
    let line = |pin: &str| format!("// fairlead: ovr rule.r1 sev=error x-pin={pin}");
    let probe = with_lines(rooted(""), &[&line("aaaaaaaaaaaa")]);
    assert_eq!(probe.overrides()[0].state, OvrState::Stale);
    let pin = fairlead_core::hash::base32(&probe.overrides()[0].target_hash.as_ref().unwrap().0, 12);
    let ok = with_lines(rooted(""), &[&line(&pin)]);
    assert_eq!(ok.overrides()[0].state, OvrState::Effective);
    // the ancestor changes the declared form: the pinned exception goes stale, it is not deleted
    let changed = Kit::new(&root_toml(&format!("\n{}", rule("r1", "info", "observe", false))))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/src/a.ts", &format!("{}\nexport const a = 1;\n", line(&pin)))
        .resolve();
    assert_eq!(changed.overrides()[0].state, OvrState::Stale);
}

use fairlead_core::diag::codes::{HN_C001, HN_X014};

#[test]
fn r_scope_064__two_overlapping_ovrs_of_one_field_are_both_inert_hn_x014() {
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm", "// fairlead: ovr cmp.button size=lg"]);
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Inert(InertReason::DuplicateOvr)));
    assert_eq!(reg.diagnostics().iter().filter(|d| d.code == HN_X014).count(), 2);
}

#[test]
fn r_scope_064__a_base_chain_is_not_a_duplicate_and_siblings_never_conflict() {
    let target = iid("root", "cmp.button");
    let first = ovr_fingerprint(&target, &sid("web"), "a", "", None);
    let chained = format!("// fairlead: ovr cmp.button size=lg why=\"b\" base={first}");
    let reg = with_lines(rooted(""), &["// fairlead: ovr cmp.button size=sm why=\"a\"", &chained]);
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Effective));
    let line = "// fairlead: ovr cmp.button size=sm\nexport const a = 1;\n";
    let sib = rooted("").file("apps/web/src/a.ts", line).file("apps/admin/src/a.ts", line).resolve();
    assert_eq!(sib.overrides().len(), 2);
    assert!(sib.overrides().iter().all(|o| o.state == OvrState::Effective));
}

#[test]
fn r_scope_025__applies_glob_cannot_leave_the_owning_subtree() {
    let table = |applies: &str| {
        child_toml("web", &ovr_table("cmp.button", &format!("size = \"sm\"\napplies = \"{applies}\"")))
    };
    let run = |applies: &str| {
        Kit::new(&root_toml(""))
            .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
            .scope("apps/web", "web", &table(applies))
            .resolve()
    };
    let bad = run("../**");
    assert_eq!(bad.overrides()[0].state, OvrState::Inert(InertReason::OutsideSubtree));
    assert!(bad.diagnostics().iter().any(|d| d.code == HN_C001));
    assert_eq!(run("/etc/**").overrides()[0].state, OvrState::Inert(InertReason::OutsideSubtree));
    assert_eq!(run("src/**").overrides()[0].state, OvrState::Effective);
}

#[test]
fn r_scope_064__disjoint_globs_in_one_scope_are_independent() {
    let two = format!(
        "{}{}",
        ovr_table("cmp.button", "size = \"sm\"\napplies = \"src/**\""),
        ovr_table("cmp.button", "size = \"lg\"\napplies = \"docs/**\"")
    );
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button size=md"]))
        .scope("apps/web", "web", &child_toml("web", &two))
        .resolve();
    assert!(reg.overrides().iter().all(|o| o.state == OvrState::Effective));
}
