#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: integration tests may unwrap
#![allow(non_snake_case)] // reason: test names start with the rule id
//! The S1..S5 pipeline over a real directory, via the public API only (contract Section 5).
use camino::{Utf8Path, Utf8PathBuf};
use fairlead_core::cache::{Cache, CacheKey};
use fairlead_core::config::{config_hash_of, control_diagnostics, load_controls};
use fairlead_core::discover::{DiscoverRequest, discover};
use fairlead_core::extract::{ExtractPlan, StyleKinds, extract_files};
use fairlead_core::facts::collect_facts;
use fairlead_core::walk::{WalkOptions, apply_child_excludes, walk};
use semver::Version;
use std::collections::BTreeMap;

const ROOT: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[settings]\nfairlead-version = \"0.1\"\n[scan]\nexclude = [\"dist/\"]\n[agents]\ntargets = [\"AGENTS.md\"]\n[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\n";
const WEB: &str = "[fairlead]\nscope = \"web\"\n[scan]\nexclude = [\"gen-out/\"]\n";

/// (path, bytes, is_text): text files are converted to CRLF for the CRLF checkout.
fn tree() -> Vec<(&'static str, Vec<u8>, bool)> {
    let t = |p: &'static str, s: &str| (p, s.as_bytes().to_vec(), true);
    vec![
        t("fairlead/control.toml", ROOT),
        t(
            "fairlead/gen/reg/tok.reg",
            "# scope root edition=1\ntok.primary css=--primary\n",
        ),
        t("web/fairlead/control.toml", WEB),
        t(
            "web/src/app.css",
            ":root {\n  --primary: #2563eb;\n}\n.dark {\n  --primary: #60a5fa;\n}\n",
        ),
        t(
            "web/src/App.tsx",
            "// fairlead: def cmp.app sym=App\nexport function App() { return null; }\n",
        ),
        t("web/gen-out/skipped.js", "// fairlead: def cmp.never\n"),
        t("dist/skipped.js", "// fairlead: def cmp.never\n"),
        t(
            "src/lib.rs",
            "// fairlead: use tok.primary\npub fn f() {}\n",
        ),
        t("AGENTS.md", "pointer block\n"),
        (
            "src/legacy.rs",
            b"// fairlead: def cmp.l\n// caf\xE9\n".to_vec(),
            false,
        ),
        ("assets/logo.bin", vec![0u8, 1, 2, 3], false),
    ]
}

fn materialise(dir: &Utf8Path, crlf: bool) {
    for (p, bytes, is_text) in tree() {
        let full = dir.join(p);
        std::fs::create_dir_all(full.parent().unwrap().as_std_path()).unwrap();
        let out = if crlf && is_text {
            String::from_utf8(bytes)
                .unwrap()
                .replace('\n', "\r\n")
                .into_bytes()
        } else {
            bytes
        };
        std::fs::write(full.as_std_path(), out).unwrap();
    }
}

fn tmp() -> (tempfile::TempDir, Utf8PathBuf) {
    let td = tempfile::tempdir().unwrap();
    let p = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
    (td, p)
}

/// Everything that must not depend on cache state, thread count, line endings or cwd.
fn run(dir: &Utf8Path, cwd: &str, threads: usize, use_cache: bool) -> Vec<String> {
    let d = discover(&DiscoverRequest {
        root_flag: None,
        env_root: None,
        cwd: dir.join(cwd),
        binary_version: Version::new(0, 1, 0),
        require_root: true,
    })
    .unwrap();
    let w = walk(
        &d.root,
        &WalkOptions::from_root_control(&d.root_control, Some(threads)),
    )
    .unwrap();
    let set = load_controls(&d.root, &w).unwrap();
    let w = apply_child_excludes(w, &set).unwrap();
    let plan =
        ExtractPlan::from_controls(&d.root, &set, StyleKinds::from_controls(&set), vec![]).unwrap();
    let key = CacheKey {
        fairlead_version: "0.1.0".into(),
        grammar: 1,
        config_hash: config_hash_of(&set),
        as_of: None,
    };
    let cache = Cache::open(&d.root, use_cache, key);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    let corpus = pool
        .install(|| extract_files(&w, &d.root, &plan, &cache, None))
        .unwrap();
    let facts = collect_facts(&d.root, &w, &set, false).unwrap();

    let mut out = Vec::new();
    out.push(format!(
        "root scope={} rules={}",
        d.root_control.fairlead.scope.as_str(),
        set.declared_rule_count()
    ));
    for (path, rec) in &corpus.records {
        let directives = rec.extraction.as_ref().map(|e| {
            e.directives
                .iter()
                .map(|x| {
                    format!(
                        "{:?} {}",
                        x.verb,
                        x.ids
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect::<Vec<_>>()
                .join(";")
        });
        let css: Vec<String> = rec
            .css_tokens
            .iter()
            .map(|t| format!("{}@{}={}", t.name, t.context, t.value))
            .collect();
        out.push(format!(
            "{path} {:?} {} skip={:?} dirs={:?} css={:?}",
            rec.class, rec.hash, rec.skip, directives, css
        ));
    }
    for (path, bytes) in &facts.generated_on_disk {
        out.push(format!(
            "generated {path} {}",
            fairlead_core::hash::hash_normalised(bytes)
        ));
    }
    out.push(format!(
        "agents {:?}",
        facts
            .agent_files
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    ));
    let mut diags: Vec<String> = control_diagnostics(&set)
        .iter()
        .chain(corpus.diagnostics().iter())
        .map(|x| format!("{} {}", x.code.0, x.location.path))
        .collect();
    diags.sort();
    out.extend(diags);
    out
}

fn snapshot(dir: &Utf8Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.as_std_path().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == ".fairlead") {
                    continue;
                }
                stack.push(p);
            } else {
                out.insert(p.to_string_lossy().into_owned(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

#[test]
fn r_arch_071__pipeline_is_identical_for_cold_warm_deleted_cache_and_thread_counts() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let baseline = run(&dir, ".", 1, false);
    assert!(baseline.len() > 10, "{baseline:#?}");
    assert_eq!(baseline, run(&dir, ".", 4, true), "cold cache, 4 threads");
    assert_eq!(baseline, run(&dir, ".", 4, true), "warm cache");
    std::fs::remove_dir_all(dir.join(".fairlead").as_std_path()).unwrap();
    assert_eq!(baseline, run(&dir, ".", 2, true), "deleted cache");
}

#[test]
fn r_arch_111__pipeline_is_identical_for_lf_and_crlf_checkouts() {
    let (_g1, lf) = tmp();
    let (_g2, crlf) = tmp();
    materialise(&lf, false);
    materialise(&crlf, true);
    assert_eq!(run(&lf, ".", 2, false), run(&crlf, ".", 2, false));
}

#[test]
fn r_arch_043__pipeline_is_independent_of_the_invocation_directory() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let from_root = run(&dir, ".", 2, false);
    assert_eq!(from_root, run(&dir, "web/src", 2, false));
    assert_eq!(from_root, run(&dir, "src", 2, false));
}

#[test]
fn r_arch_021__excludes_and_skips_are_applied_by_the_pipeline() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let out = run(&dir, ".", 2, false).join("\n");
    assert!(!out.contains("dist/skipped.js"), "root [scan] exclude");
    assert!(
        !out.contains("web/gen-out/skipped.js"),
        "child [scan] exclude"
    );
    assert!(
        out.contains("web/src/app.css")
            && out.contains("--primary@:root=#2563eb")
            && out.contains("--primary@.dark=#60a5fa")
    );
    assert!(out.contains("src/legacy.rs") && out.contains("NotUtf8"));
    assert!(out.contains("assets/logo.bin") && out.contains("Binary"));
    assert!(out.contains("rules=1"));
}

#[test]
fn r_cli_010__the_pipeline_changes_nothing_outside_dot_fairlead() {
    let (_g, dir) = tmp();
    materialise(&dir, false);
    let before = snapshot(&dir);
    let _ = run(&dir, ".", 2, true);
    let _ = run(&dir, ".", 2, true);
    assert_eq!(
        before,
        snapshot(&dir),
        "readers write only under .fairlead/ (R-ARCH-140)"
    );
    assert!(
        dir.join(".fairlead").exists(),
        "the cache itself was written"
    );
}
