//! `cargo xtask retention-sim --years 5 --seals-per-year 12 --seed N [--bin <fairlead>]
//! [--digest-out <file>]` (0.3 P7-T2, exit criterion 2): a synthetic history of change docs and
//! seals in a temporary repository with compaction on, run twice from the same seed. It checks
//! that both runs write byte-identical ledgers, that a second `seal --compact-only` is a no-op,
//! and that the leaf `context` brief is the same size at year 1 and at the last year. The
//! ledger digest it prints (and writes with `--digest-out`) is compared across operating
//! systems by the CI job.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Options {
    pub years: u32,
    pub per_year: u32,
    pub seed: u64,
    pub bin: Option<PathBuf>,
    pub digest_out: Option<PathBuf>,
}

/// A small deterministic generator (SplitMix64): the same seed gives the same history.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

/// A draft ID from the generator (`d-` and 8 Crockford base32 symbols).
pub fn draft_id(rng: &mut Rng) -> String {
    const A: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";
    let mut s = String::from("d-");
    for _ in 0..8 {
        let i = usize::try_from(rng.below(32)).unwrap_or(0);
        s.push(char::from(A.get(i).copied().unwrap_or(b'0')));
    }
    s
}

/// The recorded date of seal `n` (0-based): the first of each month from January 2026.
pub fn seal_date(n: u32, per_year: u32) -> String {
    let per_year = per_year.clamp(1, 12);
    let year = 2026 + n / per_year;
    let month = 1 + (n % per_year) * (12 / per_year);
    format!("{year}-{month:02}-01")
}

struct Sim {
    dir: PathBuf,
    bin: PathBuf,
}

impl Sim {
    fn git(&self, args: &[&str], date: &str) -> Result<(), String> {
        let st = Command::new("git")
            .args([
                "-c",
                "user.name=sim",
                "-c",
                "user.email=sim@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.autocrlf=false",
            ])
            .args(args)
            .env("GIT_AUTHOR_DATE", format!("{date}T00:00:00Z"))
            .env("GIT_COMMITTER_DATE", format!("{date}T00:00:00Z"))
            .current_dir(&self.dir)
            .output()
            .map_err(|e| e.to_string())?;
        if st.status.success() {
            Ok(())
        } else {
            Err(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&st.stderr)
            ))
        }
    }
    fn fl(&self, args: &[&str]) -> Result<(i32, String), String> {
        let o = Command::new(&self.bin)
            .args(args)
            .arg("--no-cache")
            .current_dir(&self.dir)
            .output()
            .map_err(|e| e.to_string())?;
        Ok((
            o.status.code().unwrap_or(5),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        ))
    }
    fn write(&self, rel: &str, text: &str) -> Result<(), String> {
        let p = self.dir.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, text).map_err(|e| e.to_string())
    }
}

/// One run of the history: the ledger files (path to bytes) and the leaf brief sizes per year.
/// One run's ledger files (path to bytes), leaf brief sizes per year, and the output of the
/// final `seal --compact-only`.
type History = (BTreeMap<String, Vec<u8>>, Vec<usize>, String);

fn history(bin: &Path, o: &Options) -> Result<History, String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let sim = Sim {
        dir: tmp.path().join("repo"),
        bin: bin.to_path_buf(),
    };
    std::fs::create_dir_all(&sim.dir).map_err(|e| e.to_string())?;
    let first = seal_date(0, o.per_year);
    sim.git(&["init", "-q", "-b", "main"], &first)?;
    sim.write("src/lib.rs", "pub fn a() {}\n")?;
    sim.write("CODEOWNERS", "* @owner\n")?;
    sim.write("web/fairlead/control.toml", "[fairlead]\nscope = \"web\"\n")?;
    sim.write("web/src/page.ts", "export const page = 1;\n")?;
    let (code, out) = sim.fl(&["init", "--yes", "--no-baseline", "--as-of", &first])?;
    if code != 0 {
        return Err(format!("init: {out}"));
    }
    let ctl = sim.dir.join("fairlead/control.toml");
    let text = std::fs::read_to_string(&ctl).map_err(|e| e.to_string())?;
    std::fs::write(
        &ctl,
        format!(
            "{text}\n[retain]\nmin-age = \"90d\"\nroll-after = 4\n\n[retain.chg]\nttl = \"8v\"\n"
        ),
    )
    .map_err(|e| e.to_string())?;
    sim.git(&["add", "-A"], &first)?;
    sim.git(&["commit", "-q", "-m", "init"], &first)?;
    let mut rng = Rng::new(o.seed);
    let mut sizes = Vec::new();
    let total = o.years * o.per_year;
    let mut last = first.clone();
    for n in 0..total {
        let date = seal_date(n, o.per_year);
        let docs = 1 + rng.below(3);
        for k in 0..docs {
            let id = draft_id(&mut rng);
            let (scope, dir, covers) = if rng.below(2) == 0 {
                ("root", "fairlead", "src/**")
            } else {
                ("web", "web/fairlead", "src/**")
            };
            sim.write(
                &format!("{dir}/docs/changes/{id}-change.md"),
                &format!("+++\nid = \"{id}\"\nkind = \"change\"\nstatus = \"ready\"\nimpact = \"fix\"\nscope = \"{scope}\"\ntitle = \"Change {n} {k}\"\nsum = \"Synthetic change {n}.{k}\"\nout = \"\"\nwhy = \"\"\ncovers = [\"{covers}\"]\ncreated = \"0.1.0\"\n+++\n# Change {n} {k}\n"),
            )?;
        }
        sim.git(&["add", "-A"], &date)?;
        sim.git(&["commit", "-q", "-m", &format!("docs {n}")], &date)?;
        let (code, out) = sim.fl(&["seal", "--yes", "--anywhere", "--as-of", &date])?;
        if code != 0 {
            let (_, check) = sim.fl(&["check", "--format", "terse", "--as-of", &date])?;
            return Err(format!("seal {n} on {date}: {out}{check}"));
        }
        sim.git(&["add", "-A"], &date)?;
        sim.git(&["commit", "-q", "-m", &format!("seal {n}")], &date)?;
        if (n + 1) % o.per_year == 0 {
            let (_, brief) = sim.fl(&[
                "context",
                "web/src/page.ts",
                "--format",
                "terse",
                "--as-of",
                &date,
            ])?;
            sizes.push(brief.len());
        }
        last = date;
    }
    let (_, again) = sim.fl(&["seal", "--compact-only", "--yes", "--as-of", &last])?;
    let mut ledgers = BTreeMap::new();
    let mut stack = vec![sim.dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())?.flatten() {
            let p = e.path();
            if p.is_dir() {
                if !p.ends_with(".git") {
                    stack.push(p);
                }
            } else if p
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/ledger/hist/")
            {
                let rel = p
                    .strip_prefix(&sim.dir)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                ledgers.insert(rel, std::fs::read(&p).map_err(|e| e.to_string())?);
            }
        }
    }
    Ok((ledgers, sizes, again))
}

/// A digest of every ledger file (path and bytes), for the cross-OS comparison.
pub fn digest(ledgers: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (p, b) in ledgers {
        for byte in p.as_bytes().iter().chain([0u8].iter()).chain(b.iter()) {
            h ^= u64::from(*byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

pub fn run(root: &Path, o: &Options) -> Result<Vec<String>, String> {
    let bin = crate::merge_sim::fairlead_bin(root, o.bin.as_ref())?;
    let (a, sizes, again) = history(&bin, o)?;
    let (b, _, _) = history(&bin, o)?;
    let mut out = Vec::new();
    let lines: usize = a
        .values()
        .map(|x| x.iter().filter(|c| **c == b'\n').count())
        .sum();
    out.push(format!(
        "ledger files={} lines={lines} digest={}",
        a.len(),
        digest(&a)
    ));
    if a != b {
        return Err("two runs from the same seed wrote different ledger bytes".into());
    }
    out.push("two runs: byte-identical".into());
    if !again.contains("nothing is due") {
        return Err(format!("a second compaction was not a no-op: {again}"));
    }
    out.push("seal --compact-only after the history: no-op".into());
    let (Some(first), Some(lastb)) = (sizes.first(), sizes.last()) else {
        return Err("no brief was measured".into());
    };
    out.push(format!(
        "leaf brief bytes: year 1 {first}, year {} {lastb}",
        sizes.len()
    ));
    // the brief does not grow with history: the last year's is at most 10% above the first's
    if *lastb > first + first / 10 {
        return Err(format!(
            "the leaf brief grew with history: {first} -> {lastb}"
        ));
    }
    if let Some(p) = &o.digest_out {
        std::fs::write(p, format!("{}\n", digest(&a))).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_ret_021__the_history_generator_is_deterministic() {
        let ids = |seed| {
            let mut r = Rng::new(seed);
            (0..5).map(|_| draft_id(&mut r)).collect::<Vec<_>>()
        };
        assert_eq!(ids(7), ids(7));
        assert_ne!(ids(7), ids(8));
        assert!(ids(7).iter().all(|i| i.len() == 10 && i.starts_with("d-")));
        assert_eq!(seal_date(0, 12), "2026-01-01");
        assert_eq!(seal_date(13, 12), "2027-02-01");
        assert_eq!(seal_date(3, 4), "2026-10-01");
    }
}
