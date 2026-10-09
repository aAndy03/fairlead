//! `gen/members`: path-derived membership of one scope, kept apart from the registry shards so
//! folder moves touch only this file (R-FILE-022, R-PORT-001). Sorted bytewise by ID.

use std::collections::BTreeMap;

use crate::render::line::TerseLine;
use crate::render::sanitize::clean_data;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MembersView {
    pub id: String,
    /// Directory of the scope relative to the repo root; empty for the root scope.
    pub dir: String,
    pub parent: Option<String>,
    /// Scope ids from the root down to and including this scope.
    pub chain: Vec<String>,
    /// Direct child scopes: id to directory.
    pub children: BTreeMap<String, String>,
}

fn dir_text(dir: &str) -> String {
    if dir.is_empty() {
        ".".to_string()
    } else {
        clean_data(dir, 240)
    }
}

pub fn members_body(v: &MembersView) -> String {
    let mut rows: Vec<(String, String)> = Vec::new();
    for (child, dir) in &v.children {
        let id = format!("scope.{child}");
        rows.push((
            id.clone(),
            TerseLine::new(&id).with("dir", &dir_text(dir)).render(),
        ));
    }
    let self_id = format!("self.{}", v.id);
    let up = v.parent.clone().unwrap_or_else(|| "-".to_string());
    let line = TerseLine::new(&self_id)
        .with("dir", &dir_text(&v.dir))
        .with("up", &up)
        .with("chain", &v.chain.join(","))
        .render();
    rows.push((self_id, line));
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (_, line) in rows {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ParsedMembers {
    pub id: Option<String>,
    pub dir: Option<String>,
    pub up: Option<String>,
    pub chain: Vec<String>,
    pub children: BTreeMap<String, String>,
}

/// Lenient reader for the file as last written by `sync` (C-35). Header, blank and malformed
/// lines are skipped; a root `dir=.` reads back as `.`, `up=-` as `-`.
pub fn parse_members(text: &str) -> ParsedMembers {
    let mut out = ParsedMembers::default();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Ok(t) = TerseLine::parse(line) else {
            continue;
        };
        if let Some(child) = t.id.strip_prefix("scope.") {
            out.children
                .insert(child.to_string(), t.get("dir").unwrap_or("").to_string());
        } else if let Some(id) = t.id.strip_prefix("self.") {
            out.id = Some(id.to_string());
            out.dir = t.get("dir").map(str::to_string);
            out.up = t.get("up").map(str::to_string);
            out.chain = t
                .get("chain")
                .map(|c| c.split(',').map(str::to_string).collect())
                .unwrap_or_default();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(
        non_snake_case,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    fn web() -> MembersView {
        MembersView {
            id: "web".to_string(),
            dir: "apps/web".to_string(),
            parent: Some("root".to_string()),
            chain: vec!["root".to_string(), "web".to_string()],
            children: BTreeMap::from([
                ("ui".to_string(), "apps/web/ui".to_string()),
                ("api".to_string(), "apps/web/api".to_string()),
            ]),
        }
    }

    #[test]
    fn r_file_022__members_lines_sorted_children_then_self() {
        assert_eq!(
            members_body(&web()),
            "scope.api dir=apps/web/api\nscope.ui dir=apps/web/ui\nself.web dir=apps/web up=root chain=root,web\n"
        );
    }

    #[test]
    fn r_file_022__root_scope_prints_dot_and_dash() {
        let root = MembersView {
            id: "root".to_string(),
            dir: String::new(),
            parent: None,
            chain: vec!["root".to_string()],
            children: BTreeMap::from([("web".to_string(), "apps/web".to_string())]),
        };
        assert_eq!(
            members_body(&root),
            "scope.web dir=apps/web\nself.root dir=. up=- chain=root\n"
        );
    }

    #[test]
    fn r_file_022__a_folder_move_changes_only_the_members_text() {
        let mut moved = web();
        moved.dir = "packages/web".to_string();
        assert_ne!(members_body(&web()), members_body(&moved));
    }

    #[test]
    fn r_file_022__parse_inverts_the_writer_and_skips_the_header() {
        let text = format!(
            "# scope web edition=1 body=aaaaaaaaaaaaaaaaaaaa\n{}",
            members_body(&web())
        );
        let p = parse_members(&text);
        assert_eq!(p.id.as_deref(), Some("web"));
        assert_eq!(p.dir.as_deref(), Some("apps/web"));
        assert_eq!(p.up.as_deref(), Some("root"));
        assert_eq!(p.chain, vec!["root".to_string(), "web".to_string()]);
        assert_eq!(p.children, web().children);
    }
}
