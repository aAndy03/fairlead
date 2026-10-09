#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod support;

use fairlead_core::diag::codes::HN_C003;
use fairlead_core::registry::ScopeState;
use support::*;

const ALLOW: &str = "[[allow]]\ntarget = \"rules.r1\"\nfield = \"sev\"\nfor = [\"web\"]\nwhy = \"migrating\"\nexp = \"2027-03-31\"\n";

fn kit(root_extra: &str) -> Kit {
    Kit::new(&root_toml(&format!("\n{}{}", rule("r1", "error", "enforce", false), root_extra)))
        .scope("apps/web", "web", &child_toml("web", ""))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
}

#[test]
fn r_scope_036__allow_in_the_home_scope_is_valid() {
    let reg = kit(ALLOW).resolve();
    let g = &reg.grants()[0];
    assert!(g.valid);
    assert_eq!(g.id, "root:allow.1");
    assert_eq!((g.granting.clone(), g.target.clone()), (sid("root"), iid("root", "rule.r1")));
    assert_eq!(g.field.as_deref(), Some("sev"));
    assert_eq!(g.for_scopes, vec![sid("web")]);
}

#[test]
fn r_scope_058__grant_authored_outside_the_home_scope_is_invalid_and_not_re_delegable() {
    let forward = "[[allow]]\ntarget = \"rule.r1\"\nfor = [\"inner\"]\nwhy = \"forwarded\"\nexp = \"2027-03-31\"\n";
    let reg = Kit::new(&root_toml(&format!("\n{}", rule("r1", "error", "enforce", false))))
        .scope("apps/web", "web", &child_toml("web", forward))
        .scope("apps/web/inner", "inner", &child_toml("inner", ""))
        .resolve();
    assert_eq!(reg.grants().len(), 1);
    assert!(!reg.grants()[0].valid);
    assert!(reg.diagnostics().iter().any(|d| d.code == HN_C003 && d.location.path.as_str() == "apps/web/fairlead/control.toml"));
}

#[test]
fn r_scope_053__req_is_kept_sorted_and_never_effective() {
    let req = "[[req]]\nto = \"root\"\ntarget = \"layer-ranks.widget\"\nset = \"2\"\nwhy = \"needs a rank\"\nblocking = true\n";
    let bad = "[[req]]\nto = \"inner\"\ntarget = \"x.y\"\nwhy = \"sideways\"\n";
    let reg = kit("")
        .scope("apps/admin", "admin", &child_toml("admin", &format!("{req}{bad}")))
        .resolve();
    assert_eq!(reg.requests().len(), 1); // `to` must be a proper ancestor
    let r = &reg.requests()[0];
    assert_eq!((r.from.clone(), r.to.clone(), r.blocking), (sid("admin"), sid("root"), true));
    assert_eq!(r.set.as_deref(), Some("2"));
    assert_eq!(reg.view(&sid("admin")).items.len(), reg.view(&sid("root")).items.len());
}

#[test]
fn r_scope_037__child_cannot_set_vocabulary_keys_and_gets_an_escalation_proposal() {
    let child = child_toml("web", "[settings]\nowners = [\"@x\"]\n\n[policies]\n\"rules.*\" = \"open\"\n");
    let reg = Kit::new(&root_toml(&format!("\n{}", rule("r1", "error", "enforce", false))))
        .scope("apps/web", "web", &child)
        .resolve();
    assert_eq!(reg.scope(&sid("web")).unwrap().state, ScopeState::Resolved);
    assert!(reg.scope(&sid("web")).unwrap().owners.is_empty()); // owners are root-only in 0.1
    assert_eq!(reg.item(&iid("root", "rule.r1")).unwrap().policy, fairlead_core::config::Policy::Sealed);
    let p = reg.proposals();
    assert_eq!(p.len(), 2, "one proposal per non-delegable key attempted");
    assert_eq!((p[0].from.clone(), p[0].to.clone()), (sid("web"), sid("root")));
}
