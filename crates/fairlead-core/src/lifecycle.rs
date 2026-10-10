//! Environments, maturity and lifecycle (chapter 21): the root vocabulary, item axes and due dates.
use std::collections::{BTreeMap, BTreeSet};

use crate::config::VocabConfig;

/// Theme variant words: values of a variation axis, never envs (R-ENV-037).
pub const THEME_WORDS: [&str; 6] = ["light", "dark", "brand", "theme", "high-contrast", "dim"];

/// The fixed lifecycle states (R-ENV-004).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum State {
    Proposed,
    Active,
    Deprecated,
    Removed,
    Rejected,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Proposed => "proposed",
            State::Active => "active",
            State::Deprecated => "deprecated",
            State::Removed => "removed",
            State::Rejected => "rejected",
        }
    }
    fn parse(w: &str) -> Option<State> {
        Some(match w {
            "proposed" => State::Proposed,
            "active" => State::Active,
            "deprecated" => State::Deprecated,
            "removed" => State::Removed,
            "rejected" => State::Rejected,
            _ => return None,
        })
    }
}

/// The root vocabulary, with aliases resolved (R-ENV-002, R-ENV-003).
#[derive(Clone, Copy, Debug)]
pub struct Vocab<'a> {
    pub cfg: &'a VocabConfig,
}

/// What a word means on the env axis.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EnvWord {
    /// The slot (index in `env.order`, 0 = least exposed).
    Slot(usize),
    /// `all`: every declared env.
    All,
    /// A theme variant word (R-ENV-037).
    Theme,
    Unknown,
}

impl<'a> Vocab<'a> {
    pub fn new(cfg: &'a VocabConfig) -> Self {
        Self { cfg }
    }
    /// True once the root declares an env path; until then the env axis is not checked.
    pub fn has_envs(&self) -> bool {
        !self.cfg.env_order.is_empty()
    }
    pub fn has_maturity(&self) -> bool {
        !self.cfg.maturity.is_empty()
    }
    pub fn env(&self, word: &str) -> EnvWord {
        let canon = self.cfg.env_alias.get(word).map_or(word, String::as_str);
        if let Some(i) = self.cfg.env_order.iter().position(|e| e == canon) {
            EnvWord::Slot(i)
        } else if word == "all" {
            EnvWord::All
        } else if THEME_WORDS.contains(&word) {
            EnvWord::Theme
        } else {
            EnvWord::Unknown
        }
    }
    pub fn env_name(&self, slot: usize) -> &str {
        self.cfg.env_order.get(slot).map_or("?", String::as_str)
    }
    pub fn all_envs(&self) -> BTreeSet<usize> {
        (0..self.cfg.env_order.len()).collect()
    }
    /// The canonical word of an env alias (`qa` -> `staging`), if `word` is an alias.
    pub fn canonical_env(&self, word: &str) -> Option<&str> {
        self.cfg.env_alias.get(word).map(String::as_str)
    }
    pub fn rank(&self, word: &str) -> Option<i64> {
        self.cfg.maturity.get(word).copied()
    }
    /// The lowest declared rank (R-ENV-006).
    pub fn lowest_rank(&self) -> Option<i64> {
        self.cfg.maturity.values().min().copied()
    }
    /// A maturity word for a rank (the first word in key order with that rank).
    pub fn rank_word(&self, rank: i64) -> String {
        self.cfg
            .maturity
            .iter()
            .find(|(_, r)| **r == rank)
            .map_or_else(|| rank.to_string(), |(w, _)| w.clone())
    }
    /// A lifecycle state word or its display alias (R-ENV-004).
    pub fn state(&self, word: &str) -> Option<State> {
        let canon = self
            .cfg
            .lifecycle_alias
            .get(word)
            .map_or(word, String::as_str);
        State::parse(canon)
    }
}

/// The resolved env and maturity axes of one item.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Axes {
    pub envs: BTreeSet<usize>,
    /// Rank per env slot; an env without an entry has no maturity.
    pub mat: BTreeMap<usize, i64>,
}

/// A word that resolved to nothing (H-VOC-UNKNOWN, R-ENV-003, R-ENV-037).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UnknownWord {
    pub key: &'static str,
    pub word: String,
    pub theme: bool,
}

/// `in=` words to slots. Unknown and theme words are returned, never guessed.
pub fn parse_in(v: &Vocab<'_>, words: &[String], bad: &mut Vec<UnknownWord>) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for w in words {
        match v.env(w) {
            EnvWord::Slot(i) => {
                out.insert(i);
            }
            EnvWord::All => out.extend(v.all_envs()),
            EnvWord::Theme => bad.push(UnknownWord {
                key: "in",
                word: w.clone(),
                theme: true,
            }),
            EnvWord::Unknown => bad.push(UnknownWord {
                key: "in",
                word: w.clone(),
                theme: false,
            }),
        }
    }
    out
}

/// `mat=<word>` (every env of the item) or `mat=<env>:<word>,...` (per env, R-ENV-010).
pub fn parse_mat(
    v: &Vocab<'_>,
    words: &[String],
    envs: &BTreeSet<usize>,
    bad: &mut Vec<UnknownWord>,
) -> BTreeMap<usize, i64> {
    let mut out = BTreeMap::new();
    let unknown_mat = |w: &str, bad: &mut Vec<UnknownWord>| {
        bad.push(UnknownWord {
            key: "mat",
            word: w.to_string(),
            theme: false,
        });
    };
    for w in words {
        match w.split_once(':') {
            Some((env, m)) => {
                let slots: Vec<usize> = match v.env(env) {
                    EnvWord::Slot(i) => vec![i],
                    EnvWord::All => envs.iter().copied().collect(),
                    EnvWord::Theme | EnvWord::Unknown => {
                        bad.push(UnknownWord {
                            key: "mat",
                            word: env.to_string(),
                            theme: v.env(env) == EnvWord::Theme,
                        });
                        continue;
                    }
                };
                match v.rank(m) {
                    Some(r) => out.extend(slots.into_iter().map(|s| (s, r))),
                    None => unknown_mat(m, bad),
                }
            }
            None => match v.rank(w) {
                Some(r) => out.extend(envs.iter().map(|s| (*s, r))),
                None => unknown_mat(w, bad),
            },
        }
    }
    out
}

/// Envs ordered by exposure where a later env is MORE mature than an earlier one (R-ENV-011):
/// `(earlier, later)` slot pairs.
pub fn order_violations(axes: &Axes) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let pairs: Vec<(usize, i64)> = axes.mat.iter().map(|(s, r)| (*s, *r)).collect();
    for (i, (ea, ra)) in pairs.iter().enumerate() {
        for (eb, rb) in pairs.iter().skip(i + 1) {
            if rb > ra {
                out.push((*ea, *eb));
            }
        }
    }
    out
}

/// The lowest rank an item has in any env (what a dependent relies on, R-ENV-020).
pub fn floor_rank(axes: &Axes) -> Option<i64> {
    axes.mat.values().min().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> VocabConfig {
        let mut c = VocabConfig {
            env_order: vec!["dev".into(), "staging".into(), "prod".into()],
            ..Default::default()
        };
        c.env_alias.insert("qa".into(), "staging".into());
        for (w, r) in [("alpha", 0), ("beta", 1), ("rc", 2), ("stable", 3)] {
            c.maturity.insert(w.into(), r);
        }
        c.lifecycle_alias
            .insert("sunsetting".into(), "deprecated".into());
        c
    }

    #[test]
    fn r_env_003__alias_valid_unknown_word_reported() {
        let c = cfg();
        let v = Vocab::new(&c);
        let mut bad = Vec::new();
        let envs = parse_in(&v, &["qa".into(), "canary".into()], &mut bad);
        assert_eq!(envs, BTreeSet::from([1]), "qa is staging forever");
        assert_eq!(bad.len(), 1);
        assert_eq!((bad[0].word.as_str(), bad[0].theme), ("canary", false));
        assert_eq!(v.state("sunsetting"), Some(State::Deprecated));
        assert_eq!(v.state("archived"), None);
    }

    #[test]
    fn r_env_037__theme_word_is_not_an_env() {
        let c = cfg();
        let mut bad = Vec::new();
        parse_in(&Vocab::new(&c), &["dark".into()], &mut bad);
        assert!(bad[0].theme);
    }

    #[test]
    fn r_env_010__mat_applies_to_every_env_or_per_env() {
        let c = cfg();
        let v = Vocab::new(&c);
        let mut bad = Vec::new();
        let envs = parse_in(&v, &["all".into()], &mut bad);
        assert_eq!(parse_mat(&v, &["beta".into()], &envs, &mut bad).len(), 3);
        let per = parse_mat(
            &v,
            &["dev:beta".into(), "staging:alpha".into()],
            &envs,
            &mut bad,
        );
        assert_eq!(per, BTreeMap::from([(0, 1), (1, 0)]));
        assert!(bad.is_empty());
    }

    #[test]
    fn r_env_011__later_env_more_mature_is_an_order_violation() {
        let ok = Axes {
            envs: BTreeSet::from([0, 2]),
            mat: BTreeMap::from([(0, 3), (2, 1)]),
        };
        let bad = Axes {
            envs: BTreeSet::from([0, 2]),
            mat: BTreeMap::from([(0, 1), (2, 3)]),
        };
        assert!(order_violations(&ok).is_empty());
        assert_eq!(order_violations(&bad), [(0, 2)]);
    }
}
