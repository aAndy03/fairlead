//! Per-file extraction records (contract 4.5). Task 19 creates the site types; Tasks 21 and 22 add
//! `FileRecord`, `ExtractPlan`, `extract_file`, `extract_files` and `Corpus`.
pub mod styles;

pub use styles::{StyleKinds, extract_css_tokens, foreign_ranges};

use fairlead_syntax::diag::Span;
use serde::{Deserialize, Serialize};

/// One value site of a CSS custom property (R-FILE-034).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CssTokenSite {
    pub name: String,
    pub value: String,
    /// Enclosing selector or at-rule chain, outermost first, joined by " > ".
    pub context: String,
    pub span: Span,
    pub in_foreign: bool,
}

/// A literal value in a property context (R-CHK-019); produced by the P5 scanner.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct LiteralSite {
    pub property: String,
    pub value_norm: String,
    pub raw: String,
    pub span: Span,
    pub projection_hint: Option<String>,
}

/// A secret-like value; never carries the value itself (R-AGT-023).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SecretHit {
    pub class: String,
    pub span: Span,
    pub fp_prefix: String,
}

/// Everything derived from a file's text besides its `Extraction`; cached by content hash and plan.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct Extras {
    pub css_tokens: Vec<CssTokenSite>,
    pub literals: Vec<LiteralSite>,
    pub secrets: Vec<SecretHit>,
    pub pattern_hits: Vec<PatternHit>,
}

use crate::classify::{ClassifyRules, FileClass, classify, scans_directives};
use crate::config::ControlSet;
use crate::discover::Root;
use crate::error::Error;
use crate::hash::{ContentHash, base32, hash_content};
use crate::path::RepoPath;
use crate::walk::{SkipReason, WalkEntry};
use fairlead_syntax::delimiters::{CommentTable, Leader};
use fairlead_syntax::extract::{ExtractConfig, Extraction, ScanStatus, extract as syntax_extract};
use std::sync::Arc;

pub mod literals;

pub use literals::{LEAK_PROPERTIES, PatternHit, PatternProbe, scan_literals, scan_patterns};

/// Per-file record (contract 4.5). Equal for equal bytes, path and plan, whatever the cache state.
#[derive(Clone, PartialEq, Debug)]
pub struct FileRecord {
    pub path: RepoPath,
    pub class: FileClass,
    pub size: u64,
    /// Normalised for UTF-8 text, raw bytes otherwise (R-ARCH-076, R-PORT-016).
    pub hash: ContentHash,
    /// `None` when the class forbids scanning, a `[scan] skip` glob matches, or the file was not read.
    pub extraction: Option<Arc<Extraction>>,
    pub css_tokens: Vec<CssTokenSite>,
    pub literals: Vec<LiteralSite>,
    pub secrets: Vec<SecretHit>,
    /// Matches of `check = "pattern"` rules (amendment A-13).
    pub pattern_hits: Vec<PatternHit>,
    pub skip: Option<SkipReason>,
}

/// Everything extraction needs from the control files, computed once per run.
#[derive(Clone, Debug)]
pub struct ExtractPlan {
    pub syntax: ExtractConfig,
    pub classify: ClassifyRules,
    pub styles: StyleKinds,
    /// CSS properties named by `applies=` of declared tokens (sorted, deduplicated); empty = no literal scan.
    pub leak_properties: Vec<String>,
    /// One probe per `check = "pattern"` rule (amendment A-13).
    pub pattern_rules: Vec<PatternProbe>,
}

impl ExtractPlan {
    /// Build the plan from the root control file: marker, `[comments]`, `[scan]`, plus the class rules.
    /// Build the plan from the control files (amendment A-12): style kinds, pattern probes and the
    /// leak property list are all derived here.
    pub fn from_controls(root: &Root, controls: &ControlSet) -> Result<ExtractPlan, Error> {
        let styles = StyleKinds::from_controls(controls);
        let wants_leak = controls.ok_files().any(|cf| {
            cf.rules
                .values()
                .any(|r| matches!(r.check, crate::config::RuleCheckDecl::Leak { .. }))
        });
        let leak: Vec<String> = if wants_leak {
            LEAK_PROPERTIES.iter().map(|p| (*p).to_string()).collect()
        } else {
            Vec::new()
        };
        let mut plan = Self::with_parts(root, controls, styles, leak)?;
        for cf in controls.ok_files() {
            for r in cf.rules.values() {
                if let crate::config::RuleCheckDecl::Pattern { forbid, .. } = &r.check {
                    let key = format!("{}:{}", cf.fairlead.scope.as_str(), r.name);
                    match PatternProbe::new(key, &cf.dir, &r.glob, &r.exclude, forbid) {
                        Ok(p) => plan.pattern_rules.push(p),
                        Err(e) => return Err(Error::Internal(format!("rule {}: {e}", r.name))),
                    }
                }
            }
        }
        Ok(plan)
    }

    /// Plan from explicit parts (tests and `from_controls`).
    pub fn with_parts(
        root: &Root,
        controls: &ControlSet,
        styles: StyleKinds,
        mut leak_properties: Vec<String>,
    ) -> Result<ExtractPlan, Error> {
        let Some(rc) = controls.root_control() else {
            return Err(Error::Usage(
                "the root control file did not parse; extraction needs its [scan] and [comments]"
                    .into(),
            ));
        };
        let mut comments = CommentTable::builtin();
        for (ext, specs) in &rc.comments {
            let mut leaders = Vec::new();
            for s in specs {
                // Validated at load (parse_comments); a failure here would be an internal inconsistency.
                match Leader::from_spec(s) {
                    Some(l) => leaders.push(l),
                    None => {
                        return Err(Error::Internal(format!("comment leader {s:?} for .{ext}")));
                    }
                }
            }
            comments = comments.with_overrides(ext, leaders);
        }
        leak_properties.sort();
        leak_properties.dedup();
        Ok(ExtractPlan {
            syntax: ExtractConfig {
                marker: rc
                    .fairlead
                    .marker
                    .clone()
                    .unwrap_or_else(|| "fairlead".to_string()),
                comments,
                header_skip: rc.scan.header_skip.clone(),
                max_file: rc.scan.max_file,
                max_directives: rc.scan.max_directives,
            },
            classify: ClassifyRules::from_controls(root, controls),
            styles,
            leak_properties,
            pattern_rules: Vec::new(),
        })
    }

    /// 16 base32 characters over everything that changes extraction output; part of the cache key
    /// of cached `Extras`.
    pub fn fingerprint(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(&self.syntax.config_fingerprint());
        for p in &self.leak_properties {
            h.update(p.as_bytes());
            h.update(&[0]);
        }
        h.update(b"styles\0");
        for e in &self.styles.extensions {
            h.update(e.as_bytes());
            h.update(&[0]);
        }
        for d in &self.styles.declared {
            h.update(d.as_str().as_bytes());
            h.update(&[0]);
        }
        base32(h.finalize().as_bytes(), 16)
    }
}

/// Which text-derived extras exist for this path and class (the cache reads them only if true).
pub fn extras_wanted(plan: &ExtractPlan, path: &RepoPath, class: FileClass) -> bool {
    let scanned_authored = class == FileClass::Authored && !plan.classify.skips(path);
    (scanned_authored
        && (plan.styles.applies(path)
            || !plan.leak_properties.is_empty()
            || !plan.pattern_rules.is_empty()))
        || plan.classify.in_managerial_folder(path)
}

/// `SkipReason` for a syntax status (C-37).
pub fn skip_of(ex: &Extraction) -> Option<SkipReason> {
    match ex.status {
        ScanStatus::SkippedTooLarge => Some(SkipReason::TooLarge),
        ScanStatus::SkippedBinary => Some(SkipReason::Binary),
        ScanStatus::SkippedNotUtf8 => Some(SkipReason::NotUtf8),
        ScanStatus::Scanned | ScanStatus::NoMarker => None,
    }
}

fn compute_extras(
    path: &RepoPath,
    bytes: &[u8],
    class: FileClass,
    scanned: bool,
    plan: &ExtractPlan,
    ex: Option<&Extraction>,
) -> Extras {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Extras::default();
    };
    let ext = path.extension().to_ascii_lowercase();
    let mut out = Extras::default();
    if scanned && class == FileClass::Authored && plan.styles.applies(path) {
        let foreign = ex.map(foreign_ranges).unwrap_or_default();
        out.css_tokens = extract_css_tokens(text, &foreign);
    }
    if scanned && class == FileClass::Authored && !plan.leak_properties.is_empty() {
        out.literals = scan_literals(text, &ext, &plan.leak_properties);
    }
    if scanned && class == FileClass::Authored && !plan.pattern_rules.is_empty() {
        out.pattern_hits = scan_patterns(text, path, &plan.pattern_rules);
    }
    if plan.classify.in_managerial_folder(path) {
        out.secrets = crate::secrets::scan_secrets(text);
    }
    out
}

/// Pure per file (R-ARCH-030). `bytes` are already read.
pub fn extract_file(entry: &WalkEntry, bytes: &[u8], plan: &ExtractPlan) -> FileRecord {
    let class = classify(&entry.path, &plan.classify);
    let ext = entry.path.extension().to_ascii_lowercase();
    let scanned = scans_directives(class) && !plan.classify.skips(&entry.path);
    let extraction = scanned.then(|| Arc::new(syntax_extract(bytes, &ext, &plan.syntax)));
    let skip = extraction.as_deref().and_then(skip_of);
    let extras = if skip.is_none() {
        compute_extras(
            &entry.path,
            bytes,
            class,
            scanned,
            plan,
            extraction.as_deref(),
        )
    } else {
        Extras::default()
    };
    FileRecord {
        path: entry.path.clone(),
        class,
        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        hash: hash_content(bytes),
        extraction,
        css_tokens: extras.css_tokens,
        literals: extras.literals,
        secrets: extras.secrets,
        pattern_hits: extras.pattern_hits,
        skip,
    }
}

use crate::cache::{Cache, FileRow};
use crate::diag::{Diagnostic, Location, Severity, codes, sort_diagnostics};
use crate::walk::WalkResult;
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;

const PROBE_BYTES: u64 = 8 * 1024;

/// Records of one run, keyed and iterated by path (R-ARCH-060).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Corpus {
    pub records: BTreeMap<RepoPath, FileRecord>,
}

impl Corpus {
    /// Syntax diagnostics of every scanned file plus the `HN-I001` note for binary files, sorted.
    /// Over-size notes come from the walk; non-UTF-8 notes (`HN-I003`) from the syntax extraction.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for rec in self.records.values() {
            if let Some(ex) = &rec.extraction {
                out.extend(
                    ex.diags
                        .iter()
                        .cloned()
                        .map(|d| Diagnostic::from_syntax(&rec.path, d)),
                );
            }
            if rec.skip == Some(SkipReason::Binary) {
                out.push(
                    Diagnostic::new(
                        codes::HN_I001,
                        Severity::Info,
                        Location::file(rec.path.clone()),
                        "file skipped: binary (NUL byte in the first 8 KB)",
                    )
                    .with_hint(
                        "add the path to [scan] skip or [scan] exclude to silence this note",
                    ),
                );
            }
        }
        sort_diagnostics(&mut out);
        out
    }
}

enum Read {
    Binary,
    Bytes(Vec<u8>),
}

/// Open once; read the first 8 KB; stop at a NUL byte; otherwise read the rest.
fn read_probing(abs: &camino::Utf8Path, rel: &RepoPath) -> Result<Option<Read>, Error> {
    let mut f = match std::fs::File::open(abs.as_std_path()) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(rel.as_str(), "open", &e, false)),
    };
    let mut head = Vec::new();
    if let Err(e) = f.by_ref().take(PROBE_BYTES).read_to_end(&mut head) {
        return Err(Error::io(rel.as_str(), "read", &e, false));
    }
    if head.contains(&0) {
        return Ok(Some(Read::Binary));
    }
    if let Err(e) = f.read_to_end(&mut head) {
        return Err(Error::io(rel.as_str(), "read", &e, false));
    }
    Ok(Some(Read::Bytes(head)))
}

fn extras_key(fp: &str, plan: &ExtractPlan, path: &RepoPath) -> String {
    format!(
        "{fp}-m{}-s{}",
        u8::from(plan.classify.in_managerial_folder(path)),
        u8::from(plan.styles.applies(path))
    )
}

/// A record for a file that is not read: no extraction, `skip` set, stand-in hash (see Task 22 note).
fn unread_record(e: &WalkEntry, class: FileClass, skip: SkipReason, head: &[u8]) -> FileRecord {
    let mut stand_in = format!(
        "fairlead:{}:{}:",
        if skip == SkipReason::TooLarge {
            "too-large"
        } else {
            "binary"
        },
        e.size
    )
    .into_bytes();
    stand_in.extend_from_slice(head);
    FileRecord {
        path: e.path.clone(),
        class,
        size: e.size,
        hash: crate::hash::hash_raw(&stand_in),
        extraction: None,
        css_tokens: Vec::new(),
        literals: Vec::new(),
        secrets: Vec::new(),
        pattern_hits: Vec::new(),
        skip: Some(skip),
    }
}

fn from_cache(
    e: &WalkEntry,
    class: FileClass,
    row: &FileRow,
    plan: &ExtractPlan,
    cache: &Cache,
    fp: &str,
) -> Option<FileRecord> {
    let scanned = scans_directives(class) && !plan.classify.skips(&e.path);
    let extraction = if scanned {
        Some(cache.get_extraction(&row.hash)?)
    } else {
        None
    };
    let extras = if extras_wanted(plan, &e.path, class) {
        cache.get_extras(&row.hash, &extras_key(fp, plan, &e.path))?
    } else {
        Extras::default()
    };
    let skip = extraction.as_deref().and_then(skip_of);
    Some(FileRecord {
        path: e.path.clone(),
        class,
        size: e.size,
        hash: row.hash,
        extraction,
        css_tokens: extras.css_tokens,
        literals: extras.literals,
        secrets: extras.secrets,
        pattern_hits: extras.pattern_hits,
        skip,
    })
}

type Processed = Option<(FileRecord, Option<(RepoPath, FileRow)>)>;

fn process(
    e: &WalkEntry,
    root: &Root,
    plan: &ExtractPlan,
    cache: &Cache,
    fp: &str,
) -> Result<Processed, Error> {
    let class = classify(&e.path, &plan.classify);
    if e.size > plan.syntax.max_file {
        let head = read_head_best_effort(&root.dir.join(e.path.as_str()));
        return Ok(Some((
            unread_record(e, class, SkipReason::TooLarge, &head),
            None,
        )));
    }
    if let Some(row) = cache.file_row(&e.path)
        && row.size == e.size
        && row.mtime_ns == e.mtime_ns
        && let Some(rec) = from_cache(e, class, &row, plan, cache, fp)
    {
        return Ok(Some((rec, None)));
    }
    let abs = root.dir.join(e.path.as_str());
    match read_probing(&abs, &e.path)? {
        None => Ok(None),
        Some(Read::Binary) => Ok(Some((
            unread_record(e, class, SkipReason::Binary, &[]),
            None,
        ))),
        Some(Read::Bytes(bytes)) => {
            let rec = extract_file(e, &bytes, plan);
            if let Some(ex) = &rec.extraction {
                cache.put_extraction(&rec.hash, ex);
            }
            if extras_wanted(plan, &e.path, class) && rec.skip.is_none() {
                let extras = Extras {
                    css_tokens: rec.css_tokens.clone(),
                    literals: rec.literals.clone(),
                    secrets: rec.secrets.clone(),
                    pattern_hits: rec.pattern_hits.clone(),
                };
                cache.put_extras(&rec.hash, &extras_key(fp, plan, &e.path), &extras);
            }
            let row = FileRow {
                size: e.size,
                mtime_ns: e.mtime_ns,
                hash: rec.hash,
            };
            Ok(Some((rec, Some((e.path.clone(), row)))))
        }
    }
}

/// First 8 KB of an over-large file (never the whole file); empty on any error.
fn read_head_best_effort(abs: &camino::Utf8Path) -> Vec<u8> {
    let mut head = Vec::new();
    if let Ok(f) = std::fs::File::open(abs.as_std_path()) {
        let _ = f.take(PROBE_BYTES).read_to_end(&mut head);
    }
    head
}

/// S4: I/O plus rayon; the result is a `BTreeMap`, so output order never depends on scheduling.
pub fn extract_files(
    walk: &WalkResult,
    root: &Root,
    plan: &ExtractPlan,
    cache: &Cache,
    only: Option<&BTreeSet<RepoPath>>,
) -> Result<Corpus, Error> {
    let fp = plan.fingerprint();
    let entries: Vec<&WalkEntry> = walk
        .files
        .iter()
        .filter(|e| only.is_none_or(|o| o.contains(&e.path)))
        .collect();
    let results: Vec<Result<Processed, Error>> = entries
        .par_iter()
        .map(|e| process(e, root, plan, cache, &fp))
        .collect();
    let mut records = BTreeMap::new();
    let mut rows = Vec::new();
    for r in results {
        if let Some((rec, row)) = r? {
            if let Some(row) = row {
                rows.push(row);
            }
            records.insert(rec.path.clone(), rec);
        }
    }
    if !rows.is_empty() {
        cache.put_file_rows(&rows);
    }
    Ok(Corpus { records })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ControlSet, parse_control};
    use crate::diag::codes;
    use crate::hash::{hash_normalised, hash_raw};
    use crate::testutil::{ROOT_CONTROL, test_root};
    use camino::Utf8Path;

    pub(super) fn plan(extra_root: &str, leak: &[&str]) -> ExtractPlan {
        let mut set = ControlSet::default();
        let p = RepoPath::new("fairlead/control.toml").unwrap();
        set.files.insert(
            p.clone(),
            parse_control(&p, format!("{ROOT_CONTROL}{extra_root}").as_bytes(), true),
        );
        ExtractPlan::with_parts(
            &test_root(Utf8Path::new("/r")),
            &set,
            StyleKinds::default(),
            leak.iter().map(|s| s.to_string()).collect(),
        )
        .unwrap()
    }

    fn entry(path: &str, bytes: &[u8]) -> WalkEntry {
        WalkEntry {
            path: RepoPath::new(path).unwrap(),
            size: bytes.len() as u64,
            mtime_ns: 0,
        }
    }

    fn rec(path: &str, bytes: &[u8], plan: &ExtractPlan) -> FileRecord {
        extract_file(&entry(path, bytes), bytes, plan)
    }

    const RS: &[u8] = b"// fairlead: def cmp.a\nfn a() {}\n";

    #[test]
    fn r_arch_030__extraction_is_a_pure_function_of_bytes_path_and_plan() {
        let p = plan("", &[]);
        let a = rec("src/does-not-exist-on-disk.rs", RS, &p);
        let b = rec("src/does-not-exist-on-disk.rs", RS, &p);
        assert_eq!(a, b);
        let ex = a.extraction.as_ref().expect("authored source is scanned");
        assert_eq!(ex.directives.len(), 1);
        assert_eq!(
            (a.class, a.size, a.skip),
            (FileClass::Authored, RS.len() as u64, None)
        );
        assert_eq!(a.hash, hash_normalised(RS));
    }

    #[test]
    fn r_arch_076__crlf_and_lf_checkouts_give_the_same_hash_and_directives() {
        let p = plan("", &[]);
        let lf = rec("src/a.rs", RS, &p);
        let crlf_bytes = String::from_utf8(RS.to_vec())
            .unwrap()
            .replace('\n', "\r\n");
        let crlf = rec("src/a.rs", crlf_bytes.as_bytes(), &p);
        assert_eq!(lf.hash, crlf.hash);
        assert_eq!(
            lf.extraction.as_ref().unwrap().directives.len(),
            crlf.extraction.as_ref().unwrap().directives.len()
        );
    }

    #[test]
    fn r_port_016__non_utf8_is_hashed_raw_and_skipped_for_directives() {
        let p = plan("", &[]);
        let bytes = b"// fairlead: def cmp.l\n// caf\xE9\n";
        let r = rec("src/legacy.rs", bytes, &p);
        assert_eq!(r.hash, hash_raw(bytes));
        assert_eq!(r.skip, Some(SkipReason::NotUtf8));
        assert!(r.extraction.as_ref().unwrap().directives.is_empty());
        assert!(
            r.extraction
                .as_ref()
                .unwrap()
                .diags
                .iter()
                .any(|d| d.code == codes::HN_I003)
        );
    }

    #[test]
    fn r_arch_023__nul_in_the_first_8k_and_oversize_are_skip_records() {
        let p = plan("[scan]\nmax-file = 100\n", &[]);
        let mut bin = vec![b'a'; 50];
        bin[10] = 0;
        let r = rec("assets/x.bin", &bin, &p);
        assert_eq!(r.skip, Some(SkipReason::Binary));
        let big = vec![b'a'; 101];
        let r = rec("assets/big.txt", &big, &p);
        assert_eq!(r.skip, Some(SkipReason::TooLarge));
        assert_eq!(
            r.hash,
            hash_normalised(&big),
            "C-37: the skip record still carries its hash"
        );
    }

    #[test]
    fn r_file_004__reference_files_are_never_scanned() {
        let p = plan("", &[]);
        let r = rec(
            "fairlead/ref/syntax.md",
            b"<!-- fairlead: def cmp.x -->\n",
            &p,
        );
        assert_eq!(r.class, FileClass::Reference);
        assert!(r.extraction.is_none());
        assert_eq!(r.hash, hash_normalised(b"<!-- fairlead: def cmp.x -->\n"));
    }

    #[test]
    fn r_file_005_and_009__vendor_owned_and_external_generated_text_is_ignored() {
        let p = plan(
            "[[vendor]]\npath = \"ui/**\"\ntool = \"shadcn\"\n[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n",
            &[],
        );
        let a = rec("ui/button.tsx", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!(
            (a.class, a.extraction.is_none()),
            (FileClass::VendorOwned, true)
        );
        let b = rec("src/api.gen.ts", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!(
            (b.class, b.extraction.is_none()),
            (FileClass::ExternalGenerated, true)
        );
    }

    #[test]
    fn r_file_009__external_generated_folders_declared_by_convention_are_ignored() {
        let p = plan(
            "[[convention]]\nglob = \"proto/**\"\nclass = \"external-generated\"\n",
            &[],
        );
        let r = rec("proto/a.rs", b"// fairlead: def cmp.evil\n", &p);
        assert_eq!(
            (r.class, r.extraction.is_none()),
            (FileClass::ExternalGenerated, true)
        );
        assert_eq!(r.hash, hash_normalised(b"// fairlead: def cmp.evil\n"));
    }

    #[test]
    fn r_agt_018__skip_globs_stop_scanning_but_not_hashing() {
        let p = plan("[scan]\nskip = [\"tests/fixtures/**\"]\n", &[]);
        let r = rec("tests/fixtures/a.rs", RS, &p);
        assert!(r.extraction.is_none());
        assert_eq!(r.class, FileClass::Authored);
        assert_eq!(r.hash, hash_normalised(RS));
    }

    #[test]
    fn r_file_034__css_tokens_only_for_authored_stylesheets_with_foreign_flags() {
        let p = plan("[[vendor]]\npath = \"vendor/**\"\ntool = \"x\"\n", &[]);
        let css = b"/* fairlead: def foreign.shadcn tool=shadcn +end */\n  --chart-1: red;\n/* fairlead: end */\n:root { --mine: 1; }\n";
        let r = rec("app/globals.css", css, &p);
        let got: Vec<(&str, bool)> = r
            .css_tokens
            .iter()
            .map(|t| (t.name.as_str(), t.in_foreign))
            .collect();
        assert_eq!(got, [("--chart-1", true), ("--mine", false)]);
        assert!(
            rec("vendor/theme.css", css, &p).css_tokens.is_empty(),
            "vendor-owned CSS is not indexed"
        );
        assert!(rec("src/a.ts", css, &p).css_tokens.is_empty());
    }

    #[test]
    fn r_agt_018__p5_scanners_are_called_only_where_the_contract_says() {
        // With the P3 stubs both stay empty; the gating is observable through `extras_wanted`.
        let p = plan("", &["background-color"]);
        let path = |s: &str| RepoPath::new(s).unwrap();
        assert!(
            extras_wanted(&p, &path("src/a.tsx"), FileClass::Authored),
            "literal scan needs leak properties"
        );
        assert!(
            extras_wanted(&p, &path("fairlead/docs/x.md"), FileClass::Authored),
            "secrets: managerial folder"
        );
        assert!(!extras_wanted(
            &p,
            &path("src/a.tsx"),
            FileClass::VendorOwned
        ));
        let none = plan("", &[]);
        assert!(!extras_wanted(
            &none,
            &path("src/a.tsx"),
            FileClass::Authored
        ));
        assert!(
            extras_wanted(&none, &path("src/a.css"), FileClass::Authored),
            "css tokens"
        );
    }

    #[test]
    fn r_syn_039__plan_uses_marker_comments_and_limits_from_the_root_control_file() {
        let p = plan("[scan]\nmax-file = 77\nmax-directives = 3\n", &[]);
        assert_eq!((p.syntax.max_file, p.syntax.max_directives), (77, 3));
        assert_eq!(p.syntax.marker, "fairlead");
        assert_eq!(plan("", &[]).fingerprint(), plan("", &[]).fingerprint());
        assert_ne!(
            plan("", &[]).fingerprint(),
            plan("", &["color"]).fingerprint()
        );
        assert_ne!(
            plan("", &[]).fingerprint(),
            plan("[scan]\nmax-file = 5\n", &[]).fingerprint()
        );
    }

    use crate::cache::{Cache, CacheKey};
    use crate::testutil::{utf8_tmp, write_bytes, write_file};
    use crate::walk::{WalkOptions, walk};
    use std::collections::BTreeSet;

    struct Fixture {
        _guard: tempfile::TempDir,
        dir: camino::Utf8PathBuf,
        root: crate::discover::Root,
        walked: crate::walk::WalkResult,
        plan: ExtractPlan,
    }

    fn fixture() -> Fixture {
        let (guard, dir) = utf8_tmp();
        write_file(
            &dir,
            "fairlead/control.toml",
            &format!("{ROOT_CONTROL}[scan]\nmax-file = 1000\nskip = [\"tests/fixtures/**\"]\n"),
        );
        write_file(
            &dir,
            "fairlead/ref/syntax.md",
            "<!-- fairlead: def cmp.x -->\n",
        );
        write_file(&dir, "src/a.rs", "// fairlead: def cmp.a\nfn a() {}\n");
        write_file(&dir, "src/b.rs", "fn b() {}\n");
        write_bytes(
            &dir,
            "src/legacy.rs",
            b"// fairlead: def cmp.l\n// caf\xE9\n",
        );
        write_bytes(&dir, "assets/logo.bin", &[0u8, 1, 2, 3, 0, 0, 7]);
        write_bytes(&dir, "assets/big.txt", &vec![b'a'; 3000]);
        write_file(
            &dir,
            "web/app.css",
            ":root { --a: 1; }\n.dark { --a: 2; }\n",
        );
        write_file(&dir, "tests/fixtures/f.rs", "// fairlead: def cmp.f\n");
        let root = test_root(&dir);
        let walked = walk(
            &root,
            &WalkOptions {
                max_file: 1000,
                ..WalkOptions::default()
            },
        )
        .unwrap();
        let set = crate::config::load_controls(&root, &walked).unwrap();
        let plan = ExtractPlan::with_parts(&root, &set, StyleKinds::default(), vec![]).unwrap();
        Fixture {
            _guard: guard,
            dir,
            root,
            walked,
            plan,
        }
    }

    fn cache_for(f: &Fixture, enabled: bool) -> Cache {
        Cache::open(
            &f.root,
            enabled,
            CacheKey {
                fairlead_version: "0.1.0".into(),
                grammar: 1,
                config_hash: hash_raw(b"cfg"),
                as_of: None,
            },
        )
    }

    #[test]
    fn r_arch_071__cold_warm_and_deleted_cache_give_identical_corpora() {
        let f = fixture();
        let cold = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let warm = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        std::fs::remove_dir_all(f.dir.join(".fairlead").as_std_path()).unwrap();
        let rebuilt =
            extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let off = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert_eq!(cold, warm);
        assert_eq!(cold, rebuilt);
        assert_eq!(
            cold, off,
            "R-CHK-013: --no-cache and a warm cache give identical results"
        );
        assert_eq!(cold.records.len(), f.walked.files.len());
    }

    #[test]
    fn r_chk_013__the_cache_never_changes_results() {
        let f = fixture();
        let on = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let on_again =
            extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let off = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert_eq!(on_again, off);
        assert_eq!(on, off);
        assert_eq!(on.diagnostics(), off.diagnostics());
    }

    #[test]
    fn r_arch_071__a_warm_run_does_not_open_unchanged_files() {
        let f = fixture();
        let cold = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        std::fs::remove_file(f.dir.join("src/a.rs").as_std_path()).unwrap();
        let warm = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        assert_eq!(
            warm.records.get(&RepoPath::new("src/a.rs").unwrap()),
            cold.records.get(&RepoPath::new("src/a.rs").unwrap())
        );
        let uncached =
            extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        assert!(
            !uncached
                .records
                .contains_key(&RepoPath::new("src/a.rs").unwrap()),
            "a vanished file is dropped, not an error"
        );
    }

    #[test]
    fn r_arch_023__skip_records_for_binary_oversize_and_non_utf8() {
        let f = fixture();
        let c = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        let get = |p: &str| &c.records[&RepoPath::new(p).unwrap()];
        assert_eq!(get("assets/logo.bin").skip, Some(SkipReason::Binary));
        assert!(get("assets/logo.bin").extraction.is_none());
        assert_eq!(get("assets/big.txt").skip, Some(SkipReason::TooLarge));
        assert_eq!(get("src/legacy.rs").skip, Some(SkipReason::NotUtf8));
        assert_eq!(
            get("src/legacy.rs").hash,
            hash_raw(b"// fairlead: def cmp.l\n// caf\xE9\n"),
            "R-PORT-016"
        );
        assert_eq!(get("src/a.rs").skip, None);
        assert_eq!(
            get("src/a.rs")
                .extraction
                .as_ref()
                .unwrap()
                .directives
                .len(),
            1
        );
        assert!(get("src/b.rs").extraction.is_some());
        assert!(
            get("tests/fixtures/f.rs").extraction.is_none(),
            "[scan] skip"
        );
        assert!(
            get("fairlead/ref/syntax.md").extraction.is_none(),
            "R-FILE-004"
        );
        assert_eq!(get("web/app.css").css_tokens.len(), 2);
    }

    #[test]
    fn r_arch_023__diagnostics_report_every_skip_exactly_once() {
        let f = fixture();
        let c = extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap();
        let d = c.diagnostics();
        let line = |code: crate::diag::DiagCode, path: &str| {
            d.iter()
                .filter(|x| x.code == code && x.location.path.as_str() == path)
                .count()
        };
        assert_eq!(line(codes::HN_I001, "assets/logo.bin"), 1, "binary: info");
        assert_eq!(
            line(codes::HN_I003, "src/legacy.rs"),
            1,
            "non-UTF-8: warning, from the syntax extraction"
        );
        assert_eq!(
            line(codes::HN_I001, "assets/big.txt"),
            0,
            "the over-size note already came from the walk"
        );
        assert!(
            d.windows(2)
                .all(|w| (&w[0].location.path, w[0].location.line)
                    <= (&w[1].location.path, w[1].location.line))
        );
    }

    #[test]
    fn r_arch_080__thread_count_does_not_change_the_corpus() {
        let f = fixture();
        let run = |n: usize| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .unwrap()
                .install(|| {
                    extract_files(&f.walked, &f.root, &f.plan, &cache_for(&f, false), None).unwrap()
                })
        };
        assert_eq!(run(1), run(4));
    }

    #[test]
    fn r_arch_075__only_limits_extraction_to_the_given_paths() {
        let f = fixture();
        let only: BTreeSet<RepoPath> = [
            RepoPath::new("src/a.rs").unwrap(),
            RepoPath::new("web/app.css").unwrap(),
        ]
        .into_iter()
        .collect();
        let c = extract_files(
            &f.walked,
            &f.root,
            &f.plan,
            &cache_for(&f, false),
            Some(&only),
        )
        .unwrap();
        assert_eq!(
            c.records.keys().map(RepoPath::as_str).collect::<Vec<_>>(),
            ["src/a.rs", "web/app.css"]
        );
    }

    #[test]
    fn r_arch_071__changed_content_with_a_new_mtime_is_re_extracted() {
        let f = fixture();
        let cache = cache_for(&f, true);
        let first = extract_files(&f.walked, &f.root, &f.plan, &cache, None).unwrap();
        write_file(&f.dir, "src/b.rs", "// fairlead: def cmp.b\nfn b() {}\n");
        let walked2 = walk(
            &f.root,
            &WalkOptions {
                max_file: 1000,
                ..WalkOptions::default()
            },
        )
        .unwrap();
        let second = extract_files(&walked2, &f.root, &f.plan, &cache_for(&f, true), None).unwrap();
        let b = RepoPath::new("src/b.rs").unwrap();
        assert_eq!(
            first.records[&b]
                .extraction
                .as_ref()
                .unwrap()
                .directives
                .len(),
            0
        );
        assert_eq!(
            second.records[&b]
                .extraction
                .as_ref()
                .unwrap()
                .directives
                .len(),
            1
        );
        assert_ne!(first.records[&b].hash, second.records[&b].hash);
    }
}
