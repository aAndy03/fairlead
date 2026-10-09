//! Sorted, parallel file listing (R-ARCH-020..026, R-SCOPE-006/007, R-FILE-023).
use crate::config::ControlFile;
use crate::diag::{Diagnostic, Location, Severity, codes, sort_diagnostics};
use crate::discover::Root;
use crate::error::{Error, IoFailure};
use crate::path::{PathIssue, RepoPath, find_case_collisions};
use camino::Utf8Path;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{WalkBuilder, WalkState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::UNIX_EPOCH;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WalkOptions {
    /// Gitignore-syntax patterns relative to the root (`[scan] exclude`).
    pub excludes: Vec<String>,
    /// Files larger than this are listed but never read (`[scan] max-file`).
    pub max_file: u64,
    pub threads: Option<usize>,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            excludes: Vec::new(),
            max_file: 2_000_000,
            threads: None,
        }
    }
}

impl WalkOptions {
    /// Options from the root control file's `[scan]` table.
    pub fn from_root_control(c: &ControlFile, threads: Option<usize>) -> Self {
        Self {
            excludes: c.scan.exclude.clone(),
            max_file: c.scan.max_file,
            threads,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WalkEntry {
    pub path: RepoPath,
    pub size: u64,
    pub mtime_ns: i128,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum SkipReason {
    TooLarge,
    Binary,
    NotUtf8,
    SymlinkOrJunction,
    Excluded,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct WalkResult {
    pub files: Vec<WalkEntry>,
    pub skipped: Vec<(RepoPath, SkipReason)>,
    pub ignored_managerial: Vec<RepoPath>,
    pub diags: Vec<Diagnostic>,
}

#[derive(Default)]
struct Acc {
    files: Vec<WalkEntry>,
    skipped: Vec<(RepoPath, SkipReason)>,
    diags: Vec<Diagnostic>,
    errors: Vec<String>,
}

struct Visit {
    root_dir: std::path::PathBuf,
    folder: String,
    max_file: u64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn build_excludes(root: &Utf8Path, patterns: &[String]) -> Result<Gitignore, Error> {
    let mut b = GitignoreBuilder::new(root.as_std_path());
    for p in patterns {
        b.add_line(None, p)
            .map_err(|e| Error::Usage(format!("invalid [scan] exclude pattern {p:?}: {e}")))?;
    }
    b.build()
        .map_err(|e| Error::Usage(format!("invalid [scan] exclude patterns: {e}")))
}

fn mtime_ns(md: &std::fs::Metadata) -> i128 {
    match md.modified() {
        Ok(t) => match t.duration_since(UNIX_EPOCH) {
            Ok(d) => i128::try_from(d.as_nanos()).unwrap_or(i128::MAX),
            Err(e) => -i128::try_from(e.duration().as_nanos()).unwrap_or(i128::MAX),
        },
        Err(_) => 0,
    }
}

/// Pruning filter run by the walker before it descends (keeps `node_modules/` cheap).
fn keep_entry(root_dir: &std::path::Path, excludes: &Gitignore, e: &ignore::DirEntry) -> bool {
    if e.depth() == 0 {
        return true;
    }
    let name = e.file_name();
    if name == ".git" || name == ".fairlead" {
        return false;
    }
    let is_dir = e.file_type().is_some_and(|t| t.is_dir());
    // A `.git` FILE marks a submodule checkout or a linked worktree: stop (R-ARCH-022).
    if is_dir && std::fs::symlink_metadata(e.path().join(".git")).is_ok_and(|m| m.is_file()) {
        return false;
    }
    match e.path().strip_prefix(root_dir) {
        Ok(rel) => !excludes
            .matched_path_or_any_parents(rel, is_dir)
            .is_ignore(),
        Err(_) => true,
    }
}

fn visit(v: &Visit, acc: &Mutex<Acc>, res: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
    let entry = match res {
        Ok(e) => e,
        Err(err) => {
            lock(acc).errors.push(err.to_string());
            return WalkState::Continue;
        }
    };
    if entry.depth() == 0 {
        return WalkState::Continue;
    }
    let Some(ft) = entry.file_type() else {
        return WalkState::Continue;
    };
    let Ok(rel_native) = entry.path().strip_prefix(&v.root_dir) else {
        return WalkState::Continue;
    };
    let Some(rel) = Utf8Path::from_path(rel_native) else {
        let lossy = rel_native.to_string_lossy().replace('\\', "/");
        let path = RepoPath::new(&lossy).unwrap_or_else(|_| RepoPath::root());
        lock(acc).diags.push(
            Diagnostic::new(
                codes::HN_P001,
                Severity::Error,
                Location::file(path),
                "path is not valid UTF-8; skipped",
            )
            .with_hint("rename the file"),
        );
        return WalkState::Continue;
    };
    let Ok(rp) = RepoPath::from_native_rel(rel) else {
        return WalkState::Continue;
    };
    if ft.is_symlink() {
        let mut g = lock(acc);
        if rp.as_str().split('/').any(|s| s == v.folder) {
            g.diags.push(
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(rp.clone()),
                    "symlink or junction inside a managerial folder (R-SCOPE-006)",
                )
                .with_hint("replace the link with a real file"),
            );
        }
        g.skipped.push((rp, SkipReason::SymlinkOrJunction));
        return WalkState::Continue;
    }
    if !ft.is_file() {
        return WalkState::Continue;
    }
    let md = match entry.metadata() {
        Ok(m) => m,
        Err(err) => {
            lock(acc).errors.push(err.to_string());
            return WalkState::Continue;
        }
    };
    let size = md.len();
    let mut g = lock(acc);
    if size > v.max_file {
        g.skipped.push((rp.clone(), SkipReason::TooLarge));
        g.diags.push(
            Diagnostic::new(
                codes::HN_I001,
                Severity::Info,
                Location::file(rp.clone()),
                format!(
                    "file skipped: {size} bytes is over the max-file limit of {}",
                    v.max_file
                ),
            )
            .with_hint("raise [scan] max-file or add the path to [scan] skip"),
        );
    }
    g.files.push(WalkEntry {
        path: rp,
        size,
        mtime_ns: mtime_ns(&md),
    });
    WalkState::Continue
}

/// `HN-P001` diagnostics for a sorted file list: case-only collisions and path hygiene.
pub fn path_diagnostics(files: &[WalkEntry]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for group in find_case_collisions(files.iter().map(|f| &f.path)) {
        let Some(first) = group.first() else { continue };
        let names: Vec<&str> = group.iter().map(RepoPath::as_str).collect();
        let mut d = Diagnostic::new(
            codes::HN_P001,
            Severity::Error,
            Location::file(first.clone()),
            format!("paths differ only in case: {}", names.join(", ")),
        )
        .with_hint("rename one of them; case-insensitive filesystems cannot hold both");
        d.related = names.get(1).map(|s| s.to_string());
        out.push(d);
    }
    for f in files {
        for issue in f.path.hygiene() {
            let message = match issue {
                PathIssue::NotNfc => "path is not NFC-normalised".to_string(),
                PathIssue::ReservedName { segment } => {
                    format!("path segment {segment:?} is a Windows-reserved name")
                }
                PathIssue::TrailingDotOrSpace { segment } => {
                    format!("path segment {segment:?} ends with a dot or space")
                }
            };
            out.push(
                Diagnostic::new(
                    codes::HN_P001,
                    Severity::Error,
                    Location::file(f.path.clone()),
                    message,
                )
                .with_hint("rename the file; Windows and NTFS cannot represent it"),
            );
        }
    }
    out
}

fn detect_ignored_managerial(root: &Root, files: &[WalkEntry]) -> (Vec<RepoPath>, Vec<Diagnostic>) {
    let listed: BTreeSet<&RepoPath> = files.iter().map(|f| &f.path).collect();
    let mut dirs: BTreeSet<RepoPath> = BTreeSet::new();
    dirs.insert(RepoPath::root());
    for f in files {
        let mut cur = f.path.parent();
        while let Some(d) = cur {
            if !dirs.insert(d.clone()) {
                break;
            }
            cur = d.parent();
        }
    }
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for d in &dirs {
        let Ok(control) = d.join(&root.folder).and_then(|f| f.join("control.toml")) else {
            continue;
        };
        if listed.contains(&control) {
            continue;
        }
        let abs = root.dir.join(control.as_str());
        if std::fs::metadata(abs.as_std_path()).is_ok_and(|m| m.is_file()) {
            diags.push(
                Diagnostic::new(
                    codes::HN_C001,
                    Severity::Error,
                    Location::file(control.clone()),
                    "managerial path is matched by an ignore rule (.gitignore, .fairleadignore or [scan] exclude); authored rules would be lost on clone (R-FILE-023)",
                )
                .with_hint("remove the ignore rule that covers the managerial folder"),
            );
            out.push(control);
        }
    }
    out.sort();
    (out, diags)
}

/// S2: parallel listing, gathered and sorted (R-ARCH-024). I/O.
pub fn walk(root: &Root, opts: &WalkOptions) -> Result<WalkResult, Error> {
    let excludes = build_excludes(&root.dir, &opts.excludes)?;
    let root_std = root.dir.as_std_path().to_path_buf();
    let mut b = WalkBuilder::new(&root_std);
    b.hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .add_custom_ignore_filename(".fairleadignore")
        .threads(opts.threads.unwrap_or(0));
    {
        let root_dir = root_std.clone();
        b.filter_entry(move |e| keep_entry(&root_dir, &excludes, e));
    }
    let acc = Arc::new(Mutex::new(Acc::default()));
    let ctx = Arc::new(Visit {
        root_dir: root_std,
        folder: root.folder.clone(),
        max_file: opts.max_file,
    });
    b.build_parallel().run(|| {
        let acc = Arc::clone(&acc);
        let ctx = Arc::clone(&ctx);
        Box::new(move |res| visit(&ctx, &acc, res))
    });
    let mut acc = match Arc::try_unwrap(acc) {
        Ok(m) => m.into_inner().unwrap_or_else(PoisonError::into_inner),
        Err(shared) => std::mem::take(&mut *lock(&shared)),
    };
    if !acc.errors.is_empty() {
        acc.errors.sort();
        let detail = acc.errors.remove(0);
        return Err(Error::Io(IoFailure {
            path: root.dir.to_string(),
            op: "walk",
            kind: std::io::ErrorKind::Other,
            detail,
            user_path: false,
        }));
    }
    acc.files.sort_by(|a, b| a.path.cmp(&b.path));
    acc.skipped.sort();
    let mut diags = acc.diags;
    diags.extend(path_diagnostics(&acc.files));
    let (ignored_managerial, extra) = detect_ignored_managerial(root, &acc.files);
    diags.extend(extra);
    sort_diagnostics(&mut diags);
    Ok(WalkResult {
        files: acc.files,
        skipped: acc.skipped,
        ignored_managerial,
        diags,
    })
}

/// Apply each child control file's `[scan] exclude` (relative to its own directory) to a finished
/// walk. Excluded files move to `skipped` as `SkipReason::Excluded`. The root's excludes were
/// already applied by `walk` through `WalkOptions`.
pub fn apply_child_excludes(
    walk: WalkResult,
    set: &crate::config::ControlSet,
) -> Result<WalkResult, Error> {
    let mut matchers: Vec<(RepoPath, Gitignore)> = Vec::new();
    for c in set.ok_files() {
        if c.dir.is_root() || c.scan.exclude.is_empty() {
            continue;
        }
        matchers.push((
            c.dir.clone(),
            build_excludes(Utf8Path::new(""), &c.scan.exclude)?,
        ));
    }
    if matchers.is_empty() {
        return Ok(walk);
    }
    let mut kept = Vec::new();
    let mut skipped = walk.skipped;
    for e in walk.files {
        let excluded = matchers.iter().any(|(dir, gi)| {
            if e.path == *dir || !e.path.is_within(dir) {
                return false;
            }
            let rel = e.path.as_str().get(dir.as_str().len() + 1..).unwrap_or("");
            !rel.is_empty() && gi.matched_path_or_any_parents(rel, false).is_ignore()
        });
        if excluded {
            skipped.push((e.path, SkipReason::Excluded));
        } else {
            kept.push(e);
        }
    }
    skipped.sort();
    Ok(WalkResult {
        files: kept,
        skipped,
        ..walk
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;
    use crate::testutil::{test_root, utf8_tmp, write_bytes, write_file};

    fn paths(r: &WalkResult) -> Vec<&str> {
        r.files.iter().map(|f| f.path.as_str()).collect()
    }

    fn opts(excludes: &[&str]) -> WalkOptions {
        WalkOptions {
            excludes: excludes.iter().map(|s| s.to_string()).collect(),
            ..WalkOptions::default()
        }
    }

    #[test]
    fn r_arch_024__result_is_sorted_bytewise_and_thread_count_independent() {
        let (_g, t) = utf8_tmp();
        for f in ["b.txt", "a/z.txt", "a/b.txt", "Z.txt", "A2/x.txt", "a.txt"] {
            write_file(&t, f, f);
        }
        let root = test_root(&t);
        let one = walk(
            &root,
            &WalkOptions {
                threads: Some(1),
                ..WalkOptions::default()
            },
        )
        .unwrap();
        let many = walk(
            &root,
            &WalkOptions {
                threads: Some(4),
                ..WalkOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            paths(&one),
            ["A2/x.txt", "Z.txt", "a.txt", "a/b.txt", "a/z.txt", "b.txt"]
        );
        assert_eq!(one, many);
        assert_eq!(one.files[0].size, "A2/x.txt".len() as u64);
        assert!(one.files[0].mtime_ns > 0);
    }

    #[test]
    fn r_arch_025__walk_works_outside_a_git_repository_and_honours_gitignore() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".gitignore", "ignored.txt\nbuild/\n");
        write_file(&t, "ignored.txt", "x");
        write_file(&t, "build/out.js", "x");
        write_file(&t, "src/keep.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(
            paths(&r),
            [".gitignore", "src/keep.rs"],
            "hidden files are listed, ignored ones are not"
        );
    }

    #[test]
    fn r_arch_020__fairleadignore_is_honoured_and_git_info_exclude_is_not_read() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".fairleadignore", "scratch/\n");
        write_file(&t, "scratch/a.txt", "x");
        write_file(&t, ".git/info/exclude", "secret.txt\n");
        write_file(&t, "secret.txt", "x");
        write_file(&t, "src/a.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), [".fairleadignore", "secret.txt", "src/a.rs"]);
    }

    #[test]
    fn r_arch_021__default_excludes_live_in_options_not_in_code() {
        let (_g, t) = utf8_tmp();
        for f in [
            "target/debug/x.rs",
            "node_modules/p/i.js",
            "src/a.gen.ts",
            "src/b.ts",
        ] {
            write_file(&t, f, "x");
        }
        let root = test_root(&t);
        let none = walk(&root, &WalkOptions::default()).unwrap();
        assert_eq!(
            none.files.len(),
            4,
            "nothing is excluded unless the control file says so"
        );
        let some = walk(&root, &opts(&["target/", "node_modules/", "**/*.gen.ts"])).unwrap();
        assert_eq!(paths(&some), ["src/b.ts"]);
    }

    #[test]
    fn r_arch_022__git_and_cache_dirs_are_never_listed() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".git/config", "x");
        write_file(&t, ".fairlead/cache/x.json", "{}");
        write_file(&t, "a/.fairlead/y", "x");
        write_file(&t, "a.txt", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["a.txt"]);
    }

    #[test]
    fn r_arch_022__submodule_and_linked_worktree_boundaries_stop_the_walk() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "sub/.git", "gitdir: ../.git/modules/sub\n");
        write_file(&t, "sub/inner.rs", "x");
        write_file(&t, "plain/inner.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["plain/inner.rs"]);
        // The root itself may be a linked worktree: its own `.git` file is not a boundary.
        let (_g2, t2) = utf8_tmp();
        write_file(&t2, ".git", "gitdir: /elsewhere\n");
        write_file(&t2, "src/a.rs", "x");
        let r2 = walk(&test_root(&t2), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r2), ["src/a.rs"]);
    }

    #[cfg(unix)]
    #[test]
    fn r_scope_006__symlinks_are_not_followed_and_are_errors_inside_a_managerial_folder() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "real/x.txt", "x");
        write_file(&t, "fairlead/control.toml", "x");
        std::os::unix::fs::symlink(t.join("real").as_std_path(), t.join("link").as_std_path())
            .unwrap();
        std::os::unix::fs::symlink(
            t.join("real/x.txt").as_std_path(),
            t.join("fairlead/docs-link").as_std_path(),
        )
        .unwrap();
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert_eq!(paths(&r), ["fairlead/control.toml", "real/x.txt"]);
        assert!(r.skipped.contains(&(
            RepoPath::new("link").unwrap(),
            SkipReason::SymlinkOrJunction
        )));
        let errs: Vec<&Diagnostic> = r
            .diags
            .iter()
            .filter(|d| d.code == codes::HN_C001)
            .collect();
        assert_eq!(errs.len(), 1, "{:?}", r.diags);
        assert_eq!(errs[0].location.path.as_str(), "fairlead/docs-link");
    }

    #[test]
    fn r_scope_007__case_collisions_and_hygiene_are_hn_p001_on_every_platform() {
        let mk = |p: &str| WalkEntry {
            path: RepoPath::new(p).unwrap(),
            size: 0,
            mtime_ns: 0,
        };
        let files = vec![
            mk("Ui/a.rs"),
            mk("ui/b.rs"),
            mk("src/CON.txt"),
            mk("docs/ok.md"),
            mk("cafe\u{301}.txt"),
        ];
        let d = path_diagnostics(&files);
        let msgs: Vec<String> = d.iter().map(|x| x.message.clone()).collect();
        assert!(d.iter().all(|x| x.code == codes::HN_P001));
        assert!(
            msgs.iter()
                .any(|m| m.contains("Ui") && m.contains("ui") && m.contains("case")),
            "{msgs:?}"
        );
        assert!(
            msgs.iter()
                .any(|m| m.contains("CON.txt") && m.contains("reserved")),
            "{msgs:?}"
        );
        assert!(msgs.iter().any(|m| m.contains("NFC")), "{msgs:?}");
        assert_eq!(d.len(), 3);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn r_scope_007__walk_reports_a_real_case_collision_on_a_case_sensitive_filesystem() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "Ui/a.rs", "x");
        write_file(&t, "ui/b.rs", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert!(
            r.diags
                .iter()
                .any(|d| d.code == codes::HN_P001 && d.message.contains("Ui")),
            "{:?}",
            r.diags
        );
    }

    #[test]
    fn r_file_023__managerial_path_matched_by_an_ignore_rule_is_reported() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".gitignore", "fairlead/\n");
        write_file(&t, "fairlead/control.toml", "x");
        write_file(&t, "web/fairlead/control.toml", "x");
        write_file(&t, "web/src/b.rs", "x");
        write_file(&t, "src/a.rs", "x");
        let r = walk(&test_root(&t), &opts(&["web/fairlead/"])).unwrap();
        assert_eq!(
            r.ignored_managerial,
            vec![
                RepoPath::new("fairlead/control.toml").unwrap(),
                RepoPath::new("web/fairlead/control.toml").unwrap()
            ]
        );
        assert_eq!(
            r.diags.iter().filter(|d| d.code == codes::HN_C001).count(),
            2
        );
        assert!(!paths(&r).contains(&"fairlead/control.toml"));
    }

    #[test]
    fn r_file_023__a_listed_managerial_folder_is_not_reported() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", "x");
        let r = walk(&test_root(&t), &WalkOptions::default()).unwrap();
        assert!(r.ignored_managerial.is_empty() && r.diags.is_empty());
    }

    #[test]
    fn r_arch_023__over_large_files_are_listed_skipped_and_noted_without_being_read() {
        let (_g, t) = utf8_tmp();
        write_bytes(&t, "big.bin", &vec![b'a'; 3000]);
        write_file(&t, "small.txt", "ab");
        let r = walk(
            &test_root(&t),
            &WalkOptions {
                max_file: 1000,
                ..WalkOptions::default()
            },
        )
        .unwrap();
        assert_eq!(paths(&r), ["big.bin", "small.txt"]);
        assert_eq!(
            r.skipped,
            vec![(RepoPath::new("big.bin").unwrap(), SkipReason::TooLarge)]
        );
        let note = r
            .diags
            .iter()
            .find(|d| d.code == codes::HN_I001)
            .expect("an info diagnostic");
        assert_eq!(note.severity, crate::diag::Severity::Info);
        assert_eq!(note.location.path.as_str(), "big.bin");
    }

    #[test]
    fn r_arch_020__invalid_exclude_pattern_is_a_usage_error() {
        let (_g, t) = utf8_tmp();
        let e = walk(&test_root(&t), &opts(&["{unclosed"])).unwrap_err();
        assert_eq!(e.class(), crate::error::ErrorClass::Usage);
    }

    #[test]
    fn r_scope_056__child_scan_exclude_applies_below_its_own_directory_only() {
        use crate::config::load_controls;
        use crate::testutil::{CHILD_CONTROL, ROOT_CONTROL};
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(
            &t,
            "web/fairlead/control.toml",
            &format!("{CHILD_CONTROL}[scan]\nexclude = [\"dist/\", \"*.snap\"]\n"),
        );
        for f in [
            "dist/root.js",
            "web/dist/a.js",
            "web/src/b.ts",
            "web/src/x.snap",
            "api/dist/c.js",
        ] {
            write_file(&t, f, "x");
        }
        let root = test_root(&t);
        let w = walk(&root, &WalkOptions::default()).unwrap();
        let set = load_controls(&root, &w).unwrap();
        let w2 = apply_child_excludes(w, &set).unwrap();
        let kept: Vec<&str> = w2.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            kept,
            [
                "api/dist/c.js",
                "dist/root.js",
                "fairlead/control.toml",
                "web/fairlead/control.toml",
                "web/src/b.ts"
            ]
        );
        assert!(w2.skipped.contains(&(
            RepoPath::new("web/dist/a.js").unwrap(),
            SkipReason::Excluded
        )));
        assert!(w2.skipped.contains(&(
            RepoPath::new("web/src/x.snap").unwrap(),
            SkipReason::Excluded
        )));
    }
}
