#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)] // reason: tests
use camino::Utf8PathBuf;
use fairlead_core::discover::DiscoverRequest;
use fairlead_core::error::ErrorClass;
use fairlead_core::pipeline::{LoadRequest, WriteSession, load, resolve_loaded};
use fairlead_core::time::AsOf;

const ROOT_CONTROL: &str =
    "[fairlead]\nscope = \"root\"\nedition = 1\nroot = true\nmarker = \"fairlead\"\n\n[settings]\n";

fn temp_root(with_control: bool) -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    if with_control {
        std::fs::create_dir_all(dir.path().join("fairlead")).unwrap();
        std::fs::write(
            dir.path().join("fairlead").join("control.toml"),
            ROOT_CONTROL,
        )
        .unwrap();
    }
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    (dir, root)
}

fn request(root: &Utf8PathBuf) -> LoadRequest {
    LoadRequest {
        discover: DiscoverRequest {
            root_flag: Some(root.clone()),
            env_root: None,
            cwd: root.clone(),
            binary_version: semver::Version::new(0, 1, 0),
            require_root: true,
        },
        use_cache: false,
        threads: Some(1),
        want_changed: false,
        only: None,
    }
}

#[test]
fn r_cli_004__load_without_a_root_is_a_usage_error() {
    let (_dir, root) = temp_root(false);
    let error = load(&request(&root)).err().unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
}

#[test]
fn r_arch_001__load_then_resolve_yields_the_root_scope() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let registry = resolve_loaded(&loaded, AsOf::parse("2026-10-09").unwrap());
    assert_eq!(registry.root_scope().0.as_str(), "root");
}

#[test]
fn r_cli_008__readers_never_take_the_writer_lock() {
    let (_dir, root) = temp_root(true);
    let _loaded = load(&request(&root)).unwrap();
    assert!(!root.join(".fairlead").join("lock").exists());
}

use fairlead_core::check::options::Ramp;
use fairlead_core::check::{CheckOptions, MaxFindings};
use fairlead_core::context::ContextTarget;
use fairlead_core::pipeline::{
    check_context, check_loaded, context_target, explain_target, to_root_relative,
};

fn check_options() -> CheckOptions {
    CheckOptions {
        as_of: AsOf::parse("2026-10-09").unwrap(),
        changed: false,
        files_from: None,
        paths: Vec::new(),
        rule_filter: None,
        max: MaxFindings::Default,
        ramp: Ramp {
            show_observe: false,
            deny_warnings: false,
            nudge_days: [60, 120],
        },
        for_agent: false,
        no_baseline: false,
        require_declarations: false,
        budget: None,
    }
}

#[test]
fn r_chk_040__an_empty_root_is_stale_until_synced_then_checks_clean_with_exit_0() {
    use fairlead_core::outcome::ExitCode;
    use fairlead_core::pipeline::{WriteSession, commit_plan, plan_sync};
    use fairlead_core::write::{BaselineMode, SyncRequest};
    let (_dir, root) = temp_root(true);
    let as_of = AsOf::parse("2026-10-09").unwrap();
    let options = check_options();
    let exit = || {
        let loaded = load(&request(&root)).unwrap();
        let registry = resolve_loaded(&loaded, as_of);
        let report = check_loaded(&loaded, &registry, &options).unwrap();
        assert!(report.zero_declarations);
        report.outcome(&options).exit_code()
    };
    // R-CHK-023: the shards `sync` would write are missing, so the root is stale (exit 3).
    assert_eq!(exit(), ExitCode::StaleGenerated);
    let session = WriteSession::open(&request(&root).discover, &SyncProbe).unwrap();
    let loaded = load(&request(&root)).unwrap();
    let registry = resolve_loaded(&loaded, as_of);
    let sync = SyncRequest {
        baseline: BaselineMode::Off,
        accept_vendor: false,
        force: false,
        scope: None,
    };
    let plan = plan_sync(&loaded, &registry, &options, &sync).unwrap();
    commit_plan(&session, plan).unwrap();
    drop(session);
    assert_eq!(exit(), ExitCode::Ok);
}

/// The test process is the only writer.
struct SyncProbe;
impl fairlead_core::write::PidLiveness for SyncProbe {
    fn is_alive(&self, _pid: u32, _host: &str) -> fairlead_core::write::LivenessAnswer {
        fairlead_core::write::LivenessAnswer::Dead
    }
}

#[test]
fn r_arch_001__a_missing_baseline_file_means_no_baseline() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let registry = resolve_loaded(&loaded, AsOf::parse("2026-10-09").unwrap());
    let context = check_context(&loaded, &registry, &check_options());
    assert!(context.baseline.is_none());
    assert!(context.diags.is_empty());
}

#[test]
fn r_cli_032__a_task_query_is_a_usage_error_until_0_3() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let error = context_target(&loaded.root, &loaded.root.dir, Some("add a progress bar"))
        .err()
        .unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
    assert!(error.to_string().contains("task queries need 0.3"));
}

#[test]
fn r_cli_032__no_argument_is_the_session_start() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let target = context_target(&loaded.root, &loaded.root.dir, None).unwrap();
    assert!(matches!(target, ContextTarget::SessionStart));
}

#[test]
fn r_cli_032__an_existing_path_is_a_path_query() {
    let (_dir, root) = temp_root(true);
    std::fs::create_dir_all(root.join("apps/web")).unwrap();
    std::fs::write(root.join("apps/web/app.ts"), "export {};\n").unwrap();
    let loaded = load(&request(&root)).unwrap();
    match context_target(&loaded.root, &loaded.root.dir, Some("apps/web/app.ts")).unwrap() {
        ContextTarget::Path(path) => assert_eq!(path.as_str(), "apps/web/app.ts"),
        _ => panic!("expected a path query"),
    }
}

#[test]
fn r_cli_032__an_id_shaped_word_is_an_id_lookup() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let target = context_target(&loaded.root, &loaded.root.dir, Some("cmp.button")).unwrap();
    assert!(matches!(target, ContextTarget::IdLookup(_)));
}

#[test]
fn r_cli_032__a_path_above_the_root_is_a_usage_error() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let error = to_root_relative(&loaded.root, &loaded.root.dir, "../../../../outside")
        .err()
        .unwrap();
    assert!(matches!(error.class(), ErrorClass::Usage));
}

#[test]
fn r_cli_035__an_existing_file_beats_an_id_shaped_word_but_not_a_topic() {
    let (_dir, root) = temp_root(true);
    std::fs::write(root.join("cmp.button"), "x\n").unwrap();
    let loaded = load(&request(&root)).unwrap();
    let file = explain_target(&loaded.root, &loaded.root.dir, "cmp.button").unwrap();
    assert!(matches!(
        file,
        fairlead_core::explain::ExplainTarget::File(_)
    ));
    let topic = explain_target(&loaded.root, &loaded.root.dir, "syntax").unwrap();
    assert!(matches!(
        topic,
        fairlead_core::explain::ExplainTarget::Topic(_)
    ));
}

use std::collections::BTreeMap;

use fairlead_core::pipeline::{commit_plan, plan_new};
use fairlead_core::write::{LivenessAnswer, NewRequest, PidLiveness, WriterLock};

struct AlwaysAlive;

impl PidLiveness for AlwaysAlive {
    fn is_alive(&self, _pid: u32, _host: &str) -> LivenessAnswer {
        LivenessAnswer::Alive
    }
}

fn rule_request() -> NewRequest {
    NewRequest {
        kind: "rule".to_string(),
        name: "no-raw-hex".to_string(),
        scope: None,
        fields: BTreeMap::new(),
        print: false,
        at: None,
        hoist: false,
    }
}

#[test]
fn r_arch_001__commit_plan_writes_and_releases_the_lock() {
    let (_dir, root) = temp_root(true);
    let session = WriteSession::open(&request(&root).discover, &AlwaysAlive).unwrap();
    let loaded = load(&request(&root)).unwrap();
    let as_of = AsOf::parse("2026-10-09").unwrap();
    let registry = resolve_loaded(&loaded, as_of);
    let plan = plan_new(&loaded, &registry, &rule_request(), as_of).unwrap();
    let outcome = commit_plan(&session, plan).unwrap();
    drop(session);
    assert!(!outcome.report.written.is_empty());
    assert!(!root.join(".fairlead").join("lock").exists());
}

#[test]
fn r_cli_008__a_held_lock_refuses_the_commit_with_a_refusal() {
    let (_dir, root) = temp_root(true);
    let loaded = load(&request(&root)).unwrap();
    let as_of = AsOf::parse("2026-10-09").unwrap();
    let registry = resolve_loaded(&loaded, as_of);
    let plan = plan_new(&loaded, &registry, &rule_request(), as_of).unwrap();
    let probe = AlwaysAlive;
    let _held = WriterLock::acquire(&loaded.root, &probe).unwrap();
    // A-63: the second writer is refused when it opens its session, before any commit.
    let _ = plan;
    let error = WriteSession::open(&request(&root).discover, &probe)
        .err()
        .unwrap();
    assert!(matches!(error.class(), ErrorClass::Refused));
}
