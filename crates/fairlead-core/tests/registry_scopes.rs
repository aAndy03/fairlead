#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::registry::{
    EffectiveView, LedgerView, Lookup, Override, Registry, Scope, ScopeId, ScopeState,
};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn r_arch_061__registry_types_are_send_sync_plain_data() {
    assert_send_sync::<Registry>();
    assert_send_sync::<EffectiveView>();
    assert_send_sync::<LedgerView>();
    assert_send_sync::<Override>();
    assert_send_sync::<Scope>();
    assert_send_sync::<Lookup>();
    let _ = |s: &ScopeState, i: &ScopeId| (s.clone(), i.clone());
}

use fairlead_core::diag::codes::HN_C007;
use support::*;

fn two_level() -> Kit {
    Kit::new(&root_toml(""))
        .scope("apps/web", "web-ui", &child_toml("web-ui", ""))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
}

#[test]
fn r_scope_001__scope_id_is_the_stored_slug_not_the_path() {
    let reg = two_level().resolve();
    let web = reg.scope(&sid("web-ui")).unwrap();
    assert_eq!(web.dir.as_str(), "apps/web");
    assert_eq!(web.parent, Some(sid("root")));
    assert_eq!(web.depth, 1);
    let inner = reg.scope(&sid("inner")).unwrap();
    assert_eq!(inner.parent, Some(sid("web-ui")));
    assert_eq!(inner.depth, 2);
    let ids: Vec<String> = reg.scopes().map(|s| s.id.0.as_str().to_string()).collect();
    assert_eq!(ids, vec!["inner", "root", "web-ui"]); // sorted by id, not by path
    assert_eq!(reg.root_scope(), &sid("root"));
}

#[test]
fn r_scope_002__identity_survives_a_directory_move() {
    let a = two_level().resolve();
    let b = Kit::new(&root_toml(""))
        .scope("packages/ui", "web-ui", &child_toml("web-ui", ""))
        .scope("packages/ui/inner", "inner", &child_toml("inner", ""))
        .resolve();
    assert_eq!(a.chain(&sid("inner")), b.chain(&sid("inner")));
    assert_eq!(a.scope(&sid("web-ui")).unwrap().dir.as_str(), "apps/web");
    assert_eq!(b.scope(&sid("web-ui")).unwrap().dir.as_str(), "packages/ui");
    assert!(a.diagnostics().is_empty() && b.diagnostics().is_empty());
}

#[test]
fn r_scope_003__membership_is_nearest_ancestor_or_self() {
    let reg = two_level().resolve();
    let of = |p: &str| reg.scope_of(&path(p)).cloned();
    assert_eq!(of("apps/web/src/a.rs"), Some(sid("web-ui")));
    assert_eq!(of("apps/web/inner/x.rs"), Some(sid("inner")));
    assert_eq!(of("apps/web"), Some(sid("web-ui")));
    assert_eq!(of("apps/web/fairlead/docs/x.md"), Some(sid("web-ui")));
    assert_eq!(of("apps/admin/x.rs"), Some(sid("root")));
    assert_eq!(of("README.md"), Some(sid("root")));
}

#[test]
fn r_scope_005__duplicate_scope_id_is_hn_c007_and_the_loser_is_unresolved() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/a", "same", &child_toml("same", ""))
        .scope("apps/b", "same", &child_toml("same", ""))
        .resolve();
    let c007: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C007).collect();
    assert_eq!(c007.len(), 1);
    assert_eq!(c007[0].location.path.as_str(), "apps/b/fairlead/control.toml");
    assert_eq!(reg.scope(&sid("same")).unwrap().dir.as_str(), "apps/a");
    let loser = reg.scope_of(&path("apps/b/x.rs")).unwrap();
    assert_ne!(loser, &sid("same"));
    assert!(matches!(reg.scope(loser).unwrap().state, ScopeState::Unresolved { .. }));
}

#[test]
fn r_scope_015__leaf_folder_without_managerial_folder_stays_implicit() {
    let reg = two_level()
        .file("apps/web/src/leaf/a.css", "/* fairlead: use tok.x */\n:root{}\n")
        .resolve();
    assert_eq!(reg.scope_of(&path("apps/web/src/leaf/a.css")), Some(&sid("web-ui")));
    assert!(reg.scopes().all(|s| !s.dir.as_str().ends_with("leaf")));
}

use fairlead_core::diag::codes::{HN_C001, HN_C008};

#[test]
fn r_scope_005__scope_missing_from_parent_shard_is_hn_c008() {
    let reg = two_level().no_members().resolve();
    let c008: Vec<_> = reg.diagnostics().iter().filter(|d| d.code == HN_C008).collect();
    assert_eq!(c008.len(), 2); // web-ui and inner; the root needs no registration
    assert_eq!(c008[0].location.path.as_str(), "apps/web/fairlead/control.toml");
    assert!(c008[0].message.contains("web-ui"));
    // registered scopes raise nothing
    assert!(two_level().resolve().diagnostics().iter().all(|d| d.code != HN_C008));
}

#[test]
fn r_scope_054__unparseable_control_leaves_the_subtree_unresolved() {
    let reg = Kit::new(&root_toml(""))
        .scope("apps/web", "web-ui", "[fairlead\nscope = ")
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
        .scope("apps/admin", "admin", &child_toml("admin", ""))
        .resolve();
    let web = reg.scope(&sid("web-ui")).expect("id is recovered from the parent shard");
    match &web.state {
        ScopeState::Unresolved { reason } => assert!(reason.iter().any(|d| d.code == HN_C001)),
        other => panic!("expected Unresolved, got {other:?}"),
    }
    assert!(web.control.is_none());
    assert!(matches!(reg.scope(&sid("inner")).unwrap().state, ScopeState::Unresolved { .. }));
    assert_eq!(reg.scope(&sid("admin")).unwrap().state, ScopeState::Resolved);
    assert_eq!(reg.scope(&sid("root")).unwrap().state, ScopeState::Resolved);
    assert_eq!(reg.scope_of(&path("apps/web/a.rs")), Some(&sid("web-ui"))); // no silent fallback to root
}

#[test]
fn r_scope_054__scope_in_sparse_absent_dir_is_absent_from_tree() {
    let reg = Kit::new(&root_toml(""))
        .phantom("apps/gone", "gone")
        .sparse_absent("apps/gone")
        .resolve();
    let gone = reg.scope(&sid("gone")).unwrap();
    assert_eq!(gone.state, ScopeState::AbsentFromTree);
    assert_eq!(gone.parent, Some(sid("root")));
    assert!(gone.control.is_none());
}

#[test]
fn r_scope_055__diagnostics_do_not_depend_on_how_the_root_was_selected() {
    let build = |k: Kit| k.no_members().resolve();
    let base = || {
        Kit::new(&root_toml(""))
            .scope("apps/web", "web-ui", "[fairlead\nscope = ")
            .scope("apps/web/inner", "inner", &child_toml("inner", ""))
    };
    let a = build(base());
    let b = build(base().search_source());
    assert_eq!(a.diagnostics(), b.diagnostics());
    let ids = |r: &fairlead_core::registry::Registry| r.scopes().map(|s| s.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&a), ids(&b));
}
