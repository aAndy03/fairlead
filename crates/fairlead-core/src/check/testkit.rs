#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
//! Test-only in-memory repository. The ONLY place touching P3/P4 constructors: if their
//! signatures differ from the contract, fix them here and nowhere else.
use crate::{
    check::baseline::Baseline,
    classify::ClassifyRules,
    config::RuleCheckDecl,
    config::{ApprovalsFile, ControlSet, parse_control},
    discover::{Edition, Pin, Root, RootSource},
    extract::{Corpus, ExtractPlan, StyleKinds, extract_file, literals::PatternProbe},
    facts::{Facts, GitFacts},
    hash::ContentHash,
    manifest::ManifestHints,
    path::RepoPath,
    registry::{Registry, ResolveInput, resolve},
    time::AsOf,
    walk::WalkEntry,
};
use camino::Utf8PathBuf;
use fairlead_syntax::{delimiters::CommentTable, extract::ExtractConfig};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const ROOT_CONTROL: &str = r#"
[fairlead]
edition = 1
root = true
scope = "root"
marker = "fairlead"

[settings]
folder = "fairlead"
fairlead-version = "0.1"
owners = ["@platform"]
"#;
const LEAK_PROPS: [&str; 6] = [
    "color",
    "background-color",
    "border-color",
    "fill",
    "stroke",
    "padding",
];

pub(crate) fn control_path(dir: &str) -> RepoPath {
    let p = if dir.is_empty() {
        "fairlead/control.toml".to_string()
    } else {
        format!("{dir}/fairlead/control.toml")
    };
    RepoPath::new(&p).unwrap()
}
pub(crate) fn empty_facts() -> Facts {
    Facts {
        generated_on_disk: BTreeMap::new(),
        agent_files: BTreeMap::new(),
        codeowners: None,
        vendor_hashes: BTreeMap::new(),
        vendor_lock: BTreeMap::new(),
        approvals: ApprovalsFile {
            approvals: Vec::new(),
        },
        git: GitFacts {
            available: false,
            changed: None,
            sparse_absent: Vec::new(),
            unreadable: None,
            coverage: None,
        },
        ledgers: Default::default(),
        doc_body_hashes: Default::default(),
        version_texts: Default::default(),
        pending_journal: false,
        pending_move: false,
        tombstones: Default::default(),
        authority: Vec::new(),
        manifest_hints: ManifestHints {
            workspace_members: BTreeSet::new(),
            default_excludes: Vec::new(),
        },
    }
}

pub(crate) struct World {
    root_toml: String,
    children: Vec<(String, String)>,
    files: Vec<(String, String)>,
    facts: Facts,
    baseline: Option<Baseline>,
}
impl World {
    pub(crate) fn new() -> World {
        World {
            root_toml: ROOT_CONTROL.to_string(),
            children: Vec::new(),
            files: Vec::new(),
            facts: empty_facts(),
            baseline: None,
        }
    }
    /// Append TOML to the root control file.
    pub(crate) fn root(mut self, extra: &str) -> World {
        self.root_toml.push('\n');
        self.root_toml.push_str(extra);
        self
    }
    /// Child scope at `dir` (e.g. "apps/web"); `toml` must start with its `[fairlead]` header.
    pub(crate) fn scope(mut self, dir: &str, toml: &str) -> World {
        self.children.push((dir.into(), toml.into()));
        self
    }
    pub(crate) fn file(mut self, path: &str, text: &str) -> World {
        self.files.push((path.into(), text.into()));
        self
    }
    pub(crate) fn facts(mut self, f: impl FnOnce(&mut Facts)) -> World {
        f(&mut self.facts);
        self
    }
    pub(crate) fn baseline(mut self, b: Baseline) -> World {
        self.baseline = Some(b);
        self
    }

    pub(crate) fn build(self) -> Built {
        let as_of = AsOf::parse("2026-10-09").unwrap();
        let root = Root {
            dir: Utf8PathBuf::from("/repo"),
            folder: "fairlead".to_string(),
            source: RootSource::Flag,
            edition: Edition(1),
            pin: Pin {
                version: None,
                edition: Edition(1),
            },
            config_hash: ContentHash([0; 32]),
        };
        let mut controls = ControlSet {
            files: BTreeMap::new(),
        };
        let mut add = |dir: &str, toml: &str, is_root: bool| {
            let p = control_path(dir);
            let parsed = parse_control(&p, toml.as_bytes(), is_root);
            controls.files.insert(p, parsed);
        };
        add("", &self.root_toml, true);
        for (dir, toml) in &self.children {
            add(dir, toml, false);
        }
        let mut probes = Vec::new();
        for cf in controls.files.values().filter_map(|r| r.as_ref().ok()) {
            for r in cf.rules.values() {
                if let RuleCheckDecl::Pattern { forbid, .. } = &r.check {
                    let key = format!("{}:{}", cf.fairlead.scope.as_str(), r.name);
                    probes.push(
                        PatternProbe::new(key, &cf.dir, &r.glob, &r.exclude, forbid).unwrap(),
                    );
                }
            }
        }
        let plan = ExtractPlan {
            syntax: ExtractConfig {
                marker: "fairlead".to_string(),
                comments: CommentTable::builtin(),
                header_skip: Vec::new(),
                max_file: 2_000_000,
                max_directives: 500,
            },
            classify: ClassifyRules::from_controls(&root, &controls),
            styles: StyleKinds::default(), // Verify: StyleKinds::default() exists (P3)
            leak_properties: LEAK_PROPS.iter().map(|s| (*s).to_string()).collect(),
            pattern_rules: probes,
            env_scan: controls
                .root_control()
                .is_some_and(|c| c.lifecycle_declared()),
            grave_scan: controls
                .root_control()
                .is_some_and(|c| c.graveyard.declared),
        };
        let mut corpus = Corpus {
            records: BTreeMap::new(),
        };
        for (path, text) in &self.files {
            let rp = RepoPath::new(path).unwrap();
            let entry = WalkEntry {
                path: rp.clone(),
                size: text.len() as u64,
                mtime_ns: 0,
            };
            corpus
                .records
                .insert(rp, extract_file(&entry, text.as_bytes(), &plan));
        }
        crate::extract::attach::apply_attach(&mut corpus, &controls);
        let registry = resolve(&ResolveInput {
            root: &root,
            controls: &controls,
            corpus: &corpus,
            facts: &self.facts,
            as_of,
        });
        Built {
            root,
            controls,
            corpus,
            facts: self.facts,
            registry,
            baseline: self.baseline,
            as_of,
        }
    }
}

#[allow(
    dead_code,
    reason = "root and controls are kept for P6/P7 tests built on this kit"
)]
pub(crate) struct Built {
    pub(crate) root: Root,
    pub(crate) controls: ControlSet,
    pub(crate) corpus: Corpus,
    pub(crate) facts: Facts,
    pub(crate) registry: Registry,
    pub(crate) baseline: Option<Baseline>,
    pub(crate) as_of: AsOf,
}

use crate::{
    check::{CheckCx, CheckOptions, RuleSet, rules::Check},
    diag::Diagnostic,
    generate::GeneratedSet,
    registry::ScopeState,
};
use fairlead_syntax::diag::DiagCode;

impl Built {
    pub(crate) fn options(&self) -> CheckOptions {
        CheckOptions::new(self.as_of)
    }
    /// The generated files `sync` would write are on disk, as after a clean `sync` (R-CHK-023).
    pub(crate) fn synced(mut self) -> Built {
        let edition = crate::check::builtin::edition_of(&self.registry);
        let set = crate::generate::generate(
            &self.registry,
            &self.corpus,
            &self.facts,
            self.baseline.as_ref(),
            edition,
        );
        for (p, f) in set.files {
            let bytes: std::sync::Arc<[u8]> = f.render(edition).into();
            self.facts.generated_on_disk.insert(p, bytes);
        }
        self
    }
    pub(crate) fn run_check(&self, c: &dyn Check) -> Vec<Diagnostic> {
        self.run_check_with(c, &GeneratedSet::default())
    }
    /// One check over every resolved scope, sorted by (path, line, col, code, message).
    pub(crate) fn run_check_with(
        &self,
        c: &dyn Check,
        generated: &GeneratedSet,
    ) -> Vec<Diagnostic> {
        let (opts, rules) = (self.options(), RuleSet::collect(&self.registry));
        let cx = CheckCx {
            registry: &self.registry,
            corpus: &self.corpus,
            facts: &self.facts,
            generated,
            rules: &rules,
            options: &opts,
        };
        let mut out = Vec::new();
        for s in self
            .registry
            .scopes()
            .filter(|s| matches!(s.state, ScopeState::Resolved))
        {
            c.run(&cx, &s.id, &mut out);
        }
        out.sort_by(|a, b| {
            (
                &a.location.path,
                a.location.line,
                a.location.col,
                a.code,
                &a.message,
            )
                .cmp(&(
                    &b.location.path,
                    b.location.line,
                    b.location.col,
                    b.code,
                    &b.message,
                ))
        });
        out
    }
}
/// A hand-built finding at `path:line` (state and stage as given).
pub(crate) fn finding(
    path: &str,
    line: u32,
    code: DiagCode,
    sev: fairlead_syntax::diag::Severity,
    stage: crate::config::Stage,
    state: crate::check::FindingState,
) -> crate::check::Finding {
    use crate::{check::Finding, diag::Location, hash::Fingerprint, registry::ScopeId};
    Finding {
        code,
        severity: sev,
        stage,
        location: Location {
            path: RepoPath::new(path).unwrap(),
            line,
            col: 1,
        },
        scope: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("root").unwrap()),
        rule: None,
        related: None,
        message: "m".into(),
        excerpt: None,
        intent: None,
        fix: String::new(),
        occurrence: 0,
        fingerprint: Fingerprint::from_digest(&[u8::try_from(line).unwrap(); 32]),
        state,
        waived_by: None,
    }
}

pub(crate) fn of(ds: &[Diagnostic], c: DiagCode) -> Vec<&Diagnostic> {
    ds.iter().filter(|d| d.code == c).collect()
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn r_arch_050__testkit_builds_a_registry_without_a_filesystem() {
        let b = World::new()
            .file("src/a.css", ":root { --x: #fff; }")
            .build();
        assert_eq!(b.registry.scopes().count(), 1);
        assert_eq!(b.corpus.records.len(), 1);
    }
}
