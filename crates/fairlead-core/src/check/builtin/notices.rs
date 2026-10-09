//! Notices: nothing verified, skipped files (R-CHK-003, R-CHK-057, C-37, A-56).
use crate::{
    check::{CheckCx, builtin::control_path, finding::DiagB, rules::Check},
    diag::{Diagnostic, codes::*},
    registry::ScopeId,
    walk::SkipReason,
};
use fairlead_syntax::diag::DiagCode;

pub struct NoticesCheck;
const CODES: [DiagCode; 3] = [HN_N001, HN_I001, HN_I003];

impl Check for NoticesCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        if scope == reg.root_scope()
            && reg.view(scope).items.is_empty()
            && let Some(cp) = control_path(reg, scope)
        {
            out.push(
                DiagB::new(
                    HN_N001,
                    &cp,
                    0,
                    0,
                    "0 rules declared, 0 IDs: nothing verified",
                )
                .hint("declare a rule or an ID: fairlead new rule, fairlead new <kind>")
                .text("nothing verified")
                .build(),
            );
        }
        for (path, rec) in cx
            .corpus
            .records
            .iter()
            .filter(|(p, _)| reg.scope_of(p) == Some(scope))
        {
            let (code, msg) = match rec.skip {
                Some(SkipReason::TooLarge) => (HN_I001, "file skipped: larger than the scan limit"),
                Some(SkipReason::Binary) => (HN_I001, "file skipped: binary content"),
                Some(SkipReason::SymlinkOrJunction) => {
                    (HN_I001, "symbolic link or junction not followed")
                }
                Some(SkipReason::NotUtf8) => {
                    (HN_I003, "file is not valid UTF-8 and was not scanned")
                }
                Some(SkipReason::Excluded) | None => continue,
            };
            out.push(
                DiagB::new(code, path, 0, 0, msg)
                    .hint("convert to UTF-8, or add the path to [scan] skip")
                    .text(path.as_str())
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
        path::RepoPath,
    };

    #[test]
    fn r_chk_003__zero_rules_and_zero_ids_print_the_nothing_verified_notice() {
        let d = World::new().build().run_check(&NoticesCheck);
        let n = of(&d, c::HN_N001);
        assert_eq!(
            (n.len(), n[0].message.as_str()),
            (1, "0 rules declared, 0 IDs: nothing verified")
        );
        let with_rule = World::new()
            .root("[rules.r]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"no x\"\n")
            .build()
            .run_check(&NoticesCheck);
        assert!(of(&with_rule, c::HN_N001).is_empty());
    }

    #[test]
    fn r_chk_057__skipped_files_are_listed_never_silently_green() {
        let mut b = World::new()
            .file("src/big.bin", "x")
            .file("src/legacy.txt", "x")
            .build();
        let (a, l) = (
            RepoPath::new("src/big.bin").unwrap(),
            RepoPath::new("src/legacy.txt").unwrap(),
        );
        b.corpus.records.get_mut(&a).unwrap().skip = Some(SkipReason::Binary);
        b.corpus.records.get_mut(&l).unwrap().skip = Some(SkipReason::NotUtf8);
        let d = b.run_check(&NoticesCheck);
        assert_eq!((of(&d, c::HN_I001).len(), of(&d, c::HN_I003).len()), (1, 1));
    }
}
