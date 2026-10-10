//! Vendor-owned files against `vendored.lock` (R-FILE-040, R-FILE-041).
use crate::{
    check::{CheckCx, finding::DiagB, rules::Check},
    diag::{Diagnostic, codes::*},
    path::RepoPath,
    registry::ScopeId,
};
use fairlead_syntax::diag::DiagCode;

pub struct VendorCheck;
const CODES: [DiagCode; 3] = [HN_G004, HN_G005, HN_G006];
const ACCEPT: &str = "accept with fairlead sync --accept-vendor";

impl Check for VendorCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        // R-FILE-043: a foreign region's anchor vanished from a file changed since HEAD.
        for (path, id, line) in cx
            .facts
            .foreign_lost
            .iter()
            .filter(|(p, _, _)| cx.registry.scope_of(p) == Some(scope))
        {
            out.push(
                crate::check::finding::DiagB::new(
                    HN_G006,
                    path,
                    *line,
                    1,
                    format!("the {id} region lost its anchors (it was at line {line} in HEAD)"),
                )
                .hint("restore the `def foreign.<tool> +end ... end` comments around the tool's text; the declarations in the managerial folder are unaffected")
                .text(id.clone())
                .build(),
            );
        }
        let (reg, f) = (cx.registry, cx.facts);
        let mine = |p: &RepoPath| reg.scope_of(p) == Some(scope);
        // CI-10: a mismatch git also reports changed is an uncommitted edit, anything else is drift.
        let edited = |p: &RepoPath| f.git.changed.as_ref().is_some_and(|c| c.0.contains(p));
        for (p, actual) in f.vendor_hashes.iter().filter(|(p, _)| mine(p)) {
            let (code, msg, hint) = match f.vendor_lock.get(p) {
                Some(l) if l == actual => continue,
                Some(_) if edited(p) => (
                    HN_G005,
                    "vendor-owned file edited in the working tree",
                    "put variants in a fairlead-owned wrapper unit",
                ),
                Some(_) => (
                    HN_G004,
                    "vendor-owned file changed: hash differs from vendored.lock",
                    ACCEPT,
                ),
                None => (
                    HN_G004,
                    "vendor-owned file is not recorded in vendored.lock",
                    ACCEPT,
                ),
            };
            out.push(
                DiagB::new(code, p, 0, 0, msg)
                    .hint(hint)
                    .text(p.as_str())
                    .build(),
            );
        }
        for p in f
            .vendor_lock
            .keys()
            .filter(|p| mine(p) && !f.vendor_hashes.contains_key(*p))
        {
            out.push(
                DiagB::new(
                    HN_G004,
                    p,
                    0,
                    0,
                    "locked vendor-owned file no longer exists",
                )
                .hint(ACCEPT)
                .text(p.as_str())
                .build(),
            );
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
        facts::{ChangedSet, Facts},
        hash::ContentHash,
    };
    use fairlead_syntax::diag::Severity;

    fn p() -> RepoPath {
        RepoPath::new("ui/button.tsx").unwrap()
    }
    fn hashes(actual: Option<u8>, lock: Option<u8>, changed: bool) -> impl FnOnce(&mut Facts) {
        move |f| {
            if let Some(a) = actual {
                f.vendor_hashes.insert(p(), ContentHash([a; 32]));
            }
            if let Some(l) = lock {
                f.vendor_lock.insert(p(), ContentHash([l; 32]));
            }
            if changed {
                f.git.available = true;
                f.git.changed = Some(ChangedSet([p()].into_iter().collect()));
            }
        }
    }
    fn run(actual: Option<u8>, lock: Option<u8>, changed: bool) -> Vec<Diagnostic> {
        World::new()
            .facts(hashes(actual, lock, changed))
            .build()
            .run_check(&VendorCheck)
    }

    #[test]
    fn r_file_040__r_chk_050__a_changed_hash_is_g004_at_warn() {
        let d = run(Some(1), Some(2), false);
        let g = of(&d, c::HN_G004);
        assert_eq!((g.len(), g[0].severity), (1, Severity::Warn));
    }

    #[test]
    fn r_file_040__unchanged_is_clean_while_unlocked_and_vanished_files_are_drift() {
        assert!(run(Some(1), Some(1), false).is_empty());
        assert_eq!(of(&run(Some(1), None, false), c::HN_G004).len(), 1);
        assert_eq!(of(&run(None, Some(1), false), c::HN_G004).len(), 1);
    }

    #[test]
    fn r_file_041__an_uncommitted_edit_of_a_vendor_file_is_g005() {
        let d = run(Some(1), Some(2), true);
        assert_eq!((of(&d, c::HN_G005).len(), of(&d, c::HN_G004).len()), (1, 0));
    }
}
