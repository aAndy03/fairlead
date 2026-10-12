//! Change docs (20-versions): draft IDs, the status ladder and the frontmatter keys seal reads.
//! Pure: built from the extracted frontmatter of Markdown docs in the corpus.

use crate::extract::Corpus;
use crate::path::RepoPath;
use crate::version::{Impact, Version};
use std::collections::BTreeMap;
use std::fmt;

/// Doc kinds that follow the change-doc lifecycle and are sealed (20-versions, 21-lifecycle,
/// 22-editions). Other doc kinds (ADRs, handoffs, user kinds) are not change docs.
pub const CHANGE_KINDS: [&str; 9] = [
    "change",
    "chore",
    "mechanical",
    "bulk",
    "revert",
    "retarget",
    "early-removal",
    "revive",
    "upgrade",
];

const CROCKFORD: &str = "0123456789abcdefghjkmnpqrstvwxyz";

/// `d-` plus 8 lowercase Crockford base32 symbols (40 bits, R-VER-040). Not a `kind.name` ID
/// (R-VER-049).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DraftId(String);

impl DraftId {
    pub fn parse(s: &str) -> Option<DraftId> {
        let body = s.strip_prefix("d-")?;
        (body.len() == 8 && body.chars().all(|c| CROCKFORD.contains(c)))
            .then(|| DraftId(s.to_owned()))
    }

    /// The draft ID for 40 random bits.
    pub fn from_bits(bits: [u8; 5]) -> DraftId {
        DraftId(format!("d-{}", crate::hash::base32(&bits, 8)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DraftId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `draft -> ready -> sealed`, or `abandoned` (20-versions, concepts).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Status {
    Draft,
    Ready,
    Sealed,
    Abandoned,
}

impl Status {
    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "draft" => Some(Status::Draft),
            "ready" => Some(Status::Ready),
            "sealed" => Some(Status::Sealed),
            "abandoned" => Some(Status::Abandoned),
            _ => None,
        }
    }

    /// Pending docs decide the open version's bump: draft and ready (R-VER-002).
    pub fn is_pending(self) -> bool {
        matches!(self, Status::Draft | Status::Ready)
    }
}

/// The frontmatter of one change doc.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ChangeDoc {
    pub path: RepoPath,
    pub id: DraftId,
    pub kind: String,
    pub status: Status,
    pub impact: Option<Impact>,
    pub scope: Option<String>,
    pub title: Option<String>,
    pub covers: Vec<String>,
    pub created: Option<String>,
    pub sealed: Option<Version>,
    pub body_hash: Option<String>,
    pub ordinal: Option<u32>,
    pub rolling: bool,
    pub hotfix: bool,
    /// Other keys, kept verbatim (`of`, `supersedes`, `backport-of`, `revives`, `x-` keys...).
    pub extra: BTreeMap<String, String>,
}

fn str_key(t: &toml::Table, k: &str) -> Option<String> {
    t.get(k).and_then(|v| v.as_str()).map(str::to_owned)
}

/// Parses a doc's frontmatter table. `Ok(None)` when it is not a change doc (another kind or no
/// `kind`); `Err` names the first invalid key of a change doc.
pub fn parse_change_doc(path: &RepoPath, t: &toml::Table) -> Result<Option<ChangeDoc>, String> {
    let Some(kind) = str_key(t, "kind").filter(|k| CHANGE_KINDS.contains(&k.as_str())) else {
        return Ok(None);
    };
    let id = str_key(t, "id")
        .and_then(|s| DraftId::parse(&s))
        .ok_or("`id` must be a draft ID `d-` plus 8 base32 symbols")?;
    let status = str_key(t, "status")
        .and_then(|s| Status::parse(&s))
        .ok_or("`status` must be draft, ready, sealed or abandoned")?;
    let impact = match str_key(t, "impact") {
        None => None,
        Some(s) => Some(Impact::parse(&s).ok_or("`impact` must be break, add or fix")?),
    };
    let sealed = match str_key(t, "sealed") {
        None => None,
        Some(s) => Some(Version::parse(&s).ok_or("`sealed` must be MAJOR.MINOR.PATCH")?),
    };
    let covers = match t.get("covers") {
        None => Vec::new(),
        Some(v) => v
            .as_array()
            .ok_or("`covers` must be an array of strings")?
            .iter()
            .map(|x| x.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or("`covers` must be an array of strings")?,
    };
    let ordinal = match t.get("ordinal") {
        None => None,
        Some(v) => Some(
            v.as_integer()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or("`ordinal` must be a positive integer")?,
        ),
    };
    const KNOWN: [&str; 13] = [
        "id",
        "kind",
        "status",
        "impact",
        "scope",
        "covers",
        "created",
        "sealed",
        "body-hash",
        "ordinal",
        "rolling",
        "hotfix",
        "title",
    ];
    let extra = t
        .iter()
        .filter(|(k, _)| !KNOWN.contains(&k.as_str()))
        .map(|(k, v)| {
            (
                k.clone(),
                v.as_str().map_or_else(|| v.to_string(), str::to_owned),
            )
        })
        .collect();
    Ok(Some(ChangeDoc {
        path: path.clone(),
        id,
        kind,
        status,
        impact,
        scope: str_key(t, "scope"),
        title: str_key(t, "title"),
        covers,
        created: str_key(t, "created"),
        sealed,
        body_hash: str_key(t, "body-hash"),
        ordinal,
        rolling: t.get("rolling").and_then(|v| v.as_bool()).unwrap_or(false),
        hotfix: t.get("hotfix").and_then(|v| v.as_bool()).unwrap_or(false),
        extra,
    }))
}

/// R-PORT-024 (0.3): whether a doc reference names the doc `id` of `scope`. Outside its own
/// scope a doc is written `<scope>:d-<draft>`; an unqualified `d-<draft>` names it from any
/// scope (draft IDs are random, so the bare form is unambiguous in practice).
pub fn doc_ref_is(raw: &str, id: &DraftId, scope: &str) -> bool {
    match raw.trim().split_once(':') {
        Some((s, d)) => s == scope && d == id.as_str(),
        None => raw.trim() == id.as_str(),
    }
}

/// R-VER-060: a change doc that is not sealed (its `def`s declare nothing yet).
pub fn is_unsealed_change_doc(t: &toml::Table) -> bool {
    let s = |k: &str| t.get(k).and_then(|v| v.as_str());
    s("kind").is_some_and(|k| CHANGE_KINDS.contains(&k)) && s("status") != Some("sealed")
}

/// Every change doc in the corpus, sorted by path, plus `(path, problem)` for invalid ones.
pub fn collect_change_docs(corpus: &Corpus) -> (Vec<ChangeDoc>, Vec<(RepoPath, String)>) {
    let mut docs = Vec::new();
    let mut bad = Vec::new();
    for (path, rec) in &corpus.records {
        let Some(fm) = rec.extraction.as_ref().and_then(|e| e.frontmatter.as_ref()) else {
            continue;
        };
        match parse_change_doc(path, &fm.table) {
            Ok(Some(d)) => docs.push(d),
            Ok(None) => {}
            Err(e) => bad.push((path.clone(), e)),
        }
    }
    (docs, bad)
}

/// `b3:` and 20 base32 symbols of the normalised body after the frontmatter: written once by
/// seal (R-VER-012) and compared by `check` (R-VER-054, `H-SEAL-TAMPER`).
pub fn body_hash(text: &str) -> String {
    let body = text
        .strip_prefix("+++")
        .and_then(|r| r.split_once("\n+++"))
        .map_or(text, |(_, b)| b);
    format!(
        "b3:{}",
        crate::hash::base32(&crate::hash::hash_normalised(body.as_bytes()).0, 20)
    )
}

/// R-VER-030, R-CHK-045: whether a doc's `covers` globs match a file. Globs are relative to the
/// doc's home scope directory (`scope_dir`, empty for the root) and never reach outside it;
/// entries that parse as IDs are validated elsewhere and cover no path in 0.2.
pub fn covers_path(d: &ChangeDoc, scope_dir: &str, path: &RepoPath) -> bool {
    let rel = if scope_dir.is_empty() || scope_dir == "." {
        Some(path.as_str())
    } else {
        path.as_str()
            .strip_prefix(scope_dir)
            .and_then(|r| r.strip_prefix('/'))
    };
    let Some(rel) = rel else {
        return false;
    };
    d.covers.iter().any(|c| {
        fairlead_syntax::ids::Id::parse(c).is_err()
            && globset::Glob::new(c).is_ok_and(|g| g.compile_matcher().is_match(rel))
    })
}

/// Sealed versions: the file names `<folder>/versions/<MAJOR.MINOR.PATCH>.md` at the root, sorted.
pub fn version_files(corpus: &Corpus, folder: &str) -> Vec<Version> {
    let prefix = format!("{folder}/versions/");
    let mut v: Vec<Version> = corpus
        .records
        .keys()
        .filter_map(|p| {
            p.as_str()
                .strip_prefix(&prefix)
                .filter(|rest| !rest.contains('/'))
                .and_then(crate::version::version_of_file_name)
        })
        .collect();
    v.sort();
    v
}

/// The version line of a tree (R-VER-001 to R-VER-003): its version files and the impacts of
/// its pending change docs; `start` is `version.start` (R-VER-003).
pub fn line_of(corpus: &Corpus, folder: &str, start: Version) -> crate::version::VersionLine {
    let (docs, _) = collect_change_docs(corpus);
    let pending = docs
        .iter()
        .filter(|d| d.status.is_pending())
        .map(|d| d.impact.unwrap_or(Impact::Fix));
    crate::version::derive_line(version_files(corpus, folder), pending, start)
}

/// R-VER-031: what keeps a `ready` doc from being ready (empty when it is).
pub fn readiness_problems(d: &ChangeDoc) -> Vec<&'static str> {
    let mut out = Vec::new();
    if d.status != Status::Ready {
        return out;
    }
    if d.covers.is_empty() {
        out.push("`covers` is empty");
    }
    if d.impact.is_none() {
        out.push("`impact` is missing");
    }
    out
}

/// The (scope, kind) a doc's ordinal is counted in (R-VER-041): its `scope` key, else the root.
fn ordinal_key(d: &ChangeDoc) -> (String, String) {
    (
        d.scope.clone().unwrap_or_else(|| "root".to_owned()),
        d.kind.clone(),
    )
}

/// Comparison key of `created`: a version sorts by value, any other text after all versions.
fn created_key(d: &ChangeDoc) -> (Option<Version>, String) {
    let c = d.created.clone().unwrap_or_default();
    (Version::parse(&c), c)
}

/// R-VER-041: ordinals for the docs being sealed, per scope and kind, continuing from
/// `max(existing) + 1` in (`created`, draft ID) order. Docs that already carry an ordinal keep it.
pub fn assign_ordinals(sealing: &[&ChangeDoc], existing: &[&ChangeDoc]) -> BTreeMap<DraftId, u32> {
    let mut next: BTreeMap<(String, String), u32> = BTreeMap::new();
    for d in existing {
        if let Some(o) = d.ordinal {
            let e = next.entry(ordinal_key(d)).or_insert(0);
            *e = (*e).max(o);
        }
    }
    let mut order: Vec<&&ChangeDoc> = sealing.iter().filter(|d| d.ordinal.is_none()).collect();
    order.sort_by(|a, b| {
        (created_key(a), &a.id)
            .partial_cmp(&(created_key(b), &b.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out = BTreeMap::new();
    for d in order {
        let n = next.entry(ordinal_key(d)).or_insert(0);
        *n += 1;
        out.insert(d.id.clone(), *n);
    }
    for d in sealing {
        if let Some(o) = d.ordinal {
            out.insert(d.id.clone(), o);
        }
    }
    out
}

/// The alias map body of one scope (R-VER-046): `d-<draft> kind=<k> ord=<n>` per sealed doc that
/// has an ordinal, sorted by draft ID. A pure function of the docs; it holds no counter.
pub fn alias_body(docs: &[&ChangeDoc]) -> String {
    let mut rows: Vec<(&DraftId, String)> = docs
        .iter()
        .filter(|d| d.status == Status::Sealed)
        .filter_map(|d| {
            d.ordinal
                .map(|o| (&d.id, format!("{} kind={} ord={o:04}\n", d.id, d.kind)))
        })
        .collect();
    rows.sort();
    rows.into_iter().map(|(_, l)| l).collect()
}

/// R-VER-042 `H-ID-COLLISION`: groups of docs that share a draft ID but differ in content
/// (`hash_of` gives each doc's content hash). Identical copies are not a collision.
pub fn id_collisions<H: PartialEq>(
    docs: &[ChangeDoc],
    hash_of: impl Fn(&ChangeDoc) -> H,
) -> Vec<Vec<&ChangeDoc>> {
    let mut by_id: BTreeMap<&DraftId, Vec<&ChangeDoc>> = BTreeMap::new();
    for d in docs {
        by_id.entry(&d.id).or_default().push(d);
    }
    by_id
        .into_values()
        .filter(|g| g.len() > 1 && g.iter().any(|d| hash_of(d) != hash_of(g[0])))
        .collect()
}

/// R-VER-042 `H-ORD-DUP`: groups of docs with the same ordinal in one scope and kind.
pub fn ordinal_duplicates(docs: &[ChangeDoc]) -> Vec<Vec<&ChangeDoc>> {
    let mut by_ord: BTreeMap<((String, String), u32), Vec<&ChangeDoc>> = BTreeMap::new();
    for d in docs {
        if let Some(o) = d.ordinal {
            by_ord.entry((ordinal_key(d), o)).or_default().push(d);
        }
    }
    by_ord.into_values().filter(|g| g.len() > 1).collect()
}

/// A filename slug: lowercase ASCII words joined by `-`, at most 48 characters.
pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for word in title
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if !s.is_empty() {
            s.push('-');
        }
        s.push_str(&word.to_ascii_lowercase());
    }
    s.truncate(48);
    let s = s.trim_end_matches('-').to_owned();
    if s.is_empty() { "change".into() } else { s }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    fn table(s: &str) -> toml::Table {
        s.parse().unwrap()
    }

    fn p() -> RepoPath {
        RepoPath::new("fairlead/docs/changes/d-7k3m9qxa-diff.md").unwrap()
    }

    #[test]
    fn r_ver_040__draft_id_has_the_fixed_form() {
        assert!(DraftId::parse("d-7k3m9qxa").is_some());
        for bad in [
            "d-7k3m9qx",
            "d-7k3m9qxaa",
            "d-7K3M9QXA",
            "d-7k3m9qxi",
            "x-7k3m9qxa",
            "7k3m9qxa",
        ] {
            assert!(DraftId::parse(bad).is_none(), "{bad}");
        }
        let id = DraftId::from_bits([0xff, 0, 0x12, 0x34, 0x56]);
        assert!(DraftId::parse(id.as_str()).is_some(), "{id}");
    }

    #[test]
    fn r_ver_049__draft_id_is_not_a_kind_name_id() {
        assert!(fairlead_syntax::ids::Id::parse("d-7k3m9qxa").is_err());
    }

    #[test]
    fn r_ver_012__change_doc_frontmatter_parses() {
        let t = table(
            "id = \"d-7k3m9qxa\"\nkind = \"change\"\nstatus = \"sealed\"\nsealed = \"1.1.0\"\nbody-hash = \"b3:5f2e\"\nordinal = 42\nimpact = \"add\"\nscope = \"engines\"\ncovers = [\"src/diff/**\", \"engine.diff-v2\"]\ncreated = \"1.0.2\"\nx-team = \"core\"\n",
        );
        let d = parse_change_doc(&p(), &t).unwrap().unwrap();
        assert_eq!(d.status, Status::Sealed);
        assert_eq!(d.sealed, Version::parse("1.1.0"));
        assert_eq!(d.ordinal, Some(42));
        assert_eq!(d.covers.len(), 2);
        assert_eq!(d.created.as_deref(), Some("1.0.2"));
        assert_eq!(d.extra.get("x-team").map(String::as_str), Some("core"));
    }

    #[test]
    fn r_ver_012__other_doc_kinds_are_not_change_docs() {
        let t = table("kind = \"adr\"\nid = \"d-7k3m9qxa\"\n");
        assert_eq!(parse_change_doc(&p(), &t), Ok(None));
        assert_eq!(parse_change_doc(&p(), &table("title = \"x\"\n")), Ok(None));
    }

    #[test]
    fn r_ver_040__invalid_change_doc_names_the_key() {
        let t = table("kind = \"change\"\nid = \"chg-1\"\nstatus = \"draft\"\n");
        assert!(parse_change_doc(&p(), &t).unwrap_err().contains("`id`"));
        let t = table("kind = \"change\"\nid = \"d-7k3m9qxa\"\nstatus = \"done\"\n");
        assert!(parse_change_doc(&p(), &t).unwrap_err().contains("`status`"));
        let t =
            table("kind = \"change\"\nid = \"d-7k3m9qxa\"\nstatus = \"draft\"\nsealed = \"1.1\"\n");
        assert!(parse_change_doc(&p(), &t).unwrap_err().contains("`sealed`"));
    }

    #[test]
    fn r_ver_031__ready_requires_covers_and_impact() {
        let t = table("kind = \"change\"\nid = \"d-7k3m9qxa\"\nstatus = \"ready\"\n");
        let d = parse_change_doc(&p(), &t).unwrap().unwrap();
        assert_eq!(
            readiness_problems(&d),
            vec!["`covers` is empty", "`impact` is missing"]
        );
        let t = table("kind = \"change\"\nid = \"d-7k3m9qxa\"\nstatus = \"draft\"\n");
        let draft = parse_change_doc(&p(), &t).unwrap().unwrap();
        assert!(
            readiness_problems(&draft).is_empty(),
            "drafts may be incomplete"
        );
    }

    fn doc(id: &str, kind: &str, created: &str, ord: Option<u32>, status: Status) -> ChangeDoc {
        ChangeDoc {
            path: RepoPath::new(&format!("fairlead/docs/changes/{id}.md")).unwrap(),
            id: DraftId::parse(id).unwrap(),
            kind: kind.into(),
            status,
            impact: Some(Impact::Add),
            scope: Some("root".into()),
            title: None,
            covers: vec!["src/**".into()],
            created: Some(created.into()),
            sealed: None,
            body_hash: None,
            ordinal: ord,
            rolling: false,
            hotfix: false,
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn r_ver_041__ordinals_continue_per_scope_and_kind_in_created_then_id_order() {
        let old = doc("d-00000009", "change", "0.9.0", Some(41), Status::Sealed);
        let other_kind = doc("d-0000000a", "revert", "0.9.0", Some(7), Status::Sealed);
        let late = doc("d-00000001", "change", "0.10.0", None, Status::Ready);
        let early_b = doc("d-00000003", "change", "0.2.0", None, Status::Ready);
        let early_a = doc("d-00000002", "change", "0.2.0", None, Status::Ready);
        let rev = doc("d-00000004", "revert", "0.2.0", None, Status::Ready);
        let got = assign_ordinals(&[&late, &early_b, &early_a, &rev], &[&old, &other_kind]);
        let ord = |id: &str| got[&DraftId::parse(id).unwrap()];
        assert_eq!(
            (ord("d-00000002"), ord("d-00000003"), ord("d-00000001")),
            (42, 43, 44),
            "versions compare by value: 0.2.0 before 0.10.0, then draft ID"
        );
        assert_eq!(ord("d-00000004"), 8, "each kind counts on its own");
    }

    #[test]
    fn r_ver_046__alias_map_is_a_pure_function_of_docs() {
        let a = doc("d-00000002", "change", "0.2.0", Some(42), Status::Sealed);
        let b = doc("d-00000001", "change", "0.2.0", Some(43), Status::Sealed);
        let draft = doc("d-00000005", "change", "0.2.0", None, Status::Draft);
        let body = alias_body(&[&a, &b, &draft]);
        assert_eq!(
            body,
            "d-00000001 kind=change ord=0043\nd-00000002 kind=change ord=0042\n"
        );
        assert_eq!(
            body,
            alias_body(&[&draft, &b, &a]),
            "input order is irrelevant"
        );
    }

    #[test]
    fn r_ver_042__same_draft_id_with_a_different_body_collides() {
        let a = doc("d-00000001", "change", "0.2.0", None, Status::Draft);
        let mut b = a.clone();
        b.path = RepoPath::new("apps/x/fairlead/docs/changes/d-00000001.md").unwrap();
        let docs = vec![a.clone(), b.clone()];
        assert!(id_collisions(&docs, |_| 1).is_empty(), "identical copies");
        let g = id_collisions(&docs, |d| d.path.as_str().len());
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn r_ver_042__duplicate_ordinal_in_one_scope_and_kind() {
        let a = doc("d-00000001", "change", "0.2.0", Some(5), Status::Sealed);
        let b = doc("d-00000002", "change", "0.2.0", Some(5), Status::Sealed);
        let c = doc("d-00000003", "revert", "0.2.0", Some(5), Status::Sealed);
        let all = [a, b, c];
        let dups = ordinal_duplicates(&all);
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].len(), 2);
    }

    #[test]
    fn r_ver_040__slug_is_lowercase_ascii_words() {
        assert_eq!(
            slug("Adds diff v2; deprecates legacy!"),
            "adds-diff-v2-deprecates-legacy"
        );
        assert_eq!(slug("   "), "change");
    }
}
