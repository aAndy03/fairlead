//! Fs seam, atomic replace, journal, quarantine (R-ARCH-145, R-CLI-007).
use super::{io_err, repo_path};
use crate::error::Error;
use crate::hash::{base32, hash_normalised};
use crate::path::RepoPath;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration;

/// Every byte the write layer puts on disk goes through this seam, so tests can inject failures.
pub trait Fs {
    fn read(&self, p: &Utf8Path) -> io::Result<Option<Vec<u8>>>; // None when absent
    fn create_dir_all(&self, p: &Utf8Path) -> io::Result<()>;
    fn write_new(&self, p: &Utf8Path, bytes: &[u8]) -> io::Result<()>; // create_new + write + fsync
    fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> io::Result<()>; // replaces an existing target
    fn remove_file(&self, p: &Utf8Path) -> io::Result<()>; // Ok when absent
    fn set_executable(&self, p: &Utf8Path) -> io::Result<()>;
    fn pause(&self, ms: u64);
}

pub struct RealFs;

impl Fs for RealFs {
    fn read(&self, p: &Utf8Path) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(p.as_std_path()) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn create_dir_all(&self, p: &Utf8Path) -> io::Result<()> {
        std::fs::create_dir_all(p.as_std_path())
    }
    fn write_new(&self, p: &Utf8Path, bytes: &[u8]) -> io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(p.as_std_path())?;
        f.write_all(bytes)?;
        f.sync_all()
    }
    fn rename(&self, from: &Utf8Path, to: &Utf8Path) -> io::Result<()> {
        std::fs::rename(from.as_std_path(), to.as_std_path())
    }
    fn remove_file(&self, p: &Utf8Path) -> io::Result<()> {
        match std::fs::remove_file(p.as_std_path()) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
    fn set_executable(&self, p: &Utf8Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(p.as_std_path(), std::fs::Permissions::from_mode(0o755))
        }
        #[cfg(not(unix))]
        {
            let _ = p;
            Ok(())
        }
    }
    fn pause(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

pub const RENAME_ATTEMPTS: usize = 6;

/// Sibling temp name, recorded in the journal: `.<name>.<tag>.tmp`.
pub fn temp_name(target: &Utf8Path, tag: &str) -> Utf8PathBuf {
    target.with_file_name(format!(
        ".{}.{tag}.tmp",
        target.file_name().unwrap_or("file")
    ))
}

/// Bounded retry with doubling back-off (10, 20, 40, 80, 160 ms) for Windows antivirus/editor locks (R-ARCH-145).
pub fn rename_with_retry(fs: &dyn Fs, from: &Utf8Path, to: &Utf8Path) -> io::Result<()> {
    let mut delay = 10u64;
    let mut last = None;
    for attempt in 0..RENAME_ATTEMPTS {
        match fs.rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                if attempt + 1 < RENAME_ATTEMPTS {
                    fs.pause(delay);
                    delay = (delay * 2).min(250);
                }
            }
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("rename failed")))
}

/// Temp file in the target's directory, fsync, rename. A failure leaves the target untouched and no temp behind.
pub fn atomic_replace(fs: &dyn Fs, target: &Utf8Path, bytes: &[u8], tag: &str) -> io::Result<()> {
    if let Some(dir) = target.parent() {
        fs.create_dir_all(dir)?;
    }
    let tmp = temp_name(target, tag);
    fs.remove_file(&tmp)?;
    fs.write_new(&tmp, bytes)?;
    if let Err(e) = rename_with_retry(fs, &tmp, target) {
        let _ = fs.remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

pub(crate) fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct JournalEntry {
    pub path: String,           // repo-relative target
    pub temp: String,           // repo-relative staged file
    pub backup: Option<String>, // repo-relative copy of the prior content (None for a new file)
    pub prior: Option<String>,  // hex blake3 of the prior raw bytes
    pub new_hash: String,       // hex blake3 of the new raw bytes
    pub exec: bool,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct JournalDoc {
    pub tx: String,
    pub entries: Vec<JournalEntry>,
    /// The write-set kind (`Move`, `Seal`...), so `check` can name an interrupted `move`.
    #[serde(default)]
    pub kind: Option<String>,
}

/// `.fairlead/sync.journal` (R-CLI-007).
pub struct Journal {
    path: Utf8PathBuf,
}

impl Journal {
    pub fn at(root_dir: &Utf8Path) -> Journal {
        Journal {
            path: root_dir.join(".fairlead").join("sync.journal"),
        }
    }
    pub fn read(&self, fs: &dyn Fs) -> Result<Option<JournalDoc>, Error> {
        let Some(b) = fs
            .read(&self.path)
            .map_err(|e| io_err("read", self.path.as_str(), &e))?
        else {
            return Ok(None);
        };
        serde_json::from_slice(&b)
            .map(Some)
            .map_err(|e| Error::Internal(format!("corrupt journal {}: {e}", self.path)))
    }
    pub fn write(&self, fs: &dyn Fs, doc: &JournalDoc) -> Result<(), Error> {
        let mut b = serde_json::to_vec_pretty(doc).map_err(|e| Error::Internal(e.to_string()))?;
        b.push(b'\n');
        atomic_replace(fs, &self.path, &b, "j")
            .map_err(|e| io_err("journal", self.path.as_str(), &e))
    }
    pub fn clear(&self, fs: &dyn Fs) -> Result<(), Error> {
        fs.remove_file(&self.path)
            .map_err(|e| io_err("journal", self.path.as_str(), &e))
    }
}

/// `.fairlead/quarantine/<name>.<12 chars of the normalised content hash>` (R-CLI-026).
pub fn quarantine_path(gen_path: &RepoPath, bytes: &[u8]) -> Result<RepoPath, Error> {
    let name = gen_path.as_str().rsplit('/').next().unwrap_or("file");
    repo_path(&format!(
        ".fairlead/quarantine/{name}.{}",
        base32(&hash_normalised(bytes).0, 12)
    ))
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::write::testutil::*;

    fn p(d: &tempfile::TempDir, rel: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(d.path().join(rel)).unwrap()
    }

    #[test]
    fn r_arch_145__atomic_replace_writes_target_and_leaves_no_temp() {
        let d = tempfile::tempdir().unwrap();
        atomic_replace(&RealFs, &p(&d, "a/b.txt"), b"one\n", "t").unwrap();
        atomic_replace(&RealFs, &p(&d, "a/b.txt"), b"two\n", "t").unwrap();
        assert_eq!(std::fs::read(d.path().join("a/b.txt")).unwrap(), b"two\n");
        assert_eq!(tree(d.path()), ["a/b.txt"]);
    }

    #[test]
    fn r_arch_145__failed_write_leaves_the_target_untouched() {
        let d = tempfile::tempdir().unwrap();
        atomic_replace(&RealFs, &p(&d, "x.txt"), b"old\n", "t").unwrap();
        let fs = FailingFs::new().write_fails_on(1);
        assert!(atomic_replace(&fs, &p(&d, "x.txt"), b"new\n", "t").is_err());
        assert_eq!(std::fs::read(d.path().join("x.txt")).unwrap(), b"old\n");
        assert_eq!(tree(d.path()), ["x.txt"], "no temp file left");
    }

    #[test]
    fn r_cli_007__rename_retries_transient_failures_with_backoff() {
        let d = tempfile::tempdir().unwrap();
        let fs = FailingFs::new().renames_fail(1, 3);
        atomic_replace(&fs, &p(&d, "x.txt"), b"new\n", "t").unwrap();
        assert_eq!(fs.rename_calls(), 4);
    }

    #[test]
    fn r_cli_007__rename_retry_is_bounded_and_cleans_up() {
        let d = tempfile::tempdir().unwrap();
        let fs = FailingFs::new().renames_fail(1, 1000);
        assert!(atomic_replace(&fs, &p(&d, "x.txt"), b"new\n", "t").is_err());
        assert_eq!(fs.rename_calls(), RENAME_ATTEMPTS);
        assert!(tree(d.path()).is_empty());
    }

    #[test]
    fn r_arch_145__journal_roundtrips_and_clear_is_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let j = Journal::at(&p(&d, ""));
        assert_eq!(j.read(&RealFs).unwrap(), None);
        let doc = JournalDoc {
            tx: "abc".into(),
            entries: vec![JournalEntry {
                path: "a.txt".into(),
                temp: ".a.txt.abc.tmp".into(),
                backup: None,
                prior: None,
                new_hash: "00".into(),
                exec: false,
            }],
            kind: None,
        };
        j.write(&RealFs, &doc).unwrap();
        assert_eq!(j.read(&RealFs).unwrap(), Some(doc));
        j.clear(&RealFs).unwrap();
        j.clear(&RealFs).unwrap();
    }

    #[test]
    fn r_cli_026__quarantine_path_is_name_dot_content_hash() {
        let a = quarantine_path(&rp("fairlead/gen/reg/rule.reg"), b"edited\n").unwrap();
        let b = quarantine_path(&rp("apps/web/fairlead/gen/reg/rule.reg"), b"edited\n").unwrap();
        assert_eq!(a, b);
        assert!(a.as_str().starts_with(".fairlead/quarantine/rule.reg."));
        assert_eq!(a.as_str().rsplit('.').next().unwrap().len(), 12);
        assert_ne!(
            a,
            quarantine_path(&rp("fairlead/gen/reg/rule.reg"), b"other\n").unwrap()
        );
    }
}
