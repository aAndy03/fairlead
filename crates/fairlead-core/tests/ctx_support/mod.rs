//! In-memory world for context tests: control-file texts and file texts in, `Registry` out.
//! Verify: every `fairlead_core::...` path is the contract's module name; fix only the `use` lines on first compile.
#![allow(dead_code)]
use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use fairlead_core::cache::{Cache, CacheKey};
use fairlead_core::check::fingerprint::{FingerprintInput, fingerprint};
use fairlead_core::check::{CheckReport, Finding, FindingState, ReportHeader, RuleId, Summary};
use fairlead_core::classify::ClassifyRules;
use fairlead_core::config::{ApprovalsFile, ContextConfig, ControlSet, Stage, parse_control};
use fairlead_core::context::{
    Budget, ContextInput, ContextOutput, ContextRequest, ContextTarget, build,
};
use fairlead_core::diag::Location;
use fairlead_core::discover::{Edition, Pin, Root, RootSource};
use fairlead_core::extract::styles::StyleKinds;
use fairlead_core::extract::{Corpus, ExtractPlan, extract_file};
use fairlead_core::facts::{Facts, GitFacts};
use fairlead_core::hash::ContentHash;
use fairlead_core::manifest::ManifestHints;
use fairlead_core::path::RepoPath;
use fairlead_core::registry::{Registry, ResolveInput, ScopeId, resolve};
use fairlead_core::time::AsOf;
use fairlead_core::walk::WalkEntry;
use fairlead_syntax::delimiters::CommentTable;
use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::extract::ExtractConfig;
use fairlead_syntax::ids::{Id, ScopeSlug};

pub const ROOT: &str = r##"
[fairlead]
edition = 1
root = true
scope = "root"
marker = "fairlead"

[settings]
folder = "fairlead"
fairlead-version = "0.1"
purpose = "Sync app: desktop shell, web UI, core engines."
owners = ["@web"]

[policies.waiver]
approval = false

[kinds.tok]
title = "design token"
shape = "either"

[kinds.cmp]
title = "component"
shape = "either"

[rules.no-raw-hex]
check = "pattern"
forbid = "#[0-9a-fA-F]{6}"
msg = "raw hex colour"
sev = "error"
stage = "enforce"
glob = ["**/*.tsx"]
waivable = true
why = "tokens not hex"

[rules.no-eval]
check = "pattern"
forbid = "eval\\("
msg = "no eval"
sev = "error"
stage = "enforce"
why = "no eval"

[rules.prefer-const]
check = "pattern"
forbid = "\\blet\\b"
msg = "prefer const"
sev = "warn"
stage = "warn"
why = "prefer const"
"##;

pub const WEB: &str = r#"
[fairlead]
scope = "web"

[settings]
purpose = "Web UI: components and tokens."

[[ovr]]
target = "rule.no-raw-hex"
op = "set"
sev = "warn"
why = "legacy charts"
owner = "@web"
exp = "2026-12-31"
"#;

pub const SETTINGS: &str = r#"
[fairlead]
scope = "settings"

[settings]
purpose = "user preferences UI"
"#;

pub const PANEL: &str =
    "// fairlead: def cmp.panel\n// fairlead: use tok.primary cmp.button\nconst x = 1;\n";
pub const TOKENS: &str = "/* fairlead: def tok.primary */\n:root { --primary: #2563eb; }\n";
pub const PREFS: &str = "// fairlead: use cmp.panel tok.primary\nconst y = 2;\n";
pub const HANDOFF6: &str = "+++
directives = ['def handoff.0006 created=2026-10-08 sum=\"Finished the settings form; next: autosave\"']
+++
# Handoff 6
";
pub const HANDOFF7: &str = "+++
directives = ['def handoff.0007 created=2026-10-09 sum=\"Autosave landed; next: review drift\"']
+++
# Handoff 7
";

pub struct World {
    pub root: Root,
    pub corpus: Corpus,
    pub registry: Registry,
    pub cache: Cache,
    pub report: CheckReport,
    pub config: ContextConfig,
}

#[derive(Default)]
pub struct WorldBuilder {
    controls: Vec<(String, String)>,
    files: Vec<(String, String)>,
}

impl WorldBuilder {
    pub fn control(mut self, dir: &str, toml: &str) -> Self {
        self.controls.push((dir.to_string(), toml.to_string()));
        self
    }
    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.files.push((path.to_string(), text.to_string()));
        self
    }
    pub fn build(self) -> World {
        let root = Root {
            dir: Utf8PathBuf::from("/virtual"),
            folder: "fairlead".into(),
            source: RootSource::Flag,
            edition: Edition(1),
            pin: Pin {
                version: None,
                edition: Edition(1),
            },
            config_hash: ContentHash([0; 32]),
        };
        let mut set = ControlSet {
            files: BTreeMap::new(),
        };
        let mut texts: Vec<(RepoPath, String)> = Vec::new();
        for (dir, text) in &self.controls {
            let p = if dir.is_empty() {
                "fairlead/control.toml".to_string()
            } else {
                format!("{dir}/fairlead/control.toml")
            };
            let path = RepoPath::new(&p).unwrap();
            set.files.insert(
                path.clone(),
                parse_control(&path, text.as_bytes(), dir.is_empty()),
            );
            texts.push((path, text.clone()));
        }
        for (p, t) in &self.files {
            texts.push((RepoPath::new(p).unwrap(), t.clone()));
        }
        let plan = ExtractPlan {
            syntax: ExtractConfig {
                marker: "fairlead".into(),
                comments: CommentTable::builtin(),
                header_skip: vec![],
                max_file: 2_000_000,
                max_directives: 500,
            },
            classify: ClassifyRules::from_controls(&root, &set),
            styles: StyleKinds::default(),
            leak_properties: vec![],
            pattern_rules: vec![],
        };
        let mut corpus = Corpus {
            records: BTreeMap::new(),
        };
        for (path, text) in &texts {
            let entry = WalkEntry {
                path: path.clone(),
                size: text.len() as u64,
                mtime_ns: 0,
            };
            corpus
                .records
                .insert(path.clone(), extract_file(&entry, text.as_bytes(), &plan));
        }
        let facts = Facts {
            generated_on_disk: BTreeMap::new(),
            agent_files: BTreeMap::new(),
            codeowners: None,
            vendor_hashes: BTreeMap::new(),
            vendor_lock: BTreeMap::new(),
            approvals: ApprovalsFile { approvals: vec![] },
            git: GitFacts {
                available: false,
                changed: None,
                sparse_absent: vec![],
                unreadable: None,
            },
            ledgers: Default::default(),
            doc_body_hashes: Default::default(),
            version_texts: Default::default(),
            manifest_hints: ManifestHints {
                workspace_members: BTreeSet::new(),
                default_excludes: vec![],
            },
        };
        let as_of = AsOf::parse("2026-10-09").unwrap();
        let registry = resolve(&ResolveInput {
            root: &root,
            controls: &set,
            corpus: &corpus,
            facts: &facts,
            as_of,
        });
        let config = match set
            .files
            .get(&RepoPath::new("fairlead/control.toml").unwrap())
        {
            Some(Ok(c)) => c.context.clone(),
            _ => panic!("the root control file must parse"),
        };
        let cache = Cache::open(
            &root,
            false,
            CacheKey {
                fairlead_version: "0.1.0".into(),
                grammar: 1,
                config_hash: ContentHash([0; 32]),
                as_of: None,
            },
        );
        World {
            root,
            corpus,
            registry,
            cache,
            report: report(vec![], false),
            config,
        }
    }
}

pub fn standard_with(root: &str, web: &str, settings: &str) -> WorldBuilder {
    WorldBuilder::default()
        .control("", root)
        .control("apps/web", web)
        .control("apps/web/src/settings", settings)
        .file("apps/web/src/Panel.tsx", PANEL)
        .file("apps/web/src/tokens.css", TOKENS)
        .file("apps/web/src/settings/Prefs.tsx", PREFS)
        .file("fairlead/docs/handoff/0006.md", HANDOFF6)
        .file("fairlead/docs/handoff/0007.md", HANDOFF7)
        .file("README.md", "# readme\n")
}

pub fn standard() -> WorldBuilder {
    standard_with(ROOT, WEB, SETTINGS)
}

impl World {
    pub fn run(&self, req: &ContextRequest) -> ContextOutput {
        build(&ContextInput {
            registry: &self.registry,
            corpus: &self.corpus,
            report: &self.report,
            config: &self.config,
            request: req,
            cache: &self.cache,
        })
    }
    pub fn text(&self, req: &ContextRequest) -> String {
        self.run(req).brief.render_terse()
    }
}

pub fn request(target: ContextTarget) -> ContextRequest {
    ContextRequest {
        target,
        budget: Budget::Default,
        scope_pin: None,
        full: false,
        depth: None,
        digest_only: false,
        exact_tokens: false,
        as_of: AsOf::parse("2026-10-09").unwrap(),
    }
}

pub fn path_req(p: &str) -> ContextRequest {
    request(ContextTarget::Path(RepoPath::new(p).unwrap()))
}

pub fn session_req() -> ContextRequest {
    request(ContextTarget::SessionStart)
}

pub fn scope_id(s: &str) -> ScopeId {
    ScopeId(ScopeSlug::parse(s).unwrap())
}

pub fn report(findings: Vec<Finding>, partial: bool) -> CheckReport {
    CheckReport {
        header: ReportHeader {
            fairlead_version: "0.1.0".into(),
            edition: Edition(1),
            as_of: AsOf::parse("2026-10-09").unwrap(),
            scopes: 3,
            files: 9,
            rules: 3,
        },
        findings,
        summary: Summary::default(),
        partial,
        unknown_scopes: vec![],
        absent_scopes: vec![],
        zero_declarations: false,
    }
}

pub fn finding(
    code: DiagCode,
    sev: Severity,
    path: &str,
    line: u32,
    state: FindingState,
    rule: Option<&str>,
) -> Finding {
    let scope = scope_id("root");
    let rp = RepoPath::new(path).unwrap();
    let rid = rule.map(|r| RuleId(Id::parse(&format!("rule.{r}")).unwrap().name));
    let fp = fingerprint(&FingerprintInput {
        code,
        rule: rid.as_ref(),
        scope: &scope,
        path: &rp,
        text: "x",
        occurrence: line,
    });
    Finding {
        code,
        severity: sev,
        stage: Stage::Enforce,
        location: Location {
            path: rp,
            line,
            col: 1,
        },
        scope,
        rule: rid,
        related: None,
        message: "m".into(),
        excerpt: None,
        intent: None,
        fix: "f".into(),
        occurrence: line,
        fingerprint: fp,
        state,
        waived_by: None,
    }
}
