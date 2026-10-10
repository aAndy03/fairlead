//! Cascade structure of a stylesheet (R-FILE-035(c), R-CHK-047): `@layer` order statements,
//! `@layer` blocks, `@import`s (with their `layer(...)`) and where the first rule starts.
//! Textual, comment-aware, never evaluated.
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub struct CssFacts {
    /// `@layer a, b, c;` statements: line and the names in order.
    pub layer_orders: Vec<(u32, Vec<String>)>,
    /// `@layer name { ... }` blocks: line and name.
    pub layer_blocks: Vec<(u32, String)>,
    /// `@import "x" [layer(y)];`: line, target and layer.
    pub imports: Vec<(u32, String, Option<String>)>,
    /// Line of the first rule or at-rule other than `@import`, `@layer` statements and `@charset`.
    pub first_rule: Option<u32>,
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i..].find("*/") {
            Some(j) => {
                // keep the newlines of the comment so line numbers stay true
                out.extend(rest[i..i + j + 2].chars().filter(|c| *c == '\n'));
                rest = &rest[i + j + 2..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn line_no(i: usize) -> u32 {
    u32::try_from(i + 1).unwrap_or(u32::MAX)
}

pub fn scan_css(text: &str) -> CssFacts {
    let clean = strip_comments(text);
    let mut out = CssFacts::default();
    let mut depth = 0i32;
    for (i, raw) in clean.lines().enumerate() {
        let line = raw.trim();
        let n = line_no(i);
        if depth == 0 && !line.is_empty() {
            if let Some(rest) = line.strip_prefix("@layer") {
                let rest = rest.trim();
                if let Some(names) = rest.strip_suffix(';') {
                    out.layer_orders.push((
                        n,
                        names
                            .split(',')
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    ));
                } else {
                    let name = rest
                        .split(['{', ' '])
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_owned();
                    if !name.is_empty() {
                        out.layer_blocks.push((n, name));
                    }
                    out.first_rule.get_or_insert(n);
                }
            } else if let Some(rest) = line.strip_prefix("@import") {
                let target = rest
                    .split(['"', '\''])
                    .nth(1)
                    .map(str::to_owned)
                    .unwrap_or_else(|| rest.trim().trim_end_matches(';').to_owned());
                let layer = rest
                    .split("layer(")
                    .nth(1)
                    .and_then(|r| r.split(')').next())
                    .map(|s| s.trim().to_owned());
                out.imports.push((n, target, layer));
            } else if !line.starts_with("@charset") && !line.starts_with('}') {
                out.first_rule.get_or_insert(n);
            }
        }
        depth += i32::try_from(line.matches('{').count()).unwrap_or(0);
        depth -= i32::try_from(line.matches('}').count()).unwrap_or(0);
        depth = depth.max(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_047__layer_statements_blocks_and_imports() {
        let css = "/* header */\n@import \"tailwindcss\" layer(base);\n@layer theme, base, components, utilities;\n@import \"./x.css\" layer(utilities);\n@layer components {\n  .btn { color: red; }\n}\n.card { padding: 1px; }\n";
        let f = scan_css(css);
        assert_eq!(
            f.layer_orders,
            [(
                3,
                vec![
                    "theme".into(),
                    "base".into(),
                    "components".into(),
                    "utilities".into()
                ]
            )]
        );
        assert_eq!(f.layer_blocks, [(5, "components".to_owned())]);
        assert_eq!(f.imports.len(), 2);
        assert_eq!(
            f.imports[1],
            (4, "./x.css".into(), Some("utilities".into()))
        );
        assert_eq!(f.first_rule, Some(5));
    }
}
