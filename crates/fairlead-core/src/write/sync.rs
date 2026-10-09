//! `fairlead sync` (R-CLI-025..027).
use super::new::{folder_of, under};
use super::writeset::{PlannedWrite, WriteKind, WriteSet, WriteSetKind};
use super::{Plan, PlanAction, PlanLine, plain_diff, quarantine_path};
use crate::classify::FileClass;
use crate::discover::Edition;
use crate::error::Error;
use crate::facts::Facts;
use crate::generate::{GenKind, GeneratedFile, GeneratedSet, HeaderVerdict, verify_header};
use crate::hash::hash_normalised;
use crate::registry::{Registry, ScopeId};
use std::collections::BTreeSet;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BaselineMode {
    Off,
    Prune,
    AcceptNew { why: String },
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SyncRequest {
    pub baseline: BaselineMode,
    pub accept_vendor: bool,
    pub force: bool,
    pub scope: Option<ScopeId>,
}

fn cone(reg: &Registry, s: &ScopeId) -> Result<BTreeSet<ScopeId>, Error> {
    if reg.scope(s).is_none() {
        return Err(Error::Usage(format!("unknown scope {}", s.0.as_str())));
    }
    Ok(reg
        .scopes()
        .filter(|sc| reg.chain(&sc.id).contains(&s))
        .map(|sc| sc.id.clone())
        .collect())
}

/// R-SCOPE-044, R-FILE-021: a scope's `gen/` is written only from that scope's own facts.
fn own_facts(reg: &Registry, folder: &str, g: &GeneratedFile) -> Result<(), Error> {
    let sc = reg
        .scope(&g.scope)
        .ok_or_else(|| Error::Internal(format!("{} names an unknown scope", g.path.as_str())))?;
    let prefix = under(sc.dir.as_str(), &format!("{folder}/"));
    let rest = g.path.as_str().strip_prefix(prefix.as_str());
    if rest.is_some_and(|r| r.starts_with("gen/") || r == "vendored.lock") {
        Ok(())
    } else {
        Err(Error::Internal(format!(
            "{} is not inside scope {}'s own {folder}/gen/ (R-SCOPE-044)",
            g.path.as_str(),
            g.scope.0.as_str()
        )))
    }
}

/// Added and removed non-header lines of a membership shard, capped at 20 (R-PORT-001).
pub(crate) fn members_diff(old: &str, new: &str) -> Vec<String> {
    let body = |t: &str| -> BTreeSet<String> {
        t.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (o, n) = (body(old), body(new));
    let mut out: Vec<String> = o
        .difference(&n)
        .map(|l| format!("view: -{l}"))
        .chain(n.difference(&o).map(|l| format!("view: +{l}")))
        .collect();
    out.sort();
    if out.len() > 20 {
        let extra = out.len() - 20;
        out.truncate(20);
        out.push(format!("view: +{extra} more changes"));
    }
    out
}

pub fn sync_plan_from(
    req: &SyncRequest,
    reg: &Registry,
    generated: &GeneratedSet,
    facts: &Facts,
    edition: Edition,
) -> Result<Plan, Error> {
    let folder = folder_of(reg);
    let cone = req.scope.as_ref().map(|s| cone(reg, s)).transpose()?;
    let mut regular: Vec<(PlannedWrite, PlanLine)> = Vec::new();
    let mut quarantined: Vec<(PlannedWrite, PlanLine)> = Vec::new();
    let mut hand_edited = 0usize;
    let (mut notes, mut unchanged) = (Vec::new(), 0usize);
    let replace = |g: &GeneratedFile, want: Vec<u8>, have: &[u8]| -> (PlannedWrite, PlanLine) {
        let diff = plain_diff(
            &String::from_utf8_lossy(have),
            &String::from_utf8_lossy(&want),
            40,
        );
        (
            PlannedWrite {
                path: g.path.clone(),
                kind: WriteKind::Replace,
                bytes: want,
                class: FileClass::Generated,
                expect_prior: Some(hash_normalised(have)),
            },
            PlanLine {
                action: PlanAction::Edit,
                path: g.path.clone(),
                detail: "regenerated".into(),
                diff: Some(diff),
            },
        )
    };
    for (path, g) in &generated.files {
        if cone.as_ref().is_some_and(|c| !c.contains(&g.scope)) {
            continue;
        }
        own_facts(reg, &folder, g)?;
        let want = g.render(Edition(edition.0));
        let Some(have) = facts.generated_on_disk.get(path) else {
            let diff = plain_diff("", &String::from_utf8_lossy(&want), 40);
            regular.push((
                PlannedWrite {
                    path: path.clone(),
                    kind: WriteKind::Create,
                    bytes: want,
                    class: FileClass::Generated,
                    expect_prior: None,
                },
                PlanLine {
                    action: PlanAction::Create,
                    path: path.clone(),
                    detail: "new generated file".into(),
                    diff: Some(diff),
                },
            ));
            continue;
        };
        let have: &[u8] = have.as_ref();
        if have == want.as_slice() {
            unchanged += 1;
            continue;
        }
        if matches!(g.kind, GenKind::Members) {
            notes.extend(members_diff(
                &String::from_utf8_lossy(have),
                &String::from_utf8_lossy(&want),
            ));
        }
        match verify_header(have) {
            HeaderVerdict::Ok | HeaderVerdict::HasConflictMarkers => {
                regular.push(replace(g, want, have))
            }
            HeaderVerdict::HandEdited => {
                hand_edited += 1;
                let q = quarantine_path(path, have)?;
                let diff = plain_diff(
                    &String::from_utf8_lossy(have),
                    &String::from_utf8_lossy(&want),
                    40,
                ); // `-` lines are what would be lost
                quarantined.push((
                    PlannedWrite {
                        path: q.clone(),
                        kind: WriteKind::Create,
                        bytes: have.to_vec(),
                        class: FileClass::Authored,
                        expect_prior: None,
                    },
                    PlanLine {
                        action: PlanAction::Quarantine,
                        path: path.clone(),
                        detail: format!("hand-edited; copy saved to {}", q.as_str()),
                        diff: Some(diff),
                    },
                ));
                if req.force {
                    regular.push(replace(g, want, have));
                }
            }
        }
    }
    let mut set = WriteSet::new(WriteSetKind::Sync);
    let mut lines = Vec::new();
    let apply_regular = hand_edited == 0 || req.force;
    for (w, l) in quarantined {
        set.add(w)?;
        lines.push(l);
    }
    for (w, l) in regular {
        if apply_regular {
            set.add(w)?;
            lines.push(l);
        } else {
            lines.push(PlanLine {
                action: PlanAction::Skip,
                path: l.path,
                detail: "not applied: sync refused (hand-edited file)".into(),
                diff: None,
            });
        }
    }
    if unchanged > 0 {
        notes.push(format!("{unchanged} generated file(s) unchanged"));
    }
    if req.scope.is_none() {
        let (reg_dir, members) = (
            format!("{folder}/gen/reg/"),
            format!("{folder}/gen/members"),
        );
        for p in facts
            .generated_on_disk
            .keys()
            .filter(|p| !generated.files.contains_key(*p))
        {
            if p.as_str().contains(&reg_dir) || p.as_str().ends_with(&members) {
                notes.push(format!(
                    "orphan generated file {} (sync does not delete in 0.1)",
                    p.as_str()
                ));
            }
        }
    }
    Ok(Plan::new(set, lines, notes))
}

use super::refuse;
use crate::check::baseline::Baseline;
use crate::check::{CheckInput, CheckReport, Finding, FindingState};
use crate::config::ApprovalsFile;
use crate::error::RefusalKind;
use crate::generate::generate;
use crate::time::AsOf;

pub(crate) fn baseline_for(
    mode: &BaselineMode,
    current: Option<&Baseline>,
    findings: &[Finding],
    as_of: AsOf,
    approvals: &ApprovalsFile,
) -> Result<Option<Baseline>, Error> {
    match mode {
        BaselineMode::Off => Ok(None),
        BaselineMode::Prune => Ok(current.map(|b| b.prune(findings))),
        BaselineMode::AcceptNew { why } => {
            if why.trim().is_empty() {
                return Err(Error::Usage("--accept-new needs --why TEXT".into()));
            }
            let base = current.cloned().unwrap_or(Baseline {
                entries: Default::default(),
            });
            let new: Vec<Finding> = findings
                .iter()
                .filter(|f| f.state == FindingState::New)
                .cloned()
                .collect();
            base.accept_new(&new, why, as_of, approvals).map(Some).map_err(|r| {
                refuse(RefusalKind::Floor, format!("baseline growth refused: {r:?}"),
                       "floor findings are never accepted; authority-family findings need a `fairlead approve` record (R-CHK-052)")
            })
        }
    }
}

/// Facts as `generate` should see them: recorded vendor hashes survive unless `--accept-vendor`.
pub(crate) fn effective_facts(accept_vendor: bool, facts: &Facts) -> Facts {
    let mut f = facts.clone();
    if !accept_vendor {
        for (p, h) in f.vendor_hashes.iter_mut() {
            if let Some(rec) = facts.vendor_lock.get(p) {
                *h = *rec;
            }
        }
        for (p, rec) in &facts.vendor_lock {
            f.vendor_hashes.entry(p.clone()).or_insert_with(|| *rec);
        }
    }
    f
}

pub(crate) fn vendor_notes(facts: &Facts, accept: bool) -> Vec<String> {
    facts
        .vendor_hashes
        .iter()
        .filter(|(p, h)| facts.vendor_lock.get(*p).is_some_and(|rec| rec != *h))
        .map(|(p, _)| {
            if accept {
                format!("accepted vendor hash: {}", p.as_str())
            } else {
                format!(
                    "vendor file changed (HN-G004): {}; accept with --accept-vendor",
                    p.as_str()
                )
            }
        })
        .collect()
}

pub fn sync_plan(
    req: &SyncRequest,
    input: &CheckInput<'_>,
    report: &CheckReport,
    edition: Edition,
) -> Result<Plan, Error> {
    let as_of = AsOf(input.options.as_of.0);
    let baseline = baseline_for(
        &req.baseline,
        input.baseline,
        &report.findings,
        as_of,
        &input.facts.approvals,
    )?;
    let facts = effective_facts(req.accept_vendor, input.facts);
    let generated = generate(
        input.registry,
        input.corpus,
        &facts,
        baseline.as_ref(),
        Edition(edition.0),
    );
    let mut plan = sync_plan_from(req, input.registry, &generated, &facts, edition)?;
    match (&req.baseline, input.baseline, &baseline) {
        (BaselineMode::Prune, None, _) => plan.notes.push(
            "no baseline to prune (create one with `fairlead init` or `--accept-new --why`)".into(),
        ),
        (BaselineMode::AcceptNew { .. }, cur, Some(grown)) => {
            let added = grown
                .entries
                .len()
                .saturating_sub(cur.map_or(0, |b| b.entries.len()));
            plan.notes.push(format!(
                "baseline: accepted {added} new finding(s) with acc={}",
                as_of.0
            ));
        }
        _ => {}
    }
    plan.notes
        .extend(vendor_notes(input.facts, req.accept_vendor));
    Ok(plan)
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::error::{Error, RefusalKind};
    use crate::generate::{GenKind, GeneratedFile};
    use crate::write::testutil::*;
    use crate::write::{PlanAction, WriteKind};
    use std::sync::Arc;

    fn sid(s: &str) -> ScopeId {
        ScopeId(fairlead_syntax::ids::ScopeSlug::parse(s).unwrap())
    }
    fn gf(path: &str, scope: &str, kind: GenKind, body: &str) -> GeneratedFile {
        GeneratedFile {
            path: rp(path),
            scope: sid(scope),
            kind,
            body: body.into(),
        }
    }
    fn gset(files: Vec<GeneratedFile>) -> GeneratedSet {
        GeneratedSet {
            files: files.into_iter().map(|f| (f.path.clone(), f)).collect(),
        }
    }
    fn rendered(f: &GeneratedFile) -> Vec<u8> {
        f.render(Edition(1))
    }
    fn on_disk(facts: &mut Facts, path: &str, bytes: Vec<u8>) {
        facts.generated_on_disk.insert(rp(path), Arc::from(bytes));
    }
    fn req() -> SyncRequest {
        SyncRequest {
            baseline: BaselineMode::Off,
            accept_vendor: false,
            force: false,
            scope: None,
        }
    }
    fn run(r: &SyncRequest, g: &GeneratedSet, facts: &Facts) -> Result<Plan, Error> {
        let (_root, reg) = registry(
            &[
                ("fairlead/control.toml", ROOT_CONTROL),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        );
        sync_plan_from(r, &reg, g, facts, Edition(1))
    }
    const ROOT_MEMBERS: &str = "fairlead/gen/members";
    const ROOT_EXC: &str = "fairlead/gen/reg/exc.reg";

    #[test]
    fn r_cli_027__no_change_writes_nothing() {
        let f = gf(ROOT_MEMBERS, "root", GenKind::Members, "web path=apps/web");
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, rendered(&f));
        let p = run(&req(), &gset(vec![f]), &facts).unwrap();
        assert!(p.is_noop() && p.lines.is_empty() && p.refusal().is_none());
    }

    #[test]
    fn r_cli_025__stale_files_are_replaced_new_ones_created_and_the_second_run_is_a_no_op() {
        let old = gf(ROOT_MEMBERS, "root", GenKind::Members, "web path=apps/web");
        let new = gf(
            ROOT_MEMBERS,
            "root",
            GenKind::Members,
            "web path=apps/web\nui path=apps/ui",
        );
        let exc = gf(ROOT_EXC, "root", GenKind::Exceptions, "");
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, rendered(&old));
        let g = gset(vec![new, exc]);
        let p = run(&req(), &g, &facts).unwrap();
        let m = &p.set.writes[&rp(ROOT_MEMBERS)];
        assert_eq!(
            (&m.kind, m.class),
            (&WriteKind::Replace, crate::classify::FileClass::Generated)
        );
        assert_eq!(
            m.expect_prior,
            Some(crate::hash::hash_normalised(&rendered(&old)))
        );
        assert_eq!(p.set.writes[&rp(ROOT_EXC)].kind, WriteKind::Create);
        p.set
            .validate(&test_root(std::path::Path::new("/repo")))
            .unwrap();
        for w in p.set.writes.values() {
            on_disk(&mut facts, w.path.as_str(), w.bytes.clone());
        }
        assert!(
            run(&req(), &g, &facts).unwrap().is_noop(),
            "idempotent (R-CLI-027)"
        );
    }

    #[test]
    fn r_cli_026__a_hand_edited_file_is_quarantined_and_nothing_else_is_applied() {
        let f = gf(ROOT_MEMBERS, "root", GenKind::Members, "web path=apps/web");
        let exc = gf(ROOT_EXC, "root", GenKind::Exceptions, "a k=1");
        let edited = String::from_utf8(rendered(&f))
            .unwrap()
            .replace("web path=apps/web", "web path=HACKED");
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, edited.clone().into_bytes());
        let g = gset(vec![f, exc]);
        let p = run(&req(), &g, &facts).unwrap();
        let q = crate::write::quarantine_path(&rp(ROOT_MEMBERS), edited.as_bytes()).unwrap();
        assert_eq!(
            p.set.writes.keys().collect::<Vec<_>>(),
            [&q],
            "only the quarantine copy; the stale exc file waits"
        );
        assert_eq!(p.set.writes[&q].bytes, edited.as_bytes(), "verbatim");
        assert_eq!(p.refusal().unwrap().kind, RefusalKind::HandEditedGenerated);
        let ql = p
            .lines
            .iter()
            .find(|l| l.action == PlanAction::Quarantine)
            .unwrap();
        assert!(
            ql.diff.as_deref().unwrap().contains("-web path=HACKED"),
            "shows what would be lost"
        );
        p.set
            .validate(&test_root(std::path::Path::new("/repo")))
            .unwrap();
    }

    #[test]
    fn r_file_007__force_quarantines_first_then_overwrites() {
        let f = gf(ROOT_MEMBERS, "root", GenKind::Members, "web path=apps/web");
        let edited = String::from_utf8(rendered(&f))
            .unwrap()
            .replace("apps/web", "elsewhere");
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, edited.clone().into_bytes());
        let mut r = req();
        r.force = true;
        let p = run(&r, &gset(vec![f.clone()]), &facts).unwrap();
        assert_eq!(p.set.writes.len(), 2);
        assert_eq!(p.set.writes[&rp(ROOT_MEMBERS)].bytes, rendered(&f));
        assert!(p.refusal().is_none());
    }

    #[test]
    fn r_file_025__conflict_markers_are_regenerated_without_force() {
        let f = gf(ROOT_MEMBERS, "root", GenKind::Members, "web path=apps/web");
        let merged = String::from_utf8(rendered(&f)).unwrap().replacen(
            '\n',
            "\n<<<<<<< HEAD\n=======\n>>>>>>> other\n",
            1,
        );
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, merged.into_bytes());
        let p = run(&req(), &gset(vec![f]), &facts).unwrap();
        assert_eq!(p.set.writes.len(), 1);
        assert!(
            p.refusal().is_none() && !p.lines.iter().any(|l| l.action == PlanAction::Quarantine)
        );
    }

    #[test]
    fn r_scope_044__a_generated_file_must_live_in_its_own_scopes_gen_folder() {
        let stray = gf(
            "apps/web/fairlead/gen/members",
            "root",
            GenKind::Members,
            "x",
        );
        assert!(matches!(
            run(&req(), &gset(vec![stray]), &facts_empty()),
            Err(Error::Internal(_))
        ));
        let outside = gf("apps/web/elsewhere/members", "web", GenKind::Members, "x");
        assert!(matches!(
            run(&req(), &gset(vec![outside]), &facts_empty()),
            Err(Error::Internal(_))
        ));
        let ok = gf(
            "apps/web/fairlead/gen/members",
            "web",
            GenKind::Members,
            "x",
        );
        assert!(run(&req(), &gset(vec![ok]), &facts_empty()).is_ok());
    }

    #[test]
    fn r_file_021__every_planned_generated_path_lies_under_its_own_scopes_gen_folder() {
        let g = gset(vec![
            gf(ROOT_MEMBERS, "root", GenKind::Members, "a"),
            gf(
                "apps/web/fairlead/gen/members",
                "web",
                GenKind::Members,
                "b",
            ),
        ]);
        let p = run(&req(), &g, &facts_empty()).unwrap();
        for (path, f) in &g.files {
            assert!(p.set.writes.contains_key(path));
            let prefix = if f.scope.0.as_str() == "web" {
                "apps/web/fairlead/gen/"
            } else {
                "fairlead/gen/"
            };
            assert!(path.as_str().starts_with(prefix), "{}", path.as_str());
        }
    }

    #[test]
    fn r_scope_063__scope_flag_limits_the_write_cone_to_the_subtree() {
        let g = gset(vec![
            gf(ROOT_MEMBERS, "root", GenKind::Members, "a"),
            gf(
                "apps/web/fairlead/gen/members",
                "web",
                GenKind::Members,
                "b",
            ),
        ]);
        let mut r = req();
        r.scope = Some(sid("web"));
        let p = run(&r, &g, &facts_empty()).unwrap();
        assert_eq!(
            p.set.writes.keys().map(|k| k.as_str()).collect::<Vec<_>>(),
            ["apps/web/fairlead/gen/members"]
        );
        r.scope = Some(sid("nope"));
        assert!(matches!(run(&r, &g, &facts_empty()), Err(Error::Usage(_))));
        assert_eq!(
            run(&req(), &g, &facts_empty()).unwrap().set.writes.len(),
            2,
            "no flag: the whole rebuild in one pass"
        );
    }

    #[test]
    fn r_port_001__a_changed_membership_shard_produces_a_view_diff() {
        let old = gf(
            ROOT_MEMBERS,
            "root",
            GenKind::Members,
            "web path=apps/web\ndocs path=docs",
        );
        let new = gf(
            ROOT_MEMBERS,
            "root",
            GenKind::Members,
            "web path=packages/web\ndocs path=docs",
        );
        let mut facts = facts_empty();
        on_disk(&mut facts, ROOT_MEMBERS, rendered(&old));
        let p = run(&req(), &gset(vec![new]), &facts).unwrap();
        assert!(
            p.notes.contains(&"view: -web path=apps/web".to_owned())
                && p.notes.contains(&"view: +web path=packages/web".to_owned()),
            "{:?}",
            p.notes
        );
        assert!(members_diff("a\nb", "a\nb").is_empty());
        let many: String = (0..30).map(|i| format!("m{i}\n")).collect();
        let d = members_diff("", &many);
        assert_eq!(d.len(), 21);
        assert_eq!(d.last().unwrap(), "view: +10 more changes");
    }

    #[test]
    fn r_cli_046__orphan_generated_shards_are_reported_never_deleted() {
        let mut facts = facts_empty();
        on_disk(
            &mut facts,
            "fairlead/gen/reg/gone.reg",
            b"# scope root\n".to_vec(),
        );
        let p = run(&req(), &gset(vec![]), &facts).unwrap();
        assert!(
            p.is_noop()
                && p.notes
                    .iter()
                    .any(|n| n.contains("fairlead/gen/reg/gone.reg") && n.contains("orphan"))
        );
    }

    use crate::check::baseline::{Baseline, BaselineEntry};
    use crate::config::ApprovalsFile;
    use crate::diag::codes::{HN_F001, HN_R001};
    use crate::hash::{ContentHash, Fingerprint};
    use crate::time::AsOf;

    fn fp(s: &str) -> Fingerprint {
        Fingerprint::parse(s).unwrap()
    }
    fn bl(entries: &[(&str, crate::diag::DiagCode)]) -> Baseline {
        Baseline {
            entries: entries
                .iter()
                .map(|(f, c)| {
                    (
                        fp(f),
                        BaselineEntry {
                            code: *c,
                            path: rp("src/a.rs"),
                            accepted: None,
                        },
                    )
                })
                .collect(),
        }
    }
    fn today() -> AsOf {
        AsOf::parse("2026-10-09").unwrap()
    }
    fn approvals() -> ApprovalsFile {
        ApprovalsFile { approvals: vec![] }
    }

    #[test]
    fn r_chk_032__prune_only_shrinks_off_writes_nothing_and_absent_baselines_stay_absent() {
        let current = bl(&[("aaaaaaaaaaaa", HN_R001), ("bbbbbbbbbbbb", HN_R001)]);
        let still = vec![finding(HN_R001, "src/a.rs", "aaaaaaaaaaaa")];
        assert_eq!(
            baseline_for(
                &BaselineMode::Off,
                Some(&current),
                &still,
                today(),
                &approvals()
            )
            .unwrap(),
            None
        );
        let pruned = baseline_for(
            &BaselineMode::Prune,
            Some(&current),
            &still,
            today(),
            &approvals(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            pruned.entries.len(),
            1,
            "the fixed entry is dropped, none is added"
        );
        let brand_new = vec![finding(HN_R001, "src/a.rs", "cccccccccccc")];
        assert!(
            !baseline_for(
                &BaselineMode::Prune,
                Some(&current),
                &brand_new,
                today(),
                &approvals()
            )
            .unwrap()
            .unwrap()
            .entries
            .contains_key(&fp("cccccccccccc"))
        );
        assert_eq!(
            baseline_for(&BaselineMode::Prune, None, &still, today(), &approvals()).unwrap(),
            None
        );
    }

    #[test]
    fn r_chk_032__accept_new_needs_why_marks_acc_date_and_refuses_floor_findings() {
        let current = bl(&[]);
        let new = vec![finding(HN_R001, "src/a.rs", "cccccccccccc")];
        let mode = |w: &str| BaselineMode::AcceptNew { why: w.into() };
        assert!(matches!(
            baseline_for(&mode("  "), Some(&current), &new, today(), &approvals()),
            Err(Error::Usage(_))
        ));
        let grown = baseline_for(
            &mode("legacy module"),
            Some(&current),
            &new,
            today(),
            &approvals(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            grown.entries[&fp("cccccccccccc")].accepted,
            Some(jiff::civil::date(2026, 10, 9))
        );
        let floor = vec![finding(HN_F001, "fairlead/x.md", "dddddddddddd")];
        match baseline_for(
            &mode("because"),
            Some(&current),
            &floor,
            today(),
            &approvals(),
        ) {
            Err(Error::Refused(f)) => assert_eq!(f.kind, RefusalKind::Floor),
            other => panic!("{:?}", other.map(|b| b.map(|b| b.entries.len()))),
        }
    }

    #[test]
    fn r_file_040__vendor_lock_keeps_recorded_hashes_until_accept_vendor() {
        let (a, b, c) = (rp("v/a.ts"), rp("v/b.ts"), rp("v/c.ts"));
        let h = |n: u8| ContentHash([n; 32]);
        let mut facts = facts_empty();
        facts.vendor_hashes = [(a.clone(), h(2)), (b.clone(), h(9))].into(); // a changed, b new
        facts.vendor_lock = [(a.clone(), h(1)), (c.clone(), h(7))].into(); // c vanished
        let kept = effective_facts(false, &facts);
        assert_eq!(
            (
                kept.vendor_hashes[&a],
                kept.vendor_hashes[&b],
                kept.vendor_hashes[&c]
            ),
            (h(1), h(9), h(7))
        );
        let accepted = effective_facts(true, &facts);
        assert_eq!(accepted.vendor_hashes, facts.vendor_hashes);
        assert_eq!(
            vendor_notes(&facts, false),
            ["vendor file changed (HN-G004): v/a.ts; accept with --accept-vendor"]
        );
        assert_eq!(vendor_notes(&facts, true), ["accepted vendor hash: v/a.ts"]);
    }

    #[test]
    fn r_cli_027__sync_plan_twice_is_idempotent_and_every_write_validates() {
        use crate::check::options::Ramp;
        use crate::check::{CheckInput, CheckOptions, MaxFindings};
        use crate::extract::Corpus;
        use std::collections::BTreeMap;
        let (root, reg) = registry(
            &[
                ("fairlead/control.toml", ROOT_CONTROL),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        );
        let corpus = Corpus {
            records: BTreeMap::new(),
        };
        let opts = CheckOptions {
            as_of: today(),
            changed: false,
            files_from: None,
            paths: vec![],
            rule_filter: None,
            max: MaxFindings::Default,
            ramp: Ramp {
                show_observe: false,
                deny_warnings: false,
                nudge_days: [60, 120],
            },
            for_agent: false,
            no_baseline: false,
            require_declarations: false,
            budget: None,
        };
        let rep = report(vec![]);
        let r = req();
        let mut facts = facts_empty();
        let plan = |facts: &Facts| {
            let input = CheckInput {
                registry: &reg,
                corpus: &corpus,
                facts,
                load_diags: &[],
                baseline: None,
                options: &opts,
            };
            sync_plan(&r, &input, &rep, Edition(1)).unwrap()
        };
        let first = plan(&facts);
        first.set.validate(&root).unwrap();
        for w in first.set.writes.values() {
            facts
                .generated_on_disk
                .insert(w.path.clone(), Arc::from(w.bytes.clone()));
        }
        assert!(plan(&facts).is_noop());
    }
}
