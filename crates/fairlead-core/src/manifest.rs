//! Cargo workspace hints (R-ARCH-006, R-SCOPE-016). Read-only; failure means "no hints".
use crate::discover::Root;
use crate::path::RepoPath;
use crate::walk::WalkResult;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::{BTreeMap, BTreeSet};

/// Oracle for eligibility hints and the default excludes `init` proposes; never authoritative.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ManifestHints {
    pub workspace_members: BTreeSet<RepoPath>,
    pub default_excludes: Vec<String>,
    /// R-HIER-027 (0.3): path dependencies between the crates of the tree, crate directory to the
    /// directories it depends on (`dependencies`, `build-dependencies` and their `target.*` forms;
    /// `dev-dependencies` excluded), read from the manifests directly (D-50).
    pub crate_deps: BTreeMap<RepoPath, BTreeSet<RepoPath>>,
}

/// `base/rel` with `.` and `..` resolved; `None` when it leaves the root.
fn join_rel(base: &RepoPath, rel: &str) -> Option<RepoPath> {
    let mut parts: Vec<&str> = if base.is_root() {
        Vec::new()
    } else {
        base.as_str().split('/').collect()
    };
    for seg in rel.trim_end_matches('/').split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    if parts.is_empty() {
        Some(RepoPath::root())
    } else {
        RepoPath::new(&parts.join("/")).ok()
    }
}

/// The dependency tables of a manifest that make a build edge (R-HIER-027).
fn dep_tables(table: &toml::Table) -> Vec<&toml::Table> {
    let mut out: Vec<&toml::Table> = ["dependencies", "build-dependencies"]
        .iter()
        .filter_map(|k| table.get(*k).and_then(toml::Value::as_table))
        .collect();
    if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        for t in targets.values().filter_map(toml::Value::as_table) {
            out.extend(
                ["dependencies", "build-dependencies"]
                    .iter()
                    .filter_map(|k| t.get(*k).and_then(toml::Value::as_table)),
            );
        }
    }
    out
}

/// The crate directories each crate depends on by path, directly or through
/// `workspace = true` and the workspace's `[workspace.dependencies]`.
fn crate_deps(parsed: &BTreeMap<RepoPath, toml::Table>) -> BTreeMap<RepoPath, BTreeSet<RepoPath>> {
    let crates: BTreeSet<&RepoPath> = parsed
        .iter()
        .filter(|(_, t)| t.contains_key("package"))
        .map(|(d, _)| d)
        .collect();
    let workspace_of = |dir: &RepoPath| -> Option<(&RepoPath, &toml::Table)> {
        parsed
            .iter()
            .filter(|(d, t)| t.contains_key("workspace") && (dir == *d || dir.is_within(d)))
            .max_by_key(|(d, _)| d.as_str().len())
    };
    let mut out: BTreeMap<RepoPath, BTreeSet<RepoPath>> = BTreeMap::new();
    for dir in &crates {
        let Some(table) = parsed.get(*dir) else {
            continue;
        };
        for deps in dep_tables(table) {
            for (name, v) in deps {
                let Some(spec) = v.as_table() else { continue };
                let target = if let Some(p) = spec.get("path").and_then(toml::Value::as_str) {
                    join_rel(dir, p)
                } else if spec.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
                    workspace_of(dir).and_then(|(ws, wt)| {
                        let p = wt
                            .get("workspace")?
                            .get("dependencies")?
                            .get(name)?
                            .get("path")?
                            .as_str()?;
                        join_rel(ws, p)
                    })
                } else {
                    None
                };
                if let Some(t) = target.filter(|t| crates.contains(t) && t != *dir) {
                    out.entry((*dir).clone()).or_default().insert(t);
                }
            }
        }
    }
    out
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
    let mut parsed: BTreeMap<RepoPath, toml::Table> = BTreeMap::new();
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
        parsed.insert(base.clone(), table.clone());
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
    hints.crate_deps = crate_deps(&parsed);
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
    fn r_hier_027__path_and_workspace_dependencies_are_crate_edges() {
        let (h, _t, _g) = hints(&[
            (
                "Cargo.toml",
                "[workspace]
members = [\"crates/*\"]
[workspace.dependencies]
core = { path = \"crates/core\" }
",
            ),
            ("crates/core/Cargo.toml", PKG),
            (
                "crates/app/Cargo.toml",
                "[package]
name = \"app\"
[dependencies]
core = { workspace = true }
serde = \"1\"
[dev-dependencies]
kit = { path = \"../kit\" }
",
            ),
            (
                "crates/kit/Cargo.toml",
                "[package]
name = \"kit\"
[target.'cfg(unix)'.dependencies]
core = { path = \"../core\" }
",
            ),
        ]);
        let edges: Vec<(&str, Vec<&str>)> = h
            .crate_deps
            .iter()
            .map(|(a, b)| (a.as_str(), b.iter().map(RepoPath::as_str).collect()))
            .collect();
        assert_eq!(
            edges,
            [
                ("crates/app", vec!["crates/core"]),
                ("crates/kit", vec!["crates/core"])
            ],
            "dev-dependencies are not edges"
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
