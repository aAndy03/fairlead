//! `[retain]` and `[retain.<kind>]` (33-retention-and-compaction, R-RET-010 to R-RET-013).
use std::collections::BTreeMap;

use super::schema::{Ctx, take_string, take_table, take_u32};

/// A retention TTL (R-RET-010, R-RET-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ttl {
    /// `"8v"`: sealed versions after the record closed.
    Versions(u32),
    /// `"90d"`, `"2y"` (365 days a year): calendar days by the recorded seal timestamps.
    Days(u32),
    /// `"life"`: until the `bind` item is removed (plus one grace version).
    Life,
    /// `"never"`: only the user removes it.
    Never,
}

impl Ttl {
    pub fn parse(s: &str) -> Option<Ttl> {
        let s = s.trim();
        match s {
            "life" => return Some(Ttl::Life),
            "never" => return Some(Ttl::Never),
            _ => {}
        }
        let (n, unit) = s.split_at(s.len().checked_sub(1)?);
        let n: u32 = n.parse().ok()?;
        match unit {
            "v" => Some(Ttl::Versions(n)),
            "d" => Some(Ttl::Days(n)),
            "y" => n.checked_mul(365).map(Ttl::Days),
            _ => None,
        }
    }

    /// Whether `self` keeps records at least as long as `other` (R-RET-013). Units that do not
    /// compare (versions against days) are taken as not shorter.
    pub fn at_least(self, other: Ttl) -> bool {
        match (self, other) {
            (Ttl::Never, _) => true,
            (_, Ttl::Never) => false,
            (Ttl::Versions(a), Ttl::Versions(b)) | (Ttl::Days(a), Ttl::Days(b)) => a >= b,
            _ => true,
        }
    }

    pub fn text(self) -> String {
        match self {
            Ttl::Versions(n) => format!("{n}v"),
            Ttl::Days(n) => format!("{n}d"),
            Ttl::Life => "life".into(),
            Ttl::Never => "never".into(),
        }
    }
}

/// One `[retain.<kind>]` table; absent keys inherit.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KindRetain {
    pub hot: Option<u32>,
    pub ttl: Option<Ttl>,
    pub min_age: Option<u32>,
    pub keep_rejected: Option<bool>,
    pub bind: Option<String>,
    pub max: Option<Ttl>,
}

/// `[retain]` of one control file.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RetainConfig {
    /// The file has a `[retain]` table: at the root this turns compaction on (E3-30).
    pub declared: bool,
    /// `min-age` in days (default 90, D-39).
    pub min_age: Option<u32>,
    /// `roll-after` in versions (default 8, R-RET-026).
    pub roll_after: Option<u32>,
    pub kinds: BTreeMap<String, KindRetain>,
}

/// The effective policy of one record kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Policy {
    pub hot: u32,
    pub ttl: Ttl,
    pub min_age: u32,
    pub keep_rejected: bool,
    pub max: Option<Ttl>,
    pub roll_after: u32,
}

/// The shipped defaults of the record kinds (33, record kind table).
pub fn default_policy(kind: &str) -> Policy {
    let p = |hot, ttl| Policy {
        hot,
        ttl,
        min_age: 90,
        keep_rejected: true,
        max: None,
        roll_after: 8,
    };
    match kind {
        "adr" | "grv" | "exc" => p(3, Ttl::Never),
        "handoff" | "brainstorm" => p(1, Ttl::Versions(3)),
        _ => p(3, Ttl::Versions(8)),
    }
}

/// The policy of `kind` from the root's `[retain]` over the defaults.
pub fn root_policy(root: &RetainConfig, kind: &str) -> Policy {
    let mut p = default_policy(kind);
    if let Some(m) = root.min_age {
        p.min_age = m;
    }
    if let Some(r) = root.roll_after {
        p.roll_after = r;
    }
    if let Some(k) = root.kinds.get(kind) {
        apply(&mut p, k);
    }
    p
}

fn apply(p: &mut Policy, k: &KindRetain) {
    if let Some(h) = k.hot {
        p.hot = h;
    }
    if let Some(t) = k.ttl {
        p.ttl = t;
    }
    if let Some(m) = k.min_age {
        p.min_age = m;
    }
    if let Some(b) = k.keep_rejected {
        p.keep_rejected = b;
    }
    if k.max.is_some() {
        p.max = k.max;
    }
}

/// R-RET-013: a child's override applies only where it keeps records longer; the keys that
/// would shorten retention are returned (they are inert).
pub fn tighten(p: &mut Policy, child: &RetainConfig, kind: &str) -> Vec<String> {
    let mut inert = Vec::new();
    if let Some(m) = child.min_age {
        if m >= p.min_age {
            p.min_age = m;
        } else {
            inert.push(format!("min-age = {m}d is shorter than {}d", p.min_age));
        }
    }
    if let Some(k) = child.kinds.get(kind) {
        if let Some(h) = k.hot {
            if h >= p.hot {
                p.hot = h;
            } else {
                inert.push(format!("retain.{kind}.hot = {h} is shorter than {}", p.hot));
            }
        }
        if let Some(t) = k.ttl {
            if t.at_least(p.ttl) {
                p.ttl = t;
            } else {
                inert.push(format!(
                    "retain.{kind}.ttl = {} is shorter than {}",
                    t.text(),
                    p.ttl.text()
                ));
            }
        }
        if let Some(m) = k.min_age {
            if m >= p.min_age {
                p.min_age = m;
            } else {
                inert.push(format!(
                    "retain.{kind}.min-age = {m}d is shorter than {}d",
                    p.min_age
                ));
            }
        }
    }
    inert
}

fn days(cx: &mut Ctx<'_>, key: &str, v: Option<String>) -> Option<u32> {
    let v = v?;
    match Ttl::parse(&v) {
        Some(Ttl::Days(d)) => Some(d),
        _ => {
            cx.error(
                key,
                format!("`{key}` must be a calendar span like \"90d\" or \"2y\""),
            );
            None
        }
    }
}

pub(crate) fn parse_retain(cx: &mut Ctx<'_>, root: &mut toml::Table) -> RetainConfig {
    let mut out = RetainConfig::default();
    let Some(mut t) = take_table(cx, root, "", "retain") else {
        return out;
    };
    out.declared = true;
    let min_age = take_string(cx, &mut t, "retain", "min-age");
    out.min_age = days(cx, "retain.min-age", min_age);
    out.roll_after = take_u32(cx, &mut t, "retain", "roll-after");
    let kinds: Vec<String> = t
        .iter()
        .filter(|(_, v)| v.is_table())
        .map(|(k, _)| k.clone())
        .collect();
    for kind in kinds {
        let key = format!("retain.{kind}");
        let Some(mut k) = take_table(cx, &mut t, "retain", &kind) else {
            continue;
        };
        let mut kr = KindRetain {
            hot: take_u32(cx, &mut k, &key, "hot"),
            ..KindRetain::default()
        };
        if let Some(v) = take_string(cx, &mut k, &key, "ttl") {
            match Ttl::parse(&v) {
                Some(x) => kr.ttl = Some(x),
                None => cx.error(
                    &format!("{key}.ttl"),
                    "ttl is \"<n>v\", \"<n>d\", \"<n>y\", \"life\" or \"never\" (R-RET-010)",
                ),
            }
        }
        let m = take_string(cx, &mut k, &key, "min-age");
        kr.min_age = days(cx, &format!("{key}.min-age"), m);
        kr.keep_rejected = match k.remove("keep-rejected") {
            Some(toml::Value::Boolean(b)) => Some(b),
            Some(_) => {
                cx.error(
                    &format!("{key}.keep-rejected"),
                    "keep-rejected is true or false",
                );
                None
            }
            None => None,
        };
        kr.bind = take_string(cx, &mut k, &key, "bind");
        if let Some(v) = take_string(cx, &mut k, &key, "max") {
            kr.max = Ttl::parse(&v);
        }
        if kr.ttl == Some(Ttl::Life) && kr.bind.is_none() {
            cx.error(
                &format!("{key}.ttl"),
                "ttl = \"life\" needs bind = \"<id>\" (R-RET-010)",
            );
        }
        cx.unknown_in(&key, &k);
        out.kinds.insert(kind, kr);
    }
    cx.unknown_in("retain", &t);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_ret_010__ttl_forms() {
        assert_eq!(Ttl::parse("8v"), Some(Ttl::Versions(8)));
        assert_eq!(Ttl::parse("90d"), Some(Ttl::Days(90)));
        assert_eq!(Ttl::parse("2y"), Some(Ttl::Days(730)));
        assert_eq!(Ttl::parse("life"), Some(Ttl::Life));
        assert_eq!(Ttl::parse("never"), Some(Ttl::Never));
        assert_eq!(Ttl::parse("8w"), None);
        assert_eq!(default_policy("chg").ttl, Ttl::Versions(8));
        assert_eq!(default_policy("adr").ttl, Ttl::Never);
    }

    #[test]
    fn r_ret_013__a_child_only_lengthens_and_never_is_final() {
        let mut p = default_policy("chg");
        let mut child = RetainConfig::default();
        child.kinds.insert(
            "chg".into(),
            KindRetain {
                ttl: Some(Ttl::Versions(4)),
                hot: Some(5),
                ..KindRetain::default()
            },
        );
        let inert = tighten(&mut p, &child, "chg");
        assert_eq!(p.ttl, Ttl::Versions(8), "a shorter ttl is inert");
        assert_eq!(p.hot, 5, "a longer hot window applies");
        assert_eq!(inert.len(), 1);
        let mut never = default_policy("adr");
        child.kinds.insert(
            "adr".into(),
            KindRetain {
                ttl: Some(Ttl::Versions(100)),
                ..KindRetain::default()
            },
        );
        assert_eq!(tighten(&mut never, &child, "adr").len(), 1);
        assert_eq!(never.ttl, Ttl::Never, "never can never be shortened");
    }
}
