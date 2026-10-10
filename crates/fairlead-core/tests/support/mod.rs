#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]
use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use fairlead_core::classify::ClassifyRules;
use fairlead_core::config::{Approval, ApprovalsFile, ControlSet, parse_control};
use fairlead_core::discover::{Edition, Pin, Root, RootSource};
use fairlead_core::extract::{Corpus, ExtractPlan, StyleKinds, extract_file};
use fairlead_core::facts::{Facts, GitFacts};
use fairlead_core::hash::{Fingerprint, hash_raw};
use fairlead_core::manifest::ManifestHints;
use fairlead_core::path::RepoPath;
use fairlead_core::registry::{ItemId, Registry, ResolveInput, ScopeId, resolve};
use fairlead_core::time::AsOf;
use fairlead_core::walk::WalkEntry;
use fairlead_syntax::delimiters::CommentTable;
use fairlead_syntax::extract::ExtractConfig;
use fairlead_syntax::ids::{Id, ScopeSlug};

pub fn sid(s: &str) -> ScopeId {
    ScopeId(ScopeSlug::parse(s).unwrap())
}

/// `iid("web", "cmp.button")` is the item `web:cmp.button`.
pub fn iid(scope: &str, kind_name: &str) -> ItemId {
    let id = Id::parse(kind_name).unwrap();
    ItemId {
        scope: sid(scope),
        kind: id.kind,
        name: id.name,
    }
}

pub fn date(s: &str) -> jiff::civil::Date {
    s.parse().unwrap()
}

pub fn path(s: &str) -> RepoPath {
    RepoPath::new(s).unwrap()
}

/// Root control file: header, owners, two kinds, the example policies of chapter 10.
pub fn root_toml(extra: &str) -> String {
    format!(
        "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n\
         [settings]\nfolder = \"fairlead\"\nowners = [\"@platform\", \"@andy\"]\n\n\
         [kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n\
         [kinds.cmp]\ntitle = \"component\"\nshape = \"either\"\n\n\
         [policies]\n\"rules.*\" = \"sealed\"\n\"tok.*\" = \"needs-exception\"\n\"cmp.*\" = \"open\"\n\n{extra}\n"
    )
}

pub fn child_toml(id: &str, extra: &str) -> String {
    format!("[fairlead]\nscope = \"{id}\"\n\n{extra}\n")
}

/// A `pattern` rule table; `waivable` makes it needs-exception (R-CHK-006).
pub fn rule(name: &str, sev: &str, stage: &str, waivable: bool) -> String {
    format!(
        "[rules.{name}]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"m\"\nsev = \"{sev}\"\n\
         stage = \"{stage}\"\nglob = [\"**/*.rs\"]\nwaivable = {waivable}\nwhy = \"intent\"\n\n"
    )
}

/// `[[ovr]]` table. `extra` holds the field assignments, one `k = v` per line.
/// Verify: if P3 parses field assignments from an `[ovr.set]` sub-table instead of inline keys,
/// change only this helper.
pub fn ovr_table(target: &str, extra: &str) -> String {
    format!("[[ovr]]\ntarget = \"{target}\"\n{extra}\n\n")
}

/// A Markdown doc whose frontmatter carries directive bodies (R-SYN-036).
pub fn doc(directives: &[&str]) -> String {
    let mut s = String::from("+++\ndirectives = [\n");
    for d in directives {
        s.push_str(&format!("  '{d}',\n"));
    }
    s.push_str("]\n+++\n\n# doc\n");
    s
}

pub struct Kit {
    files: BTreeMap<String, String>,
    scopes: Vec<(String, String)>, // (dir, id)
    as_of: String,
    approvals: Vec<Approval>,
    members: bool,
    search_source: bool,
    sparse_absent: Vec<String>,
}

impl Kit {
    /// Listed in the parent's `gen/members` shard but with no control file in the tree.
    pub fn phantom(mut self, dir: &str, id: &str) -> Kit {
        self.scopes.push((dir.to_string(), id.to_string()));
        self
    }
    pub fn new(root_control: &str) -> Kit {
        let mut files = BTreeMap::new();
        files.insert(
            "fairlead/control.toml".to_string(),
            root_control.to_string(),
        );
        Kit {
            files,
            scopes: vec![],
            as_of: "2026-10-09".to_string(),
            approvals: vec![],
            members: true,
            search_source: false,
            sparse_absent: vec![],
        }
    }
    /// A child scope at `dir` with stored id `id` and control text `control`.
    pub fn scope(mut self, dir: &str, id: &str, control: &str) -> Kit {
        self.files
            .insert(format!("{dir}/fairlead/control.toml"), control.to_string());
        self.scopes.push((dir.to_string(), id.to_string()));
        self
    }
    pub fn file(mut self, p: &str, text: &str) -> Kit {
        self.files.insert(p.to_string(), text.to_string());
        self
    }
    #[allow(clippy::wrong_self_convention)] // reason: builder verb, not a conversion
    pub fn as_of(mut self, d: &str) -> Kit {
        self.as_of = d.to_string();
        self
    }
    pub fn no_members(mut self) -> Kit {
        self.members = false;
        self
    }
    pub fn search_source(mut self) -> Kit {
        self.search_source = true;
        self
    }
    pub fn sparse_absent(mut self, dir: &str) -> Kit {
        self.sparse_absent.push(dir.to_string());
        self
    }
    /// Record an approval for an override fingerprint.
    pub fn approve(mut self, fp: Fingerprint, target: &ItemId, renewal: u32) -> Kit {
        self.approvals.push(Approval {
            fp,
            target: target.to_id(),
            scope: target.scope.0.clone(),
            approver: "@andy".to_string(),
            date: date("2026-10-01"),
            renewal,
        });
        self
    }

    fn members_shards(&self) -> BTreeMap<RepoPath, std::sync::Arc<[u8]>> {
        let mut out: BTreeMap<RepoPath, std::sync::Arc<[u8]>> = BTreeMap::new();
        if !self.members {
            return out;
        }
        let dirs: Vec<&str> = self.scopes.iter().map(|(d, _)| d.as_str()).collect();
        let mut by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (dir, id) in &self.scopes {
            let mut parent = String::new();
            let mut cur = dir.as_str();
            while let Some(i) = cur.rfind('/') {
                cur = &cur[..i];
                if dirs.contains(&cur) {
                    parent = cur.to_string();
                    break;
                }
            }
            by_parent
                .entry(parent)
                .or_default()
                .push(format!("scope.{id} dir={dir}"));
        }
        for (parent, mut lines) in by_parent {
            lines.sort();
            let p = if parent.is_empty() {
                "fairlead/gen/members".to_string()
            } else {
                format!("{parent}/fairlead/gen/members")
            };
            let text = format!(
                "# scope x edition=1 fairlead=0.1.0 body=0\n{}\n",
                lines.join("\n")
            );
            out.insert(
                path(&p),
                std::sync::Arc::from(text.into_bytes().into_boxed_slice()),
            );
        }
        out
    }

    pub fn resolve(&self) -> Registry {
        let root = Root {
            dir: Utf8PathBuf::from("/repo"),
            folder: "fairlead".to_string(),
            source: if self.search_source {
                RootSource::Search
            } else {
                RootSource::Flag
            },
            edition: Edition(1),
            pin: Pin {
                version: None,
                edition: Edition(1),
            },
            config_hash: hash_raw(b""),
        };
        let mut controls = ControlSet {
            files: BTreeMap::new(),
        };
        for (p, text) in &self.files {
            if p.ends_with("fairlead/control.toml") {
                let rp = path(p);
                let is_root_dir = p == "fairlead/control.toml";
                controls
                    .files
                    .insert(rp.clone(), parse_control(&rp, text.as_bytes(), is_root_dir));
            }
        }
        let plan = ExtractPlan {
            syntax: ExtractConfig {
                marker: "fairlead".to_string(),
                comments: CommentTable::builtin(),
                header_skip: vec![],
                max_file: 2_000_000,
                max_directives: 500,
            },
            classify: ClassifyRules::from_controls(&root, &controls),
            styles: StyleKinds::default(),
            leak_properties: vec![],
            pattern_rules: vec![],
            env_scan: false,
        };
        let mut corpus = Corpus {
            records: BTreeMap::new(),
        };
        for (p, text) in &self.files {
            if p.ends_with("fairlead/control.toml") {
                continue;
            }
            let entry = WalkEntry {
                path: path(p),
                size: text.len() as u64,
                mtime_ns: 0,
            };
            corpus
                .records
                .insert(path(p), extract_file(&entry, text.as_bytes(), &plan));
        }
        let facts = Facts {
            generated_on_disk: self.members_shards(),
            agent_files: BTreeMap::new(),
            codeowners: None,
            vendor_hashes: BTreeMap::new(),
            vendor_lock: BTreeMap::new(),
            approvals: ApprovalsFile {
                approvals: self.approvals.clone(),
            },
            git: GitFacts {
                available: false,
                changed: None,
                sparse_absent: self.sparse_absent.iter().map(|d| path(d)).collect(),
                unreadable: None,
                coverage: None,
            },
            ledgers: Default::default(),
            doc_body_hashes: Default::default(),
            version_texts: Default::default(),
            pending_journal: false,
            manifest_hints: ManifestHints {
                workspace_members: BTreeSet::new(),
                default_excludes: vec![],
            },
        };
        resolve(&ResolveInput {
            root: &root,
            controls: &controls,
            corpus: &corpus,
            facts: &facts,
            as_of: AsOf::parse(&self.as_of).unwrap(),
        })
    }
}
