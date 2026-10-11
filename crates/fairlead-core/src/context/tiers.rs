//! Tiers and the content they add (R-CTX-001, R-CTX-071, 14 "Tiers").
use std::collections::BTreeMap;

use crate::check::FindingState;
use crate::context::brief::CAP_TEXT;
use crate::context::brief::{AncestorL1, Block, Entry, L0Line, Omission, fence};
use crate::context::priority::{Candidate, Priority, Relevance, Score, rank_cmp};
use crate::context::{ContextTarget, Scene, Shape, n32};
use crate::path::RepoPath;
use crate::registry::{FieldValue, Origin};
use crate::registry::{Scope, ScopeId, ScopeState};
use crate::render::sanitize::sanitize_data;

pub const L1_CAP_TOKENS: u32 = 400;
pub const L0_DEFAULT_CAP: usize = 8;
pub const PURPOSE_CAP_L0: usize = 80;
pub const PURPOSE_CAP_L1: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    L0Index,
    L1ScopeCard,
    L2Detail,
    L3Archive,
}

impl Tier {
    /// L2 is the `explain` drill-down; L3 (archive) is 0.2.
    pub const fn is_active(self) -> bool {
        !matches!(self, Tier::L3Archive)
    }
    pub const fn cap_tokens(self) -> Option<u32> {
        match self {
            Tier::L1ScopeCard => Some(L1_CAP_TOKENS),
            _ => None,
        }
    }
}

fn purpose_fence(scene: &Scene<'_>, id: &ScopeId, cap: usize) -> Option<String> {
    let c = scene.input.registry.scope(id)?.control.as_ref()?;
    let t = c.settings.purpose.as_ref()?;
    Some(fence(&c.path, &c.content_hash, t, cap))
}

fn kind_counts(scene: &Scene<'_>, id: &ScopeId) -> BTreeMap<String, u32> {
    let mut m: BTreeMap<String, u32> = BTreeMap::new();
    for k in scene.input.registry.view(id).items.keys() {
        let kn = k.to_id().kind_name();
        *m.entry(kn.split('.').next().unwrap_or("").to_string())
            .or_insert(0) += 1;
    }
    m
}

fn drift_by_scope(scene: &Scene<'_>) -> BTreeMap<ScopeId, u32> {
    let mut m: BTreeMap<ScopeId, u32> = BTreeMap::new();
    for f in &scene.input.report.findings {
        if matches!(f.state, FindingState::New | FindingState::WaiverIneffective) {
            *m.entry(f.scope.clone()).or_insert(0) += 1;
        }
    }
    m
}

fn cand(
    prio: Priority,
    score: Score,
    key: &str,
    explain: String,
    block: Block,
    entry: Entry,
) -> Candidate {
    Candidate {
        priority: prio,
        score,
        hint: u32::MAX,
        key: key.to_string(),
        explain,
        block,
        entry,
    }
}

pub(crate) fn gather_scope_card(scene: &Scene<'_>, out: &mut Vec<Candidate>) {
    let w = &scene.input.config.sev_weight;
    let explain = scene.explain_target();
    if let Some(text) = purpose_fence(scene, &scene.scope, PURPOSE_CAP_L1) {
        let s = Score::new(w.error, Relevance::Direct, 0);
        out.push(cand(
            Priority::P3Items,
            s,
            "0purpose",
            explain.clone(),
            Block::Items,
            Entry::Purpose { text },
        ));
    }
    if matches!(scene.input.request.target, ContextTarget::SessionStart) {
        let counts = kind_counts(scene, &scene.scope);
        if !counts.is_empty() {
            let s = Score::new(w.info, Relevance::Direct, 0);
            out.push(cand(
                Priority::P3Items,
                s,
                "1kinds",
                explain.clone(),
                Block::Items,
                Entry::Kinds { counts },
            ));
        }
        let count = n32(scene.input.registry.ledger(&scene.scope).entries.len());
        let s = Score::new(w.warn, Relevance::Direct, 0);
        out.push(cand(
            Priority::P2Drift,
            s,
            "1exceptions",
            explain,
            Block::Drift,
            Entry::Exceptions { count },
        ));
    }
}

fn l0_for(scene: &Scene<'_>, s: &Scope, rel: &str, drift: &BTreeMap<ScopeId, u32>) -> L0Line {
    L0Line {
        scope: Scene::slug(&s.id),
        parent: s.parent.as_ref().map(Scene::slug),
        rel: rel.to_string(),
        digest: scene.digest_of(&s.id),
        items: n32(scene.input.registry.view(&s.id).items.len()),
        drift: drift.get(&s.id).copied().unwrap_or(0),
        state: match &s.state {
            ScopeState::Resolved => None,
            ScopeState::Unresolved { .. } => Some("unresolved".to_string()),
            ScopeState::AbsentFromTree => Some("absent".to_string()),
        },
        purpose: purpose_fence(scene, &s.id, PURPOSE_CAP_L0),
        dir: sanitize_data(s.dir.as_str(), 120),
    }
}

pub(crate) fn gather_neighbours(
    scene: &Scene<'_>,
    out: &mut Vec<Candidate>,
    pre: &mut Vec<Omission>,
    shape: &mut Shape,
) {
    let reg = scene.input.registry;
    let me = &scene.scope;
    let kids: Vec<&Scope> = reg
        .scopes()
        .filter(|s| s.parent.as_ref() == Some(me))
        .collect();
    let parent = reg.scope(me).and_then(|s| s.parent.clone());
    let sibs: Vec<&Scope> = match &parent {
        Some(p) => reg
            .scopes()
            .filter(|s| s.parent.as_ref() == Some(p) && s.id != *me)
            .collect(),
        None => Vec::new(),
    };
    shape.kids = n32(kids.len());
    shape.sibs = n32(sibs.len());
    let depth = scene.input.request.depth;
    if depth == Some(0) {
        return;
    }
    let mut picked: Vec<(&Scope, &'static str, u32)> = kids
        .iter()
        .map(|s| (*s, "child", 1))
        .chain(sibs.iter().map(|s| (*s, "sibling", 2)))
        .collect();
    if let Some(d) = depth {
        let mut frontier: Vec<&Scope> = kids.clone();
        let mut level = 1u32;
        while level < d && !frontier.is_empty() {
            level += 1;
            let next: Vec<&Scope> = reg
                .scopes()
                .filter(|s| {
                    s.parent
                        .as_ref()
                        .is_some_and(|p| frontier.iter().any(|f| f.id == *p))
                })
                .collect();
            picked.extend(next.iter().map(|s| (*s, "desc", level)));
            frontier = next;
        }
    }
    let drift = drift_by_scope(scene);
    let w = &scene.input.config.sev_weight;
    let mut cands: Vec<Candidate> = picked
        .into_iter()
        .map(|(s, rel, dist)| Candidate {
            priority: Priority::P6SiblingsChildren,
            score: Score::new(w.info, Relevance::ScopeWide, dist),
            hint: 0,
            key: Scene::slug(&s.id),
            explain: sanitize_data(s.dir.as_str(), 120),
            block: Block::Children,
            entry: Entry::Child(l0_for(scene, s, rel, &drift)),
        })
        .collect();
    if depth.is_none() {
        cands.sort_by(rank_cmp);
        let rest = cands.split_off(L0_DEFAULT_CAP.min(cands.len()));
        if let Some(first) = rest.first() {
            pre.push(Omission {
                block: Block::Children,
                count: n32(rest.len()),
                explain: first.explain.clone(),
            });
        }
    }
    out.append(&mut cands);
}

/// P7: one compact card per ancestor, nearest first by score.
pub(crate) fn gather_ancestors(scene: &Scene<'_>, out: &mut Vec<Candidate>) {
    let reg = scene.input.registry;
    let w = &scene.input.config.sev_weight;
    let drift = drift_by_scope(scene);
    let n = scene.chain.len();
    for (i, id) in scene.chain.iter().take(n.saturating_sub(1)).enumerate() {
        let slug = Scene::slug(id);
        let explain = reg
            .scope(id)
            .map(|s| sanitize_data(s.dir.as_str(), 120))
            .filter(|d| !d.is_empty() && d != ".")
            .unwrap_or_else(|| "kinds".to_string());
        out.push(Candidate {
            priority: Priority::P7AncestorL1,
            score: Score::new(w.info, Relevance::ScopeWide, n32(n - 1 - i)),
            hint: 0,
            key: slug.clone(),
            explain,
            block: Block::Ancestors,
            entry: Entry::Ancestor(AncestorL1 {
                scope: slug,
                purpose: purpose_fence(scene, id, PURPOSE_CAP_L1),
                kinds: kind_counts(scene, id),
                exceptions: n32(reg.ledger(id).entries.len()),
                drift: drift.get(id).copied().unwrap_or(0),
            }),
        });
    }
}

/// Session start is the root L1 card: error rules only, ID plus title (14 "Tiers").
pub fn to_l1_card(cands: Vec<Candidate>) -> Vec<Candidate> {
    cands
        .into_iter()
        .filter_map(|mut c| match &c.entry {
            Entry::Rule(r) if r.sev != "error" => None,
            Entry::Rule(_) => {
                c.entry = c.entry.to_l0();
                Some(c)
            }
            _ => Some(c),
        })
        .collect()
}

/// Hard cap of the L1 card in estimated tokens. Mandatory (P0/P1) candidates always stay.
pub fn cap_l1(
    mut cands: Vec<Candidate>,
    cost: &dyn Fn(&Entry) -> u32,
    cap: u32,
) -> (Vec<Candidate>, Vec<Candidate>) {
    cands.sort_by(rank_cmp);
    let (mut kept, mut dropped) = (Vec::new(), Vec::new());
    let mut used = 0u32;
    for c in cands {
        let add = cost(&c.entry);
        if c.priority.is_mandatory() || used.saturating_add(add) <= cap {
            used = used.saturating_add(add);
            kept.push(c);
        } else {
            dropped.push(c);
        }
    }
    (kept, dropped)
}

pub struct HandoffRef<'a> {
    pub id: &'a str,
    pub created: &'a str,
    pub sum: &'a str,
}

/// C-33: greatest `created` stamp, ties broken by ID bytewise (the greater ID is the newer).
pub fn newest_handoff(items: &[HandoffRef<'_>]) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.created
                .as_bytes()
                .cmp(b.created.as_bytes())
                .then_with(|| a.id.as_bytes().cmp(b.id.as_bytes()))
        })
        .map(|(i, _)| i)
}

pub(crate) fn gather_handoff(scene: &Scene<'_>, out: &mut Vec<Candidate>) {
    struct Row {
        id: String,
        created: String,
        sum: String,
        path: RepoPath,
    }
    let mut rows: Vec<Row> = Vec::new();
    for item in scene.input.registry.items() {
        let id = item.id.to_id().kind_name();
        if !id.starts_with("handoff.") {
            continue;
        }
        let created = match item.fields.get("created") {
            Some(FieldValue::Scalar(s)) => s.clone(),
            _ => String::new(),
        };
        let path = match &item.origin {
            Origin::DocDef { path, .. } => path.clone(),
            Origin::ControlTable { control } => control.clone(),
        };
        rows.push(Row {
            id,
            created,
            sum: item.sum.clone().unwrap_or_default(),
            path,
        });
    }
    let refs: Vec<HandoffRef<'_>> = rows
        .iter()
        .map(|r| HandoffRef {
            id: &r.id,
            created: &r.created,
            sum: &r.sum,
        })
        .collect();
    let Some(row) = newest_handoff(&refs).and_then(|i| rows.get(i)) else {
        return;
    };
    let w = &scene.input.config.sev_weight;
    let text = format!(
        "{} {}",
        row.id,
        scene.fence_for_path(&row.path, &row.sum, CAP_TEXT)
    );
    out.push(Candidate {
        priority: Priority::P3Items,
        score: Score::new(w.error, Relevance::Direct, 0),
        hint: u32::MAX - 1,
        key: "1handoff".to_string(),
        explain: row.id.clone(),
        block: Block::Items,
        entry: Entry::Handoff { text },
    });
}

/// R-VER-047, R-VER-058: the scope's open work an agent must not miss: hotfix backfill debts with
/// their deadline, and escalations its owners have not resolved.
pub(crate) fn gather_open_work(scene: &Scene<'_>, out: &mut Vec<Candidate>) {
    let reg = scene.input.registry;
    let w = &scene.input.config.sev_weight;
    let mut push = |key: String, text: String| {
        out.push(Candidate {
            priority: Priority::P5Recent,
            score: Score::new(w.warn, Relevance::ScopeWide, 0),
            hint: 0,
            key,
            explain: "kinds".to_string(),
            block: Block::Items,
            entry: Entry::Note { text },
        });
    };
    let window = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .and_then(|c| c.docs_first.hotfix_window)
        .unwrap_or(3);
    let (docs, _) = crate::docs::collect_change_docs(scene.input.corpus);
    for d in docs.iter().filter(|d| {
        d.hotfix && d.status.is_pending() && reg.scope_of(&d.path) == Some(&scene.scope)
    }) {
        let due = d
            .extra
            .get("opened")
            .and_then(|o| o.parse::<jiff::civil::Date>().ok())
            .and_then(|o| {
                o.checked_add(jiff::Span::new().days(i64::from(window)))
                    .ok()
            })
            .map_or_else(|| "?".to_owned(), |x| x.to_string());
        push(
            format!("5hotfix-{}", d.id.as_str()),
            format!(
                "hotfix debt {} due {due}: backfill its change doc",
                d.id.as_str()
            ),
        );
    }
    for r in reg.requests().iter().filter(|r| r.from == scene.scope) {
        if crate::check::builtin::versions::escalation_resolved(reg, r) {
            continue;
        }
        push(
            format!("5req-{}", r.target),
            format!(
                "escalation open: {} to {}{}",
                sanitize_data(&r.target, 80),
                r.to.0.as_str(),
                if r.blocking { " (blocking)" } else { "" }
            ),
        );
    }
    // R-VAR-032, R-VAR-033: open variation requests filed here or addressed to a unit homed here,
    // stale ones marked; the request's text is data, fenced with its provenance.
    let corpus = scene.input.corpus;
    let requests = crate::variation::collect_requests(corpus);
    if requests.is_empty() {
        return;
    }
    let root = reg.scope(reg.root_scope()).and_then(|s| s.control.as_ref());
    let stale_after = root.map_or(4, |c| c.variation.stale_seals) as usize;
    let folder = root
        .and_then(|c| c.settings.folder.clone())
        .unwrap_or_else(|| "fairlead".into());
    let sealed = crate::docs::version_files(corpus, &folder);
    // The brief has no approvals file: an answer counts as approved unless the check report
    // says it is not (`HN-X002` at the answer doc).
    let mut answers = crate::variation::answers(&docs, &crate::config::ApprovalsFile::default());
    for a in &mut answers {
        a.approved = !scene
            .input
            .report
            .findings
            .iter()
            .any(|f| f.code == crate::diag::codes::HN_X002 && f.location.path == a.doc.path);
    }
    let awaiting = crate::variation::awaiting(reg, corpus);
    for r in &requests {
        let Some(id) = &r.id else { continue };
        let from = crate::variation::scope_of(reg, &r.path);
        let home = r
            .field("need")
            .and_then(|n| fairlead_syntax::ids::Id::parse(n).ok())
            .and_then(|n| match reg.lookup(from, &n) {
                crate::registry::Lookup::Found(i) => Some(i.scope),
                _ => None,
            });
        if *from != scene.scope && home.as_ref() != Some(&scene.scope) {
            continue;
        }
        let st = crate::variation::status(id, &answers, &awaiting);
        if st != crate::variation::ReqStatus::Open {
            continue;
        }
        let since = crate::variation::seals_since(r.created, &sealed);
        let stale = if since > stale_after {
            format!(" stale after {since} seals")
        } else {
            String::new()
        };
        let want = scene.fence_for_path(&r.path, r.field("want").unwrap_or(""), CAP_TEXT);
        push(
            format!("5var-{}", id.as_str()),
            format!(
                "request {} open{stale}: needs {} | {want}",
                id.as_str(),
                sanitize_data(r.field("need").unwrap_or("?"), 80)
            ),
        );
    }
}

/// R-LC-034, R-LC-036, R-LC-037: the graveyard that applies here, its tripwires (how often a match
/// was silenced with `revisits`), and abandoned change docs as graveyard candidates.
pub(crate) fn gather_graveyard(scene: &Scene<'_>, out: &mut Vec<Candidate>) {
    let reg = scene.input.registry;
    let (entries, _) = crate::graveyard::collect(scene.input.corpus);
    let here = crate::check::builtin::grave::effective(reg, &entries, &scene.scope);
    let (docs, _) = crate::docs::collect_change_docs(scene.input.corpus);
    let w = &scene.input.config.sev_weight;
    let mut push = |key: &str, text: String| {
        out.push(Candidate {
            priority: Priority::P5Recent,
            score: Score::new(w.warn, Relevance::ScopeWide, 0),
            hint: 0,
            key: key.to_string(),
            explain: "graveyard".to_string(),
            block: Block::Items,
            entry: Entry::Note { text },
        });
    };
    if !here.is_empty() {
        let trips: Vec<String> = here
            .iter()
            .filter_map(|e| {
                let n = e.tripwire
                    + n32(docs
                        .iter()
                        .filter(|d| crate::check::builtin::grave::revisits(d).contains(&e.id))
                        .count());
                (n > 0).then(|| format!("{}={n}", sanitize_data(&e.id, 64)))
            })
            .collect();
        let mut text = format!("graveyard: {} rejected under here", here.len());
        if !trips.is_empty() {
            text.push_str(&format!(" (tripwire {})", trips.join(" ")));
        }
        push("5graveyard", text);
    }
    for d in docs.iter().filter(|d| {
        d.status == crate::docs::Status::Abandoned && reg.scope_of(&d.path) == Some(&scene.scope)
    }) {
        push(
            &format!("5grave-{}", d.id.as_str()),
            format!(
                "graveyard candidate? {} (abandoned): {}",
                d.id.as_str(),
                sanitize_data(d.title.as_deref().unwrap_or(""), 80)
            ),
        );
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::context::brief::BriefRule;
    use crate::context::n32;

    fn note(prio: Priority, key: &str, len: usize) -> Candidate {
        Candidate {
            priority: prio,
            score: Score::new(1, Relevance::ScopeWide, 1),
            hint: 0,
            key: key.into(),
            explain: key.into(),
            block: Block::Children,
            entry: Entry::Note {
                text: "x".repeat(len),
            },
        }
    }

    fn cost(e: &Entry) -> u32 {
        n32(e.cost_bytes()).div_ceil(4)
    }

    fn rule(sev: &str) -> BriefRule {
        BriefRule {
            id: format!("rule.{sev}"),
            sev: sev.into(),
            stage: "enforce".into(),
            home: "root".into(),
            inherited: false,
            glob: vec![],
            why: None,
            ovr: None,
            l0: false,
        }
    }

    #[test]
    fn r_ctx_001__only_l3_is_inactive_and_only_l1_has_a_cap() {
        assert!(
            Tier::L0Index.is_active()
                && Tier::L1ScopeCard.is_active()
                && Tier::L2Detail.is_active()
        );
        assert!(!Tier::L3Archive.is_active());
        assert_eq!(Tier::L1ScopeCard.cap_tokens(), Some(400));
        assert_eq!(Tier::L0Index.cap_tokens(), None);
    }

    #[test]
    fn r_ctx_001__the_l1_card_is_capped_at_400_tokens_and_keeps_mandatory_content() {
        let mut c = vec![note(Priority::P1ErrorRules, "a-mandatory", 13)]; // 5 tokens
        c.extend((0..60).map(|i| note(Priority::P6SiblingsChildren, &format!("c{i:02}"), 33))); // 10 tokens each
        let (kept, dropped) = cap_l1(c, &cost, L1_CAP_TOKENS);
        assert!(kept.iter().map(|k| cost(&k.entry)).sum::<u32>() <= 400);
        assert!(kept.iter().any(|k| k.key == "a-mandatory"));
        assert_eq!((kept.len(), dropped.len()), (40, 21));
    }

    #[test]
    fn r_ctx_001__the_session_card_keeps_only_error_rules_in_l0_form() {
        let mk = |r: BriefRule| Candidate {
            priority: Priority::P1ErrorRules,
            score: Score::new(4, Relevance::Glob, 0),
            hint: 0,
            key: r.id.clone(),
            explain: r.id.clone(),
            block: Block::Rules,
            entry: Entry::Rule(r),
        };
        let out = to_l1_card(vec![
            mk(rule("error")),
            mk(rule("warn")),
            note(Priority::P6SiblingsChildren, "n", 3),
        ]);
        assert_eq!(out.len(), 2);
        assert!(
            out.iter()
                .any(|c| matches!(&c.entry, Entry::Rule(r) if r.sev == "error" && r.l0))
        );
        assert!(
            !out.iter()
                .any(|c| matches!(&c.entry, Entry::Rule(r) if r.sev == "warn"))
        );
    }

    fn h<'a>(id: &'a str, created: &'a str) -> HandoffRef<'a> {
        HandoffRef {
            id,
            created,
            sum: "s",
        }
    }

    #[test]
    fn r_ctx_008__the_newest_created_stamp_wins() {
        let v = [
            h("handoff.0007", "2026-10-08"),
            h("handoff.0006", "2026-10-09T09:00:00Z"),
        ];
        assert_eq!(newest_handoff(&v), Some(1));
    }

    #[test]
    fn r_ctx_008__ties_break_by_the_greater_id_bytewise() {
        let v = [
            h("handoff.0006", "2026-10-09"),
            h("handoff.0007", "2026-10-09"),
            h("handoff.0005", "2026-10-09"),
        ];
        assert_eq!(newest_handoff(&v), Some(1));
    }

    #[test]
    fn r_ctx_008__a_missing_stamp_sorts_oldest_and_no_handoff_gives_none() {
        let v = [h("handoff.0009", ""), h("handoff.0001", "2026-01-01")];
        assert_eq!(newest_handoff(&v), Some(1));
        assert_eq!(newest_handoff(&[]), None);
    }
}
