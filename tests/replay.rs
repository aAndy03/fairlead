#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]
//! Recall and false positives over a history (R-ROAD-004). Thresholds match
//! docs/plans/0.1/ablation-protocol.md and are asserted there by xtask.
#[path = "testkit/mod.rs"]
mod testkit;

use testkit::{RepoBuilder, TempRepo, run_fairlead};

const RECALL_MIN_PCT: usize = 70;
const FP_MAX_PCT: usize = 5;
const INTRODUCING: usize = 50;
const UNRELATED: usize = 20;

const ROOT_CONTROL: &str = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\nmarker = \"fairlead\"\n\n[settings]\nfolder = \"fairlead\"\nfairlead-version = \"0.1.0\"\nowners = [\"@andy\"]\n\n[kinds.tok]\ntitle = \"design token\"\nshape = \"doc\"\n\n[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"enforce\"\nglob = [\"**/*.tsx\"]\nwaivable = true\nwhy = \"colours come from tokens\"\n";
const TOKENS: &str = "+++\ntitle = \"Tokens\"\ndirectives = [\n  'def tok.color.primary value=\"#2563eb\" applies=color,background-color sum=\"brand\"',\n]\n+++\n";

fn t001_count(repo: &TempRepo) -> usize {
    let run = run_fairlead(
        repo.path(),
        &[
            "check",
            "--as-of",
            "2026-10-09",
            "--format",
            "terse",
            "--max",
            "0",
        ],
        &[],
    );
    run.stdout
        .lines()
        .filter(|l| l.starts_with("HN-T001"))
        .count()
}

#[test]
fn r_road_004__recall_and_false_positives_on_synthetic_history() {
    let repo = RepoBuilder::new()
        .file("fairlead/control.toml", ROOT_CONTROL)
        .file("fairlead/contract/tokens.md", TOKENS)
        .git_init(true)
        .build();
    std::fs::create_dir_all(repo.path().join("src")).expect("src");
    for i in 0..INTRODUCING {
        let body = format!(
            "// fairlead: use tok.color.primary\nexport const c{i} = {{ color: \"#2563eb\" }};\n"
        );
        std::fs::write(repo.path().join(format!("src/c{i:02}.tsx")), body).expect("write");
        repo.git(&["add", "-A"]);
        repo.git_commit(&format!("introduce literal {i}"));
    }
    for j in 0..UNRELATED {
        std::fs::write(
            repo.path().join(format!("src/u{j:02}.tsx")),
            format!("export const u{j} = {j};\n"),
        )
        .expect("write");
        repo.git(&["add", "-A"]);
        repo.git_commit(&format!("unrelated {j}"));
    }
    let shas: Vec<String> = repo
        .git(&["rev-list", "--reverse", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        shas.len(),
        1 + INTRODUCING + UNRELATED,
        "root plus the two series"
    );
    let mut counts = Vec::with_capacity(shas.len());
    for sha in &shas {
        repo.git(&["checkout", "-q", sha]);
        counts.push(t001_count(&repo));
    }
    let introduced = (1..=INTRODUCING)
        .filter(|&k| counts[k] > counts[k - 1])
        .count();
    let false_pos = (INTRODUCING + 1..shas.len())
        .filter(|&k| counts[k] > counts[k - 1])
        .count();
    assert!(
        introduced * 100 >= INTRODUCING * RECALL_MIN_PCT,
        "recall {introduced}/{INTRODUCING} is below {RECALL_MIN_PCT}%"
    );
    assert!(
        false_pos * 100 <= UNRELATED * FP_MAX_PCT,
        "{false_pos}/{UNRELATED} false positives exceed {FP_MAX_PCT}%"
    );
}
