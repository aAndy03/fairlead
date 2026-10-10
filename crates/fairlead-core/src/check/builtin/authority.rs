//! Signed authority records (R-LC-040): with `authority.signed-records = true`, `check --ci`
//! reports an authority record whose last commit has no good signature (`H-AUTH-UNSIGNED`,
//! HN-X015). The verification itself ran in the load stage; this check only reads its result.
use fairlead_syntax::diag::DiagCode;

use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::registry::ScopeId;

pub struct AuthorityCheck;
const CODES: [DiagCode; 1] = [HN_X015];

impl Check for AuthorityCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        for r in cx
            .facts
            .authority
            .iter()
            .filter(|r| !r.signed && cx.registry.scope_of(&r.path) == Some(scope))
        {
            let msg = match &r.commit {
                Some(c) => format!("authority record written by unsigned commit {c}"),
                None => "authority record is not committed, so it carries no signature".into(),
            };
            out.push(
                DiagB::new(HN_X015, &r.path, 1, 1, msg)
                    .hint("sign the commit (git commit -S); CODEOWNERS review stays the baseline (R-LC-038)")
                    .text(r.path.as_str().to_string())
                    .build(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;
    use crate::facts::AuthorityRecord;
    use crate::path::RepoPath;

    #[test]
    fn r_lc_040__unsigned_record_is_reported_signed_is_not() {
        let ds = World::new()
            .facts(|f| {
                f.authority = vec![
                    AuthorityRecord {
                        path: RepoPath::new("fairlead/approvals.toml").unwrap(),
                        commit: Some("0123456789".into()),
                        signed: false,
                    },
                    AuthorityRecord {
                        path: RepoPath::new("fairlead/docs/graveyard/x.md").unwrap(),
                        commit: Some("abcdef0123".into()),
                        signed: true,
                    },
                ];
            })
            .build()
            .run_check(&AuthorityCheck);
        let x = of(&ds, c::HN_X015);
        assert_eq!(x.len(), 1);
        assert!(x[0].message.contains("unsigned commit 0123456789"));
    }
}
