//! The graveyard (R-LC-030 to R-LC-039): rejected proposals as TOML-frontmatter docs in
//! `docs/graveyard/`, their match terms, and fingerprints that catch re-introduction under a new
//! name. Matching is lexical and fingerprint-based; synonyms and rewrites evade it (R-LC-039).
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::path::RepoPath;

/// Shingle size and min-hash width (R-LC-031).
pub const SHINGLE_K: usize = 5;
pub const MINHASH_N: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GrvState {
    Rejected,
    Parked,
    Revisited,
}

impl GrvState {
    fn parse(s: &str) -> Option<GrvState> {
        Some(match s {
            "rejected" => GrvState::Rejected,
            "parked" => GrvState::Parked,
            "revisited" => GrvState::Revisited,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            GrvState::Rejected => "rejected",
            GrvState::Parked => "parked",
            GrvState::Revisited => "revisited",
        }
    }
}

/// A `match` term: `/regex/`, a glob (has `*`, `?` or `/`), else words matched whole.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Term {
    Words(String),
    Glob(String),
    Regex(String),
}

impl Term {
    pub fn parse(s: &str) -> Term {
        if s.len() > 2 && s.starts_with('/') && s.ends_with('/') {
            Term::Regex(s[1..s.len() - 1].to_string())
        } else if s.contains(['*', '?', '/']) {
            Term::Glob(s.to_string())
        } else {
            Term::Words(s.to_lowercase())
        }
    }
    pub fn text(&self) -> &str {
        match self {
            Term::Words(s) | Term::Glob(s) | Term::Regex(s) => s,
        }
    }
}

/// The stored fingerprint: normalised identifiers and 32 min-hash values (R-LC-031).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Fingerprint {
    pub ids: BTreeSet<String>,
    pub minhash: Vec<u32>,
}

impl Fingerprint {
    /// `b3:` and 8 hex digits per value; empty when no material was found.
    pub fn minhash_text(&self) -> String {
        if self.minhash.is_empty() {
            return String::new();
        }
        let hex: String = self.minhash.iter().map(|v| format!("{v:08x}")).collect();
        format!("b3:{hex}")
    }
    pub fn parse_minhash(s: &str) -> Vec<u32> {
        let Some(hex) = s.strip_prefix("b3:") else {
            return Vec::new();
        };
        if hex.len() != MINHASH_N * 8 {
            return Vec::new();
        }
        (0..MINHASH_N)
            .filter_map(|i| hex.get(i * 8..i * 8 + 8))
            .filter_map(|h| u32::from_str_radix(h, 16).ok())
            .collect()
    }
    /// The inline TOML table seal writes.
    pub fn toml_inline(&self) -> String {
        let ids: Vec<String> = self.ids.iter().map(|i| format!("{i:?}")).collect();
        format!(
            "{{ ids = [{}], minhash = {:?} }}",
            ids.join(", "),
            self.minhash_text()
        )
    }
}

/// One graveyard entry (R-LC-030).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    pub path: RepoPath,
    pub id: String,
    pub state: GrvState,
    pub at: Option<String>,
    pub what: String,
    pub why: String,
    pub terms: Vec<Term>,
    pub revisit_after: Option<String>,
    pub fingerprint: Option<Fingerprint>,
    /// Paths of the rejected material, read once by the seal that fingerprints the entry.
    pub material: Vec<String>,
    pub tripwire: u32,
}

/// A graveyard doc lives in a `docs/graveyard/` folder.
pub fn is_graveyard_path(p: &RepoPath) -> bool {
    let s = p.as_str();
    s.ends_with(".md") && (s.contains("/docs/graveyard/") || s.starts_with("docs/graveyard/"))
}

fn strs(t: &toml::Table, k: &str) -> Vec<String> {
    match t.get(k) {
        Some(toml::Value::String(s)) => vec![s.clone()],
        Some(toml::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Parses an entry's frontmatter. `Err` names the first problem.
pub fn parse_entry(path: &RepoPath, t: &toml::Table) -> Result<Entry, String> {
    let s = |k: &str| t.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let id = s("id").ok_or("a graveyard entry needs `id`")?;
    if !id.starts_with("grv.") {
        return Err(format!(
            "graveyard id {id:?} must use the reserved kind grv"
        ));
    }
    let state = s("state")
        .as_deref()
        .and_then(GrvState::parse)
        .ok_or("`state` is rejected, parked or revisited")?;
    let fingerprint = t
        .get("fingerprint")
        .and_then(|v| v.as_table())
        .map(|f| Fingerprint {
            ids: strs(f, "ids").into_iter().collect(),
            minhash: f
                .get("minhash")
                .and_then(|m| m.as_str())
                .map(Fingerprint::parse_minhash)
                .unwrap_or_default(),
        });
    Ok(Entry {
        path: path.clone(),
        id,
        state,
        at: s("at"),
        what: s("what").unwrap_or_default(),
        why: s("why").unwrap_or_default(),
        terms: strs(t, "match").iter().map(|m| Term::parse(m)).collect(),
        revisit_after: s("revisit-after"),
        fingerprint,
        material: strs(t, "material"),
        tripwire: t
            .get("tripwire")
            .and_then(|v| v.as_integer())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
    })
}

fn normalise(id: &str) -> String {
    id.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

const DECL: [&str; 12] = [
    "fn",
    "struct",
    "enum",
    "trait",
    "type",
    "const",
    "mod",
    "function",
    "class",
    "interface",
    "let",
    "var",
];

/// Public symbol names, wire and route strings and the file basename, normalised (R-LC-031 a).
pub fn identifiers(path: &str, text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let base = path.rsplit('/').next().unwrap_or(path);
    let stem = base.split('.').next().unwrap_or(base);
    if !stem.is_empty() {
        out.insert(normalise(stem));
    }
    for line in text.lines() {
        let l = line.trim_start();
        let public = l.starts_with("pub ") || l.starts_with("export ");
        if public {
            let words: Vec<&str> = l
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
                .filter(|w| !w.is_empty())
                .collect();
            if let Some(i) = words.iter().position(|w| DECL.contains(w))
                && let Some(name) = words.get(i + 1)
            {
                out.insert(normalise(name));
            }
        }
        for q in line.split('"').skip(1).step_by(2) {
            if q.starts_with('/')
                && q.len() > 1
                && q.chars().any(|c| c.is_ascii_alphabetic())
                && !q.contains(' ')
            {
                out.insert(q.to_lowercase());
            }
        }
    }
    out.retain(|s| !s.is_empty());
    out
}

/// Tokens with comment lines and case removed (R-LC-031 b: whitespace and comment normalised).
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim_start();
        if ["//", "#", "/*", "*", "--", "<!--"]
            .iter()
            .any(|c| l.starts_with(c))
        {
            continue;
        }
        out.extend(
            l.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
                .map(str::to_lowercase),
        );
    }
    out
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn seeds() -> [u64; MINHASH_N] {
    let mut out = [0u64; MINHASH_N];
    for (i, s) in out.iter_mut().enumerate() {
        let key = [u8::try_from(i).unwrap_or(0); 32];
        let h = blake3::keyed_hash(&key, b"fairlead-minhash");
        let mut b = [0u8; 8];
        b.copy_from_slice(&h.as_bytes()[..8]);
        *s = u64::from_le_bytes(b);
    }
    out
}

/// 32 min-hash values over k=5 token shingles of `texts` together; empty without tokens.
pub fn minhash<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<u32> {
    let mut toks = Vec::new();
    for t in texts {
        toks.extend(tokens(t));
    }
    if toks.is_empty() {
        return Vec::new();
    }
    let seeds = seeds();
    let mut mins = [u32::MAX; MINHASH_N];
    let k = SHINGLE_K.min(toks.len());
    for w in toks.windows(k) {
        let h = blake3::hash(w.join(" ").as_bytes());
        let mut b = [0u8; 8];
        b.copy_from_slice(&h.as_bytes()[..8]);
        let h = u64::from_le_bytes(b);
        for (m, s) in mins.iter_mut().zip(seeds) {
            let v = u32::try_from(splitmix(h ^ s) >> 32).unwrap_or(u32::MAX);
            *m = (*m).min(v);
        }
    }
    mins.to_vec()
}

/// The fingerprint of the rejected or removed files, computed once (R-LC-031).
pub fn fingerprint(files: &[(String, String)]) -> Fingerprint {
    let mut ids = BTreeSet::new();
    for (p, t) in files {
        ids.extend(identifiers(p, t));
    }
    Fingerprint {
        ids,
        minhash: minhash(files.iter().map(|(_, t)| t.as_str())),
    }
}

/// Fraction of equal min-hash positions (an estimate of shingle Jaccard similarity).
pub fn similarity(a: &[u32], b: &[u32]) -> f64 {
    if a.len() != MINHASH_N || b.len() != MINHASH_N {
        return 0.0;
    }
    let same = a.iter().zip(b).filter(|(x, y)| x == y).count();
    same as f64 / MINHASH_N as f64
}

/// Share of the entry's identifiers found in the candidate, and the counts.
pub fn id_overlap(entry: &BTreeSet<String>, cand: &BTreeSet<String>) -> (f64, usize, usize) {
    if entry.is_empty() {
        return (0.0, 0, 0);
    }
    let hit = entry.intersection(cand).count();
    (hit as f64 / entry.len() as f64, hit, entry.len())
}

/// A candidate fingerprint matches when identifier overlap >= `ids_pct`% or shingle similarity
/// >= `sh_pct`% (R-LC-031). Returns the reason text.
pub fn fp_match(
    entry: &Fingerprint,
    cand: &Fingerprint,
    ids_pct: u32,
    sh_pct: u32,
) -> Option<String> {
    let sh = similarity(&entry.minhash, &cand.minhash);
    let (ov, hit, of) = id_overlap(&entry.ids, &cand.ids);
    let ids_hit = of > 0 && ov * 100.0 >= f64::from(ids_pct);
    let sh_hit = !entry.minhash.is_empty() && sh * 100.0 >= f64::from(sh_pct);
    (ids_hit || sh_hit).then(|| format!("shingles {sh:.2}, ids {hit}/{of}"))
}

/// Whole-word, case-insensitive match of a words term in `text`.
pub fn words_hit(term: &str, text: &str) -> bool {
    let hay: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let needle: Vec<&str> = term.split_whitespace().collect();
    !needle.is_empty()
        && hay
            .windows(needle.len())
            .any(|w| w.iter().zip(&needle).all(|(a, b)| a == b))
}

/// The word and regex terms of `entry` that `text` hits.
pub fn text_hits<'e>(entry: &'e Entry, text: &str) -> Vec<&'e Term> {
    entry
        .terms
        .iter()
        .filter(|t| match t {
            Term::Words(w) => words_hit(w, text),
            Term::Regex(r) => regex::Regex::new(r).is_ok_and(|re| re.is_match(text)),
            Term::Glob(_) => false,
        })
        .collect()
}

/// The glob terms of `entry` that `path` (relative to the entry's scope) hits.
pub fn path_hits<'e>(entry: &'e Entry, rel: &str) -> Vec<&'e Term> {
    entry
        .terms
        .iter()
        .filter(|t| match t {
            Term::Glob(g) => globset::Glob::new(g).is_ok_and(|g| g.compile_matcher().is_match(rel)),
            _ => false,
        })
        .collect()
}

/// Every graveyard entry of the corpus, and the entries that do not parse.
pub fn collect(corpus: &crate::extract::Corpus) -> (Vec<Entry>, Vec<(RepoPath, String)>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for (path, rec) in &corpus.records {
        if !is_graveyard_path(path) {
            continue;
        }
        let Some(fm) = rec.extraction.as_ref().and_then(|e| e.frontmatter.as_ref()) else {
            continue;
        };
        match parse_entry(path, &fm.table) {
            Ok(e) => out.push(e),
            Err(e) => bad.push((path.clone(), e)),
        }
    }
    (out, bad)
}

/// The entry doc with `fingerprint` set (every other key and the body kept).
pub fn with_fingerprint(text: &str, fp: &Fingerprint) -> Result<String, String> {
    set_keys(text, |doc| {
        let mut ids = toml_edit::Array::new();
        for i in &fp.ids {
            ids.push(i.as_str());
        }
        let mut t = toml_edit::InlineTable::new();
        t.insert("ids", toml_edit::Value::Array(ids));
        t.insert("minhash", toml_edit::Value::from(fp.minhash_text()));
        doc["fingerprint"] = toml_edit::value(t);
    })
}

/// Rewrites a TOML-frontmatter doc's frontmatter through `f`, keeping the body byte for byte.
pub fn set_keys(text: &str, f: impl FnOnce(&mut toml_edit::DocumentMut)) -> Result<String, String> {
    let rest = text
        .strip_prefix("+++\n")
        .ok_or("doc does not start with +++ frontmatter")?;
    let (fm, body) = rest
        .split_once("\n+++")
        .ok_or("frontmatter has no closing +++")?;
    let mut doc: toml_edit::DocumentMut = fm.parse().map_err(|e| format!("{e}"))?;
    f(&mut doc);
    let mut fm = doc.to_string();
    if !fm.ends_with('\n') {
        fm.push('\n');
    }
    Ok(format!("+++\n{fm}+++{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const TOGGLE: &str = "import { useTheme } from './theme';\nexport function ThemeToggle() {\n  const { dark, setDark } = useTheme();\n  return <button onClick={() => setDark(!dark)}>toggle</button>;\n}\nfetch(\"/api/theme\");\n";
    const RENAMED: &str = "import { useTheme } from './theme';\nexport function NightSwitch() {\n  const { dark, setDark } = useTheme();\n  return <button onClick={() => setDark(!dark)}>toggle</button>;\n}\nfetch(\"/api/theme\");\n";
    const OTHER: &str = "export const add = (a: number, b: number) => a + b;\nexport const sub = (a: number, b: number) => a - b;\n";

    /// A realistic body shared by the rejected file and its rename.
    const BODY: &str = "const palette = { light: '#fff', dark: '#000', accent: '#2563eb' };
function applyTheme(mode) {
  document.documentElement.dataset.theme = mode;
  localStorage.setItem('theme', mode);
  window.dispatchEvent(new CustomEvent('themechange', { detail: mode }));
}
function readTheme() {
  return localStorage.getItem('theme') || 'light';
}
";

    fn fp(p: &str, t: &str) -> Fingerprint {
        fingerprint(&[(p.to_string(), format!("{t}{BODY}"))])
    }

    proptest! {
        #[test]
        fn r_lc_031__fingerprint_is_deterministic(text in "[a-z ]{0,200}") {
            let a = fp("src/x.ts", &text);
            let b = fp("src/x.ts", &text);
            prop_assert_eq!(&a, &b);
            prop_assert!(a.minhash.is_empty() || a.minhash.len() == MINHASH_N);
            let back = Fingerprint::parse_minhash(&a.minhash_text());
            prop_assert_eq!(back, a.minhash);
        }
    }

    #[test]
    fn r_lc_031__thresholds_match_and_miss() {
        let entry = fp("ui/ThemeToggle.tsx", TOGGLE);
        assert!(entry.ids.contains("themetoggle") && entry.ids.contains("/api/theme"));
        let renamed = fp("ui/NightSwitch.tsx", RENAMED);
        let why = fp_match(&entry, &renamed, 60, 50).expect("a rename is caught");
        assert!(why.starts_with("shingles "), "{why}");
        let other = fingerprint(&[("src/math.ts".to_string(), OTHER.to_string())]);
        assert!(fp_match(&entry, &other, 60, 50).is_none());
        assert!(
            fp_match(&entry, &renamed, 100, 100).is_none(),
            "configurable thresholds"
        );
    }

    #[test]
    fn r_lc_030__entry_parses_terms_and_state() {
        let t: toml::Table = "id = \"grv.dark-toggle-v1\"\nstate = \"rejected\"\nat = \"1.4.0\"\nwhat = \"toggles\"\nwhy = \"drift\"\nmatch = [\"dark mode\", \"/useTheme\\\\(/\", \"ui/**/toggle*\"]\n".parse().unwrap();
        let e = parse_entry(
            &RepoPath::new("fairlead/docs/graveyard/dark.md").unwrap(),
            &t,
        )
        .unwrap();
        assert_eq!(
            e.terms,
            [
                Term::Words("dark mode".into()),
                Term::Regex("useTheme\\(".into()),
                Term::Glob("ui/**/toggle*".into())
            ]
        );
        assert_eq!(text_hits(&e, "Add a Dark Mode switch").len(), 1);
        assert_eq!(text_hits(&e, "const t = useTheme();").len(), 1);
        assert_eq!(path_hits(&e, "ui/header/toggle-night.tsx").len(), 1);
        assert!(!words_hit("dark mode", "darkmode"), "whole words only");
    }

    #[test]
    fn r_ret_042__match_terms_and_fingerprint_kept() {
        let f = fp("ui/ThemeToggle.tsx", TOGGLE);
        let text = format!(
            "id = \"grv.x\"\nstate = \"rejected\"\nmatch = [\"dark mode\"]\nfingerprint = {}\n",
            f.toml_inline()
        );
        let t: toml::Table = text.parse().unwrap();
        let e = parse_entry(&RepoPath::new("docs/graveyard/x.md").unwrap(), &t).unwrap();
        assert_eq!(e.fingerprint.as_ref(), Some(&f));
        assert_eq!(e.terms.len(), 1);
    }
}
