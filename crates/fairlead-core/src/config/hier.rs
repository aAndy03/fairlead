//! Hierarchy vocabulary of the root control file (R-HIER-001 to R-HIER-006, R-HIER-080): ladders
//! of ranked layers, free layers, cross-ladder flows, layer policies and the `[hier]` settings.
//! Everything is user data: the binary knows only "ranked layers inside a named ladder".
use std::collections::BTreeMap;

use super::schema::{
    Ctx, join_key, take_string, take_string_list, take_table, take_table_array, take_u32,
};

/// `[hier] same-rank` (R-HIER-020).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum SameRank {
    #[default]
    Allow,
    Container,
    Forbid,
}

/// `[free.<name>]` (R-HIER-005): an unranked cross-cutting layer.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FreeLayer {
    /// The ladder it belongs to, if any.
    pub of: Option<String>,
    /// Layers it may use.
    pub may_use: Vec<String>,
    /// Layers that may use it.
    pub usable_by: Vec<String>,
}

/// `[[flow]]` (R-HIER-002): a sanctioned cross-ladder edge.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Flow {
    pub from: String,
    pub to: Vec<String>,
    pub via: Option<String>,
}

/// `[hier] promote` thresholds (R-HIER-071).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Promote {
    pub fan_in_containers: u32,
    pub single_consumer_seals: u32,
}

impl Default for Promote {
    fn default() -> Self {
        Self {
            fan_in_containers: 3,
            single_consumer_seals: 4,
        }
    }
}

/// `[hier]` (R-HIER-080, R-HIER-051).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct HierSettings {
    /// The adoption level, 0 to 3; `None` when there is no `[hier]` table (hierarchy off).
    pub level: Option<u8>,
    pub same_rank: SameRank,
    pub unbound_ok: Vec<String>,
    pub governed: Vec<String>,
    pub promote: Promote,
    /// Entry types beyond the shipped five (R-HIER-040: "an `etype` from the control file").
    pub etypes: Vec<String>,
    /// R-HIER-043's N: seals a unit's digest must stay unchanged to be dormant (default 4).
    pub dormant_seals: u32,
}

/// The hierarchy tables of one control file (root only, R-HIER-001).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct HierConfig {
    /// `[layers.<ladder>]`: ladder to layer to rank.
    pub ladders: BTreeMap<String, BTreeMap<String, u32>>,
    pub free: BTreeMap<String, FreeLayer>,
    pub flows: Vec<Flow>,
    /// `[layer-policy."<ladder>.<layer>"] used-by`; the only value is `entry` (R-HIER-006).
    pub entry_only: Vec<String>,
    pub settings: HierSettings,
}

impl HierConfig {
    /// The rank of `ladder.layer`, if it is a ranked layer.
    pub fn rank(&self, layer: &str) -> Option<(&str, u32)> {
        let (ladder, name) = layer.split_once('.')?;
        let (l, ranks) = self.ladders.get_key_value(ladder)?;
        ranks.get(name).map(|r| (l.as_str(), *r))
    }

    /// The free layer a name denotes: `name`, or `<of>.name`.
    pub fn free_layer(&self, layer: &str) -> Option<(&str, &FreeLayer)> {
        if let Some((k, f)) = self.free.get_key_value(layer) {
            return Some((k.as_str(), f));
        }
        let (ladder, name) = layer.split_once('.')?;
        self.free
            .get_key_value(name)
            .filter(|(_, f)| f.of.as_deref() == Some(ladder))
            .map(|(k, f)| (k.as_str(), f))
    }

    /// Whether `layer` names a declared ranked or free layer.
    pub fn knows(&self, layer: &str) -> bool {
        self.rank(layer).is_some() || self.free_layer(layer).is_some()
    }

    /// Whether a layer pattern (`ladder.layer`, `ladder.*`, a free layer) matches a layer.
    pub fn matches(&self, pattern: &str, layer: &str) -> bool {
        if pattern == layer {
            return true;
        }
        if let Some(ladder) = pattern.strip_suffix(".*") {
            return layer.split_once('.').is_some_and(|(l, _)| l == ladder);
        }
        match (self.free_layer(pattern), self.free_layer(layer)) {
            (Some((a, _)), Some((b, _))) => a == b,
            _ => false,
        }
    }

    /// Whether a pattern can match anything declared: a known layer, or `<ladder>.*` of a known
    /// ladder.
    pub fn pattern_known(&self, pattern: &str) -> bool {
        match pattern.strip_suffix(".*") {
            Some(ladder) => self.ladders.contains_key(ladder),
            None => self.knows(pattern),
        }
    }

    pub fn is_declared(&self) -> bool {
        !self.ladders.is_empty() || !self.free.is_empty() || self.settings.level.is_some()
    }
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Parses `[layers.*]`, `[free.*]`, `[[flow]]`, `[layer-policy.*]` and `[hier]`, then checks
/// that every layer reference names something declared (R-HIER-003, R-HIER-005).
pub(crate) fn parse_hier(cx: &mut Ctx<'_>, root: &mut toml::Table) -> HierConfig {
    let mut out = HierConfig::default();
    if let Some(ladders) = take_table(cx, root, "", "layers") {
        for (ladder, v) in ladders {
            let key = join_key("layers", &ladder);
            if !valid_slug(&ladder) {
                cx.error(
                    &key,
                    format!("ladder name {ladder:?} must be a lowercase slug"),
                );
                continue;
            }
            let Some(t) = v.as_table() else {
                cx.error(&key, format!("`{key}` must be a table of layer = rank"));
                continue;
            };
            let mut ranks = BTreeMap::new();
            for (layer, r) in t {
                let lk = join_key(&key, layer);
                if !valid_slug(layer) {
                    cx.error(
                        &lk,
                        format!("layer name {layer:?} must be a lowercase slug"),
                    );
                    continue;
                }
                match r.as_integer().and_then(|n| u32::try_from(n).ok()) {
                    Some(n) => {
                        ranks.insert(layer.clone(), n);
                    }
                    None => cx.error(
                        &lk,
                        format!("the rank of `{lk}` must be a non-negative integer"),
                    ),
                }
            }
            out.ladders.insert(ladder, ranks);
        }
    }
    if let Some(free) = take_table(cx, root, "", "free") {
        for (name, v) in free {
            let key = join_key("free", &name);
            if !valid_slug(&name) {
                cx.error(
                    &key,
                    format!("free layer name {name:?} must be a lowercase slug"),
                );
                continue;
            }
            let Some(mut t) = v.as_table().cloned() else {
                cx.error(&key, format!("`{key}` must be a table"));
                continue;
            };
            if t.remove("rank").is_some() {
                cx.error(&key, "a free layer has no rank (R-HIER-005)");
            }
            let f = FreeLayer {
                of: take_string(cx, &mut t, &key, "of"),
                may_use: take_string_list(cx, &mut t, &key, "may-use").unwrap_or_default(),
                usable_by: take_string_list(cx, &mut t, &key, "usable-by").unwrap_or_default(),
            };
            cx.unknown_in(&key, &t);
            out.free.insert(name, f);
        }
    }
    for (i, mut t) in take_table_array(cx, root, "", "flow")
        .into_iter()
        .enumerate()
    {
        let key = "flow";
        let from = take_string(cx, &mut t, key, "from");
        let to = match t.remove("to") {
            Some(toml::Value::String(s)) => vec![s],
            Some(v) => super::schema::string_array(&v).unwrap_or_else(|| {
                cx.error(key, "`flow.to` must be a string or a list of strings");
                Vec::new()
            }),
            None => Vec::new(),
        };
        let via = take_string(cx, &mut t, key, "via");
        cx.unknown_in(key, &t);
        match from {
            Some(from) if !to.is_empty() => out.flows.push(Flow { from, to, via }),
            _ => cx.error(key, format!("[[flow]] {} needs `from` and `to`", i + 1)),
        }
    }
    if let Some(policies) = take_table(cx, root, "", "layer-policy") {
        for (layer, v) in policies {
            let key = join_key("layer-policy", &layer);
            let Some(mut t) = v.as_table().cloned() else {
                cx.error(&key, format!("`{key}` must be a table"));
                continue;
            };
            match take_string(cx, &mut t, &key, "used-by").as_deref() {
                Some("entry") => out.entry_only.push(layer.clone()),
                Some(_) => cx.error(&key, "used-by must be \"entry\" (R-HIER-006)"),
                None => {}
            }
            cx.unknown_in(&key, &t);
        }
    }
    if let Some(mut t) = take_table(cx, root, "", "hier") {
        let s = &mut out.settings;
        s.level = Some(match take_u32(cx, &mut t, "hier", "level") {
            Some(l) if l <= 3 => u8::try_from(l).unwrap_or(0),
            Some(_) => {
                cx.error("hier.level", "level must be 0, 1, 2 or 3 (R-HIER-080)");
                0
            }
            None => 0,
        });
        match take_string(cx, &mut t, "hier", "same-rank").as_deref() {
            None | Some("allow") => {}
            Some("container") => s.same_rank = SameRank::Container,
            Some("forbid") => s.same_rank = SameRank::Forbid,
            Some(_) => cx.error(
                "hier.same-rank",
                "same-rank must be \"allow\", \"container\" or \"forbid\"",
            ),
        }
        s.unbound_ok = take_string_list(cx, &mut t, "hier", "unbound-ok").unwrap_or_default();
        s.governed = take_string_list(cx, &mut t, "hier", "governed").unwrap_or_default();
        s.etypes = take_string_list(cx, &mut t, "hier", "etypes").unwrap_or_default();
        s.dormant_seals = take_u32(cx, &mut t, "hier", "dormant-seals").unwrap_or(4);
        if let Some(mut p) = take_table(cx, &mut t, "hier", "promote") {
            if let Some(n) = take_u32(cx, &mut p, "hier.promote", "fan-in-containers") {
                s.promote.fan_in_containers = n;
            }
            if let Some(n) = take_u32(cx, &mut p, "hier.promote", "single-consumer-seals") {
                s.promote.single_consumer_seals = n;
            }
            cx.unknown_in("hier.promote", &p);
        }
        cx.unknown_in("hier", &t);
    }
    check_refs(cx, &out);
    out
}

/// Every reference to a layer must name a declared one (R-HIER-003, R-HIER-005).
fn check_refs(cx: &mut Ctx<'_>, h: &HierConfig) {
    let mut bad = Vec::new();
    for (name, f) in &h.free {
        if let Some(of) = &f.of
            && !h.ladders.contains_key(of)
        {
            bad.push((join_key("free", name), of.clone()));
        }
        for p in f.may_use.iter().chain(&f.usable_by) {
            if !h.pattern_known(p) {
                bad.push((join_key("free", name), p.clone()));
            }
        }
    }
    for f in &h.flows {
        for p in std::iter::once(&f.from).chain(&f.to) {
            if !h.pattern_known(p) {
                bad.push(("flow".to_owned(), p.clone()));
            }
        }
    }
    for l in &h.entry_only {
        if h.rank(l).is_none() {
            bad.push((join_key("layer-policy", l), l.clone()));
        }
    }
    for (key, layer) in bad {
        cx.error(
            &key,
            format!("`{key}` names {layer:?}, which is not a declared ladder or layer"),
        );
    }
}

/// `[kinds.K] layers` must name declared layers or ladders (R-HIER-001).
pub(crate) fn check_kind_layers(
    cx: &mut Ctx<'_>,
    h: &HierConfig,
    kinds: &BTreeMap<String, super::KindDecl>,
) {
    for (name, k) in kinds {
        for p in &k.layers {
            if !h.pattern_known(p) {
                let key = join_key(&join_key("kinds", name), "layers");
                cx.error(
                    &key,
                    format!("`{key}` names {p:?}, which is not a declared ladder or layer"),
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use crate::config::parse_control;
    use crate::path::RepoPath;

    const HEAD: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n";
    const PRESET: &str = "[layers.ui]\natom = 10\nmolecule = 20\n\n[layers.core]\nengine = 30\nshell = 60\n\n[free.chrome]\nof = \"ui\"\nmay-use = [\"ui.atom\"]\n\n[[flow]]\nfrom = \"ui.*\"\nto = [\"core.engine\"]\nvia = \"bridge\"\n\n[kinds.eng]\nlayers = [\"core.engine\"]\n\n[layer-policy.\"core.shell\"]\nused-by = \"entry\"\n\n[hier]\nlevel = 2\nsame-rank = \"container\"\nunbound-ok = [\"**/tests/**\"]\npromote = { fan-in-containers = 5 }\n";

    fn parse(extra: &str, root: bool) -> Result<crate::config::ControlFile, Vec<String>> {
        let text = if root {
            format!("{HEAD}{extra}")
        } else {
            format!("[fairlead]\nscope = \"web\"\n{extra}")
        };
        let p = if root {
            "fairlead/control.toml"
        } else {
            "web/fairlead/control.toml"
        };
        parse_control(&RepoPath::new(p).unwrap(), text.as_bytes(), root)
            .map_err(|f| f.diags.into_iter().map(|d| d.message).collect())
    }

    #[test]
    fn r_hier_001__ladder_tables_root_only() {
        let c = parse(PRESET, true).unwrap();
        let h = &c.hier;
        assert_eq!(h.rank("ui.molecule"), Some(("ui", 20)));
        assert_eq!(h.rank("core.shell"), Some(("core", 60)));
        assert!(h.free_layer("chrome").is_some() && h.free_layer("ui.chrome").is_some());
        assert_eq!(h.flows[0].to, ["core.engine"]);
        assert_eq!(h.entry_only, ["core.shell"]);
        assert_eq!(c.kinds["eng"].layers, ["core.engine"]);
        assert_eq!(h.settings.level, Some(2));
        assert_eq!(h.settings.same_rank, super::SameRank::Container);
        assert_eq!(
            (
                h.settings.promote.fan_in_containers,
                h.settings.promote.single_consumer_seals
            ),
            (5, 4)
        );
        assert!(c.unknown.is_empty(), "{:?}", c.unknown);
    }

    #[test]
    fn r_hier_004__child_reorder_becomes_escalation() {
        let child = parse("[layers.ui]\natom = 30\n[hier]\nlevel = 3\n", false).unwrap();
        assert!(
            child.hier.ladders.is_empty() && child.hier.settings.level.is_none(),
            "never applied"
        );
        let named: Vec<&str> = child
            .violations
            .iter()
            .filter_map(|d| d.related.as_deref())
            .collect();
        assert_eq!(named, ["layers", "hier"]);
        assert!(
            child
                .violations
                .iter()
                .all(|d| d.hint.contains("escalation"))
        );
    }

    #[test]
    fn r_hier_003__layer_references_name_declared_layers() {
        let e = parse(
            "[layers.ui]\natom = 10\n[kinds.cmp]\nlayers = [\"ui.molecule\"]\n",
            true,
        )
        .unwrap_err();
        assert!(e.iter().any(|m| m.contains("\"ui.molecule\"")), "{e:?}");
        let e = parse(
            "[layers.ui]\natom = 10\n[[flow]]\nfrom = \"web.*\"\nto = \"ui.atom\"\n",
            true,
        )
        .unwrap_err();
        assert!(e.iter().any(|m| m.contains("\"web.*\"")), "{e:?}");
        let e = parse("[layers.ui]\natom = \"ten\"\n", true).unwrap_err();
        assert!(
            e.iter().any(|m| m.contains("non-negative integer")),
            "{e:?}"
        );
    }

    #[test]
    fn r_hier_005__free_layers_have_no_rank() {
        let e = parse("[free.platform]\nrank = 5\n", true).unwrap_err();
        assert!(e.iter().any(|m| m.contains("no rank")), "{e:?}");
    }

    #[test]
    fn r_hier_006__used_by_entry_names_a_ranked_layer() {
        let e = parse(
            "[layers.core]\nshell = 60\n[layer-policy.\"core.shell\"]\nused-by = \"anyone\"\n",
            true,
        )
        .unwrap_err();
        assert!(e.iter().any(|m| m.contains("\"entry\"")), "{e:?}");
    }

    #[test]
    fn r_hier_080__level_is_zero_to_three_and_absent_means_off() {
        assert_eq!(parse("", true).unwrap().hier.settings.level, None);
        assert_eq!(
            parse("[hier]\n", true).unwrap().hier.settings.level,
            Some(0)
        );
        assert!(parse("[hier]\nlevel = 4\n", true).is_err());
    }
}
