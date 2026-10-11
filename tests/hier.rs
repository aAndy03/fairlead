#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! 0.3 hierarchy (30-hierarchy, plans/0.3 P1) end to end through the binary.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-11";

const HIER: &str =
    "\n[kinds.ctr]\nshape = \"code\"\n\n[hier]\nlevel = 0\ngoverned = [\"src/**\"]\n";

fn repo() -> TempRepo {
    let repo = RepoBuilder::new()
        .file(
            "src/a.rs",
            "// fairlead: def ctr.core path=\"src/a.rs\"\nfn a() {}\n",
        )
        .file("src/b.rs", "fn b() {}\n")
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(&ctl, format!("{text}{HIER}")).unwrap();
    assert_eq!(repo.run(&["sync", "--as-of", AS_OF]).code, 0);
    repo
}

fn check(repo: &TempRepo) -> testkit::Run {
    repo.run(&["check", "--as-of", AS_OF, "--format", "terse"])
}

#[test]
fn r_hier_081__coverage_ratchet_blocks_decrease() {
    let repo = repo();
    let first = check(&repo);
    assert!(
        first.stdout.contains("HN-H003 error src/b.rs")
            && first
                .stdout
                .contains("hier coverage: 1/2 governed files bound (50%)"),
        "{}",
        first.stdout
    );
    assert_eq!(first.code, 1, "an orphan fails the run");
    // The baseline absorbs today's orphans (D-28) ...
    let accept = repo.run(&[
        "sync",
        "--baseline",
        "--accept-new",
        "--why",
        "orphans before adoption",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(accept.code, 0, "{}{}", accept.stdout, accept.stderr);
    let held = check(&repo);
    assert_eq!(held.code, 0, "{}", held.stdout);
    // ... and a newly unbound file is a decrease: a new finding until bound or waived.
    std::fs::write(repo.path().join("src/c.rs"), "fn c() {}\n").unwrap();
    let dropped = check(&repo);
    assert_eq!(dropped.code, 1, "{}", dropped.stdout);
    assert!(
        dropped.stdout.contains("HN-H003 error src/c.rs")
            && dropped.stdout.contains("hier coverage: 1/3"),
        "{}",
        dropped.stdout
    );
}

#[test]
fn r_hier_072__sync_writes_the_suggestions_shard() {
    let repo = RepoBuilder::new()
        .file(
            "src/ctr.rs",
            "// fairlead: def cx.top\n// fairlead: def cx.leaf parent=cx.top\n// fairlead: def cx.b parent=cx.top\n// fairlead: def cx.c parent=cx.top\nfn d() {}\n",
        )
        .file(
            "src/a.rs",
            "// fairlead: def un.shared layer=core.primitive container=cx.leaf sym=shared_helper\nfn shared_helper() {}\n\n// fairlead: def un.b layer=core.engine container=cx.b\n// fairlead: use un.shared\nfn b() {\n    shared_helper();\n}\n\n// fairlead: def un.c layer=core.engine container=cx.c\n// fairlead: use un.shared\nfn c() {\n    shared_helper();\n}\n",
        )
        .git_init(true)
        .build();
    assert_eq!(repo.run(&["init", "--yes", "--as-of", AS_OF]).code, 0);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(
        &ctl,
        format!("{text}\n[layers.core]\nprimitive = 10\nengine = 30\n\n[kinds.cx]\nshape = \"code\"\n\n[kinds.un]\nlayers = [\"core.*\"]\n\n[hier]\nlevel = 2\npromote = {{ fan-in-containers = 2 }}\n"),
    )
    .unwrap();
    let sync = repo.run(&["sync", "--as-of", AS_OF]);
    assert_eq!(sync.code, 0, "{}{}", sync.stdout, sync.stderr);
    let shard = std::fs::read_to_string(repo.path().join("fairlead/gen/hier.suggest")).unwrap();
    assert!(
        shard.contains("promote un.shared to=cx.top -- used from 2 containers"),
        "{shard}"
    );
    let check = check(&repo);
    assert_eq!(
        check.code, 0,
        "suggestions never fail the run: {}",
        check.stdout
    );
    assert!(!check.stdout.contains("HN-G00"), "{}", check.stdout);
}
