//! The ONLY module that names the git library (R-ARCH-005). Read-only in 0.1: no commit, stage or
//! push API exists (R-CLI-010). Actions that must run hooks or sign are 0.2 and use the user's `git`
//! executable from this same module.
use crate::discover::Root;
use crate::error::Error;
use crate::facts::ChangedSet;
use crate::path::RepoPath;
use std::collections::BTreeSet;

/// `GIT_INDEX_ENTRY_SKIP_WORKTREE` in the extended index flags.
const SKIP_WORKTREE: u16 = 1 << 14;

pub fn is_skip_worktree(flags_extended: u16) -> bool {
    flags_extended & SKIP_WORKTREE != 0
}

/// One repository handle; not `Sync`. Open one per worker (R-ARCH-081).
pub struct GitRepo {
    repo: git2::Repository,
    /// The root relative to the work tree, as a `dir/` prefix; empty when they coincide.
    prefix: String,
}

/// Runs `git <args>` in the root directory, in the foreground (D-26). A missing executable is a
/// usage error naming the flag (R-GIT-076); a failing command returns its stderr.
fn run_git(root: &Root, args: &[&str]) -> Result<String, Error> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root.dir.as_std_path())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Usage(
                    "git is not on the PATH; --commit and --tag need the git executable".into(),
                )
            } else {
                Error::Internal(format!("cannot run git: {e}"))
            }
        })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Error::Internal(format!(
            "git {} failed: {}",
            args.first().copied().unwrap_or(""),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

/// R-GIT-078: a rename that changes only letter case is a move of one file, not a deletion
/// plus an addition; the changed set keeps the new spelling only.
pub fn case_only_rename(old: &str, new: &str) -> bool {
    old != new && old.to_lowercase() == new.to_lowercase()
}

fn cannot_determine(what: &str, e: &git2::Error) -> Error {
    Error::CannotDetermine(format!("{what}: {e}"))
}

impl GitRepo {
    /// The common git directory, shared by every worktree (R-GIT-052).
    pub fn common_dir(&self) -> camino::Utf8PathBuf {
        camino::Utf8PathBuf::from_path_buf(self.repo.commondir().to_path_buf())
            .unwrap_or_else(|p| camino::Utf8PathBuf::from(p.to_string_lossy().into_owned()))
    }

    /// Opened from the root directory, never from the working directory (R-GIT-071).
    pub fn open(root: &Root) -> Option<GitRepo> {
        let repo = git2::Repository::open_ext(
            root.dir.as_std_path(),
            git2::RepositoryOpenFlags::empty(),
            std::iter::empty::<&std::ffi::OsStr>(),
        )
        .ok()?;
        let workdir = dunce::canonicalize(repo.workdir()?).ok()?;
        let base = dunce::canonicalize(root.dir.as_std_path()).ok()?;
        let rel = base
            .strip_prefix(&workdir)
            .ok()?
            .to_str()?
            .replace('\\', "/");
        let prefix = if rel.is_empty() {
            String::new()
        } else {
            format!("{rel}/")
        };
        Some(GitRepo { repo, prefix })
    }

    fn add(&self, set: &mut BTreeSet<RepoPath>, git_path: &str) {
        if let Some(rest) = git_path.strip_prefix(&self.prefix)
            && let Ok(p) = RepoPath::new(rest)
        {
            set.insert(p);
        }
    }

    /// R-GIT-003, R-GIT-076 (D-48): `seal --commit` makes exactly one commit of exactly `paths`
    /// with the `Fairlead-Version` trailer, through the user's `git` executable in the foreground
    /// so hooks and signing run. The only subprocess fairlead starts, beside `verify-commit`.
    /// R-LC-040: `git verify-commit <rev>` as an explicit foreground subprocess; `true` when the
    /// commit carries a good signature. Run only by `check --ci` with `authority.signed-records`.
    pub fn verify_commit(root: &Root, rev: &str) -> Result<bool, Error> {
        let out = std::process::Command::new("git")
            .args(["verify-commit", rev])
            .current_dir(root.dir.as_std_path())
            .output()
            .map_err(|e| Error::Internal(format!("cannot run git verify-commit: {e}")))?;
        Ok(out.status.success())
    }

    pub fn commit_paths(root: &Root, paths: &[RepoPath], version: &str) -> Result<(), Error> {
        // A path the seal removed that git never tracked (a draft doc sealed before its first
        // commit, renamed to its ordinal) has nothing to commit, and `git add` would refuse it.
        let mut ls = vec!["ls-files", "--"];
        ls.extend(paths.iter().map(RepoPath::as_str));
        let tracked: std::collections::BTreeSet<String> =
            run_git(root, &ls)?.lines().map(str::to_owned).collect();
        let paths: Vec<RepoPath> = paths
            .iter()
            .filter(|p| {
                root.dir.join(p.as_str()).as_std_path().exists() || tracked.contains(p.as_str())
            })
            .cloned()
            .collect();
        let mut add = vec!["add", "-A", "--"];
        add.extend(paths.iter().map(RepoPath::as_str));
        run_git(root, &add)?;
        let subject = format!("seal {version}");
        let trailer = format!("Fairlead-Version: {version}");
        let mut commit = vec![
            "commit", "-q", "-m", &subject, "-m", &trailer, "--only", "--",
        ];
        commit.extend(paths.iter().map(RepoPath::as_str));
        run_git(root, &commit).map(|_| ())
    }

    /// `seal --tag`: an annotated tag `fairlead/<version>` on the commit just made (R-VER-050 step 7).
    pub fn tag(root: &Root, version: &str) -> Result<(), Error> {
        let name = format!("fairlead/{version}");
        let msg = format!("fairlead {version}");
        run_git(root, &["tag", "-a", &name, "-m", &msg]).map(|_| ())
    }

    /// 0.2.2-T4: whether the tag `fairlead/<version>` exists (a reseal leaves it on the undone
    /// seal). False when git cannot answer.
    pub fn tag_exists(root: &Root, version: &str) -> bool {
        let r = format!("refs/tags/fairlead/{version}");
        run_git(root, &["rev-parse", "-q", "--verify", &r]).is_ok()
    }

    /// R-VER-014: the newest commit (first-parent walk from HEAD, at most 5,000 commits) whose
    /// tree changes `path`, as `(short id, YYYY-MM-DD)`. `None` without history or past the cap.
    pub fn last_commit_touching(&self, path: &RepoPath) -> Option<(String, String)> {
        let rel = format!("{}{}", self.prefix, path.as_str());
        let blob = |c: &git2::Commit<'_>| {
            c.tree()
                .ok()
                .and_then(|t| t.get_path(std::path::Path::new(&rel)).ok())
                .map(|e| e.id())
        };
        let mut commit = self.repo.head().ok()?.peel_to_commit().ok()?;
        for _ in 0..5_000 {
            let here = blob(&commit);
            let parent = commit.parent(0).ok();
            let before = parent.as_ref().and_then(blob);
            if here != before {
                here?; // deleted at HEAD's side: no current version
                let id = commit.id().to_string();
                let secs = commit.time().seconds();
                let date = jiff::Timestamp::from_second(secs)
                    .ok()?
                    .to_zoned(jiff::tz::TimeZone::UTC)
                    .date()
                    .to_string();
                return Some((id.get(..10).unwrap_or(&id).to_owned(), date));
            }
            commit = parent?;
        }
        None
    }

    /// R-VER-033: the newest first-parent commit that added `path` (the default coverage base is
    /// the commit that introduced `versions/<S>.md`). `Ok(None)` when history does not reach it,
    /// which in a shallow clone means coverage is unavailable (R-GIT-040).
    pub fn commit_adding(&self, path: &str) -> Result<Option<String>, Error> {
        let rel = format!("{}{path}", self.prefix);
        let has = |c: &git2::Commit<'_>| {
            c.tree()
                .ok()
                .is_some_and(|t| t.get_path(std::path::Path::new(&rel)).is_ok())
        };
        let Ok(head) = self.repo.head().and_then(|h| h.peel_to_commit()) else {
            return Ok(None);
        };
        if !has(&head) {
            // Sealed but not committed yet: everything since HEAD is the change set.
            return Ok(Some(head.id().to_string()));
        }
        let mut commit = head;
        loop {
            match commit.parent(0) {
                Ok(p) if has(&p) => commit = p,
                Ok(_) => return Ok(Some(commit.id().to_string())),
                // Root commit, or the parent is missing from a shallow clone.
                Err(_) if self.repo.is_shallow() => return Ok(None),
                Err(_) => return Ok(Some(commit.id().to_string())),
            }
        }
    }

    /// R-VER-033, R-GIT-075: files that differ between the tree of `base` and the working
    /// directory including the index, untracked files included, ignored files excluded.
    pub fn changed_since(&self, base: &str) -> Result<BTreeSet<RepoPath>, Error> {
        let oid = git2::Oid::from_str(base).map_err(|e| cannot_determine("bad base commit", &e))?;
        let tree = self
            .repo
            .find_commit(oid)
            .and_then(|c| c.tree())
            .map_err(|e| cannot_determine("cannot read the base tree", &e))?;
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        let diff = self
            .repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .map_err(|e| cannot_determine("cannot diff against the base", &e))?;
        let mut set = BTreeSet::new();
        let (mut news, mut olds) = (Vec::new(), Vec::new());
        for delta in diff.deltas() {
            // R-ARCH-076: a file whose content differs only in line endings (`core.autocrlf`
            // rewriting a checkout) has not changed (agent protocol A-2, E2-83).
            if delta.status() == git2::Delta::Modified && self.same_but_eol(&delta) {
                continue;
            }
            if let Some(p) = delta.new_file().path().and_then(|p| p.to_str()) {
                news.push(p.replace('\\', "/"));
            }
            if let Some(p) = delta.old_file().path().and_then(|p| p.to_str()) {
                olds.push(p.replace('\\', "/"));
            }
        }
        for n in &news {
            self.add(&mut set, n);
        }
        // A deleted path whose new spelling differs only in case is the same file, moved.
        for o in olds {
            if !news.iter().any(|n| case_only_rename(&o, n)) {
                self.add(&mut set, &o);
            }
        }
        Ok(set)
    }

    /// Whether a modified file's base blob and its working-tree bytes are equal after BOM and
    /// line-ending normalisation. Unreadable content counts as changed.
    fn same_but_eol(&self, delta: &git2::DiffDelta<'_>) -> bool {
        let Ok(old) = self.repo.find_blob(delta.old_file().id()) else {
            return false;
        };
        let (Some(dir), Some(path)) = (self.repo.workdir(), delta.new_file().path()) else {
            return false;
        };
        let Ok(now) = std::fs::read(dir.join(path)) else {
            return false;
        };
        crate::hash::hash_normalised(old.content()) == crate::hash::hash_normalised(&now)
    }

    /// The commit a revision names (`main`, `HEAD~2`, a hash), as a full hash.
    pub fn resolve(&self, rev: &str) -> Result<String, Error> {
        self.repo
            .revparse_single(rev)
            .and_then(|o| o.peel_to_commit())
            .map(|c| c.id().to_string())
            .map_err(|e| Error::Usage(format!("{rev:?} names no commit: {}", e.message())))
    }

    /// File names directly in `dir` (root-relative) at `rev`; empty when it did not exist.
    pub fn names_at(&self, rev: &str, dir: &str) -> Vec<String> {
        let rel = format!("{}{dir}", self.prefix);
        let Some(tree) = self
            .repo
            .revparse_single(rev)
            .ok()
            .and_then(|o| o.peel_to_tree().ok())
        else {
            return Vec::new();
        };
        let Ok(entry) = tree.get_path(std::path::Path::new(&rel)) else {
            return Vec::new();
        };
        let Some(sub) = entry
            .to_object(&self.repo)
            .ok()
            .and_then(|o| o.into_tree().ok())
        else {
            return Vec::new();
        };
        sub.iter()
            .filter_map(|e| e.name().ok().map(str::to_owned))
            .collect()
    }

    /// Paths staged in the index against HEAD (R-CLI-022 `--staged`).
    pub fn staged_paths(&self) -> Result<BTreeSet<RepoPath>, Error> {
        let head = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let diff = self
            .repo
            .diff_tree_to_index(head.as_ref(), None, None)
            .map_err(|e| cannot_determine("cannot diff the index", &e))?;
        let mut set = BTreeSet::new();
        for d in diff.deltas() {
            for f in [d.old_file(), d.new_file()] {
                if let Some(p) = f.path().and_then(|p| p.to_str()) {
                    self.add(&mut set, &p.replace('\\', "/"));
                }
            }
        }
        Ok(set)
    }

    /// The bytes of `path` (root-relative) at `rev`, if it existed there.
    pub fn file_at(&self, rev: &str, path: &RepoPath) -> Option<Vec<u8>> {
        let rel = format!("{}{}", self.prefix, path.as_str());
        let tree = self.repo.revparse_single(rev).ok()?.peel_to_tree().ok()?;
        let entry = tree.get_path(std::path::Path::new(&rel)).ok()?;
        let blob = entry.to_object(&self.repo).ok()?.peel_to_blob().ok()?;
        Some(blob.content().to_vec())
    }

    /// Added (`true`) and removed (`false`) lines of every file between `rev` and the work tree.
    pub fn changed_lines(&self, rev: &str) -> Result<Vec<(RepoPath, bool, String)>, Error> {
        let base = self.resolve(rev)?;
        let tree = git2::Oid::from_str(&base)
            .and_then(|o| self.repo.find_commit(o))
            .and_then(|c| c.tree())
            .map_err(|e| cannot_determine("cannot read the base tree", &e))?;
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .show_untracked_content(true);
        let diff = self
            .repo
            .diff_tree_to_workdir_with_index(Some(&tree), Some(&mut opts))
            .map_err(|e| cannot_determine("cannot diff against the base", &e))?;
        let mut out = Vec::new();
        diff.foreach(
            &mut |_, _| true,
            None,
            None,
            Some(&mut |delta, _hunk, line| {
                let added = match line.origin() {
                    '+' => true,
                    '-' => false,
                    _ => return true,
                };
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .map(|p| p.replace('\\', "/"));
                if let Some(rest) = path.as_deref().and_then(|p| p.strip_prefix(&self.prefix))
                    && let Ok(rp) = RepoPath::new(rest)
                {
                    out.push((
                        rp,
                        added,
                        String::from_utf8_lossy(line.content())
                            .trim_end()
                            .to_owned(),
                    ));
                }
                true
            }),
        )
        .map_err(|e| cannot_determine("cannot read the diff", &e))?;
        Ok(out)
    }

    /// `(detached, branch name)` of HEAD (R-GIT-065, R-VER-055). An unborn HEAD is a branch.
    pub fn head_state(&self) -> (bool, Option<String>) {
        let detached = self.repo.head_detached().unwrap_or(false);
        let branch = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().ok().map(str::to_owned))
            .or_else(|| {
                // Unborn HEAD: the symbolic target names the branch to be.
                self.repo
                    .find_reference("HEAD")
                    .ok()
                    .and_then(|r| r.symbolic_target().ok().flatten().map(str::to_owned))
                    .map(|t| t.trim_start_matches("refs/heads/").to_owned())
            });
        (detached, branch.filter(|_| !detached))
    }

    /// Paths with merge conflicts in the index (R-GIT-064).
    pub fn unmerged_paths(&self) -> Result<Vec<RepoPath>, Error> {
        let index = self
            .repo
            .index()
            .map_err(|e| cannot_determine("cannot read the git index", &e))?;
        let mut set = BTreeSet::new();
        if index.has_conflicts() {
            let conflicts = index
                .conflicts()
                .map_err(|e| cannot_determine("cannot read index conflicts", &e))?;
            for c in conflicts.flatten() {
                for entry in [c.ancestor, c.our, c.their].into_iter().flatten() {
                    if let Ok(p) = std::str::from_utf8(&entry.path) {
                        self.add(&mut set, p);
                    }
                }
            }
        }
        Ok(set.into_iter().collect())
    }

    /// Working tree plus index against HEAD, untracked non-ignored files included (C-36).
    pub fn changed_files(&self) -> Result<ChangedSet, Error> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .include_unmodified(false)
            .exclude_submodules(true)
            .renames_head_to_index(false)
            .renames_index_to_workdir(false);
        let statuses = self
            .repo
            .statuses(Some(&mut opts))
            .map_err(|e| cannot_determine("cannot read git status", &e))?;
        let mut set = BTreeSet::new();
        for entry in statuses.iter() {
            if let Ok(p) = entry.path() {
                self.add(&mut set, p);
            }
            for delta in [entry.head_to_index(), entry.index_to_workdir()]
                .into_iter()
                .flatten()
            {
                if let Some(old) = delta.old_file().path().and_then(|p| p.to_str()) {
                    self.add(&mut set, &old.replace('\\', "/"));
                }
            }
        }
        Ok(ChangedSet(set))
    }

    /// Files whose index entry has skip-worktree set and that are missing from the work tree,
    /// sorted; empty unless `core.sparseCheckout` is true (R-ARCH-026, R-GIT-007).
    pub fn sparse_absent(&self) -> Result<Vec<RepoPath>, Error> {
        let sparse = self
            .repo
            .config()
            .ok()
            .and_then(|c| c.get_bool("core.sparseCheckout").ok())
            .unwrap_or(false);
        if !sparse {
            return Ok(Vec::new());
        }
        let index = self
            .repo
            .index()
            .map_err(|e| cannot_determine("cannot read the git index", &e))?;
        let workdir = self.repo.workdir().map(std::path::Path::to_path_buf);
        let mut out = BTreeSet::new();
        for entry in index.iter() {
            if !is_skip_worktree(entry.flags_extended) {
                continue;
            }
            let Ok(path) = String::from_utf8(entry.path.clone()) else {
                continue;
            };
            if let Some(wd) = &workdir
                && wd.join(&path).exists()
            {
                continue;
            }
            self.add(&mut out, &path);
        }
        Ok(out.into_iter().collect())
    }

    /// The repository's shallow flag (R-GIT-074).
    pub fn is_shallow(&self) -> bool {
        self.repo.is_shallow()
    }
}

/// Test-only repository builders, so tests in other modules never name the git library.
#[cfg(test)]
pub(crate) mod test_support {
    use camino::Utf8Path;
    use std::collections::BTreeMap;

    pub fn init(dir: &Utf8Path) {
        git2::Repository::init(dir.as_std_path()).expect("init");
    }

    /// Stage everything (respecting ignore rules) and commit with a fixed identity and time.
    pub fn commit_all(dir: &Utf8Path) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let mut index = repo.index().expect("index");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("add_all");
        index.write().expect("write index");
        let tree = repo
            .find_tree(index.write_tree().expect("write tree"))
            .expect("tree");
        let sig =
            git2::Signature::new("t", "t@example.com", &git2::Time::new(0, 0)).expect("signature");
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, "c", &tree, &parents)
            .expect("commit");
    }

    pub fn stage(dir: &Utf8Path, rel: &str) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let mut index = repo.index().expect("index");
        index.add_path(std::path::Path::new(rel)).expect("add_path");
        index.write().expect("write index");
    }

    pub fn corrupt_index(dir: &Utf8Path) {
        std::fs::write(
            dir.join(".git/index").as_std_path(),
            b"garbage, not an index",
        )
        .expect("overwrite index");
    }

    /// Verify: needs index version 3 for the extended flag; see the Verify note on the sparse test.
    pub fn mark_skip_worktree(dir: &Utf8Path, rel: &str) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        repo.config()
            .expect("config")
            .set_bool("core.sparseCheckout", true)
            .expect("set sparse");
        let mut index = repo.index().expect("index");
        index.set_version(3).expect("index v3");
        let mut entry = index.get_path(std::path::Path::new(rel), 0).expect("entry");
        entry.flags |= 1 << 14; // GIT_INDEX_ENTRY_EXTENDED
        entry.flags_extended |= super::SKIP_WORKTREE;
        index.add(&entry).expect("add entry");
        index.write().expect("write index");
    }

    pub fn make_shallow(dir: &Utf8Path) {
        let repo = git2::Repository::open(dir.as_std_path()).expect("open");
        let head = repo
            .head()
            .expect("head")
            .peel_to_commit()
            .expect("commit")
            .id();
        std::fs::write(dir.join(".git/shallow").as_std_path(), format!("{head}\n"))
            .expect("shallow file");
    }

    /// Path to bytes of every file under `.git`.
    pub fn snapshot_git_dir(dir: &Utf8Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.join(".git").into_std_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("read_dir") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.insert(
                        p.to_string_lossy().into_owned(),
                        std::fs::read(&p).expect("read"),
                    );
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod case_tests {
    use super::*;

    #[test]
    fn r_git_078__case_only_rename_is_move() {
        assert!(case_only_rename("Src/Button.tsx", "src/button.tsx"));
        assert!(!case_only_rename("src/a.tsx", "src/a.tsx"));
        assert!(!case_only_rename("src/a.tsx", "src/b.tsx"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_root, utf8_tmp, write_file};

    fn names(c: &ChangedSet) -> Vec<&str> {
        c.0.iter().map(RepoPath::as_str).collect()
    }

    #[test]
    fn r_arch_025__no_repository_means_unavailable() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a.rs", "x");
        assert!(GitRepo::open(&test_root(&t)).is_none());
    }

    #[test]
    fn r_arch_075__changed_set_is_worktree_plus_index_with_untracked_and_without_ignored() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, ".gitignore", "*.log\n");
        for f in ["a.rs", "b.rs", "keep.rs"] {
            write_file(&t, f, "1");
        }
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        std::fs::remove_file(t.join("b.rs").as_std_path()).unwrap();
        write_file(&t, "c.rs", "x");
        write_file(&t, "dir/deep/d.rs", "x");
        write_file(&t, "noise.log", "x");
        write_file(&t, "staged.rs", "x");
        test_support::stage(&t, "staged.rs");
        let g = GitRepo::open(&test_root(&t)).expect("repository");
        let changed = g.changed_files().unwrap();
        assert_eq!(
            names(&changed),
            ["a.rs", "b.rs", "c.rs", "dir/deep/d.rs", "staged.rs"]
        );
    }

    #[test]
    fn r_git_007__a_clean_tree_has_an_empty_changed_set_and_a_fresh_repo_works_without_commits() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        assert!(g.changed_files().unwrap().0.is_empty());
        write_file(&t, "new.rs", "x");
        assert_eq!(
            names(&g.changed_files().unwrap()),
            ["new.rs"],
            "an unborn HEAD still yields the untracked file"
        );
    }

    #[test]
    fn r_git_007__a_root_inside_a_larger_repository_sees_only_its_own_subtree() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "proj/a.rs", "1");
        write_file(&t, "outside.txt", "1");
        test_support::commit_all(&t);
        write_file(&t, "proj/a.rs", "22");
        write_file(&t, "proj/new.rs", "x");
        write_file(&t, "outside.txt", "22");
        let g = GitRepo::open(&test_root(&t.join("proj")))
            .expect("found by searching up from the root");
        assert_eq!(names(&g.changed_files().unwrap()), ["a.rs", "new.rs"]);
    }

    #[test]
    fn r_arch_102__unreadable_git_state_is_cannot_determine_never_an_empty_pass() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        test_support::corrupt_index(&t);
        let g = GitRepo::open(&test_root(&t)).expect("the repository itself still opens");
        let err = g
            .changed_files()
            .expect_err("a corrupt index must not read as 'nothing changed'");
        assert!(matches!(err, Error::CannotDetermine(_)), "{err}");
        assert_eq!(err.class(), crate::error::ErrorClass::Internal);
    }

    #[test]
    fn r_git_007__skip_worktree_flag_is_bit_14_of_the_extended_flags() {
        assert!(is_skip_worktree(1 << 14));
        assert!(is_skip_worktree((1 << 14) | 1));
        assert!(!is_skip_worktree(1 << 13));
        assert!(!is_skip_worktree(0));
    }

    #[test]
    fn r_arch_026__sparse_checkout_reports_absent_files_and_is_silent_otherwise() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "web/fairlead/control.toml", "x");
        write_file(&t, "src/a.rs", "x");
        test_support::commit_all(&t);
        assert!(
            GitRepo::open(&test_root(&t))
                .unwrap()
                .sparse_absent()
                .unwrap()
                .is_empty()
        );
        // Verify: if libgit2 drops the extended flag on write on some build, delete THIS test only;
        // the pure `is_skip_worktree` test above and the empty case remain.
        test_support::mark_skip_worktree(&t, "web/fairlead/control.toml");
        std::fs::remove_dir_all(t.join("web").as_std_path()).unwrap();
        let absent = GitRepo::open(&test_root(&t))
            .unwrap()
            .sparse_absent()
            .unwrap();
        assert_eq!(
            absent,
            vec![RepoPath::new("web/fairlead/control.toml").unwrap()]
        );
    }

    #[test]
    fn r_git_007__a_skip_worktree_file_that_is_present_is_not_absent() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "web/x.rs", "x");
        test_support::commit_all(&t);
        test_support::mark_skip_worktree(&t, "web/x.rs");
        assert!(
            GitRepo::open(&test_root(&t))
                .unwrap()
                .sparse_absent()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn r_git_074__shallow_flag_is_read_from_the_repository() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        assert!(!g.is_shallow());
        test_support::make_shallow(&t);
        assert!(GitRepo::open(&test_root(&t)).unwrap().is_shallow());
    }

    #[test]
    fn r_cli_010__reading_git_state_leaves_the_repository_byte_identical() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        let before = test_support::snapshot_git_dir(&t);
        let g = GitRepo::open(&test_root(&t)).unwrap();
        let _ = g.changed_files().unwrap();
        let _ = g.sparse_absent().unwrap();
        let _ = g.is_shallow();
        assert_eq!(
            before,
            test_support::snapshot_git_dir(&t),
            "no commit, stage, ref or index write happened"
        );
    }

    #[test]
    fn r_arch_081__each_worker_opens_its_own_handle() {
        let (_g, t) = utf8_tmp();
        test_support::init(&t);
        write_file(&t, "a.rs", "1");
        test_support::commit_all(&t);
        write_file(&t, "a.rs", "22");
        write_file(&t, "b.rs", "x");
        let root = test_root(&t);
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let root = root.clone();
                std::thread::spawn(move || {
                    GitRepo::open(&root)
                        .expect("open")
                        .changed_files()
                        .expect("status")
                })
            })
            .collect();
        let results: Vec<ChangedSet> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results.windows(2).all(|w| w[0] == w[1]));
        assert_eq!(names(&results[0]), ["a.rs", "b.rs"]);
    }

    #[test]
    fn r_arch_005__the_git_library_is_named_only_in_git_rs() {
        // The needles are built from pieces so this file does not contain them.
        // R-GIT-076: the one subprocess (the `git` executable) also lives only here.
        let needles = [
            concat!("git", "2::"),
            concat!("use git", "2"),
            concat!("Command", "::new"),
            concat!("process::", "Command"),
        ];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        let mut checked = 0;
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "rs")
                    && p.file_name().is_some_and(|n| n != "git.rs")
                {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for n in needles {
                        assert!(
                            !text.contains(n),
                            "{} names the git library outside git.rs",
                            p.display()
                        );
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked > 10);
    }
}
