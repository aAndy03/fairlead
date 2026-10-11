//! Canonical values for token comparison (R-VAR-093). Pure.
//!
//! Hex (3, 4, 6 and 8 digits), `rgb()`/`rgba()` and `hsl()`/`hsla()` map to one sRGB form,
//! `#rrggbb` or `#rrggbbaa` (an opaque alpha is dropped). `oklch()`, `oklab()`, `lab()`, `lch()`,
//! `hwb()` and `color()` are compared exactly after number normalisation, with no gamut
//! conversion. Lengths are unit-aware: `rem` is `[tokens] root-px` pixels; a unitless number is
//! pixels only where the caller says so (JS style objects). Anything else is not canonical.

/// A canonical value.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// sRGB channels and alpha, 0 to 255.
    Rgb([u8; 4]),
    /// A colour function compared exactly (`oklch(0.62 0.19 259.8)`).
    Exact(String),
    /// A length in pixels.
    Px(f64),
    /// A length in another unit, compared exactly (`1.5em`, `50%`).
    Length(f64, String),
    /// A unitless number where unitless is not a length.
    Number(f64),
}

fn num(n: f64) -> String {
    let r = (n * 10000.0).round() / 10000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

impl Value {
    /// The comparison key: equal keys are equal values.
    pub fn key(&self) -> String {
        match self {
            Value::Rgb([r, g, b, 255]) => format!("#{r:02x}{g:02x}{b:02x}"),
            Value::Rgb([r, g, b, a]) => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
            Value::Exact(s) => s.clone(),
            Value::Px(n) => format!("{}px", num(*n)),
            Value::Length(n, u) => format!("{}{u}", num(*n)),
            Value::Number(n) => num(*n),
        }
    }

    /// R-VAR-096: a value too common to mean a token (`0` of any unit, a bare number).
    pub fn ambiguous(&self) -> bool {
        match self {
            Value::Px(n) | Value::Length(n, _) => *n == 0.0,
            Value::Number(_) => true,
            Value::Rgb(_) | Value::Exact(_) => false,
        }
    }

    /// R-VAR-095: the largest per-channel distance between two sRGB colours; `None` for anything
    /// else (an exact colour function has no near miss).
    pub fn distance(&self, other: &Value) -> Option<u32> {
        match (self, other) {
            (Value::Rgb(a), Value::Rgb(b)) => a
                .iter()
                .zip(b.iter())
                .map(|(x, y)| u32::from(x.abs_diff(*y)))
                .max(),
            _ => None,
        }
    }
}

/// Canonicalise `raw`; `unitless_px` reads a bare number as pixels. `None` when the format is
/// not understood (it is then skipped, never guessed).
pub fn parse(raw: &str, root_px: f64, unitless_px: bool) -> Option<Value> {
    let t = raw.trim().to_ascii_lowercase();
    if let Some(h) = t.strip_prefix('#') {
        return hex(h);
    }
    if let Some((name, inner)) = t.split_once('(') {
        let inner = inner.strip_suffix(')')?;
        return match name.trim() {
            "rgb" | "rgba" => rgb(inner),
            "hsl" | "hsla" => hsl(inner),
            "oklch" | "oklab" | "lab" | "lch" | "hwb" | "color" => exact(name.trim(), inner),
            _ => None,
        };
    }
    length(&t, root_px, unitless_px)
}

fn hex(h: &str) -> Option<Value> {
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let digits: Vec<u8> = h
        .chars()
        .filter_map(|c| c.to_digit(16).and_then(|d| u8::try_from(d).ok()))
        .collect();
    let pair = |a: u8, b: u8| a * 16 + b;
    let c = match digits.as_slice() {
        [r, g, b] => [pair(*r, *r), pair(*g, *g), pair(*b, *b), 255],
        [r, g, b, a] => [pair(*r, *r), pair(*g, *g), pair(*b, *b), pair(*a, *a)],
        [r1, r2, g1, g2, b1, b2] => [pair(*r1, *r2), pair(*g1, *g2), pair(*b1, *b2), 255],
        [r1, r2, g1, g2, b1, b2, a1, a2] => [
            pair(*r1, *r2),
            pair(*g1, *g2),
            pair(*b1, *b2),
            pair(*a1, *a2),
        ],
        _ => return None,
    };
    Some(Value::Rgb(c))
}

/// Arguments split at whitespace, commas and `/`.
fn args(inner: &str) -> Vec<&str> {
    inner
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect()
}

fn byte(x: f64) -> u8 {
    x.round().clamp(0.0, 255.0) as u8
}

fn alpha(a: Option<&&str>) -> Option<u8> {
    let Some(a) = a else { return Some(255) };
    let v = match a.strip_suffix('%') {
        Some(p) => p.parse::<f64>().ok()? / 100.0,
        None => a.parse::<f64>().ok()?,
    };
    Some(byte(v.clamp(0.0, 1.0) * 255.0))
}

fn rgb(inner: &str) -> Option<Value> {
    let p = args(inner);
    if p.len() != 3 && p.len() != 4 {
        return None;
    }
    let mut c = [0u8; 4];
    for (i, x) in p.iter().take(3).enumerate() {
        let v = match x.strip_suffix('%') {
            Some(pc) => pc.parse::<f64>().ok()? * 2.55,
            None => x.parse::<f64>().ok()?,
        };
        *c.get_mut(i)? = byte(v);
    }
    c[3] = alpha(p.get(3))?;
    Some(Value::Rgb(c))
}

fn hsl(inner: &str) -> Option<Value> {
    let p = args(inner);
    if p.len() != 3 && p.len() != 4 {
        return None;
    }
    let h = p
        .first()?
        .trim_end_matches("deg")
        .parse::<f64>()
        .ok()?
        .rem_euclid(360.0);
    let pct =
        |s: &str| -> Option<f64> { Some(s.trim_end_matches('%').parse::<f64>().ok()? / 100.0) };
    let s = pct(p.get(1)?)?.clamp(0.0, 1.0);
    let l = pct(p.get(2)?)?.clamp(0.0, 1.0);
    let k = |n: f64| (n + h / 30.0).rem_euclid(12.0);
    let a = s * l.min(1.0 - l);
    let f = |n: f64| l - a * (k(n) - 3.0).min(9.0 - k(n)).clamp(-1.0, 1.0);
    Some(Value::Rgb([
        byte(f(0.0) * 255.0),
        byte(f(8.0) * 255.0),
        byte(f(4.0) * 255.0),
        alpha(p.get(3))?,
    ]))
}

fn exact(name: &str, inner: &str) -> Option<Value> {
    let (body, a) = match inner.split_once('/') {
        Some((b, a)) => (b, Some(a.trim())),
        None => (inner, None),
    };
    let norm = |s: &str| -> String {
        match s.strip_suffix('%') {
            Some(n) => n
                .parse::<f64>()
                .map_or_else(|_| s.to_owned(), |v| format!("{}%", num(v))),
            None => s.parse::<f64>().map_or_else(|_| s.to_owned(), num),
        }
    };
    let mut parts: Vec<String> = body.split_whitespace().map(norm).collect();
    if parts.is_empty() {
        return None;
    }
    if let Some(a) = a {
        parts.push("/".into());
        parts.push(norm(a));
    }
    Some(Value::Exact(format!("{name}({})", parts.join(" "))))
}

fn length(t: &str, root_px: f64, unitless_px: bool) -> Option<Value> {
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(t.len());
    let (n, unit) = t.split_at(split);
    let n: f64 = n.parse().ok()?;
    Some(match unit {
        "" if unitless_px => Value::Px(n),
        "" => Value::Number(n),
        "px" => Value::Px(n),
        "rem" => Value::Px(n * root_px),
        "em" | "%" | "vw" | "vh" | "vmin" | "vmax" | "ch" | "ex" | "pt" | "cm" | "mm" | "in"
        | "pc" | "dvh" | "svh" | "lvh" | "dvw" => Value::Length(n, unit.to_owned()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn key(s: &str) -> Option<String> {
        parse(s, 16.0, false).map(|v| v.key())
    }

    #[test]
    fn r_var_093__hex_rgb_hsl_have_one_srgb_form() {
        assert_eq!(key("#3B82F6"), Some("#3b82f6".into()));
        assert_eq!(key("#38f"), key("#3388ff"));
        assert_eq!(key("#3388ffff"), key("#38f"), "an opaque alpha is dropped");
        assert_eq!(key("rgb(59 130 246)"), key("#3B82F6"));
        assert_eq!(key("rgba(59, 130, 246, 0.5)"), Some("#3b82f680".into()));
        assert_eq!(key("hsl(0 100% 50%)"), Some("#ff0000".into()));
        assert_eq!(key("hsl(120deg, 100%, 25%)"), Some("#008000".into()));
        assert_eq!(key("hsl(0 0% 100%)"), Some("#ffffff".into()));
    }

    #[test]
    fn r_var_093__oklch_and_color_compare_exactly_after_number_normalisation() {
        assert_eq!(
            key("OKLCH(0.620  0.19 259.80)"),
            Some("oklch(0.62 0.19 259.8)".into())
        );
        assert_eq!(
            key("oklch(62% 0.19 260 / 50%)"),
            Some("oklch(62% 0.19 260 / 50%)".into())
        );
        assert_ne!(
            key("oklch(0.62 0.19 259.8)"),
            key("#3b82f6"),
            "no gamut conversion"
        );
        let a = parse("oklch(0.62 0.19 259.8)", 16.0, false).unwrap();
        let b = parse("oklch(0.62 0.19 259.9)", 16.0, false).unwrap();
        assert_eq!(a.distance(&b), None, "no near miss for an exact form");
    }

    #[test]
    fn r_var_093__rem_equals_px_at_root_px() {
        assert_eq!(key("1rem"), key("16px"));
        assert_eq!(key(".5rem"), Some("8px".into()));
        assert_eq!(
            parse("1rem", 10.0, false).map(|v| v.key()),
            Some("10px".into())
        );
        assert_eq!(
            parse("16", 16.0, true).map(|v| v.key()),
            Some("16px".into())
        );
        assert_eq!(
            key("16"),
            Some("16".into()),
            "unitless is px only in a JS style object"
        );
        assert_ne!(key("1em"), key("16px"), "em depends on context");
        assert_eq!(key("12qq"), None, "unknown units are skipped");
    }

    #[test]
    fn r_var_096__zero_and_bare_numbers_are_ambiguous() {
        for s in ["0", "0px", "0rem", "5", "0%"] {
            assert!(parse(s, 16.0, false).unwrap().ambiguous(), "{s}");
        }
        for s in ["16px", "#000", "1rem"] {
            assert!(!parse(s, 16.0, false).unwrap().ambiguous(), "{s}");
        }
    }

    proptest! {
        #[test]
        fn r_var_093__hex_rgb_hsl_canonicalise_equal(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
            let hexed = format!("#{r:02X}{g:02X}{b:02X}");
            let rgbd = format!("rgb({r}, {g}, {b})");
            prop_assert_eq!(key(&hexed), key(&rgbd));
            // hsl of the same colour lands within one step per channel (rounding of h, s, l)
            let (rf, gf, bf) = (f64::from(r) / 255.0, f64::from(g) / 255.0, f64::from(b) / 255.0);
            let (mx, mn) = (rf.max(gf).max(bf), rf.min(gf).min(bf));
            let l = (mx + mn) / 2.0;
            let d = mx - mn;
            let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
            let h = if d == 0.0 { 0.0 } else if mx == rf { 60.0 * ((gf - bf) / d).rem_euclid(6.0) }
                else if mx == gf { 60.0 * ((bf - rf) / d + 2.0) } else { 60.0 * ((rf - gf) / d + 4.0) };
            let hs = format!("hsl({h} {}% {}%)", s * 100.0, l * 100.0);
            let a = parse(&hexed, 16.0, false).unwrap();
            let c = parse(&hs, 16.0, false).unwrap();
            prop_assert!(a.distance(&c).unwrap() <= 1, "{} vs {}", hexed, hs);
        }
    }
}
