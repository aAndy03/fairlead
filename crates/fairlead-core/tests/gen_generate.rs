#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]
//! generate() over a real Registry built in memory (no disk): S10 of the contract.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fairlead_core::check::Baseline;
use fairlead_core::config::{ApprovalsFile, ControlSet, parse_control};
use fairlead_core::discover::{Edition, Pin, Root, RootSource};
use fairlead_core::extract::Corpus;
use fairlead_core::facts::{Facts, GitFacts};
use fairlead_core::generate::{GenKind, HeaderVerdict, generate, verify_header};
use fairlead_core::hash::ContentHash;
use fairlead_core::manifest::ManifestHints;
use fairlead_core::path::RepoPath;
use fairlead_core::registry::{Registry, ResolveInput, resolve};
use fairlead_core::time::AsOf;

/// Minimal root control file: one pattern rule, which resolves to the item `rule.no-raw-hex`.
const ROOT_CONTROL: &str = r##"
[fairlead]
edition = 1
root = true
scope = "root"

[settings]
folder = "fairlead"

[rules.no-raw-hex]
check = "pattern"
forbid = "#[0-9a-fA-F]{6}"
msg = "use a token"
sev = "error"
stage = "enforce"
waivable = false
why = "colours come from tokens"
"##;

fn empty_facts() -> Facts {
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
        },
        ledgers: Default::default(),
        manifest_hints: ManifestHints {
            workspace_members: BTreeSet::new(),
            default_excludes: Vec::new(),
        },
    }
}

struct World {
    reg: Registry,
    corpus: Corpus,
    facts: Facts,
}

fn world() -> World {
    let root = Root {
        dir: camino::Utf8PathBuf::from("/repo"),
        folder: "fairlead".to_string(),
        source: RootSource::Flag,
        edition: Edition(1),
        pin: Pin {
            version: None,
            edition: Edition(1),
        },
        config_hash: ContentHash([0; 32]),
    };
    let path = RepoPath::new("fairlead/control.toml").unwrap();
    let parsed = parse_control(&path, ROOT_CONTROL.as_bytes(), true);
    assert!(parsed.is_ok(), "fixture control file must parse");
    let mut files = BTreeMap::new();
    files.insert(path, parsed);
    let controls = ControlSet { files };
    let corpus = Corpus {
        records: BTreeMap::new(),
    };
    let facts = empty_facts();
    let reg = resolve(&ResolveInput {
        root: &root,
        controls: &controls,
        corpus: &corpus,
        facts: &facts,
        as_of: AsOf(jiff::civil::date(2026, 10, 9)),
    });
    World { reg, corpus, facts }
}

#[test]
fn r_file_022__root_scope_gets_its_rule_shard_and_members_and_no_empty_shards() {
    let w = world();
    let set = generate(&w.reg, &w.corpus, &w.facts, None, Edition(1));
    let paths: Vec<&str> = set.files.keys().map(|p| p.as_str()).collect();
    assert_eq!(
        paths,
        vec!["fairlead/gen/members", "fairlead/gen/reg/rule.reg"]
    );
    let rule = &set.files[&RepoPath::new("fairlead/gen/reg/rule.reg").unwrap()];
    assert!(matches!(rule.kind, GenKind::Shard(_)));
    assert!(
        rule.body.starts_with("rule.no-raw-hex pol=sealed"),
        "{}",
        rule.body
    );
    assert!(rule.body.contains("sev=error"), "{}", rule.body);
    let members = &set.files[&RepoPath::new("fairlead/gen/members").unwrap()];
    assert_eq!(members.body, "self.root dir=. up=- chain=root\n");
}

#[test]
fn r_file_008__generate_ignores_whatever_is_on_disk() {
    let mut w = world();
    let clean = generate(&w.reg, &w.corpus, &w.facts, None, Edition(1));
    for p in [
        "fairlead/gen/reg/rule.reg",
        "fairlead/gen/members",
        "fairlead/gen/reg/kept.reg",
    ] {
        w.facts.generated_on_disk.insert(
            RepoPath::new(p).unwrap(),
            Arc::from(&b"hand written line\n"[..]),
        );
    }
    let dirty = generate(&w.reg, &w.corpus, &w.facts, None, Edition(1));
    assert_eq!(
        clean, dirty,
        "an authored line in a generated file is wiped, never kept"
    );
}

#[test]
fn r_arch_091__generate_is_deterministic_and_every_file_verifies() {
    let w = world();
    let a = generate(&w.reg, &w.corpus, &w.facts, None, Edition(1));
    let b = generate(&w.reg, &w.corpus, &w.facts, None, Edition(1));
    assert_eq!(a, b);
    for f in a.files.values() {
        let bytes = f.render(Edition(1));
        assert_eq!(verify_header(&bytes), HeaderVerdict::Ok);
        assert!(!bytes.contains(&b'\r') && bytes.ends_with(b"\n"));
    }
}

#[test]
fn r_chk_030__the_baseline_joins_the_set_only_when_given() {
    let w = world();
    assert!(
        !generate(&w.reg, &w.corpus, &w.facts, None, Edition(1))
            .files
            .values()
            .any(|f| f.kind == GenKind::Baseline)
    );
    let with = generate(
        &w.reg,
        &w.corpus,
        &w.facts,
        Some(&Baseline::default()),
        Edition(1),
    );
    let base: Vec<_> = with
        .files
        .values()
        .filter(|f| f.kind == GenKind::Baseline)
        .collect();
    assert_eq!(base.len(), 1);
    assert_eq!(base[0].body, "");
    assert!(
        String::from_utf8(base[0].render(Edition(1)))
            .unwrap()
            .starts_with("# baseline edition=1")
    );
}
