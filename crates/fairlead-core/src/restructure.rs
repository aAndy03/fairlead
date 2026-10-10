//! `move` (R-CLI-040, R-CLI-044, R-PORT-004 to R-PORT-006): the pure parts. Fairlead rewrites
//! its own text only (quoted paths and globs in control files and docs); code that mentions a moved
//! path is listed as a `code-site`, never edited.
use crate::path::RepoPath;

/// The new path of `p` when `src` moves to `dst` (`src` itself, or anything under it).
pub fn moved_path(p: &str, src: &str, dst: &str) -> Option<String> {
    if p == src {
        return Some(dst.to_owned());
    }
    p.strip_prefix(src)
        .and_then(|r| r.strip_prefix('/'))
        .map(|r| format!("{dst}/{r}"))
}

/// `p` relative to `dir` ("" is the root).
pub fn rel_to(dir: &str, p: &str) -> Option<String> {
    if dir.is_empty() {
        Some(p.to_owned())
    } else {
        p.strip_prefix(dir)
            .and_then(|r| r.strip_prefix('/'))
            .map(str::to_owned)
    }
}

/// Rewrites quoted paths and globs that start with `old` (followed by `/`, `*`, a quote or the
/// end) to start with `new`, in TOML or frontmatter text: `"old/x"`, `'old/**'`.
pub fn rewrite_quoted(text: &str, old: &str, new: &str) -> String {
    if old.is_empty() || old == new {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < text.len() {
        let c = bytes.get(i).copied().unwrap_or(0);
        if (c == b'"' || c == b'\'') && text.get(i + 1..).is_some_and(|r| r.starts_with(old)) {
            let after = text.get(i + 1 + old.len()..).and_then(|r| r.chars().next());
            if matches!(after, Some('/' | '*' | '"' | '\'')) {
                out.push(c as char);
                out.push_str(new);
                i += 1 + old.len();
                continue;
            }
        }
        let ch = text
            .get(i..)
            .and_then(|r| r.chars().next())
            .unwrap_or('\u{0}');
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    out
}

/// Lines of code that mention a moved path or file stem in an import-like statement
/// (R-CLI-044): `path:line` plus the trimmed line.
pub fn code_sites(path: &RepoPath, text: &str, needles: &[String]) -> Vec<String> {
    const IMPORTS: [&str; 8] = [
        "import", "from", "require(", "mod ", "use ", "@import", "include", "src=",
    ];
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if !IMPORTS.iter().any(|k| line.contains(k)) {
            continue;
        }
        if needles
            .iter()
            .any(|n| !n.is_empty() && line.contains(n.as_str()))
        {
            out.push(format!(
                "code-site {}:{}: {}",
                path.as_str(),
                i + 1,
                crate::render::sanitize::sanitize_data(line.trim(), 120)
            ));
        }
    }
    out
}

/// What a code line would name a moved path by: the repo path, its last two segments and, for a
/// file, its stem.
pub fn needles(src: &str) -> Vec<String> {
    let mut out = vec![src.to_owned()];
    let segs: Vec<&str> = src.split('/').collect();
    if segs.len() >= 2
        && let (Some(a), Some(b)) = (segs.get(segs.len() - 2), segs.last())
    {
        let stem = b.split('.').next().unwrap_or(b);
        out.push(format!("{a}/{stem}"));
    }
    // A relative import names the last segment: `./ui/Button`, `'../ui'`, `mod ui;`.
    if let Some(last) = segs.last() {
        let stem = last.split('.').next().unwrap_or(last);
        for form in ["/{}/", "/{}'", "/{}\"", "mod {};"] {
            out.push(form.replace("{}", stem));
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_cli_044__quoted_paths_and_globs_follow_the_move() {
        let t = "covers = [\"src/ui/**\", \"src/uix/a.rs\"]\ndirectives = ['def cmp.a applies=src/ui/b.rs']\npath = \"src/ui\"\n";
        let out = rewrite_quoted(t, "src/ui", "web/ui");
        assert_eq!(
            out,
            "covers = [\"web/ui/**\", \"src/uix/a.rs\"]\ndirectives = ['def cmp.a applies=src/ui/b.rs']\npath = \"web/ui\"\n"
        );
        assert_eq!(
            moved_path("src/ui/a.rs", "src/ui", "web/ui").as_deref(),
            Some("web/ui/a.rs")
        );
        assert_eq!(moved_path("src/uix/a.rs", "src/ui", "web/ui"), None);
    }

    #[test]
    fn r_cli_044__import_lines_naming_a_moved_path_are_code_sites() {
        let p = RepoPath::new("src/app.tsx").unwrap();
        let text = "import { Button } from './ui/Button';\nconst ui = 'ui/Button';\n";
        let sites = code_sites(&p, text, &needles("src/ui/Button.tsx"));
        assert_eq!(
            sites,
            ["code-site src/app.tsx:1: import { Button } from './ui/Button';"]
        );
    }
}
