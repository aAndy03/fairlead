//! Writer lock (R-CLI-008, R-ARCH-146).
use super::{io_err, refuse};
use crate::discover::Root;
use crate::error::{Error, RefusalKind};
use camino::Utf8PathBuf;
use std::io::Write;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LivenessAnswer {
    Dead,
    Alive,
    OtherHost,
    Unknown,
}

pub trait PidLiveness {
    fn is_alive(&self, pid: u32, host: &str) -> LivenessAnswer;
    /// This machine's name as written into the lock.
    fn local_host(&self) -> String {
        "localhost".into()
    }
    /// Opaque OS-reported start marker of `pid` (never a clock reading of ours).
    fn start_token(&self, _pid: u32) -> Option<String> {
        None
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Record {
    pid: u32,
    host: String,
    start: String,
}

impl Record {
    fn render(&self) -> String {
        format!(
            "pid={}\nhost={}\nstart={}\n",
            self.pid, self.host, self.start
        )
    }
    fn parse(text: &str) -> Option<Record> {
        let (mut pid, mut host, mut start) = (None, None, None);
        for l in text.lines() {
            match l.split_once('=')? {
                ("pid", v) => pid = v.parse().ok(),
                ("host", v) => host = Some(v.to_owned()),
                ("start", v) => start = Some(v.to_owned()),
                _ => {}
            }
        }
        Some(Record {
            pid: pid?,
            host: host?,
            start: start?,
        })
    }
}

pub struct WriterLock<'a> {
    path: Utf8PathBuf,
    me: Record,
    #[allow(dead_code)] // kept so the lock cannot outlive the probe that vouches for it
    probe: &'a dyn PidLiveness,
    notices: Vec<String>,
}

fn read_record(path: &Utf8PathBuf) -> Option<Record> {
    Record::parse(&std::fs::read_to_string(path.as_std_path()).ok()?)
}

impl<'a> WriterLock<'a> {
    pub fn notices(&self) -> &[String] {
        &self.notices
    }

    pub fn acquire(root: &Root, probe: &'a dyn PidLiveness) -> Result<WriterLock<'a>, Error> {
        let dir = root.dir.join(".fairlead");
        std::fs::create_dir_all(dir.as_std_path())
            .map_err(|e| io_err("mkdir", dir.as_str(), &e))?;
        Self::acquire_at(dir.join("lock"), ".fairlead/lock", probe)
    }

    /// R-GIT-052, D-52: the allocation lock `fairlead-alloc.lock` in the git common directory, shared
    /// by every worktree of the repository; `seal`, `move` and `upgrade` hold it besides the
    /// worktree lock. `None` without git: then only the worktree lock applies.
    pub fn acquire_alloc(
        root: &Root,
        probe: &'a dyn PidLiveness,
    ) -> Result<Option<WriterLock<'a>>, Error> {
        let Some(common) = crate::git::GitRepo::open(root).map(|r| r.common_dir()) else {
            return Ok(None);
        };
        Self::acquire_at(
            common.join("fairlead-alloc.lock"),
            "fairlead-alloc.lock",
            probe,
        )
        .map(Some)
    }

    fn acquire_at(
        path: Utf8PathBuf,
        label: &str,
        probe: &'a dyn PidLiveness,
    ) -> Result<WriterLock<'a>, Error> {
        let pid = std::process::id();
        let me = Record {
            pid,
            host: probe.local_host(),
            start: probe.start_token(pid).unwrap_or_else(|| "-".into()),
        };
        let mut notices = Vec::new();
        for _ in 0..3 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.as_std_path())
            {
                Ok(mut f) => {
                    f.write_all(me.render().as_bytes())
                        .and_then(|()| f.sync_all())
                        .map_err(|e| io_err("lock", path.as_str(), &e))?;
                    return Ok(WriterLock {
                        path,
                        me,
                        probe,
                        notices,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let Some(h) = read_record(&path) else {
                        return Err(refuse(
                            RefusalKind::LockHeld,
                            format!("the writer lock {label} is unreadable"),
                            format!("remove {label} if no fairlead command is running"),
                        ));
                    };
                    let mut answer = probe.is_alive(h.pid, &h.host);
                    if answer == LivenessAnswer::Alive
                        && h.start != "-"
                        && probe.start_token(h.pid).is_some_and(|t| t != h.start)
                    {
                        answer = LivenessAnswer::Dead; // pid reused by another process
                    }
                    if answer == LivenessAnswer::Dead {
                        notices.push(format!(
                            "took over stale writer lock of pid {} on {}",
                            h.pid, h.host
                        ));
                        let _ = std::fs::remove_file(path.as_std_path());
                        continue;
                    }
                    let hint = if answer == LivenessAnswer::OtherHost {
                        format!(
                            "the lock was taken on another host; shared mounts are unsupported; remove {label} if you are sure"
                        )
                    } else if label == ".fairlead/lock" {
                        "wait for the other fairlead command to finish".to_owned()
                    } else {
                        "another worktree of this repository is sealing, moving or upgrading; wait for it".to_owned()
                    };
                    return Err(refuse(
                        RefusalKind::LockHeld,
                        format!(
                            "another fairlead writer holds {label} (pid {} on {})",
                            h.pid, h.host
                        ),
                        hint,
                    ));
                }
                Err(e) => return Err(io_err("lock", path.as_str(), &e)),
            }
        }
        Err(refuse(
            RefusalKind::LockHeld,
            "the writer lock is contended",
            "re-run in a moment",
        ))
    }
}

impl Drop for WriterLock<'_> {
    fn drop(&mut self) {
        if read_record(&self.path).as_ref() == Some(&self.me) {
            let _ = std::fs::remove_file(self.path.as_std_path());
        }
    }
}

/// Production probe. Verify: `sysinfo` (UNVERIFIED, R-ARCH-120): MSRV 1.88, licence, and the calls
/// `System::host_name()`, `System::refresh_processes(ProcessesToUpdate::Some(&[pid]), true)`,
/// `System::process(pid)`, `Process::start_time()`. If it is rejected, replace this one type; the trait stays.
pub struct SysProbe;

impl PidLiveness for SysProbe {
    fn is_alive(&self, pid: u32, host: &str) -> LivenessAnswer {
        if host != self.local_host() {
            return LivenessAnswer::OtherHost;
        }
        let p = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        if sys.process(p).is_some() {
            LivenessAnswer::Alive
        } else {
            LivenessAnswer::Dead
        }
    }
    fn local_host(&self) -> String {
        sysinfo::System::host_name().unwrap_or_else(|| "unknown-host".into())
    }
    fn start_token(&self, pid: u32) -> Option<String> {
        let p = sysinfo::Pid::from_u32(pid);
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        sys.process(p).map(|x| x.start_time().to_string())
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::error::{Error, RefusalKind};
    use crate::write::testutil::*;

    fn held(r: Result<WriterLock<'_>, Error>) -> crate::error::Refusal {
        match r {
            Err(Error::Refused(f)) => f,
            Err(e) => panic!("{e:?}"),
            Ok(_) => panic!("lock was granted"),
        }
    }
    fn plant(root: &crate::discover::Root, pid: u32, host: &str, start: &str) {
        std::fs::create_dir_all(root.dir.join(".fairlead")).unwrap();
        std::fs::write(
            root.dir.join(".fairlead/lock"),
            format!("pid={pid}\nhost={host}\nstart={start}\n"),
        )
        .unwrap();
    }

    #[test]
    fn r_arch_146__lock_file_holds_pid_host_and_start_and_is_released_on_drop() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        let mut probe = FakeProbe::new("h1");
        probe.tokens.insert(std::process::id(), "t-1".into());
        {
            let _l = WriterLock::acquire(&root, &probe).unwrap();
            let text = std::fs::read_to_string(d.path().join(".fairlead/lock")).unwrap();
            assert_eq!(
                text,
                format!("pid={}\nhost=h1\nstart=t-1\n", std::process::id())
            );
        }
        assert!(!d.path().join(".fairlead/lock").exists());
    }

    #[test]
    fn r_cli_008__a_live_holder_refuses_with_exit_4_kind_and_names_the_holder() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        let mut probe = FakeProbe::new("h1");
        probe.answers.insert(4242, LivenessAnswer::Alive);
        plant(&root, 4242, "h1", "-");
        let f = held(WriterLock::acquire(&root, &probe));
        assert_eq!(f.kind, RefusalKind::LockHeld);
        assert!(f.message.contains("4242") && f.message.contains("h1"));
        assert!(
            d.path().join(".fairlead/lock").exists(),
            "a live lock is never broken"
        );
    }

    #[test]
    fn r_arch_142__a_dead_same_host_lock_is_taken_over_with_a_notice() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        plant(&root, 4242, "h1", "-");
        let probe = FakeProbe::new("h1"); // pid 4242 unknown => Dead
        let l = WriterLock::acquire(&root, &probe).unwrap();
        assert_eq!(l.notices().len(), 1);
        assert!(l.notices()[0].contains("4242"));
    }

    #[test]
    fn r_arch_142__pid_reuse_is_detected_through_the_start_token() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        let mut probe = FakeProbe::new("h1");
        probe.answers.insert(4242, LivenessAnswer::Alive);
        probe.tokens.insert(4242, "new-process".into());
        plant(&root, 4242, "h1", "old-process");
        assert!(
            WriterLock::acquire(&root, &probe).is_ok(),
            "same pid, different start => stale"
        );
    }

    #[test]
    fn r_arch_146__a_lock_from_another_host_is_never_broken() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        plant(&root, 1, "other-host", "-");
        let f = held(WriterLock::acquire(&root, &FakeProbe::new("h1")));
        assert_eq!(f.kind, RefusalKind::LockHeld);
        assert!(f.hint.contains("shared"), "{}", f.hint);
    }

    #[test]
    fn r_port_015__a_second_writer_in_the_same_worktree_is_refused_and_the_holder_named() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        let mut probe = FakeProbe::new("h1");
        probe
            .answers
            .insert(std::process::id(), LivenessAnswer::Alive);
        let _first = WriterLock::acquire(&root, &probe).unwrap();
        let f = held(WriterLock::acquire(&root, &probe));
        assert!(f.message.contains(&std::process::id().to_string()));
    }

    #[test]
    fn r_cli_008__an_unreadable_lock_fails_closed() {
        let d = tempfile::tempdir().unwrap();
        let root = test_root(d.path());
        std::fs::create_dir_all(d.path().join(".fairlead")).unwrap();
        std::fs::write(d.path().join(".fairlead/lock"), "garbage").unwrap();
        assert_eq!(
            held(WriterLock::acquire(&root, &FakeProbe::new("h1"))).kind,
            RefusalKind::LockHeld
        );
    }

    #[test]
    fn r_file_029__each_worktree_has_its_own_lock() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let probe = FakeProbe::new("h1");
        let _la = WriterLock::acquire(&test_root(a.path()), &probe).unwrap();
        let _lb = WriterLock::acquire(&test_root(b.path()), &probe).unwrap();
    }

    #[test]
    fn r_cli_008__the_real_probe_sees_this_process_alive() {
        let p = SysProbe;
        assert_eq!(
            p.is_alive(std::process::id(), &p.local_host()),
            LivenessAnswer::Alive
        );
        assert_eq!(
            p.is_alive(std::process::id(), "definitely-not-this-host"),
            LivenessAnswer::OtherHost
        );
    }
}
