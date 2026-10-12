//! Retention and compaction (33-retention-and-compaction): which records are due at a seal, the
//! summary line that replaces them in the ledger, quarterly rollups and the IDs that stay
//! resolvable. Pure: eligibility reads the recorded seal timestamps, never the wall clock
//! (R-RET-020).
use std::collections::{BTreeMap, BTreeSet};

use crate::config::retain::{Policy, Ttl, root_policy, tighten};
use crate::docs::{ChangeDoc, DraftId, Status};
use crate::path::RepoPath;
use crate::registry::{Registry, ScopeId};
use crate::render::line::TerseLine;
use crate::version::Version;

/// The outcomes a closed record may carry (R-RET-002, D-39).
pub const OUTCOMES: [&str; 5] = ["adopted", "rejected", "removed", "superseded", "abandoned"];

/// A record subject to retention (R-RET-001: never live state).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Record {
    pub path: RepoPath,
    pub id: DraftId,
    /// The retention kind: `chg` (change docs) or `rq` (requests).
    pub class: &'static str,
    /// The doc's own `kind` (`change`, `chore`, `request`, ...).
    pub kind: String,
    pub scope: ScopeId,
    pub title: String,
    pub opened: Option<Version>,
    /// The version the record closed in; `None` while it is open.
    pub closed: Option<Version>,
    /// `out`, as written or derived from the record's state.
    pub out: String,
    pub sum: String,
    pub why: String,
    pub alt: String,
    pub succ: String,
    pub kw: String,
    pub dnr: bool,
    pub ord: Option<u32>,
}

impl Record {
    /// Decision-grade: kept forever as a line, never folded into a rollup (R-RET-026).
    pub fn decision_grade(&self) -> bool {
        matches!(self.out.as_str(), "rejected" | "removed")
    }

    /// R-RET-002, R-RET-003: what keeps the record from compacting (empty when it can).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.sum.trim().is_empty() {
            out.push("`sum` is empty".to_owned());
        }
        if self.sum.chars().count() > 240 {
            out.push("`sum` is longer than 240 characters".to_owned());
        }
        if !OUTCOMES.contains(&self.out.as_str()) {
            out.push(format!(
                "`out` is {:?}, not one of {}",
                self.out,
                OUTCOMES.join(", ")
            ));
        }
        if matches!(self.out.as_str(), "rejected" | "removed") && self.why.trim().is_empty() {
            out.push(format!("`out = \"{}\"` needs `why`", self.out));
        }
        out
    }
}

fn extra<'a>(d: &'a ChangeDoc, k: &str) -> &'a str {
    d.extra.get(k).map_or("", String::as_str)
}

/// Every change doc and request, as records. A sealed change doc closed in its sealed version
/// (`out` defaults to `adopted`); an abandoned one in its `created` version (`abandoned`); a
/// request closed in the version that sealed its approved answer (`adopted`, or `rejected`).
pub fn collect(
    reg: &Registry,
    corpus: &crate::extract::Corpus,
    approvals: &crate::config::ApprovalsFile,
) -> Vec<Record> {
    let (docs, _) = crate::docs::collect_change_docs(corpus);
    let mut out = Vec::new();
    for d in &docs {
        let closed = match d.status {
            Status::Sealed => d.sealed,
            Status::Abandoned => d.created.as_deref().and_then(Version::parse),
            _ => None,
        };
        let default_out = match d.status {
            Status::Abandoned => "abandoned",
            Status::Sealed => "adopted",
            _ => "",
        };
        let out_w = extra(d, "out");
        out.push(Record {
            path: d.path.clone(),
            id: d.id.clone(),
            class: "chg",
            kind: d.kind.clone(),
            scope: reg
                .scope_of(&d.path)
                .cloned()
                .unwrap_or_else(|| reg.root_scope().clone()),
            title: d.title.clone().unwrap_or_default(),
            opened: d.created.as_deref().and_then(Version::parse),
            closed,
            out: if out_w.is_empty() {
                default_out.to_owned()
            } else {
                out_w.to_owned()
            },
            sum: extra(d, "sum").to_owned(),
            why: extra(d, "why").to_owned(),
            alt: extra(d, "alt").to_owned(),
            succ: extra(d, "succ").to_owned(),
            kw: extra(d, "kw").to_owned(),
            dnr: extra(d, "dnr") == "true",
            ord: d.ordinal,
        });
    }
    let answers = crate::variation::answers(&docs, approvals);
    for r in crate::variation::collect_requests(corpus) {
        let Some(id) = r.id.clone() else { continue };
        let scope = reg
            .scope_of(&r.path)
            .cloned()
            .unwrap_or_else(|| reg.root_scope().clone());
        let answer = answers.iter().find(|a| {
            a.approved
                && a.doc.status == Status::Sealed
                && a.answers
                    .iter()
                    .any(|x| crate::docs::doc_ref_is(x, &id, scope.0.as_str()))
        });
        out.push(Record {
            path: r.path.clone(),
            id,
            class: "rq",
            kind: "request".into(),
            scope,
            title: r.field("sum").unwrap_or("").to_owned(),
            opened: r.created,
            closed: answer.and_then(|a| a.doc.sealed),
            out: match answer {
                Some(a) if a.rejected() => "rejected".into(),
                Some(_) => "adopted".into(),
                None => String::new(),
            },
            sum: r.field("sum").unwrap_or("").to_owned(),
            why: answer
                .filter(|a| a.rejected())
                .map(|a| extra(a.doc, "sum").to_owned())
                .unwrap_or_default(),
            alt: String::new(),
            succ: answer
                .map(|a| a.doc.id.as_str().to_owned())
                .unwrap_or_default(),
            kw: String::new(),
            dnr: answer.is_some_and(|a| a.rejected()),
            ord: None,
        });
    }
    out.sort_by(|a, b| (&a.scope, a.id.as_str()).cmp(&(&b.scope, b.id.as_str())));
    out
}

/// The effective policy of a record: the root's `[retain]`, lengthened by the scope chain
/// (R-RET-013); the inert shortening keys are returned too.
pub fn policy(reg: &Registry, r: &Record) -> (Policy, Vec<String>) {
    let root = reg
        .scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .map(|c| c.retain.clone())
        .unwrap_or_default();
    let mut p = root_policy(&root, r.class);
    let mut inert = Vec::new();
    for s in reg.chain(&r.scope).into_iter().skip(1) {
        if let Some(c) = reg.scope(s).and_then(|x| x.control.as_ref()) {
            inert.extend(tighten(&mut p, &c.retain, r.class));
        }
    }
    (p, inert)
}

/// Whether the root turned compaction on (`[retain]` declared; E3-30).
pub fn enabled(reg: &Registry) -> bool {
    reg.scope(reg.root_scope())
        .and_then(|s| s.control.as_ref())
        .is_some_and(|c| c.retain.declared)
}

/// The recorded `sealed-at` date of each sealed version, from the version files.
pub fn stamps(
    version_texts: &BTreeMap<RepoPath, std::sync::Arc<str>>,
) -> BTreeMap<Version, jiff::civil::Date> {
    let mut out = BTreeMap::new();
    for (p, t) in version_texts {
        let Some(v) = crate::version::version_of_file_name(p.file_name()) else {
            continue;
        };
        let at = t
            .lines()
            .find_map(|l| l.strip_prefix("sealed-at = \""))
            .and_then(|r| r.get(..10))
            .and_then(|d| d.parse::<jiff::civil::Date>().ok());
        if let Some(d) = at {
            out.insert(v, d);
        }
    }
    out
}

/// R-RET-010, R-RET-011, R-RET-020: whether a closed record is due at a seal whose recorded
/// date is `now`. `sealed` lists every sealed version including the one being sealed; `stamps`
/// gives their recorded dates. The age is the number of sealed versions after the closing one.
pub fn eligible(
    r: &Record,
    p: &Policy,
    sealed: &[Version],
    stamps: &BTreeMap<Version, jiff::civil::Date>,
    now: jiff::civil::Date,
) -> bool {
    let Some(closed) = r.closed else { return false };
    let age = sealed.iter().filter(|v| **v > closed).count();
    let since = stamps
        .range(closed..)
        .next()
        .map(|(_, d)| d.until(now).map(|s| s.get_days()).unwrap_or(0))
        .unwrap_or(0);
    let since = u32::try_from(since.max(0)).unwrap_or(0);
    let by = |t: Ttl| match t {
        Ttl::Versions(n) => age > n as usize && since >= p.min_age,
        Ttl::Days(n) => since >= n,
        Ttl::Life | Ttl::Never => false,
    };
    by(p.ttl) || p.max.is_some_and(by)
}

/// R-RET-005: the summary line of a record, without the trailing ` d=` the ledger adds.
pub fn summary_line(r: &Record, git: Option<&str>, body_digest: &str) -> String {
    let mut t = TerseLine::new(r.id.as_str())
        .with("kind", &r.kind)
        .with("scope", r.scope.0.as_str());
    if let Some(o) = r.ord {
        t = t.with("ord", &format!("{o:04}"));
    }
    t = t.with("title", &r.title);
    if let Some(v) = r.opened {
        t = t.with("opened", &v.to_string());
    }
    if let Some(v) = r.closed {
        t = t.with("closed", &v.to_string());
    }
    t = t.with("out", &r.out).with("sum", &r.sum);
    for (k, v) in [
        ("why", &r.why),
        ("alt", &r.alt),
        ("succ", &r.succ),
        ("kw", &r.kw),
    ] {
        if !v.is_empty() {
            t = t.with(k, v);
        }
    }
    if r.dnr {
        t = t.with("dnr", "1");
    }
    t.with("git", git.unwrap_or("-"))
        .with("body", body_digest)
        .render()
}

/// A compacted record as its ledger line reads (R-RET-050).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Compacted {
    pub scope: String,
    pub line: String,
    pub git: Option<String>,
    /// `Some("2031Q2")` when the record was folded into a rollup.
    pub rolled: Option<String>,
}

/// Every compacted record of the ledgers by draft ID, and the rollups by their ordinal ranges.
pub fn compacted(ledgers: &BTreeMap<RepoPath, std::sync::Arc<str>>) -> BTreeMap<String, Compacted> {
    let mut out = BTreeMap::new();
    for t in ledgers.values() {
        let Ok(f) = crate::hist::YearFile::parse(t) else {
            continue;
        };
        for l in &f.lines {
            let crate::hist::HistLine::Record { text, .. } = l else {
                continue;
            };
            let Ok(tl) = TerseLine::parse(text) else {
                continue;
            };
            if !tl.id.starts_with("d-") || tl.get("sum").is_none() {
                continue;
            }
            out.insert(
                tl.id.clone(),
                Compacted {
                    scope: f.scope.clone(),
                    line: text.clone(),
                    git: tl.get("git").filter(|g| *g != "-").map(str::to_owned),
                    rolled: None,
                },
            );
        }
    }
    out
}

/// R-RET-027: the highest ordinal the ledgers hold per (scope, kind): summary lines and rollup
/// ranges, so a folded ordinal is never reused.
pub fn ledger_ordinals(
    ledgers: &BTreeMap<RepoPath, std::sync::Arc<str>>,
) -> BTreeMap<(String, String), u32> {
    let mut out: BTreeMap<(String, String), u32> = BTreeMap::new();
    for t in ledgers.values() {
        let Ok(f) = crate::hist::YearFile::parse(t) else {
            continue;
        };
        for l in &f.lines {
            let crate::hist::HistLine::Record { text, .. } = l else {
                continue;
            };
            let Ok(tl) = TerseLine::parse(text) else {
                continue;
            };
            let (Some(kind), Some(ord)) = (tl.get("kind"), tl.get("ord")) else {
                continue;
            };
            let hi = ord.rsplit("..").next().and_then(|o| o.parse::<u32>().ok());
            let scope = tl.get("scope").unwrap_or(&f.scope).to_owned();
            if let Some(hi) = hi {
                let e = out.entry((scope, kind.to_owned())).or_insert(0);
                *e = (*e).max(hi);
            }
        }
    }
    out
}

/// The quarter (`2031q2`) of a date (R-RET-026: the closing seal's recorded timestamp, UTC).
pub fn quarter(d: jiff::civil::Date) -> String {
    format!("{}q{}", d.year(), (d.month() - 1) / 3 + 1)
}

/// A rollup being built: count, lowest and highest ordinal, decisions, folded digests.
type Roll = (u32, u32, u32, u32, Vec<String>);

/// R-RET-026: in an open year file, the non-decision-grade summary lines cold for `roll_after`
/// versions beyond their TTL fold into one `roll.<yyyy>q<n>` line per (quarter, kind). Returns
/// the new record texts (sorted, rollups merged), or `None` when nothing folds.
pub fn fold(
    records: &[String],
    roll_after: u32,
    ttl_of: &dyn Fn(&str) -> Option<u32>,
    sealed: &[Version],
    stamps: &BTreeMap<Version, jiff::civil::Date>,
) -> Option<Vec<String>> {
    let mut keep: Vec<String> = Vec::new();
    // (quarter, kind) -> (count, low ord, high ord, decisions, digests)
    let mut rolls: BTreeMap<(String, String), Roll> = BTreeMap::new();
    let mut folded_any = false;
    for text in records {
        let Ok(tl) = TerseLine::parse(text) else {
            keep.push(text.clone());
            continue;
        };
        if let Some(q) = tl.id.strip_prefix("roll.") {
            // an existing rollup line merges with new folds of the same quarter and kind
            let kind = tl.get("kind").unwrap_or("").to_owned();
            let n: u32 = tl.get("n").and_then(|x| x.parse().ok()).unwrap_or(0);
            let (lo, hi) = tl
                .get("ord")
                .and_then(|o| o.split_once(".."))
                .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
                .unwrap_or((u32::MAX, 0));
            let dec: u32 = tl.get("dec").and_then(|x| x.parse().ok()).unwrap_or(0);
            let e = rolls
                .entry((q.to_owned(), kind))
                .or_insert((0, u32::MAX, 0, 0, Vec::new()));
            e.0 += n;
            e.1 = e.1.min(lo);
            e.2 = e.2.max(hi);
            e.3 += dec;
            e.4.push(tl.get("dg").unwrap_or("").to_owned());
            continue;
        }
        let decision =
            matches!(tl.get("out"), Some("rejected" | "removed")) || tl.get("dnr") == Some("1");
        let closed = tl.get("closed").and_then(Version::parse);
        let kind = tl.get("kind").unwrap_or("").to_owned();
        let cold = closed.is_some_and(|c| {
            let age = sealed.iter().filter(|v| **v > c).count() as u32;
            ttl_of(&kind).is_some_and(|t| age > t + roll_after)
        });
        let stamp = closed.and_then(|c| stamps.range(c..).next().map(|(_, d)| *d));
        match (cold && !decision && tl.id.starts_with("d-"), stamp) {
            (true, Some(at)) => {
                folded_any = true;
                let ord: u32 = tl.get("ord").and_then(|o| o.parse().ok()).unwrap_or(0);
                let e = rolls
                    .entry((quarter(at), kind))
                    .or_insert((0, u32::MAX, 0, 0, Vec::new()));
                e.0 += 1;
                e.1 = e.1.min(ord);
                e.2 = e.2.max(ord);
                e.4.push(crate::hist::record_digest(text));
            }
            _ => keep.push(text.clone()),
        }
    }
    if !folded_any {
        return None;
    }
    for ((q, kind), (n, lo, hi, dec, digests)) in rolls {
        let d = crate::hash::base32(&crate::hash::hash_raw(digests.join(" ").as_bytes()).0, 8);
        keep.push(
            TerseLine::new(&format!("roll.{q}"))
                .with("kind", &kind)
                .with("n", &n.to_string())
                .with(
                    "ord",
                    &format!("{:04}..{hi:04}", if lo == u32::MAX { 0 } else { lo }),
                )
                .with("dec", &dec.to_string())
                .with("dg", &d)
                .render(),
        );
    }
    keep.sort();
    Some(keep)
}

/// The draft IDs of every record (to tell a compacted ID from an unknown one).
pub fn live_ids(records: &[Record]) -> BTreeSet<String> {
    records.iter().map(|r| r.id.as_str().to_owned()).collect()
}

/// One record compacted by a seal (R-RET-025).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Compaction {
    pub path: RepoPath,
    pub scope: ScopeId,
    /// The summary line (without ` d=`).
    pub line: String,
    /// The record's text as read, for the delete's prior check and the body digest.
    pub old: String,
}

/// R-RET-020 to R-RET-025: the records due at a seal of `open` recorded on `now`. `Err` lists
/// every due record that cannot compact (`HN-K001`, R-RET-022). `read` returns a record's text.
pub fn plan(
    reg: &Registry,
    records: &[Record],
    sealed: &[Version],
    stamps: &BTreeMap<Version, jiff::civil::Date>,
    now: jiff::civil::Date,
    git: Option<&str>,
    read: &dyn Fn(&RepoPath) -> Option<String>,
) -> Result<Vec<Compaction>, Vec<String>> {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for r in records {
        let (p, _) = policy(reg, r);
        if !eligible(r, &p, sealed, stamps, now) {
            continue;
        }
        let problems = r.problems();
        if !problems.is_empty() {
            bad.push(format!(
                "{} ({}): {}",
                r.id.as_str(),
                r.path.as_str(),
                problems.join("; ")
            ));
            continue;
        }
        let Some(old) = read(&r.path) else { continue };
        let body = crate::hash::base32(&crate::hash::hash_normalised(old.as_bytes()).0, 8);
        out.push(Compaction {
            path: r.path.clone(),
            scope: r.scope.clone(),
            line: summary_line(r, git, &body),
            old,
        });
    }
    if bad.is_empty() { Ok(out) } else { Err(bad) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn d(s: &str) -> jiff::civil::Date {
        s.parse().unwrap()
    }
    fn rec(closed: &str) -> Record {
        Record {
            path: RepoPath::new("fairlead/docs/changes/0001-a.md").unwrap(),
            id: DraftId::parse("d-00000001").unwrap(),
            class: "chg",
            kind: "change".into(),
            scope: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("root").unwrap()),
            title: "A".into(),
            opened: Some(v(closed)),
            closed: Some(v(closed)),
            out: "adopted".into(),
            sum: "a".into(),
            why: String::new(),
            alt: String::new(),
            succ: String::new(),
            kw: String::new(),
            dnr: false,
            ord: Some(1),
        }
    }

    #[test]
    fn r_ret_010__version_ttl_waits_for_min_age() {
        let mut p = crate::config::retain::default_policy("chg");
        p.ttl = Ttl::Versions(1);
        p.min_age = 90;
        let sealed = [v("0.1.0"), v("0.1.1"), v("0.1.2")];
        let mut stamps = BTreeMap::new();
        stamps.insert(v("0.1.0"), d("2026-01-01"));
        stamps.insert(v("0.1.1"), d("2026-01-10"));
        stamps.insert(v("0.1.2"), d("2026-01-20"));
        let r = rec("0.1.0");
        assert!(
            !eligible(&r, &p, &sealed, &stamps, d("2026-01-20")),
            "age 2 > 1 but only 19 days old"
        );
        assert!(
            eligible(&r, &p, &sealed, &stamps, d("2026-04-01")),
            "90 days later"
        );
        p.ttl = Ttl::Never;
        assert!(!eligible(&r, &p, &sealed, &stamps, d("2030-01-01")));
    }

    #[test]
    fn r_ret_001__live_state_never_compacted() {
        // records are change docs and requests only; an open record (no closing version) and
        // every registry item, rule and shape are never candidates
        let mut r = rec("0.1.0");
        r.closed = None;
        let p = crate::config::retain::default_policy("chg");
        assert!(!eligible(
            &r,
            &p,
            &[v("9.0.0")],
            &BTreeMap::new(),
            d("2099-01-01")
        ));
        let src = include_str!("retention.rs");
        let collect = &src
            [src.find("pub fn collect(").unwrap()..src.find("/// The effective policy").unwrap()];
        assert!(
            !collect.contains("reg.items()") && !collect.contains("rules"),
            "only docs are records"
        );
    }

    #[test]
    fn r_ret_020__wall_clock_never_read() {
        let src = include_str!("retention.rs");
        let code = &src[..src.find("#[cfg(test)]").unwrap()];
        for banned in [
            "Zoned::now",
            "Timestamp::now",
            "SystemTime",
            "Instant::now",
            "jiff::civil::Date::today",
        ] {
            assert!(!code.contains(banned), "{banned}");
        }
    }

    #[test]
    fn r_ret_005__summary_line_has_required_fields() {
        let mut r = rec("0.1.0");
        r.out = "rejected".into();
        r.why = "too slow".into();
        r.alt = "cache".into();
        r.succ = "d-00000002".into();
        r.kw = "perf".into();
        r.dnr = true;
        let l = summary_line(&r, Some("abc123"), "bodyhash");
        assert_eq!(
            l,
            "d-00000001 kind=change scope=root ord=0001 title=A opened=0.1.0 closed=0.1.0 out=rejected sum=a why=\"too slow\" alt=cache succ=d-00000002 kw=perf dnr=1 git=abc123 body=bodyhash"
        );
        assert!(r.problems().is_empty());
        r.why.clear();
        assert!(r.problems().iter().any(|p| p.contains("needs `why`")));
    }

    #[test]
    fn r_ret_026__fold_into_quarter_of_closing_seal() {
        let sealed: Vec<Version> = (0..20).map(|i| Version::new(0, i, 0)).collect();
        let mut stamps = BTreeMap::new();
        stamps.insert(Version::new(0, 1, 0), d("2026-05-02"));
        stamps.insert(Version::new(0, 2, 0), d("2026-08-02"));
        let line = |id: &str, ord: u32, closed: &str, out: &str| {
            format!(
                "{id} kind=change scope=root ord={ord:04} title=x closed={closed} out={out} sum=s git=- body=b"
            )
        };
        let recs = vec![
            line("d-00000001", 1, "0.1.0", "adopted"),
            line("d-00000002", 2, "0.1.0", "adopted"),
            line("d-00000003", 3, "0.1.0", "rejected"),
            line("d-00000004", 4, "0.2.0", "adopted"),
            line("d-00000005", 5, "0.18.0", "adopted"),
        ];
        let out = fold(&recs, 8, &|_| Some(8), &sealed, &stamps).unwrap();
        assert!(
            out.iter()
                .any(|l| l.starts_with("roll.2026q2 kind=change n=2 ord=0001..0002 dec=0")),
            "{out:#?}"
        );
        assert!(
            out.iter()
                .any(|l| l.starts_with("roll.2026q3 kind=change n=1 ord=0004..0004")),
            "{out:#?}"
        );
        assert!(
            out.iter().any(|l| l.starts_with("d-00000003")),
            "decision-grade stays"
        );
        assert!(
            out.iter().any(|l| l.starts_with("d-00000005")),
            "not cold yet"
        );
        // a second fold merges into the existing rollup
        assert!(
            fold(&out, 8, &|_| Some(8), &sealed, &stamps).is_none(),
            "nothing new folds"
        );
    }

    #[test]
    fn r_ret_027__ordinal_never_reused_after_fold() {
        let mut ledgers = BTreeMap::new();
        ledgers.insert(
            RepoPath::new("fairlead/ledger/hist/2026.hl").unwrap(),
            std::sync::Arc::from(format!(
                "# scope=root year=2026 prev=- edition=1\nroll.2026q2 kind=change n=2 ord=0001..0007 dec=0 dg=x d={}\n",
                crate::hist::record_digest("roll.2026q2 kind=change n=2 ord=0001..0007 dec=0 dg=x")
            )),
        );
        let floor = ledger_ordinals(&ledgers);
        assert_eq!(
            floor.get(&("root".to_owned(), "change".to_owned())),
            Some(&7)
        );
        let doc = ChangeDoc {
            path: RepoPath::new("fairlead/docs/changes/d-00000009-a.md").unwrap(),
            id: DraftId::parse("d-00000009").unwrap(),
            kind: "change".into(),
            status: Status::Ready,
            impact: None,
            scope: None,
            title: None,
            covers: vec![],
            created: None,
            sealed: None,
            body_hash: None,
            ordinal: None,
            rolling: false,
            hotfix: false,
            extra: BTreeMap::new(),
        };
        let got = crate::docs::assign_ordinals_from(&[&doc], &[], &floor);
        assert_eq!(got.get(&doc.id), Some(&8));
    }
}
