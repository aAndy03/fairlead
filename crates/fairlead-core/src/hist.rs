//! Ledger year files `<scope>/<folder>/ledger/hist/<year>.hl` (R-FILE-006, 33-retention layout):
//! append-only terse record lines, each ending in its own digest `d=`, with one checkpoint line
//! `# ckpt v=<version> n=<records> d=<digest>` per sealed version. Pure; seal writes them and
//! `check` verifies their continuity (R-RET-036, `HN-G003`).

use crate::hash::{base32, hash_normalised};
use crate::version::Version;

/// One line of a year file after the header.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HistLine {
    /// A record: the terse text without its trailing ` d=`, and the digest written for it.
    Record {
        text: String,
        d: String,
    },
    Ckpt {
        v: Version,
        n: u32,
        d: String,
    },
    End {
        d: String,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct YearFile {
    pub scope: String,
    pub year: u32,
    /// The previous year's closing digest (`prev=`), `None` for a scope's first year.
    pub prev: Option<String>,
    pub edition: u32,
    pub lines: Vec<HistLine>,
}

fn digest8(bytes: &[u8]) -> String {
    base32(&hash_normalised(bytes).0, 8)
}

/// The digest of a record line, computed over its text without ` d=` (R-RET-031).
pub fn record_digest(text: &str) -> String {
    digest8(text.trim_end().as_bytes())
}

/// A checkpoint's digest: the previous checkpoint's digest (or the header's `prev`) followed by
/// the record digests written since then, in file order.
pub fn checkpoint_digest(prev: &str, records: &[&str]) -> String {
    let mut s = String::from(prev);
    for r in records {
        s.push(' ');
        s.push_str(r);
    }
    digest8(s.as_bytes())
}

fn kv<'a>(words: &[&'a str], key: &str) -> Option<&'a str> {
    words
        .iter()
        .find_map(|w| w.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

impl YearFile {
    pub fn new(scope: &str, year: u32, prev: Option<String>, edition: u32) -> YearFile {
        YearFile {
            scope: scope.to_owned(),
            year,
            prev,
            edition,
            lines: Vec::new(),
        }
    }

    pub fn parse(text: &str) -> Result<YearFile, String> {
        let mut lines = text.lines();
        let header = lines.next().ok_or("empty ledger file")?;
        let words: Vec<&str> = header
            .strip_prefix("# ")
            .ok_or("first line is not a `# scope=... year=...` header")?
            .split_whitespace()
            .collect();
        let scope = kv(&words, "scope").ok_or("header has no scope=")?;
        let year = kv(&words, "year")
            .and_then(|y| y.parse().ok())
            .ok_or("header has no numeric year=")?;
        let prev = kv(&words, "prev").filter(|p| *p != "-").map(str::to_owned);
        let edition = kv(&words, "edition")
            .and_then(|e| e.parse().ok())
            .ok_or("header has no numeric edition=")?;
        let mut out = YearFile::new(scope, year, prev, edition);
        for (i, line) in lines.enumerate() {
            let n = i + 2;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("# ckpt ") {
                let w: Vec<&str> = rest.split_whitespace().collect();
                let v = kv(&w, "v")
                    .and_then(Version::parse)
                    .ok_or(format!("line {n}: checkpoint without a version v="))?;
                let c = kv(&w, "n")
                    .and_then(|x| x.parse().ok())
                    .ok_or(format!("line {n}: checkpoint without a count n="))?;
                let d = kv(&w, "d").ok_or(format!("line {n}: checkpoint without d="))?;
                out.lines.push(HistLine::Ckpt {
                    v,
                    n: c,
                    d: d.to_owned(),
                });
            } else if let Some(rest) = line.strip_prefix("# end ") {
                let w: Vec<&str> = rest.split_whitespace().collect();
                let d = kv(&w, "d").ok_or(format!("line {n}: end without d="))?;
                out.lines.push(HistLine::End { d: d.to_owned() });
            } else if line.starts_with('#') {
                return Err(format!("line {n}: unknown comment line"));
            } else {
                let (text, d) = line
                    .rsplit_once(" d=")
                    .ok_or(format!("line {n}: record without a trailing d="))?;
                out.lines.push(HistLine::Record {
                    text: text.to_owned(),
                    d: d.trim().to_owned(),
                });
            }
        }
        Ok(out)
    }

    /// Byte-exact text: LF only, header first (R-RET-030).
    pub fn render(&self) -> String {
        let mut s = format!(
            "# scope={} year={} prev={} edition={}\n",
            self.scope,
            self.year,
            self.prev.as_deref().unwrap_or("-"),
            self.edition
        );
        for l in &self.lines {
            match l {
                HistLine::Record { text, d } => s.push_str(&format!("{text} d={d}\n")),
                HistLine::Ckpt { v, n, d } => s.push_str(&format!("# ckpt v={v} n={n} d={d}\n")),
                HistLine::End { d } => s.push_str(&format!("# end d={d}\n")),
            }
        }
        s
    }

    /// The digest the chain continues from: the last checkpoint's, else the header's `prev`.
    pub fn last_digest(&self) -> String {
        self.lines
            .iter()
            .rev()
            .find_map(|l| match l {
                HistLine::Ckpt { d, .. } | HistLine::End { d } => Some(d.clone()),
                HistLine::Record { .. } => None,
            })
            .or_else(|| self.prev.clone())
            .unwrap_or_default()
    }

    pub fn is_final(&self) -> bool {
        matches!(self.lines.last(), Some(HistLine::End { .. }))
    }

    /// Seal: append the records of one version and its checkpoint (R-VER-059). Refuses a final
    /// year file (R-RET-032).
    pub fn append_version(&mut self, v: Version, records: &[String]) -> Result<(), String> {
        if self.is_final() {
            return Err(format!(
                "year {} of scope {} is final",
                self.year, self.scope
            ));
        }
        let prev = self.last_digest();
        let mut digests = Vec::new();
        for r in records {
            let d = record_digest(r);
            self.lines.push(HistLine::Record {
                text: r.trim_end().to_owned(),
                d: d.clone(),
            });
            digests.push(d);
        }
        let refs: Vec<&str> = digests.iter().map(String::as_str).collect();
        let n = u32::try_from(records.len()).unwrap_or(u32::MAX);
        self.lines.push(HistLine::Ckpt {
            v,
            n,
            d: checkpoint_digest(&prev, &refs),
        });
        Ok(())
    }

    /// Close the year: `# end d=` repeats its last checkpoint digest (R-RET-031).
    pub fn finalise(&mut self) {
        if !self.is_final() {
            let d = self.last_digest();
            self.lines.push(HistLine::End { d });
        }
    }

    /// R-RET-036 continuity: checkpoint versions ascend, each `n=` counts the records since the
    /// previous checkpoint, each `d=` recomputes; `# end` repeats the last checkpoint and is last.
    /// Record lines after the last checkpoint belong to no sealed version and are a break too.
    /// Returns one message per break, empty when the chain holds.
    pub fn verify(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut prev = self.prev.clone().unwrap_or_default();
        let mut last_v: Option<Version> = None;
        let mut pending: Vec<&str> = Vec::new();
        for (i, l) in self.lines.iter().enumerate() {
            match l {
                HistLine::Record { d, .. } => pending.push(d),
                HistLine::Ckpt { v, n, d } => {
                    if last_v.is_some_and(|lv| *v <= lv) {
                        out.push(format!("checkpoint v={v} does not ascend"));
                    }
                    if usize::try_from(*n).ok() != Some(pending.len()) {
                        out.push(format!(
                            "checkpoint v={v} counts n={n}, but {} records precede it",
                            pending.len()
                        ));
                    }
                    let want = checkpoint_digest(&prev, &pending);
                    if *d != want {
                        out.push(format!("checkpoint v={v} digest d={d} does not recompute"));
                    }
                    prev = d.clone();
                    last_v = Some(*v);
                    pending.clear();
                }
                HistLine::End { d } => {
                    if i + 1 != self.lines.len() {
                        out.push("`# end` is not the last line".into());
                    }
                    if *d != prev {
                        out.push(format!("`# end d={d}` does not repeat the last checkpoint"));
                    }
                }
            }
        }
        if !pending.is_empty() {
            out.push(format!(
                "{} record lines after the last checkpoint",
                pending.len()
            ));
        }
        out
    }
}

/// Across years of one scope: each year's `prev=` must equal the previous year's `# end` digest,
/// and every year but the last must be final (R-RET-031). `years` sorted by year.
pub fn verify_chain(years: &[&YearFile]) -> Vec<String> {
    let mut out = Vec::new();
    for w in years.windows(2) {
        let (a, b) = (w[0], w[1]);
        if !a.is_final() {
            out.push(format!(
                "year {} is followed by {} but has no `# end`",
                a.year, b.year
            ));
        }
        if b.prev.as_deref() != Some(a.last_digest().as_str()) {
            out.push(format!(
                "year {} has prev={} but year {} ends with d={}",
                b.year,
                b.prev.as_deref().unwrap_or("-"),
                a.year,
                a.last_digest()
            ));
        }
    }
    out
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn sample() -> YearFile {
        let mut f = YearFile::new("web", 2031, Some("5c0e91d2".into()), 1);
        f.append_version(
            v("1.8.0"),
            &[
                "d-3f9a8b11 kind=adr ord=0007 title=\"no tailwind\" out=adopted".into(),
                "d-71c2b0ee kind=change ord=0412 out=rejected".into(),
            ],
        )
        .unwrap();
        f.append_version(v("1.9.0"), &["roll.2031q2 kind=change n=214".into()])
            .unwrap();
        f
    }

    #[test]
    fn r_ret_036__appended_versions_verify_and_round_trip() {
        let f = sample();
        assert!(f.verify().is_empty(), "{:?}", f.verify());
        let text = f.render();
        assert!(text.starts_with("# scope=web year=2031 prev=5c0e91d2 edition=1\n"));
        assert!(text.contains("\n# ckpt v=1.8.0 n=2 d="));
        assert_eq!(YearFile::parse(&text).unwrap(), f);
        assert!(!text.contains('\r'));
    }

    #[test]
    fn r_ret_036__checkpoint_continuity_detects_each_break_kind() {
        let base = sample().render();
        let lines: Vec<&str> = base.lines().collect();
        // v= does not ascend
        let swapped = base.replace("v=1.9.0", "v=1.7.0");
        assert!(
            YearFile::parse(&swapped)
                .unwrap()
                .verify()
                .iter()
                .any(|m| m.contains("ascend"))
        );
        // n= miscounts
        let miscount = base.replace("v=1.8.0 n=2", "v=1.8.0 n=3");
        assert!(
            YearFile::parse(&miscount)
                .unwrap()
                .verify()
                .iter()
                .any(|m| m.contains("counts n=3"))
        );
        // a record line removed (union-merge loss) breaks n= and d=
        let dropped: String = lines
            .iter()
            .filter(|l| !l.starts_with("d-71c2b0ee"))
            .map(|l| format!("{l}\n"))
            .collect();
        let errs = YearFile::parse(&dropped).unwrap().verify();
        assert!(
            errs.iter().any(|m| m.contains("does not recompute")),
            "{errs:?}"
        );
        // a record edited in place keeps its written d= (purge path), so the chain still holds
        let purged = base.replace("title=\"no tailwind\" out=adopted", "out=adopted");
        assert!(YearFile::parse(&purged).unwrap().verify().is_empty());
        // trailing records with no checkpoint
        let trailing = format!("{base}d-00000001 kind=change d=abcdefgh\n");
        assert!(
            YearFile::parse(&trailing)
                .unwrap()
                .verify()
                .iter()
                .any(|m| m.contains("after the last checkpoint"))
        );
    }

    #[test]
    fn r_ret_031__years_chain_through_end_and_prev() {
        let mut y1 = sample();
        y1.finalise();
        assert!(y1.verify().is_empty());
        assert!(
            y1.append_version(v("2.0.0"), &[]).is_err(),
            "a final year is never appended to"
        );
        let mut y2 = YearFile::new("web", 2032, Some(y1.last_digest()), 1);
        y2.append_version(v("2.0.0"), &["d-00000002 kind=change".into()])
            .unwrap();
        assert!(verify_chain(&[&y1, &y2]).is_empty());
        let bad = YearFile::new("web", 2032, Some("zzzzzzzz".into()), 1);
        assert_eq!(verify_chain(&[&y1, &bad]).len(), 1);
        let open = sample();
        assert!(
            verify_chain(&[&open, &y2])
                .iter()
                .any(|m| m.contains("no `# end`"))
        );
    }

    #[test]
    fn r_file_006__malformed_files_are_errors_not_panics() {
        for bad in [
            "",
            "no header\n",
            "# scope=web\n",
            "# scope=web year=2031 prev=- edition=1\nrecord without digest\n",
        ] {
            assert!(YearFile::parse(bad).is_err(), "{bad:?}");
        }
    }
}
