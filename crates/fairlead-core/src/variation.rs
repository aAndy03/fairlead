//! The variation protocol model (31-variation-protocol): requests, their answers, interim
//! wrappers (`flag awaiting`) and the derived request status (R-VAR-030 to R-VAR-064). Pure.
use std::collections::BTreeMap;

use fairlead_syntax::directive::{ArgValue, Directive, Verb};

use crate::docs::{ChangeDoc, DraftId, Status as DocStatus};
use crate::extract::Corpus;
use crate::hash::Fingerprint;
use crate::path::RepoPath;
use crate::registry::{ItemId, Lookup, Registry, ScopeId};
use crate::version::Version;

/// The fields `new request` requires (R-VAR-030).
pub const REQUEST_FIELDS: [&str; 5] = ["need", "want", "because", "sum", "out"];

/// The answer types (R-VAR-040).
pub const ANSWER_TYPES: [&str; 3] = ["variant", "axis", "rejected"];

/// A consumer's request for a variation (R-VAR-030): a doc with `kind = "request"`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Request {
    pub path: RepoPath,
    pub id: Option<DraftId>,
    /// `need`, `want`, `because`, `sum`, `out` as written; a missing one is absent.
    pub fields: BTreeMap<String, String>,
    pub created: Option<Version>,
}

impl Request {
    pub fn field(&self, k: &str) -> Option<&str> {
        self.fields.get(k).map(String::as_str)
    }
    /// The required fields that are missing (`out` may be empty, the others may not).
    pub fn missing(&self) -> Vec<&'static str> {
        REQUEST_FIELDS
            .iter()
            .filter(|k| match self.field(k) {
                None => true,
                Some(v) => **k != "out" && v.trim().is_empty(),
            })
            .copied()
            .collect()
    }
}

/// Every request doc of the tree, by path.
pub fn collect_requests(corpus: &Corpus) -> Vec<Request> {
    let mut out = Vec::new();
    for (path, rec) in &corpus.records {
        let Some(fm) = rec.extraction.as_ref().and_then(|e| e.frontmatter.as_ref()) else {
            continue;
        };
        let t = &fm.table;
        if t.get("kind").and_then(|v| v.as_str()) != Some("request") {
            continue;
        }
        let s = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_owned);
        let fields = REQUEST_FIELDS
            .iter()
            .filter_map(|k| Some(((*k).to_owned(), s(k)?)))
            .collect();
        out.push(Request {
            path: path.clone(),
            id: s("id").and_then(|i| DraftId::parse(&i)),
            fields,
            created: s("created").and_then(|v| Version::parse(&v)),
        });
    }
    out
}

/// A change doc that answers requests (R-VAR-040).
#[derive(Clone, Debug)]
pub struct Answer<'a> {
    pub doc: &'a ChangeDoc,
    pub answers: Vec<String>,
    pub kind: Option<String>,
    pub approved: bool,
}

impl Answer<'_> {
    pub fn rejected(&self) -> bool {
        self.kind.as_deref() == Some("rejected")
            || self.doc.extra.get("result").map(String::as_str) == Some("rejected")
    }
}

/// R-VAR-043: the fingerprint `fairlead approve <draft>` records for an answer.
pub fn answer_fp(draft: &str, answers: &[String], kind: &str) -> Fingerprint {
    let mut a = answers.to_vec();
    a.sort();
    crate::hash::Fingerprint::from_hash(&crate::hash::hash_raw(
        format!("answer {draft} {kind} {}", a.join(",")).as_bytes(),
    ))
}

/// Every change doc with a non-empty `answers`, abandoned docs excluded.
pub fn answers<'a>(
    docs: &'a [ChangeDoc],
    approvals: &crate::config::ApprovalsFile,
) -> Vec<Answer<'a>> {
    docs.iter()
        .filter(|d| d.status != DocStatus::Abandoned)
        .filter_map(|d| {
            let answers: Vec<String> = crate::lifecycle::doc_ids(d.extra.get("answers")?)
                .into_iter()
                .filter(|a| !a.trim().is_empty())
                .collect();
            if answers.is_empty() {
                return None;
            }
            let kind = d.extra.get("type").cloned();
            let fp = answer_fp(d.id.as_str(), &answers, kind.as_deref().unwrap_or(""));
            let approved = approvals.approvals.iter().any(|a| a.fp == fp);
            Some(Answer {
                doc: d,
                answers,
                kind,
                approved,
            })
        })
        .collect()
}

/// One `flag awaiting to=<request> target=<version>` (R-VAR-060).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Awaiting {
    pub path: RepoPath,
    pub line: u32,
    pub col: u32,
    pub to: Option<String>,
    pub target: Option<String>,
    pub by_date: Option<String>,
    /// The wrapper: the nearest `def` above the flag in the same file.
    pub wrapper: Option<ItemId>,
}

fn arg(d: &Directive, k: &str) -> Option<String> {
    match &d.arg(k)?.value {
        ArgValue::Word(w) | ArgValue::Quoted(w) => Some(w.clone()),
        ArgValue::List(l) => Some(l.join(",")),
    }
}

/// Every `awaiting` flag of the tree.
pub fn awaiting(reg: &Registry, corpus: &Corpus) -> Vec<Awaiting> {
    let mut out = Vec::new();
    for (path, rec) in &corpus.records {
        let (Some(ex), Some(scope)) = (rec.extraction.as_ref(), reg.scope_of(path)) else {
            continue;
        };
        let mut last_def: Option<ItemId> = None;
        for d in &ex.directives {
            if d.verb == Verb::Def
                && let Some(id) = d.ids.first()
                && let Lookup::Found(i) = reg.lookup(scope, id)
            {
                last_def = Some(i);
            }
            if d.verb != Verb::Flag || d.bare.first().map(String::as_str) != Some("awaiting") {
                continue;
            }
            let wrapper = match d.ids.first() {
                Some(id) => match reg.lookup(scope, id) {
                    Lookup::Found(i) => Some(i),
                    _ => None,
                },
                None => last_def.clone(),
            };
            out.push(Awaiting {
                path: path.clone(),
                line: d.span.line,
                col: d.span.col,
                to: arg(d, "to"),
                target: arg(d, "target"),
                by_date: arg(d, "exp").or_else(|| arg(d, "by-date")),
                wrapper,
            });
        }
    }
    out
}

/// R-VAR-031: request status, derived, never stored.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ReqStatus {
    /// No approved answer.
    Open,
    /// An approved answer, and an `awaiting` flag still names it.
    Answered,
    /// An approved answer and no `awaiting` flag names it.
    Closed,
    /// An approved rejection.
    Rejected,
}

impl ReqStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ReqStatus::Open => "open",
            ReqStatus::Answered => "answered",
            ReqStatus::Closed => "closed",
            ReqStatus::Rejected => "rejected",
        }
    }
}

pub fn status(req: &DraftId, answers: &[Answer<'_>], awaiting: &[Awaiting]) -> ReqStatus {
    let mine: Vec<&Answer<'_>> = answers
        .iter()
        .filter(|a| a.approved && a.answers.iter().any(|x| x == req.as_str()))
        .collect();
    if mine.is_empty() {
        return ReqStatus::Open;
    }
    if mine.iter().any(|a| a.rejected()) {
        return ReqStatus::Rejected;
    }
    if awaiting
        .iter()
        .any(|w| w.to.as_deref() == Some(req.as_str()))
    {
        ReqStatus::Answered
    } else {
        ReqStatus::Closed
    }
}

/// R-VAR-032: sealed versions after the request's `created` version.
pub fn seals_since(created: Option<Version>, sealed: &[Version]) -> usize {
    match created {
        Some(c) => sealed.iter().filter(|v| **v > c).count(),
        None => 0,
    }
}

/// The scope a request or answer belongs to: its file's scope.
pub fn scope_of<'r>(reg: &'r Registry, path: &RepoPath) -> &'r ScopeId {
    reg.scope_of(path).unwrap_or(reg.root_scope())
}

/// R-VAR-063: the exception-ledger records of the interim wrappers, written by each seal to the
/// wrapper's scope: `<kind.name> kind=awaiting to=<request> target=<version>`.
pub fn awaiting_records(reg: &Registry, corpus: &Corpus) -> Vec<(RepoPath, String)> {
    awaiting(reg, corpus)
        .into_iter()
        .filter_map(|w| {
            let (to, target) = (w.to?, w.target?);
            let id = w.wrapper.as_ref().map_or_else(
                || "flag.awaiting".to_owned(),
                |i| format!("{}.{}", i.kind.as_str(), i.name.as_str()),
            );
            let line = crate::render::line::TerseLine::new(&id)
                .with("kind", "awaiting")
                .with("to", &to)
                .with("target", &target)
                .render();
            Some((w.path, line))
        })
        .collect()
}
