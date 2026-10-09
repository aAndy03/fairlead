//! Dead anchored globs after a folder move (R-PORT-002).
use crate::{
    check::{CheckCx, builtin::control_path, finding::DiagB, rules::Check},
    diag::{Diagnostic, codes::*},
    extract::literals::rel_to,
    registry::ScopeId,
};
use fairlead_syntax::diag::DiagCode;
use globset::Glob;

pub struct PathsCheck;
const CODES: [DiagCode; 1] = [HN_M001];

/// Only a glob whose first segment is literal can go dead; `**/*.css` may match nothing yet.
fn anchored(g: &str) -> bool {
    let first = g.trim_start_matches("./").split('/').next().unwrap_or("");
    !first.is_empty() && !first.contains(['*', '?', '[', '{'])
}

impl Check for PathsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let Some(sc) = reg.scope(scope) else { return };
        let (Some(ctl), Some(cp)) = (sc.control.as_ref(), control_path(reg, scope)) else {
            return;
        };
        let files: Vec<&str> = cx
            .corpus
            .records
            .keys()
            .filter_map(|p| rel_to(p.as_str(), sc.dir.as_str()))
            .collect();
        let globs = ctl
            .rules
            .values()
            .flat_map(|r| r.glob.iter().cloned())
            .chain(ctl.conventions.iter().map(|c| c.glob.clone()))
            .chain(ctl.ovrs.iter().filter_map(|o| o.applies.clone()));
        for g in globs.filter(|g| anchored(g)) {
            let Ok(m) = Glob::new(&g).map(|g| g.compile_matcher()) else {
                continue;
            };
            if !files.iter().any(|f| m.is_match(f)) {
                out.push(
                    DiagB::new(HN_M001, &cp, 0, 0, format!("glob {g} matches no file"))
                        .hint("update the glob after the move, or remove it")
                        .text(g.clone())
                        .build(),
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
    };

    const RULE: &str = "[rules.r]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"no x\"\n";

    #[test]
    fn r_port_002__an_anchored_glob_that_matches_nothing_is_m001() {
        let toml = format!(
            "{RULE}glob = [\"apps/web/**/*.css\"]\n[[convention]]\nglob = \"nowhere/(*)\"\ntransparent = true\n"
        );
        let b = World::new().root(&toml).build();
        let ds = b.run_check(&PathsCheck);
        let d = of(&ds, c::HN_M001);
        assert_eq!(d.len(), 2, "{ds:#?}"); // the rule glob and the convention glob
        assert!(
            d.iter()
                .all(|x| x.location.path.as_str() == "fairlead/control.toml")
        );
    }

    #[test]
    fn r_port_002__floating_globs_and_globs_with_a_match_are_fine() {
        let toml = format!("{RULE}glob = [\"**/*.css\", \"apps/web/**\"]\n");
        let b = World::new()
            .root(&toml)
            .file("apps/web/a.css", "a{}")
            .build();
        assert!(of(&b.run_check(&PathsCheck), c::HN_M001).is_empty());
    }
}
