//! Cargo workspace hints (R-ARCH-006, R-SCOPE-016). Read-only; failure means "no hints".
use crate::discover::Root;
use crate::path::RepoPath;
use crate::walk::WalkResult;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

/// Oracle for eligibility hints and the default excludes `init` proposes; never authoritative.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ManifestHints {
    pub workspace_members: BTreeSet<RepoPath>,
    pub default_excludes: Vec<String>,
}

fn strings(v: Option<&toml::Value>) -> Vec<String> {
    v.and_then(toml::Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn globs(base: &RepoPath, patterns: &[String]) -> GlobSet {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let p = p.trim_start_matches("./").trim_end_matches('/');
        let full = if base.is_root() {
            p.to_string()
        } else {
            format!("{}/{}", base.as_str(), p)
        };
        if let Ok(g) = GlobBuilder::new(&full).literal_separator(true).build() {
            b.add(g);
        }
    }
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

pub fn read_manifest_hints(root: &Root, walk: &WalkResult) -> ManifestHints {
    let manifests: BTreeSet<RepoPath> = walk
        .files
        .iter()
        .filter(|f| f.path.file_name() == "Cargo.toml")
        .map(|f| f.path.clone())
        .collect();
    let manifest_dirs: BTreeSet<RepoPath> = manifests.iter().filter_map(RepoPath::parent).collect();
    let mut hints = ManifestHints::default();
    let mut parsed_any = false;
    for m in &manifests {
        let abs = root.dir.join(m.as_str());
        let Ok(text) = std::fs::read_to_string(abs.as_std_path()) else {
            continue;
        };
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        parsed_any = true;
        let Some(base) = m.parent() else { continue };
        let Some(ws) = table.get("workspace").and_then(toml::Value::as_table) else {
            continue;
        };
        let include = globs(&base, &strings(ws.get("members")));
        let exclude = globs(&base, &strings(ws.get("exclude")));
        for d in &manifest_dirs {
            if *d != base
                && d.is_within(&base)
                && include.is_match(d.as_str())
                && !exclude.is_match(d.as_str())
            {
                hints.workspace_members.insert(d.clone());
            }
        }
    }
    if parsed_any {
        hints.default_excludes.push("target/".to_string());
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_root, utf8_tmp, write_file};
    use crate::walk::{WalkOptions, walk};

    fn hints(files: &[(&str, &str)]) -> (ManifestHints, camino::Utf8PathBuf, tempfile::TempDir) {
        let (g, t) = utf8_tmp();
        for (p, c) in files {
            write_file(&t, p, c);
        }
        let root = test_root(&t);
        let w = walk(&root, &WalkOptions::default()).unwrap();
        (read_manifest_hints(&root, &w), t, g)
    }

    fn members(h: &ManifestHints) -> Vec<&str> {
        h.workspace_members.iter().map(RepoPath::as_str).collect()
    }

    const PKG: &str = "[package]\nname = \"x\"\nversion = \"0.1.0\"\n";

    #[test]
    fn r_arch_006__workspace_members_expand_globs_and_honour_exclude() {
        let (h, _t, _g) = hints(&[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\", \"tools/x\"]\nexclude = [\"crates/skip\"]\n",
            ),
            ("crates/a/Cargo.toml", PKG),
            ("crates/b/Cargo.toml", PKG),
            ("crates/skip/Cargo.toml", PKG),
            ("crates/notacrate/readme.md", "x"),
            ("tools/x/Cargo.toml", PKG),
            ("other/Cargo.toml", PKG),
        ]);
        assert_eq!(members(&h), ["crates/a", "crates/b", "tools/x"]);
        assert_eq!(h.default_excludes, ["target/"]);
    }

    #[test]
    fn r_scope_016__a_nested_workspace_is_relative_to_its_own_directory() {
        let (h, _t, _g) = hints(&[
            (
                "engine/Cargo.toml",
                "[workspace]\nmembers = [\"parts/*\"]\n",
            ),
            ("engine/parts/p1/Cargo.toml", PKG),
            ("parts/p2/Cargo.toml", PKG),
        ]);
        assert_eq!(members(&h), ["engine/parts/p1"]);
    }

    #[test]
    fn r_arch_006__unparseable_manifests_yield_no_hints_and_never_fail() {
        let (h, _t, _g) = hints(&[
            ("Cargo.toml", "[workspace\nmembers = "),
            ("a/Cargo.toml", PKG),
        ]);
        assert!(h.workspace_members.is_empty());
        assert_eq!(
            h.default_excludes,
            ["target/"],
            "the parsable manifest still contributes its hint"
        );
        let (none, _t2, _g2) = hints(&[("Cargo.toml", "= broken")]);
        assert_eq!(none, ManifestHints::default());
    }

    #[test]
    fn r_arch_006__no_manifest_no_hints_and_only_cargo_toml_is_read() {
        let (h, _t, _g) = hints(&[
            ("package.json", "{\"workspaces\": [\"packages/*\"]}"),
            ("pnpm-workspace.yaml", "packages: ['a']"),
            ("packages/a/package.json", "{}"),
        ]);
        assert_eq!(h, ManifestHints::default());
    }

    #[test]
    fn r_scope_016__a_plain_package_adds_only_the_default_exclude() {
        let (h, _t, _g) = hints(&[("Cargo.toml", PKG)]);
        assert!(h.workspace_members.is_empty());
        assert_eq!(h.default_excludes, ["target/"]);
    }

    #[test]
    fn r_scope_016__manifests_are_never_modified() {
        let ws = "[workspace]\nmembers = [\"crates/*\"]\n";
        let (_h, t, _g) = hints(&[("Cargo.toml", ws), ("crates/a/Cargo.toml", PKG)]);
        assert_eq!(
            std::fs::read_to_string(t.join("Cargo.toml").as_std_path()).unwrap(),
            ws
        );
        assert_eq!(
            std::fs::read_to_string(t.join("crates/a/Cargo.toml").as_std_path()).unwrap(),
            PKG
        );
    }

    #[test]
    fn r_scope_016__a_member_without_a_listed_manifest_is_not_a_hint() {
        let (h, _t, _g) = hints(&[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"gone\", \"here\"]\n",
            ),
            ("here/Cargo.toml", PKG),
        ]);
        assert_eq!(members(&h), ["here"]);
    }
}
