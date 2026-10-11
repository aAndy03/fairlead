//! `[variation]` and `[tokens]` of the root control file (31-variation-protocol: R-VAR-020,
//! R-VAR-032, R-VAR-051, R-VAR-081, R-VAR-084, R-VAR-093, R-VAR-095).
use std::collections::BTreeMap;

use super::schema::{Ctx, take_string, take_string_list, take_table, take_u32};

/// `[variation] fork-detect` (R-VAR-084): off by default; when on, the stage of `var.fork-suspect`.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ForkDetect {
    #[default]
    Off,
    Observe,
    Warn,
    Enforce,
}

/// `[variation]` (R-VAR-020, R-VAR-032, R-VAR-051, R-VAR-081).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VariationConfig {
    /// The root has a `[variation]` table: import specifiers are extracted for
    /// `var.adapter-bypass` (R-VAR-074).
    pub declared: bool,
    /// Layers whose units are varied only through a request (R-VAR-020).
    pub request_layers: Vec<String>,
    /// Seals after which an open request is stale (R-VAR-032).
    pub stale_seals: u32,
    /// More variants than this on one base suggests an axis (R-VAR-051's V).
    pub axis_after: u32,
    pub fork_detect: ForkDetect,
    /// Reserved (R-VAR-081): only `"off"` is accepted.
    pub similarity: String,
}

impl Default for VariationConfig {
    fn default() -> Self {
        Self {
            declared: false,
            request_layers: Vec::new(),
            stale_seals: 4,
            axis_after: 3,
            fork_detect: ForkDetect::Off,
            similarity: "off".into(),
        }
    }
}

/// `[tokens]` (R-VAR-092, R-VAR-093, R-VAR-095).
#[derive(Clone, PartialEq, Debug)]
pub struct TokensConfig {
    /// Pixels per `rem` (default 16).
    pub root_px: f64,
    /// The largest per-channel distance of a near-miss colour (default 2).
    pub near: u32,
    /// Extra Tailwind prefix-to-property rows, `[tokens.tailwind]`.
    pub tailwind: BTreeMap<String, String>,
}

impl Eq for TokensConfig {}

impl Default for TokensConfig {
    fn default() -> Self {
        Self {
            root_px: 16.0,
            near: 2,
            tailwind: BTreeMap::new(),
        }
    }
}

pub(crate) fn parse_variation(cx: &mut Ctx<'_>, root: &mut toml::Table) -> VariationConfig {
    let mut out = VariationConfig::default();
    let Some(mut t) = take_table(cx, root, "", "variation") else {
        return out;
    };
    out.declared = true;
    out.request_layers =
        take_string_list(cx, &mut t, "variation", "request-layers").unwrap_or_default();
    if let Some(n) = take_u32(cx, &mut t, "variation", "stale-seals") {
        out.stale_seals = n;
    }
    if let Some(n) = take_u32(cx, &mut t, "variation", "axis-after") {
        out.axis_after = n;
    }
    match take_string(cx, &mut t, "variation", "fork-detect").as_deref() {
        None | Some("off") => {}
        Some("observe") | Some("on") => out.fork_detect = ForkDetect::Observe,
        Some("warn") => out.fork_detect = ForkDetect::Warn,
        Some("enforce") => out.fork_detect = ForkDetect::Enforce,
        Some(_) => cx.error(
            "variation.fork-detect",
            "fork-detect must be \"off\", \"observe\", \"warn\" or \"enforce\" (R-VAR-084)",
        ),
    }
    match take_string(cx, &mut t, "variation", "similarity").as_deref() {
        None | Some("off") => {}
        Some(_) => cx.error(
            "variation.similarity",
            "similarity is reserved; the only value is \"off\" (R-VAR-081)",
        ),
    }
    cx.unknown_in("variation", &t);
    out
}

pub(crate) fn parse_tokens(cx: &mut Ctx<'_>, root: &mut toml::Table) -> TokensConfig {
    let mut out = TokensConfig::default();
    let Some(mut t) = take_table(cx, root, "", "tokens") else {
        return out;
    };
    match t.remove("root-px") {
        None => {}
        Some(toml::Value::Integer(n)) if n > 0 => out.root_px = n as f64,
        Some(toml::Value::Float(f)) if f > 0.0 => out.root_px = f,
        Some(_) => cx.error("tokens.root-px", "root-px must be a positive number"),
    }
    if let Some(n) = take_u32(cx, &mut t, "tokens", "near") {
        out.near = n;
    }
    if let Some(tw) = take_table(cx, &mut t, "tokens", "tailwind") {
        for (prefix, v) in tw {
            match v.as_str() {
                Some(p) => {
                    out.tailwind.insert(prefix, p.to_ascii_lowercase());
                }
                None => cx.error(
                    "tokens.tailwind",
                    format!("tokens.tailwind.{prefix} must be a CSS property name"),
                ),
            }
        }
    }
    cx.unknown_in("tokens", &t);
    out
}
