//! Task queries (R-CTX-005, R-CTX-060 to R-CTX-062): free text that is neither a path nor an ID
//! is matched by deterministic BM25 over the tree's names and summaries. Pure.
use std::collections::{BTreeMap, BTreeSet};

use crate::extract::Corpus;
use crate::path::RepoPath;
use crate::registry::{FieldValue, Registry, ScopeId};

/// What a document of the index is.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DocKind {
    /// A graveyard entry: listed first among the hits (R-LC-034).
    Grave,
    Item,
    Doc,
    Tomb,
}

impl DocKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DocKind::Grave => "grv",
            DocKind::Item => "item",
            DocKind::Doc => "doc",
            DocKind::Tomb => "tomb",
        }
    }
}

/// One indexed document.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QDoc {
    pub kind: DocKind,
    pub scope: ScopeId,
    /// The ID or path it stands for.
    pub key: String,
    /// Its title, shown (fenced) with a hit.
    pub title: String,
    pub path: Option<RepoPath>,
    terms: Vec<String>,
}

/// The index: documents and their document frequencies.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub docs: Vec<QDoc>,
    df: BTreeMap<String, u32>,
    avg_len: f64,
}

/// Lowercase alphanumeric words of two or more characters; `kind.name-part` splits into words.
pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(str::to_lowercase)
        .collect()
}

fn field_text(v: &FieldValue) -> String {
    match v {
        FieldValue::Scalar(s) => s.clone(),
        FieldValue::List(l) => l.join(" "),
    }
}

/// R-CTX-060: the index over ID slugs, item titles (`sum`), `kw=` tags, doc headings (change
/// docs' titles, Markdown `# ` headings), graveyard entries and tombstone summaries.
pub fn build(
    reg: &Registry,
    corpus: &Corpus,
    tombstones: &BTreeMap<RepoPath, std::sync::Arc<str>>,
) -> Index {
    let mut docs = Vec::new();
    for it in reg.items() {
        let mut text = format!("{} {}", it.id.kind.as_str(), it.id.name.as_str());
        if let Some(s) = &it.sum {
            text.push(' ');
            text.push_str(s);
        }
        if let Some(kw) = it.fields.get("kw") {
            text.push(' ');
            text.push_str(&field_text(kw));
        }
        docs.push(QDoc {
            kind: DocKind::Item,
            scope: it.id.scope.clone(),
            key: format!("{}.{}", it.id.kind.as_str(), it.id.name.as_str()),
            title: it.sum.clone().unwrap_or_default(),
            path: None,
            terms: tokens(&text),
        });
    }
    let (changes, _) = crate::docs::collect_change_docs(corpus);
    let change_paths: BTreeSet<&RepoPath> = changes.iter().map(|d| &d.path).collect();
    for d in &changes {
        let title = d.title.clone().unwrap_or_default();
        let sum = d.extra.get("sum").cloned().unwrap_or_default();
        docs.push(QDoc {
            kind: DocKind::Doc,
            scope: reg
                .scope_of(&d.path)
                .cloned()
                .unwrap_or_else(|| reg.root_scope().clone()),
            key: d.id.as_str().to_owned(),
            title: title.clone(),
            path: Some(d.path.clone()),
            terms: tokens(&format!("{title} {sum}")),
        });
    }
    let (graves, _) = crate::graveyard::collect(corpus);
    for g in &graves {
        let terms: Vec<String> = g
            .terms
            .iter()
            .map(|t| match t {
                crate::graveyard::Term::Words(w)
                | crate::graveyard::Term::Regex(w)
                | crate::graveyard::Term::Glob(w) => w.clone(),
            })
            .collect();
        docs.push(QDoc {
            kind: DocKind::Grave,
            scope: reg
                .scope_of(&g.path)
                .cloned()
                .unwrap_or_else(|| reg.root_scope().clone()),
            key: g.id.clone(),
            title: g.what.clone(),
            path: Some(g.path.clone()),
            terms: tokens(&format!(
                "{} {} {} {}",
                g.id,
                g.what,
                g.why,
                terms.join(" ")
            )),
        });
    }
    for (p, rec) in &corpus.records {
        if change_paths.contains(p)
            || crate::graveyard::is_graveyard_path(p)
            || !p.as_str().ends_with(".md")
        {
            continue;
        }
        let Some(ex) = rec.extraction.as_ref() else {
            continue;
        };
        if ex.frontmatter.is_none() {
            continue;
        }
        // a registry doc's `title`, else its file name
        let heading = ex
            .frontmatter
            .as_ref()
            .and_then(|f| f.table.get("title"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .or_else(|| rec.path.file_name().strip_suffix(".md").map(str::to_owned))
            .unwrap_or_default();
        docs.push(QDoc {
            kind: DocKind::Doc,
            scope: reg
                .scope_of(p)
                .cloned()
                .unwrap_or_else(|| reg.root_scope().clone()),
            key: p.as_str().to_owned(),
            title: heading.clone(),
            path: Some(p.clone()),
            terms: tokens(&heading),
        });
    }
    for (p, text) in tombstones {
        for block in text.split("[[tomb]]").skip(1) {
            let val = |k: &str| {
                block
                    .lines()
                    .find_map(|l| l.trim().strip_prefix(&format!("{k} = \"")))
                    .and_then(|r| r.strip_suffix('"'))
                    .unwrap_or("")
                    .to_owned()
            };
            let id = val("id");
            if id.is_empty() {
                continue;
            }
            let why = val("why");
            docs.push(QDoc {
                kind: DocKind::Tomb,
                scope: reg
                    .scope_of(p)
                    .cloned()
                    .unwrap_or_else(|| reg.root_scope().clone()),
                key: id.clone(),
                title: why.clone(),
                path: Some(p.clone()),
                terms: tokens(&format!("{id} {why}")),
            });
        }
    }
    let mut df: BTreeMap<String, u32> = BTreeMap::new();
    let mut total = 0usize;
    for d in &docs {
        total += d.terms.len();
        for t in d.terms.iter().collect::<BTreeSet<_>>() {
            *df.entry(t.clone()).or_insert(0) += 1;
        }
    }
    let avg_len = if docs.is_empty() {
        1.0
    } else {
        total as f64 / docs.len() as f64
    };
    Index { docs, df, avg_len }
}

/// One scored document.
#[derive(Clone, PartialEq, Debug)]
pub struct Hit<'a> {
    pub doc: &'a QDoc,
    pub score: f64,
}

impl Index {
    /// R-CTX-060: BM25 (k1 = 1.2, b = 0.75) of every document with a positive score, best first;
    /// ties by kind (graveyard first), scope ID, then key, so two runs order alike.
    pub fn search(&self, query: &str) -> Vec<Hit<'_>> {
        let q: BTreeSet<String> = tokens(query).into_iter().collect();
        let n = self.docs.len() as f64;
        let (k1, b) = (1.2, 0.75);
        let mut out: Vec<Hit<'_>> = self
            .docs
            .iter()
            .filter_map(|d| {
                let len = d.terms.len() as f64;
                let mut score = 0.0;
                for t in &q {
                    let tf = d.terms.iter().filter(|x| *x == t).count() as f64;
                    if tf == 0.0 {
                        continue;
                    }
                    let df = f64::from(*self.df.get(t).unwrap_or(&0));
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    score += idf * tf * (k1 + 1.0)
                        / (tf + k1 * (1.0 - b + b * len / self.avg_len.max(1.0)));
                }
                // rounded so platform float noise never reorders results
                let score = (score * 1000.0).round() / 1000.0;
                (score > 0.0).then_some(Hit { doc: d, score })
            })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.doc.kind.cmp(&b.doc.kind))
                .then_with(|| a.doc.scope.cmp(&b.doc.scope))
                .then_with(|| a.doc.key.cmp(&b.doc.key))
        });
        out
    }
}

/// R-CTX-061: scopes ranked by the sum of their hits' scores, best first, ties by scope ID.
pub fn rank_scopes(hits: &[Hit<'_>]) -> Vec<(ScopeId, f64)> {
    let mut by: BTreeMap<ScopeId, f64> = BTreeMap::new();
    for h in hits {
        *by.entry(h.doc.scope.clone()).or_insert(0.0) += h.score;
    }
    let mut v: Vec<(ScopeId, f64)> = by
        .into_iter()
        .map(|(s, x)| (s, (x * 1000.0).round() / 1000.0))
        .collect();
    v.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::World;

    fn world() -> crate::check::testkit::Built {
        World::new()
            .root("[kinds.cmp]\nshape = \"doc\"\n")
            .file(
                "fairlead/contract/a.md",
                "+++\ndirectives = ['def cmp.progress-bar sum=\"Shows upload progress\" kw=upload,progress']\n+++\n# A\n",
            )
            .scope("web", "[fairlead]\nscope = \"web\"\n")
            .scope("api", "[fairlead]\nscope = \"api\"\n")
            .file("web/fairlead/contract/w.md", "+++\ndirectives = ['def cmp.toast sum=\"Notification toast\"']\n+++\n# W\n")
            .file("api/fairlead/contract/a.md", "+++\ndirectives = ['def cmp.toast sum=\"Notification toast\"']\n+++\n# A\n")
            .build()
    }

    #[test]
    fn r_ctx_060__bm25_is_deterministic_across_runs() {
        let b = world();
        let run = || {
            let idx = build(&b.registry, &b.corpus, &BTreeMap::new());
            idx.search("upload progress bar")
                .iter()
                .map(|h| format!("{} {} {}", h.doc.scope.0.as_str(), h.doc.key, h.score))
                .collect::<Vec<_>>()
        };
        let a = run();
        assert_eq!(a, run());
        assert!(a[0].starts_with("root cmp.progress-bar"), "{a:?}");
    }

    #[test]
    fn r_ctx_062__ties_break_by_scope_id() {
        let b = world();
        let idx = build(&b.registry, &b.corpus, &BTreeMap::new());
        let hits = idx.search("toast");
        let ranked = rank_scopes(&hits);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].1, ranked[1].1, "same score");
        assert_eq!(
            (ranked[0].0.0.as_str(), ranked[1].0.0.as_str()),
            ("api", "web"),
            "a tie breaks by scope ID"
        );
    }

    #[test]
    fn r_ctx_062__no_match_is_empty_never_a_guess() {
        let b = world();
        let idx = build(&b.registry, &b.corpus, &BTreeMap::new());
        assert!(idx.search("zebra quantum").is_empty());
        assert!(rank_scopes(&idx.search("zebra")).is_empty());
    }
}
