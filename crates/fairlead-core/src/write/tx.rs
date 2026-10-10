//! The write transaction (R-ARCH-141..145).
use super::journal::{
    Fs, Journal, JournalDoc, JournalEntry, RealFs, atomic_replace, hex, rename_with_retry,
    temp_name,
};
use super::lock::WriterLock;
use super::writeset::{append_plain, splice_block};
use super::{WriteKind, WriteSet, io_err, plan_hash, refuse};
use crate::discover::Root;
use crate::error::{Error, RefusalKind};
use crate::hash::{base32, hash_normalised, hash_raw};
use crate::path::RepoPath;
use camino::{Utf8Path, Utf8PathBuf};
use std::marker::PhantomData;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommitReport {
    pub written: Vec<RepoPath>,
    pub unchanged: Vec<RepoPath>,
}
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Recovery {
    Clean,
    Resumed,
    RolledBack,
}

struct Item {
    path: RepoPath,
    abs: Utf8PathBuf,
    fin: Vec<u8>,
    prior: Option<Vec<u8>>,
    unchanged: bool,
    delete: bool,
}

pub struct Transaction<'l> {
    dir: Utf8PathBuf,
    fs: &'l dyn Fs,
    set: WriteSet,
    items: Vec<Item>,
    _lock: PhantomData<&'l ()>, // proof that the writer lock is held for the whole transaction
}

static REAL_FS: RealFs = RealFs;

fn rel(dir: &Utf8Path, abs: &Utf8Path) -> String {
    abs.strip_prefix(dir)
        .map(|p| p.as_str().replace('\\', "/"))
        .unwrap_or_else(|_| abs.as_str().to_owned())
}

impl<'l> Transaction<'l> {
    /// Called first by every writer, under the lock (R-CLI-007).
    pub fn recover(root: &Root, _lock: &WriterLock<'_>) -> Result<Recovery, Error> {
        Self::recover_with(root, &REAL_FS)
    }

    /// Roll an interrupted journal back to the prior tree (`seal --abort`, R-VER-051).
    pub fn abort(root: &Root, _lock: &WriterLock<'_>) -> Result<Recovery, Error> {
        Self::abort_with(root, &REAL_FS)
    }

    pub fn abort_with(root: &Root, fs: &dyn Fs) -> Result<Recovery, Error> {
        let journal = Journal::at(&root.dir);
        let Some(doc) = journal.read(fs)? else {
            return Ok(Recovery::Clean);
        };
        rollback(fs, &root.dir, &doc)?;
        journal.clear(fs)?;
        Ok(Recovery::RolledBack)
    }

    pub fn recover_with(root: &Root, fs: &dyn Fs) -> Result<Recovery, Error> {
        let journal = Journal::at(&root.dir);
        let Some(doc) = journal.read(fs)? else {
            return Ok(Recovery::Clean);
        };
        let mut resumable = true;
        for e in &doc.entries {
            let temp_here = e.temp.is_empty()
                || fs
                    .read(&root.dir.join(&e.temp))
                    .map_err(|x| io_err("read", &e.temp, &x))?
                    .is_some();
            if !(applied(fs, &root.dir, e)? || temp_here) {
                resumable = false;
                break;
            }
        }
        let outcome = if resumable {
            apply(fs, &root.dir, &doc)?;
            cleanup(fs, &root.dir, &doc);
            Recovery::Resumed
        } else {
            rollback(fs, &root.dir, &doc)?;
            Recovery::RolledBack
        };
        journal.clear(fs)?;
        Ok(outcome)
    }

    pub fn begin(root: &Root, lock: &'l WriterLock<'_>, set: WriteSet) -> Result<Self, Error> {
        Self::begin_with(root, lock, set, &REAL_FS)
    }

    /// Same as `begin` with an injected filesystem (tests: failing writer).
    pub fn begin_with(
        root: &Root,
        _lock: &'l WriterLock<'_>,
        set: WriteSet,
        fs: &'l dyn Fs,
    ) -> Result<Self, Error> {
        set.validate(root)?;
        let mut items = Vec::new();
        for path in apply_order(&set) {
            let Some(w) = set.writes.get(&path) else {
                continue;
            };
            let abs = root.dir.join(path.as_str());
            let cur = fs
                .read(&abs)
                .map_err(|e| io_err("read", path.as_str(), &e))?;
            match (&w.expect_prior, &cur) {
                (Some(exp), Some(c)) if hash_normalised(c) != *exp => {
                    return Err(refuse(
                        RefusalKind::NoClobber,
                        format!("{} changed since it was planned", path.as_str()),
                        "re-run the command to re-plan",
                    ));
                }
                (Some(_), None) => {
                    return Err(refuse(
                        RefusalKind::NoClobber,
                        format!("{} vanished since it was planned", path.as_str()),
                        "re-run the command to re-plan",
                    ));
                }
                _ => {}
            }
            let text = String::from_utf8_lossy(&w.bytes).into_owned();
            let fin = match &w.kind {
                WriteKind::Create => match &cur {
                    Some(c) if *c != w.bytes => {
                        return Err(refuse(
                            RefusalKind::NoClobber,
                            format!("{} already exists", path.as_str()),
                            "fairlead never overwrites; pick another name or edit the file by hand",
                        ));
                    }
                    _ => w.bytes.clone(),
                },
                WriteKind::Replace | WriteKind::Delete => w.bytes.clone(),
                WriteKind::AppendBlock { marker } => {
                    let existing = cur.clone().unwrap_or_default();
                    if marker.is_empty() {
                        append_plain(&existing, &text)
                    } else {
                        splice_block(&existing, marker, &text)?
                    }
                }
            };
            let delete = w.kind == WriteKind::Delete;
            let unchanged = if delete {
                cur.is_none()
            } else {
                cur.as_deref() == Some(fin.as_slice())
            };
            items.push(Item {
                path: path.clone(),
                abs,
                fin,
                prior: cur,
                unchanged,
                delete,
            });
        }
        Ok(Transaction {
            dir: root.dir.clone(),
            fs,
            set,
            items,
            _lock: PhantomData,
        })
    }

    pub fn commit(self) -> Result<CommitReport, Error> {
        let unchanged: Vec<RepoPath> = self
            .items
            .iter()
            .filter(|i| i.unchanged)
            .map(|i| i.path.clone())
            .collect();
        let todo: Vec<&Item> = self.items.iter().filter(|i| !i.unchanged).collect();
        if todo.is_empty() {
            return Ok(CommitReport {
                written: vec![],
                unchanged,
            });
        }
        let txid = base32(&plan_hash(&self.set).0, 12);
        let journal = Journal::at(&self.dir);
        let mut created: Vec<Utf8PathBuf> = Vec::new();
        let doc = match self.stage(&todo, &txid, &mut created) {
            Ok(d) => d,
            Err(e) => {
                self.discard(&created);
                return Err(e);
            }
        };
        if let Err(e) = journal.write(self.fs, &doc) {
            self.discard(&created);
            return Err(e);
        }
        if let Err(e) = apply(self.fs, &self.dir, &doc) {
            if rollback(self.fs, &self.dir, &doc).is_ok() {
                let _ = journal.clear(self.fs);
            }
            return Err(e);
        }
        cleanup(self.fs, &self.dir, &doc);
        journal.clear(self.fs)?;
        Ok(CommitReport {
            written: todo.iter().map(|i| i.path.clone()).collect(),
            unchanged,
        })
    }

    fn stage(
        &self,
        todo: &[&Item],
        txid: &str,
        created: &mut Vec<Utf8PathBuf>,
    ) -> Result<JournalDoc, Error> {
        let mut entries = Vec::new();
        for (n, it) in todo.iter().enumerate() {
            let io = |op: &'static str, e: std::io::Error| io_err(op, it.path.as_str(), &e);
            if let Some(dir) = it.abs.parent() {
                self.fs.create_dir_all(dir).map_err(|e| io("mkdir", e))?;
            }
            let temp = temp_name(&it.abs, txid);
            if !it.delete {
                self.fs.remove_file(&temp).map_err(|e| io("clean", e))?;
                self.fs
                    .write_new(&temp, &it.fin)
                    .map_err(|e| io("stage", e))?;
                created.push(temp.clone());
            }
            let backup = match &it.prior {
                Some(prior) => {
                    let b = self.dir.join(format!(".fairlead/tx/{txid}/{n}.prior"));
                    if let Some(bd) = b.parent() {
                        self.fs.create_dir_all(bd).map_err(|e| io("mkdir", e))?;
                    }
                    self.fs.remove_file(&b).map_err(|e| io("clean", e))?;
                    self.fs.write_new(&b, prior).map_err(|e| io("backup", e))?;
                    created.push(b.clone());
                    Some(rel(&self.dir, &b))
                }
                None => None,
            };
            entries.push(JournalEntry {
                path: it.path.as_str().to_owned(),
                temp: if it.delete {
                    String::new()
                } else {
                    rel(&self.dir, &temp)
                },
                backup,
                prior: it.prior.as_ref().map(|p| hex(&hash_raw(p).0)),
                new_hash: if it.delete {
                    DELETED.to_owned()
                } else {
                    hex(&hash_raw(&it.fin).0)
                },
                exec: it.fin.starts_with(b"#!"),
            });
        }
        Ok(JournalDoc {
            tx: txid.to_owned(),
            entries,
            kind: Some(format!("{:?}", self.set.kind)),
        })
    }

    fn discard(&self, created: &[Utf8PathBuf]) {
        for p in created {
            let _ = self.fs.remove_file(p);
        }
    }
}

/// Path order, except that the commit point goes last (R-VER-050 step 6, D-14).
pub(crate) fn apply_order(set: &WriteSet) -> Vec<RepoPath> {
    let last = set.commit_point.as_ref();
    set.writes
        .keys()
        .filter(|p| Some(*p) != last)
        .chain(set.writes.keys().filter(|p| Some(*p) == last))
        .cloned()
        .collect()
}

/// `new_hash` of a delete entry: applied once the file is gone.
const DELETED: &str = "-";

pub(crate) fn applied(fs: &dyn Fs, dir: &Utf8Path, e: &JournalEntry) -> Result<bool, Error> {
    let cur = fs
        .read(&dir.join(&e.path))
        .map_err(|x| io_err("read", &e.path, &x))?;
    if e.new_hash == DELETED {
        return Ok(cur.is_none());
    }
    Ok(cur.is_some_and(|b| hex(&hash_raw(&b).0) == e.new_hash))
}

/// Roll forward: rename every staged temp over its target (skipping entries already applied).
pub(crate) fn apply(fs: &dyn Fs, dir: &Utf8Path, doc: &JournalDoc) -> Result<(), Error> {
    for e in &doc.entries {
        if applied(fs, dir, e)? {
            continue;
        }
        let target = dir.join(&e.path);
        if e.new_hash == DELETED {
            fs.remove_file(&target)
                .map_err(|x| io_err("remove", &e.path, &x))?;
            continue;
        }
        rename_with_retry(fs, &dir.join(&e.temp), &target)
            .map_err(|x| io_err("rename", &e.path, &x))?;
        if e.exec {
            fs.set_executable(&target)
                .map_err(|x| io_err("chmod", &e.path, &x))?;
        }
    }
    Ok(())
}

/// Restore applied entries from their backups (or delete files that did not exist), then clean up.
pub(crate) fn rollback(fs: &dyn Fs, dir: &Utf8Path, doc: &JournalDoc) -> Result<(), Error> {
    for e in &doc.entries {
        if !applied(fs, dir, e)? {
            continue;
        }
        let target = dir.join(&e.path);
        match &e.backup {
            Some(b) => {
                let prior = fs
                    .read(&dir.join(b))
                    .map_err(|x| io_err("read", b, &x))?
                    .ok_or_else(|| Error::Internal(format!("backup of {} is missing", e.path)))?;
                atomic_replace(fs, &target, &prior, "rb")
                    .map_err(|x| io_err("restore", &e.path, &x))?;
            }
            None => fs
                .remove_file(&target)
                .map_err(|x| io_err("remove", &e.path, &x))?,
        }
    }
    cleanup(fs, dir, doc);
    Ok(())
}

pub(crate) fn cleanup(fs: &dyn Fs, dir: &Utf8Path, doc: &JournalDoc) {
    for e in &doc.entries {
        if !e.temp.is_empty() {
            let _ = fs.remove_file(&dir.join(&e.temp));
        }
        if let Some(b) = &e.backup {
            let _ = fs.remove_file(&dir.join(b));
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::error::RefusalKind;
    use crate::write::testutil::*;
    use crate::write::{WriteKind, WriteSetKind};

    struct Env {
        d: tempfile::TempDir,
        root: Root,
        probe: FakeProbe,
    }
    fn env() -> Env {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        Env {
            d,
            root,
            probe: FakeProbe::new("h"),
        }
    }
    fn set(items: &[(&str, WriteKind, &str)]) -> WriteSet {
        let mut s = WriteSet::new(WriteSetKind::New);
        for (p, k, b) in items {
            s.add(pw(p, k.clone(), b)).unwrap();
        }
        s
    }
    fn read(e: &Env, p: &str) -> String {
        std::fs::read_to_string(e.d.path().join(p)).unwrap()
    }
    fn put(e: &Env, p: &str, t: &str) {
        let full = e.d.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, t).unwrap();
    }
    fn leftovers(e: &Env) -> Vec<String> {
        tree(e.d.path())
            .into_iter()
            .filter(|f| f.ends_with(".tmp") || f.contains("sync.journal") || f.contains(".prior"))
            .collect()
    }
    const C: WriteKind = WriteKind::Create;

    #[test]
    fn r_ver_050__the_commit_point_is_applied_last() {
        let mut s = set(&[
            ("fairlead/versions/0.2.0.md", C, "v\n"),
            ("zz/fairlead/docs/a.md", C, "a\n"),
            ("apps/fairlead/docs/b.md", C, "b\n"),
        ]);
        s.commit_point = Some(RepoPath::new("fairlead/versions/0.2.0.md").unwrap());
        let order: Vec<String> = apply_order(&s)
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(
            order,
            [
                "apps/fairlead/docs/b.md",
                "zz/fairlead/docs/a.md",
                "fairlead/versions/0.2.0.md"
            ]
        );
    }

    #[test]
    fn r_edn_032__interrupted_apply_resumes() {
        use crate::classify::FileClass;
        let e = env();
        put(&e, "fairlead/control.toml", "[fairlead]\nedition = 1\n");
        put(
            &e,
            "fairlead/gen/members",
            "# scope root edition=1 body=x\n",
        );
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let mut s = WriteSet::new(WriteSetKind::Upgrade);
        for (p, b, class) in [
            (
                "fairlead/gen/members",
                "# scope root ed=2 body=x\n",
                FileClass::Generated,
            ),
            (
                "fairlead/control.toml",
                "[fairlead]\nedition = 2\n",
                FileClass::Authored,
            ),
        ] {
            s.add(crate::write::PlannedWrite {
                path: RepoPath::new(p).unwrap(),
                kind: WriteKind::Replace,
                bytes: b.as_bytes().to_vec(),
                class,
                expect_prior: None,
            })
            .unwrap();
        }
        s.commit_point = Some(RepoPath::new("fairlead/control.toml").unwrap());
        // renames: 1 journal, 2 the generated file, 3 the control file (last) fails every attempt
        let fs = FailingFs::new().renames_fail(3, 8);
        let failed = Transaction::begin_with(&e.root, &lock, s.clone(), &fs)
            .unwrap()
            .commit();
        assert!(failed.is_err());
        assert_eq!(
            read(&e, "fairlead/control.toml"),
            "[fairlead]\nedition = 1\n",
            "never half-applied: the control file is the commit point"
        );
        let resumed = Transaction::begin(&e.root, &lock, s.clone())
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(resumed.written.len(), 2);
        assert_eq!(
            read(&e, "fairlead/control.toml"),
            "[fairlead]\nedition = 2\n"
        );
        let again = Transaction::begin(&e.root, &lock, s)
            .unwrap()
            .commit()
            .unwrap();
        assert!(again.written.is_empty(), "a second apply is a no-op");
    }

    #[test]
    fn r_ver_041__delete_and_create_rename_a_doc_and_roll_back_together() {
        let draft = "fairlead/docs/changes/d-00000001-x.md";
        let sealed = "fairlead/docs/changes/0001-x.md";
        let e = env();
        put(&e, draft, "draft\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let s = set(&[
            (sealed, C, "sealed\n"),
            (draft, WriteKind::Delete, ""),
            ("fairlead/docs/changes/z.md", C, "z\n"),
        ]);
        // renames: 1 journal, 2 the create, then the delete (no rename), 3 the last create fails
        let fs = FailingFs::new().renames_fail(3, 8); // all six attempts of rename 3
        let failed = Transaction::begin_with(&e.root, &lock, s.clone(), &fs)
            .unwrap()
            .commit();
        assert!(failed.is_err());
        assert_eq!(read(&e, draft), "draft\n", "the deleted file is restored");
        assert!(!e.d.path().join(sealed).exists());
        let r = Transaction::begin(&e.root, &lock, s.clone())
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(r.written.len(), 3);
        assert!(!e.d.path().join(draft).exists());
        assert_eq!(read(&e, sealed), "sealed\n");
        assert!(leftovers(&e).is_empty(), "{:?}", leftovers(&e));
        let again = Transaction::begin(&e.root, &lock, s)
            .unwrap()
            .commit()
            .unwrap();
        assert!(again.written.is_empty(), "a rerun is a no-op");
    }

    #[test]
    fn r_arch_143__commit_writes_files_and_a_rerun_reports_everything_unchanged() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let s = set(&[
            ("fairlead/docs/adr/a.md", C, "a\n"),
            ("fairlead/docs/adr/b.md", C, "b\n"),
        ]);
        let r = Transaction::begin(&e.root, &lock, s.clone())
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(r.written.len(), 2);
        assert_eq!(read(&e, "fairlead/docs/adr/b.md"), "b\n");
        let again = Transaction::begin(&e.root, &lock, s)
            .unwrap()
            .commit()
            .unwrap();
        assert!(again.written.is_empty() && again.unchanged.len() == 2);
        assert!(leftovers(&e).is_empty());
    }

    #[test]
    fn r_cli_028__create_never_overwrites_a_different_existing_file() {
        let e = env();
        put(&e, "fairlead/docs/adr/a.md", "mine\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        match Transaction::begin(
            &e.root,
            &lock,
            set(&[("fairlead/docs/adr/a.md", C, "theirs\n")]),
        ) {
            Err(Error::Refused(f)) => assert_eq!(f.kind, RefusalKind::NoClobber),
            other => panic!("{:?}", other.map(|_| ())),
        }
        assert_eq!(read(&e, "fairlead/docs/adr/a.md"), "mine\n");
    }

    #[test]
    fn r_arch_145__stage_failure_leaves_no_partial_state() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let fs = FailingFs::new().write_fails_on(2);
        let s = set(&[
            ("fairlead/docs/adr/a.md", C, "a\n"),
            ("fairlead/docs/adr/b.md", C, "b\n"),
            ("fairlead/docs/adr/c.md", C, "c\n"),
        ]);
        let r = Transaction::begin_with(&e.root, &lock, s, &fs)
            .unwrap()
            .commit();
        assert!(r.is_err());
        assert_eq!(
            tree(e.d.path()),
            [".fairlead/lock"],
            "no target, no temp, no journal"
        );
    }

    #[test]
    fn r_cli_007__apply_failure_is_rolled_back_to_the_prior_tree() {
        let e = env();
        put(&e, "fairlead/docs/adr/a.md", "old a\n");
        put(&e, "fairlead/docs/adr/b.md", "old b\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let fs = FailingFs::new().renames_fail(2, 7); // second logical rename fails all 6 attempts
        let s = set(&[
            ("fairlead/docs/adr/a.md", WriteKind::Replace, "new a\n"),
            ("fairlead/docs/adr/b.md", WriteKind::Replace, "new b\n"),
        ]);
        assert!(
            Transaction::begin_with(&e.root, &lock, s, &fs)
                .unwrap()
                .commit()
                .is_err()
        );
        assert_eq!(
            (
                read(&e, "fairlead/docs/adr/a.md"),
                read(&e, "fairlead/docs/adr/b.md")
            ),
            ("old a\n".into(), "old b\n".into())
        );
        assert!(leftovers(&e).is_empty(), "{:?}", leftovers(&e));
    }

    #[test]
    fn r_scope_062__output_is_staged_then_renamed_one_rename_per_file_plus_the_journal() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let fs = FailingFs::new();
        let s = set(&[
            ("fairlead/docs/adr/a.md", C, "a\n"),
            ("fairlead/docs/adr/b.md", C, "b\n"),
        ]);
        Transaction::begin_with(&e.root, &lock, s, &fs)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(
            fs.rename_calls(),
            3,
            "two targets plus the journal, never an in-place write"
        );
        assert!(leftovers(&e).is_empty());
    }

    #[test]
    fn r_arch_145__failed_commit_with_failed_rollback_keeps_the_journal() {
        let e = env();
        put(&e, "fairlead/docs/adr/a.md", "old a\n");
        put(&e, "fairlead/docs/adr/b.md", "old b\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let fs = FailingFs::new().renames_fail(3, usize::MAX); // rename 1 is the journal
        let s = set(&[
            ("fairlead/docs/adr/a.md", WriteKind::Replace, "new a\n"),
            ("fairlead/docs/adr/b.md", WriteKind::Replace, "new b\n"),
        ]);
        assert!(
            Transaction::begin_with(&e.root, &lock, s, &fs)
                .unwrap()
                .commit()
                .is_err()
        );
        assert!(e.d.path().join(".fairlead/sync.journal").exists());
        assert_eq!(
            (
                read(&e, "fairlead/docs/adr/a.md"),
                read(&e, "fairlead/docs/adr/b.md")
            ),
            ("new a\n".into(), "old b\n".into())
        );
    }

    #[test]
    fn r_cli_016__append_block_is_spliced_against_the_current_file_with_its_eol() {
        let e = env();
        put(&e, "AGENTS.md", "# T\r\ntext\r\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let mut s = WriteSet::new(WriteSetKind::Init);
        s.add(pw(
            "AGENTS.md",
            WriteKind::AppendBlock {
                marker: "fairlead".into(),
            },
            "<!-- BEGIN fairlead -->\nx\n<!-- END fairlead -->\n",
        ))
        .unwrap();
        Transaction::begin(&e.root, &lock, s)
            .unwrap()
            .commit()
            .unwrap();
        assert_eq!(
            read(&e, "AGENTS.md"),
            "# T\r\ntext\r\n<!-- BEGIN fairlead -->\r\nx\r\n<!-- END fairlead -->\r\n"
        );
    }

    #[test]
    fn r_cli_007__a_stale_expect_prior_refuses_before_anything_is_written() {
        let e = env();
        put(&e, "fairlead/docs/adr/a.md", "edited meanwhile\n");
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let mut s = WriteSet::new(WriteSetKind::New);
        let mut w = pw("fairlead/docs/adr/a.md", WriteKind::Replace, "new\n");
        w.expect_prior = Some(crate::hash::hash_normalised(b"what the planner saw\n"));
        s.add(w).unwrap();
        assert!(
            matches!(Transaction::begin(&e.root, &lock, s), Err(Error::Refused(f)) if f.kind == RefusalKind::NoClobber)
        );
    }

    #[test]
    fn r_arch_112__executable_bit_follows_a_shebang_only_on_unix() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        let mut s = WriteSet::new(WriteSetKind::Init);
        s.add(pw(
            ".git/hooks/pre-commit",
            WriteKind::Create,
            "#!/bin/sh\necho\n",
        ))
        .unwrap();
        Transaction::begin(&e.root, &lock, s)
            .unwrap()
            .commit()
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(e.d.path().join(".git/hooks/pre-commit"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o111,
                0o111
            );
        }
    }

    fn interrupted(e: &Env, lock: &WriterLock<'_>) {
        put(e, "fairlead/docs/adr/a.md", "old a\n");
        put(e, "fairlead/docs/adr/b.md", "old b\n");
        let fs = FailingFs::new().renames_fail(3, usize::MAX); // rename 1 is the journal
        let s = set(&[
            ("fairlead/docs/adr/a.md", WriteKind::Replace, "new a\n"),
            ("fairlead/docs/adr/b.md", WriteKind::Replace, "new b\n"),
        ]);
        assert!(
            Transaction::begin_with(&e.root, lock, s, &fs)
                .unwrap()
                .commit()
                .is_err()
        );
    }

    #[test]
    fn r_ver_051__abort_rolls_an_interrupted_commit_back() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        interrupted(&e, &lock);
        assert_eq!(
            Transaction::abort(&e.root, &lock).unwrap(),
            Recovery::RolledBack
        );
        assert_eq!(
            (
                read(&e, "fairlead/docs/adr/a.md"),
                read(&e, "fairlead/docs/adr/b.md")
            ),
            ("old a\n".into(), "old b\n".into())
        );
        assert!(leftovers(&e).is_empty(), "{:?}", leftovers(&e));
        assert_eq!(
            Transaction::abort(&e.root, &lock).unwrap(),
            Recovery::Clean,
            "nothing left to abort"
        );
    }

    #[test]
    fn r_arch_143__recover_resumes_an_interrupted_commit_and_the_rerun_is_a_no_op() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        interrupted(&e, &lock);
        assert_eq!(
            Transaction::recover(&e.root, &lock).unwrap(),
            Recovery::Resumed
        );
        assert_eq!(
            (
                read(&e, "fairlead/docs/adr/a.md"),
                read(&e, "fairlead/docs/adr/b.md")
            ),
            ("new a\n".into(), "new b\n".into())
        );
        assert!(leftovers(&e).is_empty());
        let s = set(&[
            ("fairlead/docs/adr/a.md", WriteKind::Replace, "new a\n"),
            ("fairlead/docs/adr/b.md", WriteKind::Replace, "new b\n"),
        ]);
        assert!(
            Transaction::begin(&e.root, &lock, s)
                .unwrap()
                .commit()
                .unwrap()
                .written
                .is_empty()
        );
    }

    #[test]
    fn r_cli_007__recover_rolls_back_when_a_staged_file_vanished() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        interrupted(&e, &lock);
        let tmp: Vec<String> = tree(e.d.path())
            .into_iter()
            .filter(|f| f.ends_with(".tmp"))
            .collect();
        assert_eq!(tmp.len(), 1, "only b's temp is still staged");
        std::fs::remove_file(e.d.path().join(&tmp[0])).unwrap();
        assert_eq!(
            Transaction::recover(&e.root, &lock).unwrap(),
            Recovery::RolledBack
        );
        assert_eq!(
            (
                read(&e, "fairlead/docs/adr/a.md"),
                read(&e, "fairlead/docs/adr/b.md")
            ),
            ("old a\n".into(), "old b\n".into())
        );
        assert!(leftovers(&e).is_empty());
    }

    #[test]
    fn r_cli_007__recover_with_no_journal_is_clean() {
        let e = env();
        let lock = WriterLock::acquire(&e.root, &e.probe).unwrap();
        assert_eq!(
            Transaction::recover(&e.root, &lock).unwrap(),
            Recovery::Clean
        );
    }
}
