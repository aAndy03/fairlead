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

    #[test]
    fn r_ver_040__slug_is_lowercase_ascii_words() {
        assert_eq!(
            slug("Adds diff v2; deprecates legacy!"),
            "adds-diff-v2-deprecates-legacy"
        );
        assert_eq!(slug("   "), "change");
    }
}
