use serde::{Deserialize, Serialize};

/// Who may write a file and what lines it may hold (D-19, R-FILE-001).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum FileClass {
    Authored,
    Generated,
    Reference,
    VendorOwned,
    Ledger,
    ExternalGenerated,
}

use crate::config::{ControlSet, ConventionAction, Layout};
use crate::diag::{Diagnostic, Location, Severity, codes};
use crate::discover::Root;
use crate::path::RepoPath;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;

/// Compiled location and declaration rules (contract 4.4).
#[derive(Clone, Debug)]
pub struct ClassifyRules {
    managerial: BTreeSet<RepoPath>,
    layout: Layout,
    vendor: GlobSet,
    external: GlobSet,
    skip: GlobSet,
}

fn add_glob(b: &mut GlobSetBuilder, dir: &RepoPath, pattern: &str) {
    let pat = pattern.trim_start_matches('/');
    let pat = if pat.ends_with('/') {
        format!("{pat}**")
    } else {
        pat.to_string()
    };
    let full = if dir.is_root() {
        pat
    } else {
        format!("{}/{}", dir.as_str(), pat)
    };
    // Patterns were validated at load (`check_glob`); a failure here simply adds nothing.
    if let Ok(g) = GlobBuilder::new(&full).literal_separator(true).build() {
        b.add(g);
    }
}

fn finish(b: GlobSetBuilder) -> GlobSet {
    b.build().unwrap_or_else(|_| GlobSet::empty())
}

impl ClassifyRules {
    pub fn from_controls(_root: &Root, controls: &ControlSet) -> Self {
        let layout = controls
            .root_control()
            .map(|c| c.layout.clone())
            .unwrap_or_default();
        let mut managerial = BTreeSet::new();
        let (mut vendor, mut external, mut skip) = (
            GlobSetBuilder::new(),
            GlobSetBuilder::new(),
            GlobSetBuilder::new(),
        );
        for c in controls.ok_files() {
            if let Some(m) = c.path.parent() {
                managerial.insert(m);
            }
            for v in &c.vendors {
                add_glob(&mut vendor, &c.dir, &v.path);
            }
            for cv in &c.conventions {
                if let ConventionAction::Class(class) = &cv.action {
                    match class {
                        FileClass::VendorOwned => add_glob(&mut vendor, &c.dir, &cv.glob),
                        _ => add_glob(&mut external, &c.dir, &cv.glob),
                    }
                }
            }
            for s in &c.scan.skip {
                add_glob(&mut skip, &c.dir, s);
            }
        }
        Self {
            managerial,
            layout,
            vendor: finish(vendor),
            external: finish(external),
            skip: finish(skip),
        }
    }

    /// The nearest managerial folder containing `path`, if any.
    fn managerial_of(&self, path: &RepoPath) -> Option<RepoPath> {
        let mut cur = path.parent();
        while let Some(d) = cur {
            if self.managerial.contains(&d) {
                return Some(d);
            }
            cur = d.parent();
        }
        None
    }

    pub fn in_managerial_folder(&self, path: &RepoPath) -> bool {
        self.managerial_of(path).is_some()
    }

    /// `[scan] skip` match (scope-relative globs): the file is not scanned for directives.
    pub fn skips(&self, path: &RepoPath) -> bool {
        self.skip.is_match(path.as_str())
    }

    /// `HN-C001` for every path declared both vendor-owned and external-generated (R-FILE-054).
    pub fn conflicts(&self, paths: &[RepoPath]) -> Vec<Diagnostic> {
        paths
            .iter()
            .filter(|p| self.vendor.is_match(p.as_str()) && self.external.is_match(p.as_str()))
            .map(|p| {
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(p.clone()),
                    "file has two writers: declared both vendor-owned and external-generated (R-FILE-054)",
                )
                .with_hint("declare it with exactly one writer")
            })
            .collect()
    }
}

/// Pure. Class by location and declaration, never by content (R-FILE-001).
pub fn classify(path: &RepoPath, rules: &ClassifyRules) -> FileClass {
    if let Some(m) = rules.managerial_of(path) {
        let rel = path
            .as_str()
            .get(m.as_str().len()..)
            .unwrap_or("")
            .trim_start_matches('/');
        let first = rel.split('/').next().unwrap_or("");
        return if first == rules.layout.r#gen || rel == "vendored.lock" {
            FileClass::Generated
        } else if first == rules.layout.ledger {
            FileClass::Ledger
        } else if first == rules.layout.reference {
            FileClass::Reference
        } else {
            FileClass::Authored
        };
    }
    if rules.vendor.is_match(path.as_str()) {
        return FileClass::VendorOwned;
    }
    if rules.external.is_match(path.as_str()) {
        return FileClass::ExternalGenerated;
    }
    FileClass::Authored
}

/// Pure. Reference, vendor-owned and external-generated files are never scanned for directives
/// (R-FILE-004/005/009).
pub fn scans_directives(class: FileClass) -> bool {
    !matches!(
        class,
        FileClass::Reference | FileClass::VendorOwned | FileClass::ExternalGenerated
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ControlSet, parse_control};
    use crate::diag::codes;
    use crate::testutil::{ROOT_CONTROL, test_root};
    use camino::Utf8Path;

    fn set(files: &[(&str, String)]) -> ControlSet {
        let mut s = ControlSet::default();
        for (path, text) in files {
            let p = RepoPath::new(path).unwrap();
            let is_root = p.as_str() == "fairlead/control.toml";
            s.files
                .insert(p.clone(), parse_control(&p, text.as_bytes(), is_root));
        }
        s
    }

    fn rules(files: &[(&str, String)]) -> ClassifyRules {
        ClassifyRules::from_controls(&test_root(Utf8Path::new("/r")), &set(files))
    }

    fn class(r: &ClassifyRules, p: &str) -> FileClass {
        classify(&RepoPath::new(p).unwrap(), r)
    }

    fn root_only() -> ClassifyRules {
        rules(&[("fairlead/control.toml", ROOT_CONTROL.to_string())])
    }

    #[test]
    fn r_file_001__every_file_under_a_managerial_folder_has_exactly_one_class() {
        let r = root_only();
        for (p, want) in [
            ("fairlead/control.toml", FileClass::Authored),
            ("fairlead/approvals.toml", FileClass::Authored),
            ("fairlead/docs/adr-0001.md", FileClass::Authored),
            ("fairlead/contract/tokens.md", FileClass::Authored),
            ("fairlead/gen/reg/tok.reg", FileClass::Generated),
            ("fairlead/gen/baseline.txt", FileClass::Generated),
            ("fairlead/gen/members", FileClass::Generated),
            ("fairlead/vendored.lock", FileClass::Generated),
            ("fairlead/ledger/hist/2026.hl", FileClass::Ledger),
            ("fairlead/ref/syntax.md", FileClass::Reference),
            ("src/main.rs", FileClass::Authored),
            ("README.md", FileClass::Authored),
        ] {
            assert_eq!(class(&r, p), want, "{p}");
        }
    }

    #[test]
    fn r_file_001__nested_scope_folders_follow_the_root_layout() {
        let child = "[fairlead]\nscope = \"web\"\n".to_string();
        let r = rules(&[
            ("fairlead/control.toml", ROOT_CONTROL.to_string()),
            ("web/fairlead/control.toml", child),
        ]);
        assert_eq!(
            class(&r, "web/fairlead/gen/reg/cmp.reg"),
            FileClass::Generated
        );
        assert_eq!(class(&r, "web/fairlead/docs/x.md"), FileClass::Authored);
        assert_eq!(
            class(&r, "web/fairlead/ref/syntax.md"),
            FileClass::Reference
        );
        assert_eq!(
            class(&r, "web/gen/x.ts"),
            FileClass::Authored,
            "`gen` outside a managerial folder is just a folder"
        );
        assert!(r.in_managerial_folder(&RepoPath::new("web/fairlead/docs/x.md").unwrap()));
        assert!(!r.in_managerial_folder(&RepoPath::new("web/src/x.ts").unwrap()));
    }

    #[test]
    fn r_file_020__layout_names_come_from_the_root_control_file() {
        let root = format!(
            "{ROOT_CONTROL}[layout]\ngen = \"generated\"\nref = \"reference\"\nledger = \"history\"\n"
        );
        let r = rules(&[("fairlead/control.toml", root)]);
        assert_eq!(
            class(&r, "fairlead/generated/reg/tok.reg"),
            FileClass::Generated
        );
        assert_eq!(
            class(&r, "fairlead/reference/syntax.md"),
            FileClass::Reference
        );
        assert_eq!(class(&r, "fairlead/history/2026.hl"), FileClass::Ledger);
        assert_eq!(
            class(&r, "fairlead/gen/reg/tok.reg"),
            FileClass::Authored,
            "the default names no longer apply"
        );
    }

    #[test]
    fn r_file_004__reference_vendor_owned_and_external_generated_are_never_scanned() {
        assert!(scans_directives(FileClass::Authored));
        assert!(
            scans_directives(FileClass::Generated),
            "CLASS-LINE (HN-C011) needs generated files scanned"
        );
        assert!(scans_directives(FileClass::Ledger));
        assert!(!scans_directives(FileClass::Reference));
        assert!(!scans_directives(FileClass::VendorOwned));
        assert!(!scans_directives(FileClass::ExternalGenerated));
    }

    #[test]
    fn r_file_005__vendor_declarations_apply_inside_the_declaring_scope_only() {
        let child = "[fairlead]\nscope = \"web\"\n[[vendor]]\npath = \"components.json\"\ntool = \"shadcn\"\n[[vendor]]\npath = \"src/ui/**\"\ntool = \"shadcn\"\n".to_string();
        let r = rules(&[
            ("fairlead/control.toml", ROOT_CONTROL.to_string()),
            ("web/fairlead/control.toml", child),
        ]);
        assert_eq!(class(&r, "web/components.json"), FileClass::VendorOwned);
        assert_eq!(class(&r, "web/src/ui/button.tsx"), FileClass::VendorOwned);
        assert_eq!(class(&r, "components.json"), FileClass::Authored);
        assert_eq!(class(&r, "api/components.json"), FileClass::Authored);
        assert_eq!(class(&r, "web/src/app.tsx"), FileClass::Authored);
    }

    #[test]
    fn r_file_044__vendor_owned_files_are_exempt_from_directive_scanning() {
        let child = "[fairlead]\nscope = \"web\"\n[[vendor]]\npath = \"vendor/\"\ntool = \"x\"\n"
            .to_string();
        let r = rules(&[
            ("fairlead/control.toml", ROOT_CONTROL.to_string()),
            ("web/fairlead/control.toml", child),
        ]);
        let c = class(&r, "web/vendor/deep/lib.js");
        assert_eq!(
            c,
            FileClass::VendorOwned,
            "a trailing slash means the whole directory"
        );
        assert!(!scans_directives(c));
    }

    #[test]
    fn r_scope_018__class_conventions_mark_external_generated_files() {
        let root = format!(
            "{ROOT_CONTROL}[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n"
        );
        let child = "[fairlead]\nscope = \"api\"\n[[convention]]\nglob = \"proto/**\"\nclass = \"external-generated\"\n".to_string();
        let r = rules(&[
            ("fairlead/control.toml", root),
            ("api/fairlead/control.toml", child),
        ]);
        assert_eq!(class(&r, "src/a.gen.ts"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "a.gen.ts"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "api/proto/x.rs"), FileClass::ExternalGenerated);
        assert_eq!(class(&r, "proto/x.rs"), FileClass::Authored);
        assert_eq!(class(&r, "src/a.ts"), FileClass::Authored);
    }

    #[test]
    fn r_file_014__class_follows_the_writer_not_the_content() {
        let r = root_only();
        assert_eq!(
            class(&r, "src/schema.gen.ts"),
            FileClass::Authored,
            "undeclared: fairlead cannot know another tool wrote it"
        );
        assert_eq!(
            class(&r, "fairlead/gen/reg/tok.reg"),
            FileClass::Generated,
            "a file fairlead writes is generated"
        );
    }

    #[test]
    fn r_file_054__a_file_with_two_declared_writers_is_reported() {
        let root = format!(
            "{ROOT_CONTROL}[[vendor]]\npath = \"gen/**\"\ntool = \"x\"\n[[convention]]\nglob = \"gen/**\"\nclass = \"external-generated\"\n"
        );
        let r = rules(&[("fairlead/control.toml", root)]);
        let paths = [
            RepoPath::new("gen/a.ts").unwrap(),
            RepoPath::new("src/b.ts").unwrap(),
        ];
        let d = r.conflicts(&paths);
        assert_eq!(d.len(), 1);
        assert_eq!(
            (d[0].code, d[0].location.path.as_str()),
            (codes::HN_C001, "gen/a.ts")
        );
        assert!(d[0].message.contains("two writers"));
        assert_eq!(
            class(&r, "gen/a.ts"),
            FileClass::VendorOwned,
            "vendor-owned wins until the conflict is fixed"
        );
    }

    #[test]
    fn r_agt_018__skip_globs_are_scope_relative_and_do_not_change_the_class() {
        let root = format!("{ROOT_CONTROL}[scan]\nskip = [\"tests/fixtures/**\"]\n");
        let child = "[fairlead]\nscope = \"web\"\n[scan]\nskip = [\"snapshots/**\"]\n".to_string();
        let r = rules(&[
            ("fairlead/control.toml", root),
            ("web/fairlead/control.toml", child),
        ]);
        let p = |s: &str| RepoPath::new(s).unwrap();
        assert!(r.skips(&p("tests/fixtures/a.rs")));
        assert!(r.skips(&p("web/snapshots/a.snap")));
        assert!(
            !r.skips(&p("snapshots/a.snap")),
            "the child's skip applies below the child only"
        );
        assert!(!r.skips(&p("src/a.rs")));
        assert_eq!(class(&r, "tests/fixtures/a.rs"), FileClass::Authored);
    }

    #[test]
    fn r_file_001__unparseable_control_files_contribute_nothing() {
        let r = rules(&[
            ("fairlead/control.toml", ROOT_CONTROL.to_string()),
            ("web/fairlead/control.toml", "[fairlead\n".to_string()),
        ]);
        assert_eq!(class(&r, "web/fairlead/gen/x.reg"), FileClass::Authored);
    }
}
