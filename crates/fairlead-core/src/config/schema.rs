//! Typed control-file schema and the reader helpers (12-directive-syntax schema index, 0.1 rows).
use crate::config::header::locate_key;
use crate::diag::{DiagCode, Diagnostic, Location, Severity, codes};
use crate::path::RepoPath;
use fairlead_syntax::directive::ArgValue;
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

// ---------------------------------------------------------------- vocabulary types
/// Override policy; "stricter" order Open < NeedsException < Sealed (R-SCOPE-030).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Policy {
    Open,
    NeedsException,
    Sealed,
}

/// The ordered ramp observe, warn, enforce (R-CHK-033).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Stage {
    Observe,
    Warn,
    Enforce,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OwnerKind {
    Human,
    Agent,
}

/// R-SCOPE-040. Defined here because the control-file `[[ovr]]` table needs it; the registry
/// re-exports it (contract issue 3).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum OvrOp {
    Loosen,
    Tighten,
    Set,
    Replace,
    Add,
    Remove,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Owner {
    pub handle: String,
    pub kind: OwnerKind,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum IndexValues {
    #[default]
    Value,
    Hash,
}

/// `[settings]`; a non-root file may set only `purpose` (C-4).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Settings {
    pub folder: Option<String>,
    pub fairlead_version: Option<String>,
    pub fairlead_range: Option<String>,
    pub index_values: IndexValues,
    pub purpose: Option<String>,
    pub owners: Vec<Owner>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanConfig {
    pub exclude: Vec<String>,
    pub skip: Vec<String>,
    pub header_skip: Vec<String>,
    pub max_file: u64,
    pub max_directives: u32,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            skip: Vec::new(),
            header_skip: Vec::new(),
            max_file: 2_000_000,
            max_directives: 500,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum KindShape {
    Doc,
    Code,
    #[default]
    Either,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ImplLimit {
    #[default]
    Any,
    None,
    N(u32),
}

/// One `[kinds.K.keys]` entry, for example `layer = { type = "enum", values = [..] }`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyDecl {
    pub ty: String,
    pub values: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KindDecl {
    pub title: Option<String>,
    pub shape: KindShape,
    pub impl_limit: ImplLimit,
    pub unused: Option<Severity>,
    pub dir: Option<String>,
    pub source_generate: bool,
    pub keys: BTreeMap<String, KeyDecl>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileKindDecl {
    pub extractor: String,
    pub binding: String,
    pub import_template: Option<String>,
}

/// Sub-folder names of a managerial folder (`[layout]`, R-FILE-020). `gen` is a reserved word in
/// edition 2024, hence `r#gen`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Layout {
    pub docs: String,
    pub contract: String,
    pub r#gen: String,
    pub ledger: String,
    pub reference: String,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            docs: "docs".into(),
            contract: "contract".into(),
            r#gen: "gen".into(),
            ledger: "ledger".into(),
            reference: "ref".into(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileItemDecl {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub extends: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VendorDecl {
    pub path: String,
    pub tool: String,
}

/// `[[ovr]]`: the keys of the `ovr` directive. `set` holds every other key as a field assignment;
/// a key written with a trailing `+` (`"tags+" = [..]`) is the append form `k+=` (TOML cannot spell
/// `+=`); P4 interprets it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OvrDecl {
    pub target: String,
    pub op: OvrOp,
    pub applies: Option<String>,
    pub set: BTreeMap<String, ArgValue>,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<String>,
    pub base: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SevWeight {
    pub error: u32,
    pub warn: u32,
    pub info: u32,
}

impl Default for SevWeight {
    fn default() -> Self {
        Self {
            error: 4,
            warn: 2,
            info: 1,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContextConfig {
    pub budget_start: u32,
    pub budget_path: u32,
    pub bytes_per_token: u32,
    pub tokenizer: Option<String>,
    pub sev_weight: SevWeight,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            budget_start: 500,
            budget_path: 700,
            bytes_per_token: 4,
            tokenizer: None,
            sev_weight: SevWeight::default(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BaselineConfig {
    /// Default `<folder>/gen/baseline.txt`, filled in by `parse_control` once the folder is known.
    pub file: String,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SecretsConfig {
    pub allow: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExceptionsConfig {
    pub max_days: Option<u32>,
    pub max_live_per_rule: u32,
}

impl Default for ExceptionsConfig {
    fn default() -> Self {
        Self {
            max_days: None,
            max_live_per_rule: 10,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EligibilityConfig {
    pub min_lines: u32,
    pub min_files: u32,
}

impl Default for EligibilityConfig {
    fn default() -> Self {
        Self {
            min_lines: 5,
            min_files: 3,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConventionAction {
    Transparent,
    Class(crate::classify::FileClass),
    /// 0.2; never produced by a 0.1 parse (a 0.1 build reports `own` as an unknown key).
    Own(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConventionDecl {
    pub glob: String,
    pub action: ConventionAction,
}

/// An unknown control key or table (`HN-C002`, R-SYN-039). `line` is a best-effort 1-based line.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnknownKey {
    pub path: String,
    pub line: u32,
}

// ---------------------------------------------------------------- reader context
pub(crate) fn join_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Collects hard errors (`HN-C001`, `HN-P001`) and unknown keys while the tables are consumed.
pub(crate) struct Ctx<'a> {
    pub path: RepoPath,
    pub text: &'a str,
    pub diags: Vec<Diagnostic>,
    pub unknown: Vec<UnknownKey>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(path: &RepoPath, text: &'a str) -> Self {
        Self {
            path: path.clone(),
            text,
            diags: Vec::new(),
            unknown: Vec::new(),
        }
    }

    pub(crate) fn error(&mut self, key: &str, message: impl Into<String>) {
        self.error_code(codes::HN_C001, key, message);
    }

    pub(crate) fn error_code(&mut self, code: DiagCode, key: &str, message: impl Into<String>) {
        let line = locate_key(self.text, key);
        self.diags.push(
            Diagnostic::new(
                code,
                Severity::Error,
                Location::at(self.path.clone(), line, 1),
                message,
            )
            .with_hint("fix the control file; `fairlead explain syntax` shows the schema"),
        );
    }

    /// Everything left in `rest` was not consumed: unknown (R-SYN-039).
    pub(crate) fn unknown_in(&mut self, prefix: &str, rest: &toml::Table) {
        for key in rest.keys() {
            let path = join_key(prefix, key);
            let line = locate_key(self.text, &path);
            self.unknown.push(UnknownKey { path, line });
        }
    }
}

pub(crate) fn take_string(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<String> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::String(s)) => Some(s),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be a string"));
            None
        }
    }
}

pub(crate) fn take_bool(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<bool> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Boolean(b)) => Some(b),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be true or false"));
            None
        }
    }
}

pub(crate) fn take_u32(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<u32> {
    take_u64(cx, t, prefix, key).and_then(|v| match u32::try_from(v) {
        Ok(v) => Some(v),
        Err(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` is too large"));
            None
        }
    })
}

pub(crate) fn take_u64(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<u64> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Integer(n)) => match u64::try_from(n) {
            Ok(v) => Some(v),
            Err(_) => {
                let k = join_key(prefix, key);
                cx.error(&k, format!("`{k}` must be a non-negative integer"));
                None
            }
        },
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be an integer"));
            None
        }
    }
}

pub(crate) fn string_array(v: &toml::Value) -> Option<Vec<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_string))
        .collect()
}

pub(crate) fn take_string_list(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<Vec<String>> {
    match t.remove(key) {
        None => None,
        Some(v) => match string_array(&v) {
            Some(list) => Some(list),
            None => {
                let k = join_key(prefix, key);
                cx.error(&k, format!("`{k}` must be a list of strings"));
                None
            }
        },
    }
}

pub(crate) fn take_table(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<toml::Table> {
    match t.remove(key) {
        None => None,
        Some(toml::Value::Table(x)) => Some(x),
        Some(_) => {
            let k = join_key(prefix, key);
            cx.error(&k, format!("`{k}` must be a table"));
            None
        }
    }
}

/// `[[name]]` arrays: every element must be a table.
pub(crate) fn take_table_array(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Vec<toml::Table> {
    let k = join_key(prefix, key);
    match t.remove(key) {
        None => Vec::new(),
        Some(toml::Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item {
                    toml::Value::Table(tb) => out.push(tb),
                    _ => cx.error(&k, format!("every `{k}` entry must be a table")),
                }
            }
            out
        }
        Some(_) => {
            cx.error(&k, format!("`{k}` must be an array of tables"));
            Vec::new()
        }
    }
}

/// A date given as a quoted string or a bare TOML date; returned as `YYYY-MM-DD` text, validated.
pub(crate) fn take_date_string(
    cx: &mut Ctx<'_>,
    t: &mut toml::Table,
    prefix: &str,
    key: &str,
) -> Option<String> {
    let k = join_key(prefix, key);
    let raw = match t.remove(key) {
        None => return None,
        Some(toml::Value::String(s)) => s,
        Some(toml::Value::Datetime(d)) => d.to_string(),
        Some(_) => {
            cx.error(&k, format!("`{k}` must be a date such as 2026-12-31"));
            return None;
        }
    };
    match raw.parse::<jiff::civil::Date>() {
        Ok(_) if raw.len() == 10 => Some(raw),
        _ => {
            cx.error(&k, format!("`{k}` must be a date YYYY-MM-DD, got {raw:?}"));
            None
        }
    }
}

/// NFC and `/` separators; rejects `..`. Used for scan patterns (gitignore syntax: leading `/`,
/// trailing `/` and `!` are legal).
pub(crate) fn clean_pattern(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s: String = raw.replace('\\', "/").nfc().collect();
    if s.split('/').any(|seg| seg == "..") {
        cx.error(key, format!("pattern {raw:?} must not contain `..`"));
        return None;
    }
    Some(s)
}

/// Declared repo-relative paths and globs: NFC, `/` separators, no `..`, no leading `/`, and no
/// Windows-reserved literal segment (`HN-P001`, R-SYN-040).
pub(crate) fn clean_rel_path(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s: String = raw.replace('\\', "/").nfc().collect();
    let s = s.strip_prefix("./").unwrap_or(&s).to_string();
    if s.is_empty() || s.starts_with('/') || s.split('/').any(|seg| seg == "..") {
        cx.error(
            key,
            format!("path {raw:?} must be non-empty, relative and free of `..`"),
        );
        return None;
    }
    for seg in s.split('/') {
        if seg.is_empty() || seg.contains(['*', '?', '[', '{']) {
            continue;
        }
        let lower = seg.to_lowercase();
        let stem = lower.split('.').next().unwrap_or("");
        if fairlead_syntax::ids::is_reserved_windows_name(&lower)
            || fairlead_syntax::ids::is_reserved_windows_name(stem)
        {
            cx.error_code(
                codes::HN_P001,
                key,
                format!("path segment {seg:?} is a Windows-reserved name"),
            );
            return None;
        }
    }
    Some(s)
}

/// A glob that must compile (globset syntax, `/` is literal); invalid is `HN-C001` at load so that
/// classification (`ClassifyRules`) can be infallible.
pub(crate) fn check_glob(cx: &mut Ctx<'_>, key: &str, pat: &str) -> bool {
    let probe = if pat.ends_with('/') {
        format!("{pat}**")
    } else {
        pat.to_string()
    };
    match globset::GlobBuilder::new(&probe)
        .literal_separator(true)
        .build()
    {
        Ok(_) => true,
        Err(e) => {
            cx.error(key, format!("invalid glob {pat:?}: {e}"));
            false
        }
    }
}

/// One folder name: a clean relative path with exactly one segment.
pub(crate) fn clean_segment(cx: &mut Ctx<'_>, key: &str, raw: &str) -> Option<String> {
    let s = clean_rel_path(cx, key, raw)?;
    if s.contains('/') {
        cx.error(key, format!("{raw:?} must be a single folder name"));
        return None;
    }
    Some(s)
}

// ---------------------------------------------------------------- simple tables
pub(crate) fn parse_scan(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ScanConfig {
    let mut out = ScanConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "scan") {
        if let Some(list) = take_string_list(cx, &mut t, "scan", "exclude") {
            out.exclude = list
                .iter()
                .filter_map(|p| clean_pattern(cx, "scan.exclude", p))
                .collect();
        }
        if let Some(list) = take_string_list(cx, &mut t, "scan", "skip") {
            out.skip = list
                .iter()
                .filter_map(|p| {
                    clean_pattern(cx, "scan.skip", p).filter(|s| check_glob(cx, "scan.skip", s))
                })
                .collect();
        }
        if let Some(list) = take_string_list(cx, &mut t, "scan", "header-skip") {
            out.header_skip = list;
        }
        if let Some(v) = take_u64(cx, &mut t, "scan", "max-file") {
            out.max_file = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "scan", "max-directives") {
            out.max_directives = v;
        }
        cx.unknown_in("scan", &t);
    }
    out
}

fn take_owners(cx: &mut Ctx<'_>, t: &mut toml::Table) -> Vec<Owner> {
    let Some(v) = t.remove("owners") else {
        return Vec::new();
    };
    let Some(items) = v.as_array() else {
        cx.error(
            "settings.owners",
            "`settings.owners` must be a list of handles",
        );
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        match item {
            toml::Value::String(h) if !h.is_empty() => out.push(Owner {
                handle: h.clone(),
                kind: OwnerKind::Human,
            }),
            toml::Value::Table(tb) => {
                let handle = tb
                    .get("handle")
                    .and_then(toml::Value::as_str)
                    .filter(|h| !h.is_empty());
                let kind = match tb.get("kind").and_then(toml::Value::as_str) {
                    None | Some("human") => Some(OwnerKind::Human),
                    Some("agent") => Some(OwnerKind::Agent),
                    Some(_) => None,
                };
                match (handle, kind) {
                    (Some(h), Some(k)) => out.push(Owner {
                        handle: h.to_string(),
                        kind: k,
                    }),
                    _ => cx.error(
                        "settings.owners",
                        "an owners entry needs a `handle` and kind \"human\" or \"agent\"",
                    ),
                }
            }
            _ => cx.error(
                "settings.owners",
                "an owners entry is a handle string or { handle, kind }",
            ),
        }
    }
    out
}

pub(crate) fn parse_settings(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Settings {
    let mut out = Settings::default();
    if let Some(mut t) = take_table(cx, root, "", "settings") {
        if let Some(f) = take_string(cx, &mut t, "settings", "folder") {
            out.folder = clean_segment(cx, "settings.folder", &f);
        }
        if let Some(v) = take_string(cx, &mut t, "settings", "fairlead-version") {
            if crate::discover::parse_pin_version(&v).is_some() {
                out.fairlead_version = Some(v);
            } else {
                cx.error(
                    "settings.fairlead-version",
                    format!("fairlead-version {v:?} must look like \"0.1\" or \"0.1.0\""),
                );
            }
        }
        out.fairlead_range = take_string(cx, &mut t, "settings", "fairlead-range");
        if let Some(v) = take_string(cx, &mut t, "settings", "index-values") {
            match v.as_str() {
                "value" => out.index_values = IndexValues::Value,
                "hash" => out.index_values = IndexValues::Hash,
                _ => cx.error(
                    "settings.index-values",
                    "index-values must be \"value\" or \"hash\"",
                ),
            }
        }
        if let Some(p) = take_string(cx, &mut t, "settings", "purpose") {
            if p.chars().count() > 160 {
                cx.error("settings.purpose", "purpose is at most 160 characters");
            } else {
                out.purpose = Some(p);
            }
        }
        out.owners = take_owners(cx, &mut t);
        cx.unknown_in("settings", &t);
    }
    out
}

pub(crate) fn parse_comments(
    cx: &mut Ctx<'_>,
    root: &mut toml::Table,
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    if let Some(t) = take_table(cx, root, "", "comments") {
        for (ext, v) in t {
            let key = join_key("comments", &ext);
            match string_array(&v) {
                Some(list) if !list.is_empty() => {
                    let mut ok = true;
                    for spec in &list {
                        // Verify: `Leader::parse` is a P2 addition (see Contract issues, item 5).
                        if fairlead_syntax::delimiters::Leader::from_spec(spec).is_none() {
                            cx.error(&key, format!("invalid comment leader {spec:?}"));
                            ok = false;
                        }
                    }
                    if ok {
                        out.insert(ext.trim_start_matches('.').to_ascii_lowercase(), list);
                    }
                }
                _ => cx.error(
                    &key,
                    format!("`{key}` must be a non-empty list of comment leaders"),
                ),
            }
        }
    }
    out
}

pub(crate) fn parse_agents(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(mut t) = take_table(cx, root, "", "agents") {
        if let Some(list) = take_string_list(cx, &mut t, "agents", "targets") {
            out = list
                .iter()
                .filter_map(|p| clean_rel_path(cx, "agents.targets", p))
                .collect();
        }
        cx.unknown_in("agents", &t);
    }
    out
}

pub(crate) fn parse_layout(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Layout {
    let mut out = Layout::default();
    if let Some(mut t) = take_table(cx, root, "", "layout") {
        for (key, slot) in [
            ("docs", &mut out.docs),
            ("contract", &mut out.contract),
            ("gen", &mut out.r#gen),
            ("ledger", &mut out.ledger),
            ("ref", &mut out.reference),
        ] {
            if let Some(v) = take_string(cx, &mut t, "layout", key) {
                if let Some(clean) = clean_segment(cx, &join_key("layout", key), &v) {
                    *slot = clean;
                }
            }
        }
        cx.unknown_in("layout", &t);
    }
    out
}

// ---------------------------------------------------------------- kinds, file kinds, items
fn valid_kind_name(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase())
}

fn parse_key_decls(cx: &mut Ctx<'_>, kind: &str, t: &mut toml::Table) -> BTreeMap<String, KeyDecl> {
    let mut out = BTreeMap::new();
    let prefix = format!("kinds.{kind}.keys");
    if let Some(keys) = take_table(cx, t, &format!("kinds.{kind}"), "keys") {
        for (name, v) in keys {
            let key = join_key(&prefix, &name);
            let Some(mut decl) = v.as_table().cloned() else {
                cx.error(&key, format!("`{key}` must be an inline table such as {{ type = \"enum\", values = [..] }}"));
                continue;
            };
            let ty = take_string(cx, &mut decl, &key, "type");
            let values = take_string_list(cx, &mut decl, &key, "values").unwrap_or_default();
            cx.unknown_in(&key, &decl);
            match ty {
                Some(ty) => {
                    out.insert(name, KeyDecl { ty, values });
                }
                None => cx.error(&key, format!("`{key}` needs a `type`")),
            }
        }
    }
    out
}

pub(crate) fn parse_kinds(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, KindDecl> {
    let mut out = BTreeMap::new();
    let Some(kinds) = take_table(cx, root, "", "kinds") else {
        return out;
    };
    for (name, v) in kinds {
        let key = join_key("kinds", &name);
        if !valid_kind_name(&name) {
            cx.error(&key, format!("kind name {name:?} must be 2 to 8 lowercase letters; put a readable label in `title`"));
            continue;
        }
        if name == "exc" {
            cx.error(
                &key,
                "`exc` is a reserved built-in kind and cannot be declared (R-SCOPE-065)",
            );
            continue;
        }
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let mut decl = KindDecl {
            title: take_string(cx, &mut t, &key, "title"),
            ..KindDecl::default()
        };
        if let Some(s) = take_string(cx, &mut t, &key, "shape") {
            match s.as_str() {
                "doc" => decl.shape = KindShape::Doc,
                "code" => decl.shape = KindShape::Code,
                "either" => decl.shape = KindShape::Either,
                _ => cx.error(&key, "shape must be \"doc\", \"code\" or \"either\""),
            }
        }
        match t.remove("impl") {
            None => {}
            Some(toml::Value::Integer(n)) => match u32::try_from(n) {
                Ok(n) => decl.impl_limit = ImplLimit::N(n),
                Err(_) => cx.error(
                    &key,
                    "impl must be a non-negative integer, \"any\" or \"none\"",
                ),
            },
            Some(toml::Value::String(s)) if s == "any" => decl.impl_limit = ImplLimit::Any,
            Some(toml::Value::String(s)) if s == "none" => decl.impl_limit = ImplLimit::None,
            Some(_) => cx.error(
                &key,
                "impl must be a non-negative integer, \"any\" or \"none\"",
            ),
        }
        if let Some(s) = take_string(cx, &mut t, &key, "unused") {
            match s.as_str() {
                "info" => decl.unused = Some(Severity::Info),
                "warn" => decl.unused = Some(Severity::Warn),
                "error" => decl.unused = Some(Severity::Error),
                _ => cx.error(&key, "unused must be \"info\", \"warn\" or \"error\""),
            }
        }
        if let Some(d) = take_string(cx, &mut t, &key, "dir") {
            decl.dir = clean_rel_path(cx, &key, &d);
        }
        if let Some(s) = take_string(cx, &mut t, &key, "source") {
            if s == "generate" {
                decl.source_generate = true;
                cx.error(&key, "source = \"generate\" needs a generator and none ships in 0.1; it is also refused for vendor-owned files (C-6, R-FILE-031)");
            } else {
                cx.error(&key, "source must be \"generate\" when present");
            }
        }
        decl.keys = parse_key_decls(cx, &name, &mut t);
        cx.unknown_in(&key, &t);
        out.insert(name, decl);
    }
    out
}

pub(crate) fn parse_file_kinds(
    cx: &mut Ctx<'_>,
    root: &mut toml::Table,
) -> BTreeMap<String, FileKindDecl> {
    let mut out = BTreeMap::new();
    let Some(fks) = take_table(cx, root, "", "file-kinds") else {
        return out;
    };
    for (name, v) in fks {
        let key = join_key("file-kinds", &name);
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let extractor = take_string(cx, &mut t, &key, "extractor");
        let binding = take_string(cx, &mut t, &key, "binding");
        let import_template = take_string(cx, &mut t, &key, "import-template");
        cx.unknown_in(&key, &t);
        match (extractor, binding) {
            (Some(extractor), Some(binding)) => {
                out.insert(
                    name,
                    FileKindDecl {
                        extractor,
                        binding,
                        import_template,
                    },
                );
            }
            _ => cx.error(
                &key,
                format!("`{key}` needs both `extractor` and `binding`"),
            ),
        }
    }
    out
}

fn check_id(cx: &mut Ctx<'_>, key: &str, raw: &str, allow_qualified: bool) -> bool {
    use fairlead_syntax::ids::{Id, IdQualifier};
    match Id::parse(raw) {
        Ok(id) if allow_qualified || matches!(id.qualifier, IdQualifier::Bare) => true,
        Ok(_) => {
            cx.error(
                key,
                format!("{raw:?} must be an unqualified kind.name here"),
            );
            false
        }
        Err(e) => {
            cx.error(key, format!("{raw:?} is not a valid ID: {e:?}"));
            false
        }
    }
}

pub(crate) fn parse_files(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<FileItemDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "file") {
        let before = cx.diags.len();
        let id = take_string(cx, &mut t, "file", "id");
        let kind = take_string(cx, &mut t, "file", "kind");
        let path = take_string(cx, &mut t, "file", "path");
        let extends = take_string(cx, &mut t, "file", "extends");
        cx.unknown_in("file", &t);
        let (Some(id), Some(kind), Some(path)) = (id, kind, path) else {
            if cx.diags.len() == before {
                cx.error("file", "a [[file]] needs `id`, `kind` and `path`");
            }
            continue;
        };
        if !check_id(cx, "file.id", &id, false) {
            continue;
        }
        if let Some(e) = &extends {
            if !check_id(cx, "file.extends", e, true) {
                continue;
            }
        }
        if let Some(path) = clean_rel_path(cx, "file.path", &path) {
            out.push(FileItemDecl {
                id,
                kind,
                path,
                extends,
            });
        }
    }
    out
}

pub(crate) fn parse_vendors(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<VendorDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "vendor") {
        let before = cx.diags.len();
        let path = take_string(cx, &mut t, "vendor", "path");
        let tool = take_string(cx, &mut t, "vendor", "tool");
        cx.unknown_in("vendor", &t);
        match (path, tool) {
            (Some(path), Some(tool)) => {
                if let Some(path) = clean_rel_path(cx, "vendor.path", &path) {
                    if check_glob(cx, "vendor.path", &path) {
                        out.push(VendorDecl { path, tool });
                    }
                }
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error("vendor", "a [[vendor]] needs `path` and `tool`");
                }
            }
        }
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-_.#/@%:*".contains(c)
}

fn to_arg_value(v: &toml::Value) -> Option<ArgValue> {
    match v {
        toml::Value::String(s) if !s.is_empty() && s.chars().all(is_word_char) => {
            Some(ArgValue::Word(s.clone()))
        }
        toml::Value::String(s) => Some(ArgValue::Quoted(s.clone())),
        toml::Value::Integer(i) => Some(ArgValue::Word(i.to_string())),
        toml::Value::Boolean(b) => Some(ArgValue::Word(b.to_string())),
        toml::Value::Array(_) => string_array(v).map(ArgValue::List),
        _ => None,
    }
}

pub(crate) fn parse_ovrs(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<OvrDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "ovr") {
        let before = cx.diags.len();
        let target = take_string(cx, &mut t, "ovr", "target");
        let op = match take_string(cx, &mut t, "ovr", "op").as_deref() {
            None | Some("set") => Some(OvrOp::Set),
            Some("loosen") => Some(OvrOp::Loosen),
            Some("tighten") => Some(OvrOp::Tighten),
            Some("replace") => Some(OvrOp::Replace),
            Some("add") => Some(OvrOp::Add),
            Some("remove") => Some(OvrOp::Remove),
            Some(other) => {
                cx.error(
                    "ovr.op",
                    format!(
                        "unknown op {other:?}; use loosen, tighten, set, replace, add or remove"
                    ),
                );
                None
            }
        };
        let applies = take_string(cx, &mut t, "ovr", "applies");
        let why = take_string(cx, &mut t, "ovr", "why");
        let owner = take_string(cx, &mut t, "ovr", "owner");
        let exp = take_date_string(cx, &mut t, "ovr", "exp");
        let base = take_string(cx, &mut t, "ovr", "base");
        let mut set = BTreeMap::new();
        for (k, v) in std::mem::take(&mut t) {
            match to_arg_value(&v) {
                Some(av) => {
                    set.insert(k, av);
                }
                None => cx.error(
                    &join_key("ovr", &k),
                    format!("`ovr.{k}` must be a string, number, bool or list of strings"),
                ),
            }
        }
        match (target, op) {
            (Some(target), Some(op)) => {
                if check_id(cx, "ovr.target", &target, true) && cx.diags.len() == before {
                    out.push(OvrDecl {
                        target,
                        op,
                        applies,
                        set,
                        why,
                        owner,
                        exp,
                        base,
                    });
                }
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error("ovr", "an [[ovr]] needs a `target`");
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;

    fn cx(text: &str) -> (Ctx<'_>, toml::Table) {
        let table: toml::Table = text.parse().unwrap();
        (
            Ctx::new(&RepoPath::new("fairlead/control.toml").unwrap(), text),
            table,
        )
    }

    #[test]
    fn r_arch_021__defaults_for_scan_live_in_config_not_in_code() {
        let (mut c, mut t) = cx("");
        let s = parse_scan(&mut c, &mut t);
        assert!(
            s.exclude.is_empty(),
            "no hard-coded default excludes: `init` writes them into control.toml"
        );
        assert_eq!((s.max_file, s.max_directives), (2_000_000, 500));
        assert!(c.diags.is_empty() && c.unknown.is_empty());
    }

    #[test]
    fn r_syn_039__scan_values_parse() {
        let text = "[scan]\nexclude = [\"target/\", \"node_modules/\"]\nskip = [\"tests/fixtures/**\"]\nheader-skip = [\"^<\\\\?php\"]\nmax-file = 1000\nmax-directives = 7\n";
        let (mut c, mut t) = cx(text);
        let s = parse_scan(&mut c, &mut t);
        assert_eq!(s.exclude, ["target/", "node_modules/"]);
        assert_eq!(s.skip, ["tests/fixtures/**"]);
        assert_eq!(s.header_skip.len(), 1);
        assert_eq!((s.max_file, s.max_directives), (1000, 7));
        assert!(c.diags.is_empty());
    }

    #[test]
    fn r_syn_039__unknown_keys_are_collected_not_fatal() {
        let (mut c, mut t) = cx("[scan]\nbogus = 1\nexclude = []\n");
        let _ = parse_scan(&mut c, &mut t);
        assert!(c.diags.is_empty());
        assert_eq!(
            c.unknown,
            vec![UnknownKey {
                path: "scan.bogus".into(),
                line: 2
            }]
        );
    }

    #[test]
    fn r_syn_039__wrong_type_is_hc001() {
        let (mut c, mut t) = cx("[scan]\nexclude = \"target/\"\nmax-file = -5\n");
        let _ = parse_scan(&mut c, &mut t);
        assert_eq!(c.diags.len(), 2);
        assert!(c.diags.iter().all(|d| d.code == codes::HN_C001));
    }

    #[test]
    fn r_syn_051__settings_use_fairlead_range_not_requires() {
        let (mut c, mut t) = cx(
            "[settings]\nfairlead-range = \">=0.1\"\nrequires = \"x\"\nfairlead-version = \"0.1\"\nfolder = \"fairlead\"\nindex-values = \"hash\"\npurpose = \"Sync app\"\n",
        );
        let s = parse_settings(&mut c, &mut t);
        assert_eq!(s.fairlead_range.as_deref(), Some(">=0.1"));
        assert_eq!(s.fairlead_version.as_deref(), Some("0.1"));
        assert_eq!(s.index_values, IndexValues::Hash);
        assert_eq!(s.purpose.as_deref(), Some("Sync app"));
        assert_eq!(
            c.unknown
                .iter()
                .map(|u| u.path.as_str())
                .collect::<Vec<_>>(),
            ["settings.requires"]
        );
        assert!(c.diags.is_empty());
    }

    #[test]
    fn r_syn_038__settings_owners_accept_handles_and_inline_tables() {
        let (mut c, mut t) = cx(
            "[settings]\nowners = [\"@platform\", { handle = \"@bot\", kind = \"agent\" }, { handle = \"@d\", kind = \"human\" }]\n",
        );
        let s = parse_settings(&mut c, &mut t);
        assert_eq!(
            s.owners,
            vec![
                Owner {
                    handle: "@platform".into(),
                    kind: OwnerKind::Human
                },
                Owner {
                    handle: "@bot".into(),
                    kind: OwnerKind::Agent
                },
                Owner {
                    handle: "@d".into(),
                    kind: OwnerKind::Human
                },
            ]
        );
        assert!(c.diags.is_empty());
        let (mut c2, mut t2) =
            cx("[settings]\nowners = [{ handle = \"@x\", kind = \"robot\" }, 5]\n");
        let _ = parse_settings(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 2);
    }

    #[test]
    fn r_syn_038__settings_purpose_is_at_most_160_chars_and_index_values_is_checked() {
        let long = "x".repeat(161);
        let text = format!(
            "[settings]\npurpose = \"{long}\"\nindex-values = \"both\"\nfairlead-version = \"zzz\"\n"
        );
        let (mut c, mut t) = cx(&text);
        let _ = parse_settings(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_syn_040__layout_names_and_paths_get_hygiene() {
        let (mut c, mut t) = cx("[layout]\ngen = \"CON\"\ndocs = \"a/b\"\nref = \"reference\"\n");
        let l = parse_layout(&mut c, &mut t);
        assert_eq!(
            c.diags.len(),
            2,
            "reserved name and multi-segment name are errors"
        );
        assert!(c.diags.iter().any(|d| d.code == codes::HN_P001));
        assert_eq!(l.reference, "reference");
        assert_eq!(
            l.contract, "contract",
            "unset keys keep their defaults (R-FILE-020)"
        );
    }

    #[test]
    fn r_syn_040__scan_patterns_are_nfc_and_forward_slashes() {
        let (mut c, mut t) = cx("[scan]\nexclude = [\"build\\\\out/\", \"cafe\\u0301/\"]\n");
        let s = parse_scan(&mut c, &mut t);
        assert_eq!(s.exclude, ["build/out/", "caf\u{e9}/"]);
    }

    #[test]
    fn r_syn_039__comments_and_agents_tables() {
        let (mut c, mut t) = cx(
            "[comments]\n\".Vue\" = [\"<!-- -->\", \"//\"]\nrs = []\n\n[agents]\ntargets = [\"AGENTS.md\", \"CLAUDE.md\", \"../x\"]\n",
        );
        let m = parse_comments(&mut c, &mut t);
        assert_eq!(
            m.get("vue").unwrap(),
            &vec!["<!-- -->".to_string(), "//".to_string()]
        );
        let a = parse_agents(&mut c, &mut t);
        assert_eq!(a, ["AGENTS.md", "CLAUDE.md"]);
        assert_eq!(
            c.diags.len(),
            2,
            "empty leader list and a `..` target are errors"
        );
    }

    #[test]
    fn r_syn_041__policy_and_stage_orders() {
        assert!(Policy::Open < Policy::NeedsException && Policy::NeedsException < Policy::Sealed);
        assert!(Stage::Observe < Stage::Warn && Stage::Warn < Stage::Enforce);
    }

    #[test]
    fn r_scope_065__kinds_exc_is_rejected_at_load() {
        let (mut c, mut t) =
            cx("[kinds.exc]\ntitle = \"x\"\n\n[kinds.tok]\ntitle = \"design token\"\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 1);
        assert_eq!(c.diags[0].code, codes::HN_C001);
        assert!(c.diags[0].message.contains("exc"), "{}", c.diags[0].message);
        assert!(kinds.contains_key("tok") && !kinds.contains_key("exc"));
    }

    #[test]
    fn r_syn_050__kind_names_are_two_to_eight_lowercase_letters() {
        let (mut c, mut t) = cx("[kinds.a]\n[kinds.tauri-command]\n[kinds.Cmp]\n[kinds.variant]\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
        assert_eq!(kinds.keys().collect::<Vec<_>>(), ["variant"]);
    }

    #[test]
    fn r_syn_050__kind_fields_parse() {
        let text = "[kinds.cmp]\ntitle = \"component\"\nshape = \"code\"\nimpl = 1\nunused = \"warn\"\ndir = \"docs/components/\"\n[kinds.cmp.keys]\nlayer = { type = \"enum\", values = [\"atom\", \"molecule\"] }\n\n[kinds.adr]\nshape = \"doc\"\nimpl = \"none\"\n";
        let (mut c, mut t) = cx(text);
        let kinds = parse_kinds(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let cmp = &kinds["cmp"];
        assert_eq!(
            (cmp.shape, cmp.impl_limit, cmp.unused),
            (KindShape::Code, ImplLimit::N(1), Some(Severity::Warn))
        );
        assert_eq!(cmp.dir.as_deref(), Some("docs/components/"));
        assert_eq!(
            cmp.keys["layer"],
            KeyDecl {
                ty: "enum".into(),
                values: vec!["atom".into(), "molecule".into()]
            }
        );
        let adr = &kinds["adr"];
        assert_eq!(
            (adr.shape, adr.impl_limit),
            (KindShape::Doc, ImplLimit::None)
        );
        assert_eq!(KindDecl::default().shape, KindShape::Either);
    }

    #[test]
    fn r_file_031__source_generate_is_rejected_in_0_1() {
        let (mut c, mut t) = cx("[kinds.tok]\nsource = \"generate\"\n");
        let kinds = parse_kinds(&mut c, &mut t);
        assert_eq!(c.diags.len(), 1);
        assert!(
            c.diags[0].message.contains("generator"),
            "{}",
            c.diags[0].message
        );
        assert!(kinds["tok"].source_generate);
        let (mut c2, mut t2) = cx("[kinds.tok]\nsource = \"hand\"\n");
        let _ = parse_kinds(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
    }

    #[test]
    fn r_file_050__file_items_parse_with_extends() {
        let text = "[[file]]\nid = \"file.console-styles\"\nkind = \"stylesheet\"\npath = \"app\\\\overrides.css\"\nextends = \"root:file.styles\"\n\n[[file]]\nid = \"file.styles\"\nkind = \"stylesheet\"\npath = \"app/globals.css\"\n";
        let (mut c, mut t) = cx(text);
        let files = parse_files(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "app/overrides.css");
        assert_eq!(files[0].extends.as_deref(), Some("root:file.styles"));
        assert_eq!(files[1].extends, None);
    }

    #[test]
    fn r_file_050__file_item_requires_id_kind_path_and_a_bare_id() {
        let (mut c, mut t) = cx(
            "[[file]]\nkind = \"stylesheet\"\npath = \"a.css\"\n\n[[file]]\nid = \"web:file.x\"\nkind = \"k\"\npath = \"a.css\"\n\n[[file]]\nid = \"nodot\"\nkind = \"k\"\npath = \"a.css\"\n",
        );
        let files = parse_files(&mut c, &mut t);
        assert!(files.is_empty());
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_file_005__vendor_decl_needs_path_and_tool() {
        let (mut c, mut t) = cx(
            "[[vendor]]\npath = \"components.json\"\ntool = \"shadcn\"\n\n[[vendor]]\npath = \"ui\\\\**\"\n\n[[vendor]]\ntool = \"x\"\n\n[[vendor]]\npath = \"[bad\"\ntool = \"t\"\n",
        );
        let v = parse_vendors(&mut c, &mut t);
        assert_eq!(
            v,
            vec![VendorDecl {
                path: "components.json".into(),
                tool: "shadcn".into()
            }]
        );
        assert_eq!(
            c.diags.len(),
            3,
            "missing tool, missing path, and a glob that does not compile"
        );
    }

    #[test]
    fn r_file_020__file_kinds_need_extractor_and_binding() {
        let (mut c, mut t) = cx(
            "[file-kinds.stylesheet]\nextractor = \"css-custom-properties\"\nbinding = \"import\"\nimport-template = \"@import '{path}';\"\n\n[file-kinds.broken]\nextractor = \"x\"\n",
        );
        let fk = parse_file_kinds(&mut c, &mut t);
        assert_eq!(
            fk["stylesheet"].import_template.as_deref(),
            Some("@import '{path}';")
        );
        assert_eq!(fk.len(), 1);
        assert_eq!(c.diags.len(), 1);
    }

    #[test]
    fn r_syn_039__ovr_table_parses_known_keys_and_field_sets() {
        let text = "[[ovr]]\ntarget = \"rule.no-raw-hex\"\nop = \"loosen\"\napplies = \"src/legacy/**\"\nwhy = \"legacy\"\nowner = \"@platform\"\nexp = 2026-12-31\nstage = \"warn\"\n\"globs+\" = [\"a\", \"b\"]\nnote = \"two words\"\n";
        let (mut c, mut t) = cx(text);
        let o = parse_ovrs(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(o.len(), 1);
        assert_eq!(
            (o[0].op, o[0].exp.as_deref(), o[0].owner.as_deref()),
            (OvrOp::Loosen, Some("2026-12-31"), Some("@platform"))
        );
        assert_eq!(o[0].set["stage"], ArgValue::Word("warn".into()));
        assert_eq!(
            o[0].set["globs+"],
            ArgValue::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(o[0].set["note"], ArgValue::Quoted("two words".into()));
    }

    #[test]
    fn r_syn_039__ovr_defaults_and_errors() {
        let (mut c, mut t) = cx(
            "[[ovr]]\ntarget = \"rule.x\"\nwhy = \"w\"\n\n[[ovr]]\nwhy = \"no target\"\n\n[[ovr]]\ntarget = \"rule.y\"\nop = \"explode\"\nexp = \"soon\"\n",
        );
        let o = parse_ovrs(&mut c, &mut t);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].op, OvrOp::Set);
        assert_eq!(c.diags.len(), 3, "missing target, bad op, bad exp");
    }
}
