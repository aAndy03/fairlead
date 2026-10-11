//! Chapter 21 control tables: `[env]`, `[env.alias]`, `[maturity]`, `[lifecycle.alias]`,
//! `[bind.<env>]`, `[lc]`, `[authority]`, `[graveyard]` (root only, R-ENV-001) and the scope
//! `[defaults]` (`in`, `mat`; any scope, R-ENV-005).
use std::collections::BTreeMap;

use super::schema::{
    Ctx, join_key, string_array, take_bool, take_string, take_string_list, take_table, take_u32,
};

/// The fixed lifecycle states (R-ENV-004); only their display aliases are configurable.
pub const LIFECYCLE_STATES: [&str; 5] = ["proposed", "active", "deprecated", "removed", "rejected"];

/// `[bind.<env>]` (or the shorthand `[bind] <env> = "feature:x"`): the real gate tokens of an env.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BindDecl {
    /// Rust gate tokens (`feature="devtools"`, `cfg(debug_assertions)`, `feature:devtools`).
    pub rust: Vec<String>,
    /// TS flag tokens (`import.meta.env.DEV`, `process.env.NODE_ENV`).
    pub ts: Vec<String>,
}

/// The root vocabulary. Empty `env_order` means no env path is declared (the axes are off).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VocabConfig {
    pub env_order: Vec<String>,
    pub env_alias: BTreeMap<String, String>,
    pub maturity: BTreeMap<String, i64>,
    pub lifecycle_alias: BTreeMap<String, String>,
    pub bind: BTreeMap<String, BindDecl>,
}

/// `[lc]`: the due-date window (R-LC-004) and the stale-plan threshold (R-LC-006).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LcConfig {
    pub warn_window: u32,
    pub stale_after: u32,
    /// The root has an `[lc]` table.
    pub declared: bool,
}

impl Default for LcConfig {
    fn default() -> Self {
        Self {
            warn_window: 1,
            stale_after: 3,
            declared: false,
        }
    }
}

/// `[authority]` (R-LC-040).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AuthorityConfig {
    pub signed_records: bool,
}

/// `[graveyard]`: match thresholds in percent (R-LC-031: ids 60, shingles 50).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GraveyardConfig {
    pub ids_percent: u32,
    pub shingles_percent: u32,
    /// The root has a `[graveyard]` table: files are fingerprinted for re-introduction checks.
    pub declared: bool,
}

impl Default for GraveyardConfig {
    fn default() -> Self {
        Self {
            ids_percent: 60,
            shingles_percent: 50,
            declared: false,
        }
    }
}

/// `[[attach]]` (R-SYN-047): directives for files that cannot carry comments, bound in file form
/// to every file matching `path` (a glob relative to the declaring scope).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AttachDecl {
    pub path: String,
    pub directives: Vec<String>,
}

pub(crate) fn parse_attach(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<AttachDecl> {
    use fairlead_syntax::directive::{Verb, parse_body};
    let mut out = Vec::new();
    for mut t in super::schema::take_table_array(cx, root, "", "attach") {
        let path = take_string(cx, &mut t, "attach", "path")
            .and_then(|g| super::schema::clean_rel_path(cx, "attach.path", &g))
            .filter(|g| super::schema::check_glob(cx, "attach.path", g));
        let directives = take_string_list(cx, &mut t, "attach", "directives").unwrap_or_default();
        cx.unknown_in("attach", &t);
        let mut ok = true;
        for d in &directives {
            match parse_body(d) {
                Err(e) => {
                    cx.error("attach.directives", format!("{d:?}: {}", e.message));
                    ok = false;
                }
                Ok(b) if b.verb == Verb::End || b.explicit.is_some() => {
                    cx.error(
                        "attach.directives",
                        format!("{d:?}: an attached directive binds the whole file: no `end`, no +N or +end"),
                    );
                    ok = false;
                }
                Ok(_) => {}
            }
        }
        match path {
            Some(path) if ok && !directives.is_empty() => out.push(AttachDecl { path, directives }),
            Some(_) if directives.is_empty() => {
                cx.error("attach", "an [[attach]] needs `directives`")
            }
            None => cx.error("attach", "an [[attach]] needs `path`"),
            _ => {}
        }
    }
    out
}

/// Tables whose keys are G1 vocabulary: never delegable (R-SCOPE-051, R-SCOPE-057).
pub const G1_TABLES: [&str; 22] = [
    "fairlead",
    "settings",
    "scan",
    "comments",
    "agents",
    "layout",
    "env",
    "maturity",
    "lifecycle",
    "bind",
    "flagwords",
    "layers",
    "free",
    "flow",
    "layer-policy",
    "hier",
    "policies",
    "version",
    "lc",
    "authority",
    "graveyard",
    "secrets",
];

/// `[delegate]` (R-SCOPE-057): `"<key pattern>" = "tighten"`; a pattern is a dotted key, or a
/// prefix ending in `.*`.
pub(crate) fn parse_delegate(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(t) = take_table(cx, root, "", "delegate") {
        for (pat, v) in t {
            let k = join_key("delegate", &pat);
            let first = pat.split('.').next().unwrap_or("");
            if G1_TABLES.contains(&first) {
                cx.error(
                    &k,
                    format!("`{pat}` is a vocabulary (G1) key: it cannot be delegated"),
                );
            } else if v.as_str() != Some("tighten") {
                cx.error(&k, format!("`{k}` must be \"tighten\""));
            } else {
                out.insert(pat, "tighten".to_owned());
            }
        }
    }
    out
}

/// A stripped child value, kept for the delegation judgement (comparable, unlike `toml::Value`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Kept {
    Int(i64),
    Str(String),
    /// Any other value, as TOML text.
    Other(String),
}

impl Kept {
    pub fn of(v: &toml::Value) -> Kept {
        match v {
            toml::Value::Integer(i) => Kept::Int(*i),
            toml::Value::String(s) => Kept::Str(s.clone()),
            other => Kept::Other(other.to_string()),
        }
    }
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Kept::Int(i) => Some(*i),
            _ => None,
        }
    }
}

/// Does a `[delegate]` pattern cover the dotted key?
pub fn delegate_matches(pattern: &str, key: &str) -> bool {
    pattern == key
        || pattern
            .strip_suffix(".*")
            .is_some_and(|pre| key.starts_with(&format!("{pre}.")))
}

/// The 0.1 defaults of the numeric G2 keys a delegation compares against.
fn g2_default(key: &str) -> Option<i64> {
    match key {
        "exceptions.max-live-per-rule" => Some(10),
        _ => None,
    }
}

fn level_rank(s: &str) -> Option<u8> {
    Some(match s {
        "observe" | "info" => 0,
        "warn" => 1,
        "enforce" | "error" => 2,
        _ => return None,
    })
}

/// R-SCOPE-034, R-SCOPE-057: a delegated child value is accepted only when it is at least as
/// strict as the root's: a lower limit (`max-*`), a higher minimum (`min-*`), a higher
/// severity or stage word; anything else only when equal.
pub fn tightens(key: &str, child: &Kept, root: Option<&Kept>) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    match (child, root) {
        (Kept::Int(c), r) => {
            let base = r.and_then(Kept::as_integer).or_else(|| g2_default(key));
            match base {
                None => true,
                Some(b) if last.starts_with("min-") => *c >= b,
                Some(b) => *c <= b,
            }
        }
        (Kept::Str(c), Some(Kept::Str(r))) => match (level_rank(c), level_rank(r)) {
            (Some(a), Some(b)) => a >= b,
            _ => c == r,
        },
        (Kept::Str(c), None) => level_rank(c).is_some(),
        (c, Some(r)) => c == r,
        (_, None) => false,
    }
}

/// `[scope]` (0.2 part): `portable` and `detached` (R-PORT-013). `provides` and `requires`
/// arrive with 0.3 portability and stay unknown keys here.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ScopeConfig {
    pub portable: bool,
    pub detached: bool,
    /// R-SCOPE-029: what sibling scopes may name with a qualified ID (`kind.name` or `kind.*`).
    pub provides: Vec<String>,
    /// R-HIER-004, D-65: the layers (or `<ladder>.*`) units of this scope may name; empty: all.
    pub accepts: Vec<String>,
    /// R-HIER-004, D-65: local words for layers (`widget = "ui.molecule"`); never a reorder.
    pub layer_alias: BTreeMap<String, String>,
    /// R-HIER-081, D-65: orphan files of this scope are never absorbed by the baseline.
    pub strict_coverage: bool,
}

pub(crate) fn parse_scope_cfg(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ScopeConfig {
    let mut out = ScopeConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "scope") {
        out.portable = take_bool(cx, &mut t, "scope", "portable").unwrap_or(false);
        out.detached = take_bool(cx, &mut t, "scope", "detached").unwrap_or(false);
        out.provides = take_string_list(cx, &mut t, "scope", "provides").unwrap_or_default();
        out.accepts = take_string_list(cx, &mut t, "scope", "accepts").unwrap_or_default();
        out.strict_coverage = take_bool(cx, &mut t, "scope", "strict-coverage").unwrap_or(false);
        if let Some(a) = take_table(cx, &mut t, "scope", "layer-alias") {
            for (word, layer) in a {
                match layer.as_str() {
                    Some(l) if word_ok(&word) => {
                        out.layer_alias.insert(word, l.to_owned());
                    }
                    _ => cx.error(
                        "scope.layer-alias",
                        format!(
                            "`scope.layer-alias.{word}` must map a lowercase word to a layer name"
                        ),
                    ),
                }
            }
        }
        cx.unknown_in("scope", &t);
    }
    out
}

/// `[defaults]`: the scope's env set and default maturity (R-ENV-005, R-ENV-006).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct AxisDefaults {
    pub envs: Option<Vec<String>>,
    pub mat: Option<String>,
}

fn word_ok(w: &str) -> bool {
    !w.is_empty()
        && w.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn words(cx: &mut Ctx<'_>, key: &str, list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in list {
        if !word_ok(&w) {
            cx.error(key, format!("{w:?} in `{key}` is not a lowercase word"));
        } else if out.contains(&w) {
            cx.error(key, format!("{w:?} appears twice in `{key}`"));
        } else {
            out.push(w);
        }
    }
    out
}

fn alias_table(
    cx: &mut Ctx<'_>,
    t: toml::Table,
    key: &str,
    targets: &[&str],
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (alias, v) in t {
        let k = join_key(key, &alias);
        match v.as_str() {
            Some(to)
                if targets.contains(&to)
                    && word_ok(&alias)
                    && !targets.contains(&alias.as_str()) =>
            {
                out.insert(alias, to.to_string());
            }
            Some(to) => cx.error(
                &k,
                format!("`{k}` must alias a lowercase word to one of {targets:?}, not {to:?}"),
            ),
            None => cx.error(&k, format!("`{k}` must be a string")),
        }
    }
    out
}

pub(crate) fn parse_vocab(cx: &mut Ctx<'_>, root: &mut toml::Table) -> VocabConfig {
    let mut out = VocabConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "env") {
        let order = take_string_list(cx, &mut t, "env", "order").unwrap_or_default();
        out.env_order = words(cx, "env.order", order);
        if let Some(a) = take_table(cx, &mut t, "env", "alias") {
            let targets: Vec<&str> = out.env_order.iter().map(String::as_str).collect();
            out.env_alias = alias_table(cx, a, "env.alias", &targets);
        }
        cx.unknown_in("env", &t);
    }
    if let Some(t) = take_table(cx, root, "", "maturity") {
        for (word, v) in t {
            let k = join_key("maturity", &word);
            match v.as_integer() {
                Some(n) if n >= 0 && word_ok(&word) => {
                    out.maturity.insert(word, n);
                }
                _ => cx.error(
                    &k,
                    format!("`{k}` must map a lowercase word to a rank >= 0"),
                ),
            }
        }
    }
    if let Some(mut t) = take_table(cx, root, "", "lifecycle") {
        if let Some(a) = take_table(cx, &mut t, "lifecycle", "alias") {
            out.lifecycle_alias = alias_table(cx, a, "lifecycle.alias", &LIFECYCLE_STATES);
        }
        cx.unknown_in("lifecycle", &t);
    }
    if let Some(t) = take_table(cx, root, "", "bind") {
        for (env, v) in t {
            let k = join_key("bind", &env);
            let mut b = BindDecl::default();
            match v {
                toml::Value::Table(mut bt) => {
                    b.rust = take_string_list(cx, &mut bt, &k, "rust").unwrap_or_default();
                    b.ts = take_string_list(cx, &mut bt, &k, "ts").unwrap_or_default();
                    cx.unknown_in(&k, &bt);
                }
                toml::Value::String(s) => b.rust.push(s),
                other => match string_array(&other) {
                    Some(l) => b.rust = l,
                    None => cx.error(&k, format!("`{k}` must be a table, a string or a list")),
                },
            }
            out.bind.insert(env, b);
        }
    }
    out
}

pub(crate) fn parse_lc(cx: &mut Ctx<'_>, root: &mut toml::Table) -> LcConfig {
    let mut out = LcConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "lc") {
        out.declared = true;
        if let Some(v) = take_u32(cx, &mut t, "lc", "warn-window") {
            out.warn_window = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "lc", "stale-after") {
            out.stale_after = v.max(1);
        }
        cx.unknown_in("lc", &t);
    }
    out
}

pub(crate) fn parse_authority(cx: &mut Ctx<'_>, root: &mut toml::Table) -> AuthorityConfig {
    let mut out = AuthorityConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "authority") {
        out.signed_records = take_bool(cx, &mut t, "authority", "signed-records").unwrap_or(false);
        cx.unknown_in("authority", &t);
    }
    out
}

pub(crate) fn parse_graveyard(cx: &mut Ctx<'_>, root: &mut toml::Table) -> GraveyardConfig {
    let mut out = GraveyardConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "graveyard") {
        out.declared = true;
        for (key, slot) in [
            ("ids-percent", &mut out.ids_percent),
            ("shingles-percent", &mut out.shingles_percent),
        ] {
            if let Some(v) = take_u32(cx, &mut t, "graveyard", key) {
                if v > 100 {
                    cx.error(&join_key("graveyard", key), "a percentage is 0 to 100");
                } else {
                    *slot = v;
                }
            }
        }
        cx.unknown_in("graveyard", &t);
    }
    out
}

pub(crate) fn parse_defaults(cx: &mut Ctx<'_>, root: &mut toml::Table) -> AxisDefaults {
    let mut out = AxisDefaults::default();
    if let Some(mut t) = take_table(cx, root, "", "defaults") {
        out.envs = match t.remove("in") {
            None => None,
            Some(toml::Value::String(s)) => Some(vec![s]),
            Some(v) => match string_array(&v) {
                Some(l) => Some(l),
                None => {
                    cx.error(
                        "defaults.in",
                        "`defaults.in` must be a word or a list of words",
                    );
                    None
                }
            },
        };
        out.mat = take_string(cx, &mut t, "defaults", "mat");
        cx.unknown_in("defaults", &t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::RepoPath;

    fn parse(text: &str) -> (VocabConfig, usize) {
        let mut t: toml::Table = text.parse().unwrap();
        let mut cx = Ctx::new(&RepoPath::new("fairlead/control.toml").unwrap(), text);
        let v = parse_vocab(&mut cx, &mut t);
        (v, cx.diags.len())
    }

    #[test]
    fn r_env_002__envs_ordered_by_exposure() {
        let (v, errs) = parse(
            "[env]\norder = [\"dev\", \"staging\", \"prod\"]\n[maturity]\nalpha = 0\nbeta = 1\nrc = 2\nstable = 5\n",
        );
        assert_eq!(errs, 0);
        assert_eq!(
            v.env_order,
            ["dev", "staging", "prod"],
            "declared order, not sorted"
        );
        assert_eq!(
            (v.maturity["alpha"], v.maturity["stable"]),
            (0, 5),
            "gaps allowed"
        );
    }

    #[test]
    fn r_env_003__aliases_must_point_at_a_declared_word() {
        let (v, errs) = parse(
            "[env]\norder = [\"dev\", \"prod\"]\n[env.alias]\nlocal = \"dev\"\nqa = \"staging\"\n[lifecycle.alias]\nsunsetting = \"deprecated\"\n",
        );
        assert_eq!(errs, 1, "qa -> staging names no env");
        assert_eq!(v.env_alias.get("local").map(String::as_str), Some("dev"));
        assert_eq!(v.lifecycle_alias["sunsetting"], "deprecated");
    }

    #[test]
    fn r_env_030__bind_accepts_the_table_and_the_shorthand() {
        let (v, errs) = parse(
            "[env]\norder = [\"dev\", \"prod\"]\n[bind.dev]\nrust = ['feature=\"devtools\"']\nts = [\"import.meta.env.DEV\"]\n[bind]\nprod = \"cfg(not(debug_assertions))\"\n",
        );
        assert_eq!(errs, 0);
        assert_eq!(v.bind["dev"].ts, ["import.meta.env.DEV"]);
        assert_eq!(v.bind["prod"].rust, ["cfg(not(debug_assertions))"]);
    }
}
