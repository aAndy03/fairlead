//! `fairlead approve` (R-CLI-037..039).
use super::new::{directive_value, op_word};
use super::writeset::{PlannedWrite, WriteKind, WriteSet, WriteSetKind};
use super::{Plan, PlanAction, PlanLine, refuse, repo_path};
use crate::classify::FileClass;
use crate::config::{Approval, ApprovalsFile, Owner, OwnerKind};
use crate::error::{Error, RefusalKind};
use crate::facts::Facts;
use crate::hash::Fingerprint;
use crate::registry::{ApprovalStatus, FieldChange, FieldValue, Override, Registry, ScopeId};
use crate::render::sanitize::sanitize_data;
use crate::time::AsOf;
use fairlead_syntax::ids::{Id, IdQualifier};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ApproveRequest {
    pub target: String,
    pub as_handle: Option<String>,
    pub list: bool,
    pub revoke: Option<Fingerprint>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ApprovePreview {
    pub fp: Fingerprint,
    /// A-21: the overridden item.
    pub target: Id,
    pub ovr_line: String,
    pub scope: ScopeId,
    pub exp: Option<jiff::civil::Date>,
    pub confirm_prefix: String,
}

const DEFAULT_MAX_RENEWALS: u32 = 2;

pub(crate) fn select_override<'a>(
    overrides: &'a [Override],
    target: &str,
) -> Result<&'a Override, Error> {
    if let Ok(fp) = Fingerprint::parse(target) {
        return overrides
            .iter()
            .find(|o| o.id.0 == fp)
            .ok_or_else(|| Error::Usage(format!("no override with fingerprint {target}")));
    }
    let id = Id::parse(target)
        .map_err(|_| Error::Usage(format!("{target:?} is neither a fingerprint nor an ID")))?;
    if id.kind_name().starts_with("change.") {
        return Err(Error::Reserved {
            verb: "approve <change doc>",
            milestone: "0.2",
        });
    }
    let bare = matches!(id.qualifier, IdQualifier::Bare);
    let hits: Vec<&Override> = overrides
        .iter()
        .filter(|o| o.approval == ApprovalStatus::Missing)
        .filter(|o| {
            if bare {
                o.target.to_id().kind_name() == id.kind_name()
            } else {
                o.target.to_id() == id
            }
        })
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(Error::Usage(format!("no pending override on {target}"))),
        many => Err(Error::Usage(format!(
            "{} pending overrides on {target}; approve one fingerprint: {}",
            many.len(),
            many.iter()
                .map(|o| o.id.0.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

pub(crate) fn preview_of(o: &Override, already: bool) -> ApprovePreview {
    let mut line = format!("ovr {} op={}", o.target.to_id(), op_word(o.op));
    for (k, c) in &o.set {
        let v = match c {
            FieldChange::Replace(FieldValue::Scalar(s)) => s.clone(),
            FieldChange::Replace(FieldValue::List(l)) => l.join(","),
            FieldChange::Append(l) => format!("+{}", l.join(",")),
        };
        line.push_str(&format!(" {k}={}", directive_value(&v)));
    }
    if let Some(w) = &o.why {
        line.push_str(&format!(" why={}", directive_value(w)));
    }
    if let Some(w) = &o.owner {
        line.push_str(&format!(" owner={}", directive_value(w)));
    }
    if let Some(d) = o.exp {
        line.push_str(&format!(" exp={d}"));
    }
    if already {
        line.push_str(" (already approved)");
    }
    ApprovePreview {
        target: o.target.to_id(),
        fp: o.id.0.clone(),
        ovr_line: sanitize_data(&line, 240),
        scope: o.declared_in.clone(),
        exp: o.exp,
        confirm_prefix: o.id.0.as_str().chars().take(4).collect(),
    }
}

pub fn approve_preview(
    req: &ApproveRequest,
    reg: &Registry,
    facts: &Facts,
) -> Result<ApprovePreview, Error> {
    let o = select_override(reg.overrides(), &req.target)?;
    Ok(preview_of(
        o,
        facts.approvals.approvals.iter().any(|a| a.fp == o.id.0),
    ))
}

/// `--as` must be an `owners` handle (R-AGT-028); without it a single human owner is the default.
pub fn resolve_approver(req: &ApproveRequest, reg: &Registry) -> Result<String, Error> {
    let owners: Vec<Owner> = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .map(|c| c.settings.owners.clone())
        .unwrap_or_default();
    match &req.as_handle {
        Some(h) if owners.iter().any(|o| &o.handle == h) => Ok(h.clone()),
        Some(h) => Err(refuse(
            RefusalKind::Floor,
            format!("{h} is not listed in owners"),
            "`fairlead approve --as` accepts only a handle from the root [settings] owners (R-AGT-028)",
        )),
        None => match owners
            .iter()
            .filter(|o| o.kind == OwnerKind::Human)
            .collect::<Vec<_>>()
            .as_slice()
        {
            [one] => Ok(one.handle.clone()),
            _ => Err(Error::Usage(
                "pass --as @handle: the root owners list does not name exactly one human".into(),
            )),
        },
    }
}

/// TTY present and `CI` unset, nothing stronger (R-AGT-026); the bin fills the booleans.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ApproveGate {
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub ci_set: bool,
}

pub fn approve_gate(g: &ApproveGate, p: &ApprovePreview, typed: &str) -> Result<(), Error> {
    if g.ci_set {
        return Err(refuse(
            RefusalKind::CiSet,
            "CI is set; approve is for humans at a terminal",
            "run `fairlead approve` from an interactive shell",
        ));
    }
    if !g.stdin_tty || !g.stdout_tty {
        return Err(refuse(
            RefusalKind::NotATty,
            "stdin and stdout must be a terminal",
            "run `fairlead approve` from an interactive shell",
        ));
    }
    if typed != p.confirm_prefix {
        return Err(refuse(
            RefusalKind::PlanNotConfirmed,
            "the typed prefix does not match",
            format!("retype the first 4 characters: {}", p.confirm_prefix),
        ));
    }
    Ok(())
}

fn approval_block(
    fp: &Fingerprint,
    target: &Id,
    scope: &str,
    approver: &str,
    date: jiff::civil::Date,
    renewal: u32,
) -> String {
    let mut t = toml_edit::Table::new();
    t["fp"] = toml_edit::value(fp.as_str());
    t["target"] = toml_edit::value(target.to_string());
    t["scope"] = toml_edit::value(scope);
    t["approver"] = toml_edit::value(approver);
    t["date"] = toml_edit::value(date.to_string());
    t["renewal"] = toml_edit::value(i64::from(renewal));
    let mut aot = toml_edit::ArrayOfTables::new();
    aot.push(t);
    let mut doc = toml_edit::DocumentMut::new();
    doc.insert("approval", toml_edit::Item::ArrayOfTables(aot));
    format!("\n{doc}")
}

fn one_write(
    folder: &str,
    kind: WriteKind,
    text: &str,
    line: PlanLine,
    notes: Vec<String>,
) -> Result<Plan, Error> {
    let mut set = WriteSet::new(WriteSetKind::Approve);
    set.add(PlannedWrite {
        path: repo_path(&format!("{folder}/approvals.toml"))?,
        kind,
        bytes: text.as_bytes().to_vec(),
        class: FileClass::Authored,
        expect_prior: None,
    })?;
    Ok(Plan::new(set, vec![line], notes))
}

/// A-21: appends one `[[approval]]` to `<folder>/approvals.toml`.
pub fn approve_plan(
    folder: &str,
    p: &ApprovePreview,
    approver: &str,
    as_of: AsOf,
    existing: &ApprovalsFile,
) -> Result<Plan, Error> {
    let path = repo_path(&format!("{folder}/approvals.toml"))?;
    if existing.approvals.iter().any(|a| a.fp == p.fp) {
        let line = PlanLine {
            action: PlanAction::Skip,
            path,
            detail: "already approved (no-op)".into(),
            diff: None,
        };
        return Ok(Plan::new(
            WriteSet::new(WriteSetKind::Approve),
            vec![line],
            vec![format!("{} is already approved", p.fp.as_str())],
        ));
    }
    let target = p.target.clone();
    let renewal = existing
        .approvals
        .iter()
        .filter(|a| a.target == target && a.scope.as_str() == p.scope.0.as_str())
        .count() as u32;
    let text = approval_block(
        &p.fp,
        &target,
        p.scope.0.as_str(),
        approver,
        as_of.0,
        renewal,
    );
    let mut notes = Vec::new();
    if renewal > 0 {
        notes.push(format!(
            "renewal {renewal} of {target} in {}",
            p.scope.0.as_str()
        ));
    }
    if renewal > DEFAULT_MAX_RENEWALS {
        notes.push(format!("renewal {renewal} exceeds max-renewals {DEFAULT_MAX_RENEWALS}: root owner review expected (R-CHK-027)"));
    }
    let line = PlanLine {
        action: PlanAction::Edit,
        path,
        detail: format!("approve {}", p.fp.as_str()),
        diff: Some(super::plain_diff("", &text, 20)),
    };
    one_write(
        folder,
        WriteKind::AppendBlock {
            marker: String::new(),
        },
        &text,
        line,
        notes,
    )
}

pub fn approve_list(existing: &ApprovalsFile) -> Vec<String> {
    existing
        .approvals
        .iter()
        .map(|a: &Approval| {
            format!(
                "{} target={} scope={} approver={} date={} renewal={}",
                a.fp.as_str(),
                a.target,
                a.scope.as_str(),
                a.approver,
                a.date,
                a.renewal
            )
        })
        .collect()
}

pub fn revoke_plan(
    folder: &str,
    fp: &Fingerprint,
    existing: &ApprovalsFile,
) -> Result<Plan, Error> {
    if !existing.approvals.iter().any(|a| a.fp == *fp) {
        return Err(Error::Usage(format!(
            "no approval with fingerprint {}",
            fp.as_str()
        )));
    }
    let rest: String = existing
        .approvals
        .iter()
        .filter(|a| a.fp != *fp)
        .map(|a| {
            approval_block(
                &a.fp,
                &a.target,
                a.scope.as_str(),
                &a.approver,
                a.date,
                a.renewal,
            )
        })
        .collect();
    let text = if rest.is_empty() {
        "# no approvals recorded\n".to_owned()
    } else {
        rest.trim_start_matches('\n').to_owned()
    };
    let line = PlanLine {
        action: PlanAction::Edit,
        path: repo_path(&format!("{folder}/approvals.toml"))?,
        detail: format!("revoke {}", fp.as_str()),
        diff: None,
    };
    one_write(
        folder,
        WriteKind::Replace,
        &text,
        line,
        vec!["approvals.toml is re-rendered without the revoked record".into()],
    )
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::config::parse_approvals;
    use crate::registry::{ApprovalStatus, OvrId};
    use crate::write::testutil::*;
    use crate::write::{WriteKind, append_plain};
    use fairlead_syntax::ids::ScopeSlug;

    fn fp(s: &str) -> Fingerprint {
        Fingerprint::parse(s).unwrap()
    }
    fn tgt() -> String {
        item_id("root", "rule.no-raw-hex").to_id().to_string()
    }
    fn ap(f: &str, scope: &str, renewal: u32) -> Approval {
        Approval {
            fp: fp(f),
            target: Id::parse(&tgt()).unwrap(),
            scope: ScopeSlug::parse(scope).unwrap(),
            approver: "@lead".into(),
            date: jiff::civil::date(2026, 10, 1),
            renewal,
        }
    }
    fn file(v: Vec<Approval>) -> ApprovalsFile {
        ApprovalsFile { approvals: v }
    }
    fn pending(f: &str) -> Override {
        let mut o = ovr_fixture("apps/web/a.tsx", 3, "chart lib");
        o.id = OvrId(fp(f));
        o
    }
    fn today() -> AsOf {
        AsOf::parse("2026-10-09").unwrap()
    }

    #[test]
    fn r_cli_037__preview_shows_the_ovr_line_scope_expiry_and_a_four_char_prefix() {
        let p = preview_of(&pending("k3m9q2x7a1bz"), false);
        assert_eq!(
            p.ovr_line,
            format!(
                "ovr {} op=loosen sev=warn why=\"chart lib\" owner=@lead exp=2026-12-31",
                tgt()
            )
        );
        assert_eq!(
            (p.confirm_prefix.as_str(), p.scope.0.as_str(), p.exp),
            ("k3m9", "web", Some(jiff::civil::date(2026, 12, 31)))
        );
        assert!(
            preview_of(&pending("k3m9q2x7a1bz"), true)
                .ovr_line
                .ends_with("(already approved)")
        );
    }

    #[test]
    fn r_cli_037__targets_resolve_by_fingerprint_or_by_the_single_pending_id() {
        let (a, b) = (pending("aaaaaaaaaaaa"), pending("bbbbbbbbbbbb"));
        assert_eq!(
            select_override(std::slice::from_ref(&a), "rule.no-raw-hex")
                .unwrap()
                .id
                .0,
            fp("aaaaaaaaaaaa")
        );
        assert!(
            matches!(select_override(&[a.clone(), b.clone()], "rule.no-raw-hex"), Err(Error::Usage(m)) if m.contains("aaaaaaaaaaaa") && m.contains("bbbbbbbbbbbb"))
        );
        assert!(select_override(&[a.clone(), b], "bbbbbbbbbbbb").is_ok());
        assert!(matches!(
            select_override(std::slice::from_ref(&a), "cccccccccccc"),
            Err(Error::Usage(_))
        ));
        let mut done = a.clone();
        done.approval = ApprovalStatus::Approved { renewal: 0 };
        assert!(
            matches!(select_override(&[done], "rule.no-raw-hex"), Err(Error::Usage(m)) if m.contains("no pending"))
        );
        assert!(matches!(
            select_override(&[a], "change.x"),
            Err(Error::Reserved {
                milestone: "0.2",
                ..
            })
        ));
    }

    #[test]
    fn r_cli_037__gate_refuses_ci_then_non_tty_then_a_wrong_retype() {
        let p = preview_of(&pending("k3m9q2x7a1bz"), false);
        let g = |stdin_tty, stdout_tty, ci_set| ApproveGate {
            stdin_tty,
            stdout_tty,
            ci_set,
        };
        let kind = |r: Result<(), Error>| match r {
            Err(Error::Refused(f)) => Some(f.kind),
            _ => None,
        };
        assert_eq!(
            kind(approve_gate(&g(true, true, true), &p, "k3m9")),
            Some(RefusalKind::CiSet)
        );
        assert_eq!(
            kind(approve_gate(&g(false, true, false), &p, "k3m9")),
            Some(RefusalKind::NotATty)
        );
        assert_eq!(
            kind(approve_gate(&g(true, false, false), &p, "k3m9")),
            Some(RefusalKind::NotATty)
        );
        assert_eq!(
            kind(approve_gate(&g(true, true, false), &p, "zzzz")),
            Some(RefusalKind::PlanNotConfirmed)
        );
        assert!(approve_gate(&g(true, true, false), &p, "k3m9").is_ok());
    }

    #[test]
    fn r_cli_037__plan_appends_one_approval_table_and_keeps_existing_text() {
        let (root, _reg) = registry(&[("fairlead/control.toml", ROOT_CONTROL)], "2026-10-09");
        let p = preview_of(&pending("k3m9q2x7a1bz"), false);
        let plan = approve_plan("fairlead", &p, "@lead", today(), &file(vec![])).unwrap();
        let w = &plan.set.writes[&rp("fairlead/approvals.toml")];
        assert_eq!(
            w.kind,
            WriteKind::AppendBlock {
                marker: String::new()
            }
        );
        plan.set.validate(&root).unwrap();
        let existing = b"# reviewed by hand\n".to_vec();
        let after = append_plain(&existing, &String::from_utf8_lossy(&w.bytes));
        assert!(after.starts_with(&existing));
        let parsed = parse_approvals(&after).unwrap();
        let a = &parsed.approvals[0];
        assert_eq!(
            (
                a.fp.clone(),
                a.approver.as_str(),
                a.renewal,
                a.scope.as_str()
            ),
            (fp("k3m9q2x7a1bz"), "@lead", 0, "web")
        );
        assert_eq!(a.date, jiff::civil::date(2026, 10, 9));
        assert_eq!(
            parse_approvals(&append_plain(&after, &String::from_utf8_lossy(&w.bytes)))
                .unwrap()
                .approvals
                .len(),
            1,
            "re-append is idempotent"
        );
    }

    #[test]
    fn r_cli_038__same_fingerprint_is_a_noop_and_a_new_fingerprint_for_the_same_target_is_a_renewal()
     {
        let p = preview_of(&pending("aaaaaaaaaaaa"), false);
        let noop = approve_plan(
            "fairlead",
            &p,
            "@lead",
            today(),
            &file(vec![ap("aaaaaaaaaaaa", "web", 0)]),
        )
        .unwrap();
        assert!(noop.is_noop());
        let renewal = approve_plan(
            "fairlead",
            &p,
            "@lead",
            today(),
            &file(vec![ap("cccccccccccc", "web", 0)]),
        )
        .unwrap();
        assert!(
            String::from_utf8_lossy(&renewal.set.writes[&rp("fairlead/approvals.toml")].bytes)
                .contains("renewal = 1")
        );
        assert!(renewal.notes.iter().any(|n| n.contains("renewal 1")));
        let other_scope = approve_plan(
            "fairlead",
            &p,
            "@lead",
            today(),
            &file(vec![ap("cccccccccccc", "ui", 0)]),
        )
        .unwrap();
        assert!(
            String::from_utf8_lossy(&other_scope.set.writes[&rp("fairlead/approvals.toml")].bytes)
                .contains("renewal = 0")
        );
    }

    #[test]
    fn r_chk_027__the_third_renewal_asks_for_the_root_owner() {
        let p = preview_of(&pending("aaaaaaaaaaaa"), false);
        let history = file(vec![
            ap("cccccccccccc", "web", 0),
            ap("dddddddddddd", "web", 1),
            ap("eeeeeeeeeeee", "web", 2),
        ]);
        let plan = approve_plan("fairlead", &p, "@lead", today(), &history).unwrap();
        assert!(
            plan.notes
                .iter()
                .any(|n| n.contains("exceeds max-renewals 2") && n.contains("root owner")),
            "{:?}",
            plan.notes
        );
        assert!(
            !approve_plan(
                "fairlead",
                &p,
                "@lead",
                today(),
                &file(vec![ap("cccccccccccc", "web", 0)])
            )
            .unwrap()
            .notes
            .iter()
            .any(|n| n.contains("exceeds"))
        );
    }

    #[test]
    fn r_agt_028__as_accepts_only_a_handle_listed_in_owners() {
        let (_r, reg) = registry(&[("fairlead/control.toml", ROOT_CONTROL)], "2026-10-09");
        let rq = |h: Option<&str>| ApproveRequest {
            target: "x".into(),
            as_handle: h.map(Into::into),
            list: false,
            revoke: None,
        };
        assert_eq!(resolve_approver(&rq(Some("@lead")), &reg).unwrap(), "@lead");
        assert!(matches!(
            resolve_approver(&rq(Some("@mallory")), &reg),
            Err(Error::Refused(_))
        ));
        assert_eq!(
            resolve_approver(&rq(None), &reg).unwrap(),
            "@lead",
            "a single human owner is the default"
        );
    }

    #[test]
    fn r_cli_037__list_is_stable_and_revoke_rewrites_the_remaining_records() {
        let two = file(vec![
            ap("aaaaaaaaaaaa", "web", 0),
            ap("bbbbbbbbbbbb", "web", 1),
        ]);
        let l = approve_list(&two);
        assert_eq!(l.len(), 2);
        assert!(l[0].starts_with("aaaaaaaaaaaa target=") && l[1].ends_with("renewal=1"));
        let plan = revoke_plan("fairlead", &fp("aaaaaaaaaaaa"), &two).unwrap();
        let w = &plan.set.writes[&rp("fairlead/approvals.toml")];
        assert_eq!(w.kind, WriteKind::Replace);
        let left = parse_approvals(&w.bytes).unwrap();
        assert_eq!(left.approvals.len(), 1);
        assert_eq!(left.approvals[0].fp, fp("bbbbbbbbbbbb"));
        assert!(matches!(
            revoke_plan("fairlead", &fp("cccccccccccc"), &two),
            Err(Error::Usage(_))
        ));
        let none = revoke_plan(
            "fairlead",
            &fp("bbbbbbbbbbbb"),
            &file(vec![ap("bbbbbbbbbbbb", "web", 0)]),
        )
        .unwrap();
        assert_eq!(
            parse_approvals(&none.set.writes[&rp("fairlead/approvals.toml")].bytes)
                .unwrap()
                .approvals
                .len(),
            0
        );
    }
}
