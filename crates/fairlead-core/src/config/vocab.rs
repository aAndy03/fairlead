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
}

impl Default for LcConfig {
    fn default() -> Self {
        Self {
            warn_window: 1,
            stale_after: 3,
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
}

impl Default for GraveyardConfig {
    fn default() -> Self {
        Self {
            ids_percent: 60,
            shingles_percent: 50,
        }
    }
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
