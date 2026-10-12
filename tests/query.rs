#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! 0.3 context: task queries and the hierarchy in the brief (14-context, plans/0.3 P6).
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo};

const AS_OF: &str = "2026-10-12";

fn repo(extra: &str) -> TempRepo {
    let repo = RepoBuilder::new()
        .file(
            "fairlead/contract/a.md",
            "+++\ndirectives = ['def cmp.progress-bar sum=\"Shows upload progress\" kw=upload,progress']\n+++\n# A\n",
        )
        .file("web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\n")
        .file(
            "web/fairlead/contract/w.md",
            "+++\ndirectives = ['def cmp.toast sum=\"Notification toast\"']\n+++\n# W\n",
        )
        .file(
            "src/kit.ts",
            "// fairlead: def ctr.kit\nexport const k = 1;\n// fairlead: def un.button layer=ui.atom container=ctr.kit vis=public\nexport function Button() {}\n",
        )
        .git_init(true)
        .build();
    let init = repo.run(&["init", "--yes", "--as-of", AS_OF]);
    assert_eq!(init.code, 0, "{}", init.stderr);
    let ctl = repo.path().join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).unwrap();
    std::fs::write(
        &ctl,
        format!("{text}\n[kinds.cmp]\nshape = \"doc\"\n\n[kinds.ctr]\nshape = \"code\"\n\n[kinds.un]\nshape = \"code\"\n{extra}"),
    )
    .unwrap();
    assert_eq!(repo.run(&["sync", "--yes", "--as-of", AS_OF]).code, 0);
    repo
}

#[test]
fn r_ctx_061__a_task_query_names_the_matched_scopes_and_hits() {
    let repo = repo("");
    let run = repo.run(&[
        "context",
        "upload progress bar",
        "--budget",
        "0",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("matched: root "), "{}", run.stdout);
    assert!(run.stdout.contains("; pin with --scope"), "{}", run.stdout);
    assert!(
        run.stdout.contains("hit item cmp.progress-bar"),
        "{}",
        run.stdout
    );
}

#[test]
fn r_ctx_061__a_pinned_task_query_briefs_the_pinned_scope() {
    let repo = repo("");
    let run = repo.run(&[
        "context",
        "notification toast",
        "--scope",
        "web",
        "--budget",
        "0",
        "--as-of",
        AS_OF,
    ]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.starts_with("@web "), "{}", run.stdout);
    assert!(run.stdout.contains("hit item cmp.toast"), "{}", run.stdout);
}

#[test]
fn r_ctx_015__brief_lists_containers_and_public_surface() {
    let repo = repo("\n[layers.ui]\natom = 10\n\n[hier]\nlevel = 1\n");
    let run = repo.run(&["context", "src/kit.ts", "--budget", "0", "--as-of", AS_OF]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .contains("unit un.button layer=ui.atom in ctr.kit"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("public surface of ctr.kit: un.button"),
        "{}",
        run.stdout
    );
}
