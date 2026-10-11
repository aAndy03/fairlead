//! Import evidence (R-HIER-032, R-HIER-033, R-ARCH-034): a regex pass over TS `import ... from`,
//! `export ... from`, `require(...)` and Rust `use crate::...`. Evidence only: it confirms or
//! contradicts declared `use` edges and never creates a rank or stale verdict on its own. The
//! `ImportExtractor` seam lets a later tree-sitter or `syn` extractor replace the source without
//! rule changes (R-HIER-032).
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// One import as written: the specifier (`./button`, `crate::sync::verify`) and its line.
#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImportSpec {
    pub spec: String,
    pub line: u32,
}

/// The source of import facts; the rules never depend on which one runs.
pub trait ImportExtractor {
    fn imports(&self, text: &str, ext: &str) -> Vec<ImportSpec>;
}

/// The 0.3 extractor: regular expressions, AST-free (constraint 1 of plans/0.3).
pub struct RegexImports;

static TS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"(?m)(?:\bfrom\s*|\brequire\s*\(\s*|^\s*import\s+)['"]([^'"\n]+)['"]"#).ok()
});
static RS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+(crate(?:::\w+)+)").ok());

fn line_of(text: &str, at: usize) -> u32 {
    let n = text.get(..at).map_or(0, |t| t.matches('\n').count());
    u32::try_from(n + 1).unwrap_or(u32::MAX)
}

impl ImportExtractor for RegexImports {
    fn imports(&self, text: &str, ext: &str) -> Vec<ImportSpec> {
        let re = match ext {
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "vue" | "svelte" => &*TS,
            "rs" => &*RS,
            _ => return Vec::new(),
        };
        let Some(re) = re.as_ref() else {
            return Vec::new();
        };
        let comments = fairlead_syntax::lexer::comment_spans(text, ext);
        let in_comment = |i: usize| {
            let k = comments.partition_point(|r| r.end <= i);
            comments.get(k).is_some_and(|r| r.start <= i)
        };
        let mut out: Vec<ImportSpec> = re
            .captures_iter(text)
            .filter_map(|c| {
                let m = c.get(1)?;
                (!in_comment(m.start())).then(|| ImportSpec {
                    spec: m.as_str().to_owned(),
                    line: line_of(text, m.start()),
                })
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

/// The files an import may name, most specific first: a relative TS specifier against the
/// importing file's directory (with the usual extensions and `index` files), or a Rust
/// `crate::a::b` path against the crate's `src/` (`a/b.rs`, `a/b/mod.rs`, then `a.rs`, `a/mod.rs`).
/// Package specifiers (`react`, `@scope/x`) name no file of the tree.
pub fn candidates(from: &str, spec: &str) -> Vec<String> {
    let dir = from.rsplit_once('/').map_or("", |(d, _)| d);
    if spec.starts_with("crate::") {
        let Some(i) = from
            .find("src/")
            .filter(|i| *i == 0 || from.as_bytes().get(i - 1) == Some(&b'/'))
        else {
            return Vec::new();
        };
        let Some(src) = from.get(..i + 3) else {
            return Vec::new();
        };
        let segs: Vec<&str> = spec.trim_start_matches("crate::").split("::").collect();
        let mut out = Vec::new();
        for n in (1..=segs.len()).rev() {
            let p = segs.get(..n).map(|s| s.join("/")).unwrap_or_default();
            out.push(format!("{src}/{p}.rs"));
            out.push(format!("{src}/{p}/mod.rs"));
        }
        return out;
    }
    if !(spec.starts_with("./") || spec.starts_with("../")) {
        return Vec::new();
    }
    let mut parts: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for seg in spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Vec::new();
                }
            }
            s => parts.push(s),
        }
    }
    let base = parts.join("/");
    let mut out = vec![base.clone()];
    for ext in ["ts", "tsx", "js", "jsx", "mjs", "vue", "svelte"] {
        out.push(format!("{base}.{ext}"));
    }
    for ext in ["ts", "tsx", "js", "jsx"] {
        out.push(format!("{base}/index.{ext}"));
    }
    out
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_hier_033__ts_and_rust_imports_are_read_outside_comments() {
        let ts = "import { A } from './button';\nexport * from \"../shared/x\";\nconst y = require('./y');\n// import { Z } from './z';\nimport 'polyfill';\n";
        let specs: Vec<(String, u32)> = RegexImports
            .imports(ts, "ts")
            .into_iter()
            .map(|i| (i.spec, i.line))
            .collect();
        assert_eq!(
            specs,
            [
                ("../shared/x".to_string(), 2),
                ("./button".to_string(), 1),
                ("./y".to_string(), 3),
                ("polyfill".to_string(), 5)
            ]
        );
        let rs = "use crate::sync::verify;\npub(crate) use crate::wire;\n// use crate::gone;\nuse std::fmt;\n";
        let specs: Vec<String> = RegexImports
            .imports(rs, "rs")
            .into_iter()
            .map(|i| i.spec)
            .collect();
        assert_eq!(specs, ["crate::sync::verify", "crate::wire"]);
    }

    #[test]
    fn r_hier_033__specifiers_resolve_to_candidate_files() {
        let c = candidates("app/src/features/a.tsx", "../ui/button");
        assert!(
            c.contains(&"app/src/ui/button.tsx".to_string())
                && c.contains(&"app/src/ui/button/index.ts".to_string())
        );
        assert!(
            candidates("app/src/a.ts", "react").is_empty(),
            "a package names no file"
        );
        let r = candidates("crates/core/src/lib.rs", "crate::sync::verify");
        assert_eq!(
            r.first().map(String::as_str),
            Some("crates/core/src/sync/verify.rs")
        );
        assert!(r.contains(&"crates/core/src/sync/mod.rs".to_string()));
    }
}
