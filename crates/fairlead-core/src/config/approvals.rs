//! `<folder>/approvals.toml`: `[[approval]]` records written only by `fairlead approve` (D-34, R-CLI-037).
use crate::config::ConfigFailure;
use crate::config::header::parse_table;
use crate::config::schema::{Ctx, take_date_string, take_string, take_table_array, take_u32};
use crate::diag::sort_diagnostics;
use crate::hash::Fingerprint;
use crate::path::RepoPath;
use fairlead_syntax::ids::{Id, ScopeSlug};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Approval {
    pub fp: Fingerprint,
    pub target: Id,
    pub scope: ScopeSlug,
    pub approver: String,
    pub date: jiff::civil::Date,
    pub renewal: u32,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ApprovalsFile {
    pub approvals: Vec<Approval>,
}

/// Parse the whole file. Any bad record fails the file: a malformed approval must never be read as
/// "not approved" silently. The failure path is the root placeholder; callers use `with_path`.
pub fn parse_approvals(bytes: &[u8]) -> Result<ApprovalsFile, ConfigFailure> {
    let (text, mut table) = parse_table(bytes)?;
    let mut cx = Ctx::new(&RepoPath::root(), &text);
    let mut out = Vec::new();
    for mut t in take_table_array(&mut cx, &mut table, "", "approval") {
        let before = cx.diags.len();
        let fp = take_string(&mut cx, &mut t, "approval", "fp");
        let target = take_string(&mut cx, &mut t, "approval", "target");
        let scope = take_string(&mut cx, &mut t, "approval", "scope");
        let approver = take_string(&mut cx, &mut t, "approval", "approver");
        let date = take_date_string(&mut cx, &mut t, "approval", "date");
        let renewal = take_u32(&mut cx, &mut t, "approval", "renewal").unwrap_or(0);
        let (Some(fp), Some(target), Some(scope), Some(approver), Some(date)) =
            (fp, target, scope, approver, date)
        else {
            if cx.diags.len() == before {
                cx.error(
                    "approval",
                    "an [[approval]] needs fp, target, scope, approver and date",
                );
            }
            continue;
        };
        let fp = Fingerprint::parse(&fp)
            .map_err(|e| cx.error("approval.fp", format!("invalid fingerprint {fp:?}: {e}")))
            .ok();
        let target = Id::parse(&target)
            .map_err(|e| {
                cx.error(
                    "approval.target",
                    format!("invalid target {target:?}: {e:?}"),
                )
            })
            .ok();
        let scope = ScopeSlug::parse(&scope)
            .map_err(|e| cx.error("approval.scope", format!("invalid scope {scope:?}: {e:?}")))
            .ok();
        let date = date.parse::<jiff::civil::Date>().ok();
        if approver.trim().is_empty() {
            cx.error("approval.approver", "approver must not be empty");
        }
        if let (Some(fp), Some(target), Some(scope), Some(date)) = (fp, target, scope, date)
            && !approver.trim().is_empty()
        {
            out.push(Approval {
                fp,
                target,
                scope,
                approver,
                date,
                renewal,
            });
        }
    }
    if cx.diags.is_empty() {
        Ok(ApprovalsFile { approvals: out })
    } else {
        sort_diagnostics(&mut cx.diags);
        Err(ConfigFailure {
            path: RepoPath::root(),
            diags: cx.diags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    const TWO: &str = r#"[[approval]]
fp = "0123456789ab"
target = "rule.no-raw-hex"
scope = "web"
approver = "@dana"
date = 2026-10-09
renewal = 1

[[approval]]
fp = "zzzzzzzzzzzz"
target = "web:exc.x1"
scope = "root"
approver = "@lee"
date = "2026-10-10"
"#;

    #[test]
    fn r_cli_037__approvals_parse_in_file_order() {
        let f = parse_approvals(TWO.as_bytes()).unwrap();
        assert_eq!(f.approvals.len(), 2);
        let a = &f.approvals[0];
        assert_eq!(a.fp.as_str(), "0123456789ab");
        assert_eq!(a.target.to_string(), "rule.no-raw-hex");
        assert_eq!(
            (a.scope.as_str(), a.approver.as_str(), a.renewal),
            ("web", "@dana", 1)
        );
        assert_eq!(a.date.to_string(), "2026-10-09");
        let b = &f.approvals[1];
        assert_eq!((b.renewal, b.date.to_string().as_str()), (0, "2026-10-10"));
        assert_eq!(b.target.to_string(), "web:exc.x1");
    }

    #[test]
    fn r_cli_037__empty_file_has_no_approvals() {
        assert!(parse_approvals(b"").unwrap().approvals.is_empty());
        assert!(
            parse_approvals(b"# nothing yet\n")
                .unwrap()
                .approvals
                .is_empty()
        );
    }

    #[test]
    fn r_cli_037__bad_records_fail_closed_with_hc001() {
        for bad in [
            "[[approval]]\nfp = \"short\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"nodot\"\nscope = \"a\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"Bad Scope\"\napprover = \"@a\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"\"\ndate = 2026-01-01\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\ndate = \"soon\"\n",
            "[[approval]]\nfp = \"0123456789ab\"\ntarget = \"rule.x\"\nscope = \"a\"\napprover = \"@a\"\n",
            "approval = 5\n",
            "[[approval]\n",
        ] {
            let f = parse_approvals(bad.as_bytes()).unwrap_err();
            assert!(f.diags.iter().all(|d| d.code == codes::HN_C001), "{bad:?}");
            assert!(!f.diags.is_empty(), "{bad:?}");
        }
    }

    #[test]
    fn r_cli_037__crlf_and_bom_are_accepted() {
        let text = format!("\u{feff}{}", TWO.replace('\n', "\r\n"));
        assert_eq!(parse_approvals(text.as_bytes()).unwrap().approvals.len(), 2);
    }
}
