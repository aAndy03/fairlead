#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::config::Policy;
use fairlead_core::diag::codes::{HN_C003, HN_R005};
use fairlead_core::registry::{FieldValue, ScopeState};
use support::*;

/// `extra` of `root_toml` lands inside the `[policies]` table: policy lines first, then new tables.
fn root_with_rules() -> String {
    root_toml(&format!(
        "\"rules.r3\" = \"needs-exception\"\n\n{}{}{}",
        rule("r1", "error", "enforce", false),
        rule("w1", "warn", "observe", true),
        rule("r3", "warn", "warn", false)
    ))
}

#[test]
fn r_scope_030__rules_default_to_sealed_and_patterns_refine() {
    let reg = Kit::new(&root_with_rules()).resolve();
    let policy = |n: &str| reg.item(&iid("root", n)).unwrap().policy;
    assert_eq!(policy("rule.r1"), Policy::Sealed); // "rules.*" = "sealed"
    assert_eq!(policy("rule.r3"), Policy::NeedsException); // exact pattern beats the wildcard
    let r1 = reg.item(&iid("root", "rule.r1")).unwrap();
    assert_eq!(r1.fields["sev"], FieldValue::Scalar("error".into()));
    assert_eq!(r1.fields["stage"], FieldValue::Scalar("enforce".into()));
    assert_eq!(r1.fields["glob"], FieldValue::List(vec!["**/*.rs".into()]));
}

#[test]
fn r_chk_006__waivable_rule_is_needs_exception() {
    let reg = Kit::new(&root_with_rules()).resolve();
    assert_eq!(reg.item(&iid("root", "rule.w1")).unwrap().policy, Policy::NeedsException);
}

#[test]
fn r_syn_024__child_cannot_redefine_an_inherited_kind() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", "[kinds.cmp]\ntitle = \"mine\"\n[kinds.wdg]\ntitle = \"widget\"\n"))
        .resolve();
    let c003: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C003).collect();
    assert_eq!(c003.len(), 1);
    assert!(c003[0].message.contains("cmp"));
}

#[test]
fn r_scope_026__ancestor_declaration_wins_over_a_descendant_redefinition_hn_r005() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", &child_toml("web", &rule("r1", "info", "observe", false)))
        .resolve();
    let r005: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_R005).collect();
    assert_eq!(r005.len(), 1);
    assert_eq!(r005[0].related.as_deref(), Some("root:rule.r1"));
    assert!(reg.item(&iid("web", "rule.r1")).is_none());
    assert!(reg.item(&iid("root", "rule.r1")).is_some()); // R-SCOPE-026: the ancestor's declaration stands
}

#[test]
fn r_syn_033__unresolved_scope_has_no_view_and_never_inherits_the_ancestor_rules() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", "[fairlead\nscope = ")
        .resolve();
    assert!(matches!(reg.scope(&sid("web")).unwrap().state, ScopeState::Unresolved { .. }));
    assert!(reg.view(&sid("web")).items.is_empty());
    assert!(!reg.view(&sid("root")).items.is_empty());
}

#[test]
fn r_arch_104__error_in_one_scope_does_not_hide_an_unrelated_scope() {
    let reg = Kit::new(&root_with_rules())
        .scope("apps/web", "web", "[fairlead\nscope = ")
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .resolve();
    let admin = reg.view(&sid("admin"));
    assert!(admin.items.contains_key(&iid("root", "rule.r1")));
    assert_eq!(admin.chain, vec![sid("root"), sid("admin")]);
}

use fairlead_core::diag::codes::{HN_D001, HN_S002, HN_S011};
use fairlead_syntax::diag::Severity;


#[test]
fn r_syn_026__two_doc_defs_of_one_id_in_one_scope_is_hn_d001() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/t.md", &doc(&["def tok.primary applies=color", "def tok.primary applies=background"]))
        .resolve();
    assert_eq!(reg.diagnostics().iter().filter(|d| d.code == HN_D001).count(), 1);
    assert!(reg.item(&iid("root", "tok.primary")).is_some());
}

#[test]
fn r_scope_023__child_doc_def_of_an_inherited_id_is_hn_r005() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .resolve();
    let d: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_R005).collect();
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].location.path.as_str(), "apps/web/fairlead/docs/c.md");
    assert!(reg.item(&iid("web", "cmp.button")).is_none());
}

#[test]
fn r_scope_024__siblings_may_reuse_a_bare_name() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
        .resolve();
    assert!(reg.item(&iid("web", "cmp.card")).is_some());
    assert!(reg.item(&iid("admin", "cmp.card")).is_some());
    assert!(reg.diagnostics().iter().all(|d| d.code != HN_R005 && d.code != HN_D001));
}

#[test]
fn r_syn_024__undeclared_kind_is_hn_s002_with_declared_kinds_and_a_suggestion() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def comp.button"]))
        .resolve();
    let d = reg.diagnostics().iter().find(|d| d.code == HN_S002).unwrap();
    assert!(d.expected.as_deref().unwrap().contains("cmp"));
    assert_eq!(d.did_you_mean.as_deref(), Some("cmp.button"));
    assert_eq!(reg.items().filter(|i| i.id.scope == sid("root")).count(), 0);
}

#[test]
fn r_syn_013__unknown_def_key_is_hn_s011_warn_and_error_under_bang() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.a bogus=1 x-note=ok", "!def cmp.b bogus=1"]))
        .resolve();
    let d: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_S011).collect();
    assert_eq!(d.len(), 2); // x- keys are never reported
    assert!(d.iter().any(|d| d.severity == Severity::Warn));
    assert!(d.iter().any(|d| d.severity == Severity::Error));
    assert!(reg.item(&iid("root", "cmp.a")).is_some()); // warned, kept
    assert!(reg.item(&iid("root", "cmp.b")).is_none()); // must-understand, dropped
}

#[test]
fn r_scope_030__def_policy_key_refines_the_item_policy() {
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.locked policy=sealed sum=\"fixed\"", "def cmp.free"]))
        .resolve();
    let locked = reg.item(&iid("root", "cmp.locked")).unwrap();
    assert_eq!(locked.policy, Policy::Sealed);
    assert_eq!(locked.sum.as_deref(), Some("fixed"));
    assert_eq!(reg.item(&iid("root", "cmp.free")).unwrap().policy, Policy::Open); // "cmp.*" = "open"
}

use fairlead_core::diag::codes::HN_R007;
use fairlead_core::registry::Lookup;
use fairlead_syntax::ids::Id;

fn parse_id(s: &str) -> Id {
    Id::parse(s).unwrap()
}

fn nested() -> Kit {
    Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.button"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/fairlead/docs/c.md", &doc(&["def cmp.card", "def tok.sidebar"]))
        .file("apps/admin/fairlead/docs/c.md", &doc(&["def cmp.card"]))
}

#[test]
fn r_syn_025__unqualified_reference_resolves_to_the_nearest_declaring_scope() {
    let reg = nested()
        .file("apps/web/src/a.ts", "// fairlead: use cmp.button cmp.card :cmp.button\nexport const a = 1;\n")
        .resolve();
    let to: Vec<_> = reg.edges().iter().map(|e| e.resolved.clone()).collect();
    assert_eq!(
        to,
        vec![Some(iid("root", "cmp.button")), Some(iid("web", "cmp.card")), Some(iid("root", "cmp.button"))]
    );
    assert_eq!(reg.edges()[0].from.scope, sid("web"));
}

#[test]
fn r_scope_024__ancestor_into_descendant_reference_needs_a_qualifier() {
    let reg = nested().resolve();
    match reg.lookup(&sid("root"), &parse_id("tok.sidebar")) {
        Lookup::Missing { nearest } => assert_eq!(nearest[0].to_string(), "web:tok.sidebar"),
        other => panic!("expected Missing, got {other:?}"),
    }
    assert_eq!(
        reg.lookup(&sid("root"), &parse_id("web:tok.sidebar")),
        Lookup::Found(iid("web", "tok.sidebar"))
    );
}

#[test]
fn r_arch_062__bare_reference_with_two_candidates_is_ambiguous_hn_r007() {
    let reg = nested().file("src/top.ts", "// fairlead: use cmp.card\nexport const t = 1;\n").resolve();
    match reg.lookup(&sid("root"), &parse_id("cmp.card")) {
        Lookup::Ambiguous(c) => assert_eq!(c, vec![iid("admin", "cmp.card"), iid("web", "cmp.card")]),
        other => panic!("expected Ambiguous, got {other:?}"),
    }
    let d = reg.diagnostics().iter().find(|d| d.code == HN_R007).unwrap();
    assert_eq!(d.location.path.as_str(), "src/top.ts");
    assert!(reg.edges()[0].resolved.is_none());
}

#[test]
fn r_arch_062__missing_reference_lists_the_nearest_ids_by_edit_distance() {
    let reg = nested().resolve();
    match reg.lookup(&sid("web"), &parse_id("cmp.buton")) {
        Lookup::Missing { nearest } => {
            assert_eq!(nearest[0].to_string(), "cmp.button");
            assert!(nearest.len() <= 3);
        }
        other => panic!("expected Missing, got {other:?}"),
    }
    // wrong kind prefix: the right ID is offered
    match reg.lookup(&sid("web"), &parse_id("tok.button")) {
        Lookup::Missing { nearest } => assert_eq!(nearest[0].to_string(), "cmp.button"),
        other => panic!("expected Missing, got {other:?}"),
    }
}

#[test]
fn r_file_037__def_in_a_leaf_folder_belongs_to_the_nearest_ancestor_scope() {
    let reg = nested()
        .file("apps/web/src/organisms/sidebar/sidebar.css", "/* fairlead: def tok.sidebar */\n:root{}\n")
        .resolve();
    let sites = reg.sites_of(&iid("web", "tok.sidebar"));
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].scope, sid("web"));
    assert_eq!(sites[0].path.as_str(), "apps/web/src/organisms/sidebar/sidebar.css");
}

#[test]
fn r_syn_024__use_of_an_undeclared_kind_is_hn_s002_and_makes_no_edge() {
    let reg = nested().file("apps/web/src/a.ts", "// fairlead: use comp.x\nexport const a = 1;\n").resolve();
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_S002 && d.location.path.as_str() == "apps/web/src/a.ts"));
    assert!(reg.edges().is_empty());
}

#[test]
fn r_scope_017__home_must_be_an_ancestor_of_every_implementation_site() {
    use fairlead_core::registry::item::home_mismatches;
    let reg = Kit::new(&root_toml(""))
        .file("fairlead/docs/c.md", &doc(&["def cmp.bridge home=web"]))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .file("apps/web/src/a.ts", "// fairlead: def cmp.bridge
export const a = 1;
")
        .file("apps/admin/src/a.ts", "// fairlead: def cmp.bridge
export const a = 1;
")
        .resolve();
    let m = home_mismatches(&reg);
    assert_eq!(m.len(), 1); // the admin site; HN-C013 itself is raised by P5
    assert_eq!((m[0].1.scope.clone(), m[0].2.clone()), (sid("admin"), sid("web")));
}
