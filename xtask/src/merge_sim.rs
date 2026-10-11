//! `cargo xtask merge-sim --branches 50 --seed N` (0.2 P7-T2, exit criteria 1 and 9): builds a
//! temporary repository whose branches each declare an item, seal and edit, merges them in seeded
//! order (ledgers through `merge=union`), and after each merge runs `sync`, then `check --ci`.
//! It passes when no conflict is left in `gen/` after regeneration, every other conflicted file is
//! named by a finding (never silent), and every continuity finding is repairable by a reseal.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const AS_OF: &str = "2026-10-10";

/// Findings about the version line and the ledger chain (R-GIT-030, R-GIT-032, R-VER-051).
const CONTINUITY: [&str; 4] = ["HN-V001", "HN-V002", "HN-V005", "HN-G003"];

#[derive(Debug)]
pub struct Options {
    pub branches: usize,
    pub seed: u64,
    pub bin: Option<PathBuf>,
    /// Keep the temporary repository and print its path.
    pub keep: bool,
}

#[derive(Debug, Default)]
pub struct Report {
    pub merges: usize,
    /// Files git left conflicted, over all merges.
    pub conflicted: usize,
    /// Conflicted `gen/` files that still hold conflict markers after `sync`.
    pub gen_left: Vec<String>,
    /// Conflicted files outside `gen/` that hold markers and that no finding names.
    pub unreported: Vec<String>,
    /// Continuity findings over all merges.
    pub continuity: usize,
    /// Continuity findings whose fix is not a reseal or a re-run of seal.
    pub unrepairable: Vec<String>,
    /// `check --ci` runs that exited with something other than 0 to 3.
    pub failed_checks: Vec<String>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.gen_left.is_empty()
            && self.unreported.is_empty()
            && self.unrepairable.is_empty()
            && self.failed_checks.is_empty()
    }
}

/// splitmix64: a seeded, dependency-free generator; the merge order depends only on the seed.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// A Fisher-Yates permutation of `0..n` for `seed`.
pub fn order(n: usize, seed: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut s = seed;
    for i in (1..n).rev() {
        let j = usize::try_from(next(&mut s) % (i as u64 + 1)).unwrap_or(0);
        v.swap(i, j);
    }
    v
}

/// Conflicted paths that no line of the `check` output names.
pub fn unreported<'a>(conflicted: &'a [String], output: &str) -> Vec<&'a String> {
    conflicted
        .iter()
        .filter(|p| !output.lines().any(|l| l.contains(p.as_str())))
        .collect()
}

/// The continuity findings of a terse `check` output, and those among them that are not
/// repairable: their fix names neither a reseal nor another run of `seal`.
pub fn continuity(output: &str) -> (usize, Vec<String>) {
    let mut n = 0;
    let mut bad = Vec::new();
    for l in output.lines() {
        if !CONTINUITY.iter().any(|c| l.starts_with(c)) {
            continue;
        }
        n += 1;
        let fix = l.split_once("| fix:").map(|(_, f)| f).unwrap_or("");
        if !(fix.contains("seal --reseal") || fix.contains("fairlead seal again")) {
            bad.push(l.to_owned());
        }
    }
    (n, bad)
}

pub fn has_markers(text: &str) -> bool {
    text.lines()
        .any(|l| l.starts_with("<<<<<<< ") || l == "=======" || l.starts_with(">>>>>>> "))
}

struct Sim {
    dir: PathBuf,
    bin: PathBuf,
}

impl Sim {
    fn git(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new("git")
            .current_dir(&self.dir)
            .args([
                "-c",
                "user.name=merge-sim",
                "-c",
                "user.email=merge-sim@invalid",
            ])
            .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-10-10T12:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-10-10T12:00:00Z")
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }

    /// Runs fairlead; returns the exit code and stdout.
    fn fl(&self, args: &[&str]) -> Result<(i32, String), String> {
        let out = Command::new(&self.bin)
            .current_dir(&self.dir)
            .args(args)
            .env("NO_COLOR", "1")
            .env_remove("FAIRLEAD_ROOT")
            .env_remove("CI")
            .output()
            .map_err(|e| format!("fairlead: {e}"))?;
        Ok((
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        ))
    }

    fn fl_ok(&self, args: &[&str]) -> Result<String, String> {
        match self.fl(args)? {
            (0, s) => Ok(s),
            (c, s) => Err(format!("fairlead {} exited {c}: {s}", args.join(" "))),
        }
    }

    fn write(&self, rel: &str, text: &str, append: bool) -> Result<(), String> {
        let p = self.dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let old = if append {
            std::fs::read_to_string(&p).unwrap_or_default()
        } else {
            String::new()
        };
        std::fs::write(&p, format!("{old}{text}")).map_err(|e| e.to_string())
    }

    fn commit(&self, msg: &str) -> Result<(), String> {
        self.git(&["add", "-A"])?;
        self.git(&["commit", "-q", "--allow-empty", "-m", msg])?;
        Ok(())
    }
}

fn change_doc(id: &str, title: &str, status: &str, covers: &str) -> String {
    format!(
        "+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"{status}\"\nimpact = \"add\"\nscope = \"root\"\ntitle = \"{title}\"\ncovers = [\"{covers}\"]\ncreated = \"0.1.0\"\n+++\n\n# {title}\n\nmerge-sim branch work.\n"
    )
}

/// One branch: declare an item, seal, then edit with a new draft change doc.
fn branch(sim: &Sim, i: usize) -> Result<(), String> {
    let name = format!("b{i:03}");
    sim.git(&["checkout", "-q", "-b", &name, "base"])?;
    let src = format!("src/m{i:03}.rs");
    sim.write(&src, &format!("fn m{i:03}() {{}}\n"), false)?;
    sim.write(
        &format!("fairlead/docs/i{i:03}.md"),
        &format!("+++\ntitle = \"item {i}\"\ndirectives = [\n  'def cmp.i{i:03} sum=\"item {i}\"',\n]\n+++\n"),
        false,
    )?;
    sim.write(
        &format!("fairlead/docs/changes/d-{:08}-branch-{i:03}.md", i + 1),
        &change_doc(
            &format!("d-{:08}", i + 1),
            &format!("Branch {i:03}"),
            "ready",
            &src,
        ),
        false,
    )?;
    sim.fl_ok(&["sync", "--as-of", AS_OF])?;
    sim.commit(&format!("{name}: declare"))?;
    sim.fl_ok(&["seal", "--yes", "--as-of", AS_OF])?;
    sim.commit(&format!("{name}: seal"))?;
    sim.write(&src, &format!("fn m{i:03}_more() {{}}\n"), true)?;
    let id = format!("d-1{i:07}");
    sim.write(
        &format!("fairlead/docs/changes/{id}-more-{i:03}.md"),
        &change_doc(&id, &format!("More {i:03}"), "draft", &src),
        false,
    )?;
    sim.commit(&format!("{name}: edit"))?;
    sim.git(&["checkout", "-q", "main"])?;
    Ok(())
}

/// The fairlead binary to drive: `bin` when given, else a fresh debug build of the workspace.
pub fn fairlead_bin(root: &Path, bin: Option<&PathBuf>) -> Result<PathBuf, String> {
    if let Some(b) = bin {
        return Ok(b.clone());
    }
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "fairlead", "--locked"])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("cargo build -p fairlead failed".into());
    }
    let exe = if cfg!(windows) {
        "fairlead.exe"
    } else {
        "fairlead"
    };
    Ok(root.join("target/debug").join(exe))
}

pub fn run(root: &Path, o: &Options) -> Result<Report, String> {
    let bin = fairlead_bin(root, o.bin.as_ref())?;
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let sim = Sim {
        dir: tmp.path().join("repo"),
        bin,
    };
    std::fs::create_dir_all(&sim.dir).map_err(|e| e.to_string())?;
    sim.git(&["init", "-q", "-b", "main"])?;
    sim.write("src/a.rs", "fn a() {}\n", false)?;
    sim.fl_ok(&["init", "--yes", "--as-of", AS_OF])?;
    sim.write(
        "fairlead/control.toml",
        "\n[version]\nseal-where = \"any\"\n\n[kinds.cmp]\ntitle = \"component\"\nshape = \"either\"\n",
        true,
    )?;
    sim.fl_ok(&["sync", "--as-of", AS_OF])?;
    sim.commit("base")?;
    sim.git(&["branch", "base"])?;
    for i in 0..o.branches {
        branch(&sim, i)?;
    }
    let mut r = Report::default();
    for (k, i) in order(o.branches, o.seed).into_iter().enumerate() {
        let name = format!("b{i:03}");
        let _ = sim.git(&["merge", "-q", "--no-edit", &name]);
        let conflicted: Vec<String> = sim
            .git(&["diff", "--name-only", "--diff-filter=U"])?
            .lines()
            .map(str::to_owned)
            .collect();
        // Mark everything resolved as it stands, so `sync` may regenerate (R-GIT-035).
        sim.git(&["add", "-A"])?;
        sim.fl_ok(&["sync", "--as-of", AS_OF])?;
        sim.commit(&format!("merge {name}"))?;
        let (code, out) = sim.fl(&[
            "check", "--ci", "--max", "0", "--as-of", AS_OF, "--format", "terse",
        ])?;
        if !(0..=3).contains(&code) {
            r.failed_checks
                .push(format!("merge {k} ({name}): check exited {code}"));
        }
        let mut marked = Vec::new();
        for p in &conflicted {
            let text = std::fs::read_to_string(sim.dir.join(p)).unwrap_or_default();
            if !has_markers(&text) {
                continue;
            }
            if p.contains("/gen/") {
                r.gen_left.push(format!("merge {k} ({name}): {p}"));
            } else {
                marked.push(p.clone());
            }
        }
        for p in unreported(&marked, &out) {
            r.unreported.push(format!("merge {k} ({name}): {p}"));
        }
        let (n, bad) = continuity(&out);
        r.continuity += n;
        r.unrepairable
            .extend(bad.into_iter().map(|l| format!("merge {k} ({name}): {l}")));
        r.conflicted += conflicted.len();
        r.merges += 1;
        println!(
            "merge {k:>3} {name}: conflicted={} gen-left={} continuity={n} check-exit={code}",
            conflicted.len(),
            conflicted
                .iter()
                .filter(|p| p.contains("/gen/"))
                .filter(|p| has_markers(
                    &std::fs::read_to_string(sim.dir.join(p.as_str())).unwrap_or_default()
                ))
                .count()
        );
    }
    if o.keep {
        println!("kept {}", tmp.keep().display());
    }
    let left: BTreeSet<&String> = r.gen_left.iter().collect();
    if !left.is_empty() {
        println!("conflicts left in gen/ after sync: {}", left.len());
    }
    Ok(r)
}

#[cfg(test)]
#[allow(non_snake_case, clippy::indexing_slicing)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn merge_sim__order_is_a_seeded_permutation() {
        let a = order(50, 7);
        assert_eq!(a, order(50, 7));
        assert_ne!(a, order(50, 8));
        let mut s = a.clone();
        s.sort_unstable();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn merge_sim__a_conflict_no_finding_names_is_unreported() {
        let c = vec![
            "fairlead/versions/0.1.0.md".to_owned(),
            "fairlead/docs/x.md".to_owned(),
        ];
        let out = "HN-V001 error fairlead/versions/0.1.0.md:1:1 two branches sealed 0.1.0\n";
        assert_eq!(unreported(&c, out), vec![&c[1]]);
    }

    #[test]
    fn merge_sim__continuity_needs_a_reseal_fix() {
        let out = "HN-G003 error fairlead/ledger/hist/2026.hl:1:1 ledger chain broken | fix: a ledger is never edited by hand; reseal the later seal (fairlead seal --reseal) or restore the file\n\
                   HN-V002 warn fairlead/versions/0.1.2.md:1:1 gap | fix: restore it\n\
                   HN-D006 warn src/a.rs uncovered | fix: fairlead new change\n";
        let (n, bad) = continuity(out);
        assert_eq!(n, 2);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("HN-V002"));
    }

    #[test]
    fn merge_sim__markers_are_whole_lines() {
        assert!(has_markers("a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> x\n"));
        assert!(!has_markers("a == b\n"));
    }
}
