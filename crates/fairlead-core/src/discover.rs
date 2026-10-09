//! Root discovery and the pin check (R-CLI-004, R-CLI-005, R-ARCH-010..013, D-31, D-33).
//! Part 1: the pure pin types. Task 15 appends root lookup and I/O.
use semver::Version;
use std::cmp::Ordering;

/// Integer edition (D-45).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Edition(pub u32);

/// The edition this build reads and writes.
pub const SUPPORTED_EDITION: Edition = Edition(1);

/// Default managerial folder name (a config key, D-30).
pub const DEFAULT_FOLDER: &str = "fairlead";

/// `[settings] fairlead-version` plus the header edition. "0.1" is padded to 0.1.0 (C-12).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pin {
    pub version: Option<Version>,
    pub edition: Edition,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinRefusal {
    pub install_line: String,
    pub reason: String,
    /// `HN-C006` on the root control file, filled by `discover` (A-30); empty from `pin_check`.
    pub diags: Vec<crate::diag::Diagnostic>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PinVerdict {
    Ok,
    OkNewerBinary { info: String },
    Refused(PinRefusal),
}

/// Parse a pin such as `0.1`, `1` or `1.2.3`; `None` when it is not a version.
pub fn parse_pin_version(s: &str) -> Option<Version> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let padded = match t.matches('.').count() {
        0 => format!("{t}.0.0"),
        1 => format!("{t}.0"),
        _ => t.to_string(),
    };
    Version::parse(&padded).ok()
}

fn install_line(v: Option<&Version>) -> String {
    match v {
        Some(v) => format!("cargo install fairlead --version {v} --locked"),
        None => "cargo install fairlead --locked".to_string(),
    }
}

/// Pure (R-CLI-005, D-50(4)): another edition or an older binary refuses; a newer binary on the same
/// edition proceeds. Pre-release and build metadata of the binary are ignored.
pub fn pin_check(binary: &Version, supported: Edition, pin: &Pin) -> PinVerdict {
    if pin.edition != supported {
        return PinVerdict::Refused(PinRefusal {
            install_line: install_line(pin.version.as_ref()),
            reason: format!(
                "this binary supports edition {} but the codebase is pinned to edition {}",
                supported.0, pin.edition.0
            ),
            diags: Vec::new(),
        });
    }
    let Some(pinned) = &pin.version else {
        return PinVerdict::Ok;
    };
    let release = Version::new(binary.major, binary.minor, binary.patch);
    match release.cmp(pinned) {
        Ordering::Less => PinVerdict::Refused(PinRefusal {
            install_line: install_line(Some(pinned)),
            reason: format!("binary {binary} is older than the fairlead-version pin {pinned}"),
            diags: Vec::new(),
        }),
        Ordering::Equal => PinVerdict::Ok,
        Ordering::Greater => PinVerdict::OkNewerBinary {
            info: format!(
                "binary {binary} is newer than the fairlead-version pin {pinned} (same edition {}); proceeding",
                supported.0
            ),
        },
    }
}

#[cfg(test)]
mod pin_tests {
    use super::*;
    use semver::Version;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }
    fn pin(version: Option<&str>, edition: u32) -> Pin {
        Pin {
            version: version.map(v),
            edition: Edition(edition),
        }
    }

    #[test]
    fn r_cli_005__older_binary_is_refused_with_exact_install_line() {
        match pin_check(&v("0.1.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::Refused(r) => {
                assert_eq!(
                    r.install_line,
                    "cargo install fairlead --version 0.2.0 --locked"
                );
                assert!(r.reason.contains("older"), "{}", r.reason);
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__newer_binary_same_edition_proceeds_with_info_line() {
        match pin_check(&v("0.3.1"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)) {
            PinVerdict::OkNewerBinary { info } => {
                assert!(info.contains("0.3.1") && info.contains("0.2.0"))
            }
            other => panic!("expected OkNewerBinary, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__equal_version_and_missing_pin_are_ok() {
        assert_eq!(
            pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(Some("0.2.0"), 1)),
            PinVerdict::Ok
        );
        assert_eq!(
            pin_check(&v("0.2.0"), SUPPORTED_EDITION, &pin(None, 1)),
            PinVerdict::Ok
        );
    }

    #[test]
    fn r_cli_005__other_edition_is_refused_whichever_side_is_newer() {
        for e in [0u32, 2] {
            match pin_check(&v("9.0.0"), SUPPORTED_EDITION, &pin(Some("0.1.0"), e)) {
                PinVerdict::Refused(r) => {
                    assert!(r.reason.contains(&format!("edition {e}")), "{}", r.reason);
                    assert_eq!(
                        r.install_line,
                        "cargo install fairlead --version 0.1.0 --locked"
                    );
                }
                other => panic!("expected refusal, got {other:?}"),
            }
        }
        match pin_check(&v("1.0.0"), SUPPORTED_EDITION, &pin(None, 2)) {
            PinVerdict::Refused(r) => assert_eq!(r.install_line, "cargo install fairlead --locked"),
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn r_cli_005__prerelease_binary_satisfies_its_release_pin() {
        assert_eq!(
            pin_check(
                &v("0.1.0-alpha.2"),
                SUPPORTED_EDITION,
                &pin(Some("0.1.0"), 1)
            ),
            PinVerdict::Ok
        );
    }

    #[test]
    fn r_cli_005__pin_version_is_padded_to_three_parts() {
        assert_eq!(parse_pin_version("0.1"), Some(v("0.1.0")));
        assert_eq!(parse_pin_version("1"), Some(v("1.0.0")));
        assert_eq!(parse_pin_version(" 1.2.3 "), Some(v("1.2.3")));
        assert_eq!(parse_pin_version("x.y"), None);
        assert_eq!(parse_pin_version(""), None);
    }

    #[test]
    fn r_arch_013__supported_edition_is_one() {
        assert_eq!(SUPPORTED_EDITION, Edition(1));
    }
}

use crate::config::{
    ConfigFailure, ControlFile, parse_control, parse_header_lenient, parse_pin_lenient,
};
use crate::diag::{Diagnostic, Location, Severity, codes};
use crate::error::Error;
use crate::hash::{ContentHash, hash_normalised};
use crate::path::RepoPath;
use camino::{Utf8Path, Utf8PathBuf};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RootSource {
    Flag,
    Env,
    Search,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Root {
    /// Absolute, `dunce`-cleaned.
    pub dir: Utf8PathBuf,
    /// Managerial folder name (`[settings] folder`, default `fairlead`).
    pub folder: String,
    pub source: RootSource,
    pub edition: Edition,
    pub pin: Pin,
    /// Normalised hash of the root control file; the hash over every control file is
    /// `config::config_hash_of` (cache key).
    pub config_hash: ContentHash,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscoverRequest {
    pub root_flag: Option<Utf8PathBuf>,
    pub env_root: Option<Utf8PathBuf>,
    pub cwd: Utf8PathBuf,
    pub binary_version: Version,
    /// False only for `init`, which calls `discover_optional`.
    pub require_root: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Discovered {
    pub root: Root,
    pub root_control: ControlFile,
    pub cwd_rel: RepoPath,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RootProbe {
    NoControl,
    NotRoot,
    Root,
}

/// Pure core of the upward search: the nearest ancestor-or-self for which `probe` says `Root`.
pub fn search_upward<F: FnMut(&Utf8Path) -> RootProbe>(
    start: &Utf8Path,
    mut probe: F,
) -> Option<Utf8PathBuf> {
    start
        .ancestors()
        .find(|dir| probe(dir) == RootProbe::Root)
        .map(Utf8Path::to_path_buf)
}

/// `FAIRLEAD_ROOT`, if set and non-empty. The ONLY environment read in fairlead-core (the bin passes the
/// value in `DiscoverRequest::env_root`).
#[allow(clippy::disallowed_methods)] // reason: discover.rs is the only module allowed to read the environment
pub fn read_env_root() -> Option<Utf8PathBuf> {
    std::env::var("FAIRLEAD_ROOT")
        .ok()
        .filter(|v| !v.is_empty())
        .map(Utf8PathBuf::from)
}

enum Probed {
    No,
    NotRoot,
    Root,
    Broken(ConfigFailure),
}

fn probe_default(dir: &Utf8Path) -> Probed {
    let control = dir.join(DEFAULT_FOLDER).join("control.toml");
    let Ok(bytes) = std::fs::read(control.as_std_path()) else {
        return Probed::No;
    };
    match parse_header_lenient(&bytes) {
        Ok(h) if h.root => Probed::Root,
        Ok(_) => Probed::NotRoot,
        Err(f) => Probed::Broken(f),
    }
}

fn control_rel(folder: &str) -> RepoPath {
    RepoPath::new(&format!("{folder}/control.toml")).unwrap_or_else(|_| RepoPath::root())
}

/// Upward search; an unreadable header on the way stops it with an error (R-ARCH-011).
fn search(cwd: &Utf8Path) -> Result<Option<Utf8PathBuf>, Error> {
    let mut broken: Option<(Utf8PathBuf, ConfigFailure)> = None;
    let found = search_upward(cwd, |dir| match probe_default(dir) {
        Probed::No => RootProbe::NoControl,
        Probed::NotRoot => RootProbe::NotRoot,
        Probed::Root => RootProbe::Root,
        Probed::Broken(f) => {
            broken = Some((dir.to_path_buf(), f));
            RootProbe::Root
        }
    });
    if let Some((dir, f)) = broken {
        let mut failure = f.with_path(&control_rel(DEFAULT_FOLDER));
        for d in &mut failure.diags {
            d.message = format!("{} (control file in {dir})", d.message);
        }
        return Err(Error::from(failure));
    }
    Ok(found)
}

fn absolutize(p: &Utf8Path, cwd: &Utf8Path) -> Utf8PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

fn canonical(p: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    let c = dunce::canonicalize(p.as_std_path())
        .map_err(|e| Error::io(p.as_str(), "resolve", &e, true))?;
    Utf8PathBuf::from_path_buf(c)
        .map_err(|bad| Error::Usage(format!("path is not valid UTF-8: {}", bad.display())))
}

/// The root control file of an explicitly given root directory: the default folder first, then the
/// immediate sub-folders (sorted) whose `control.toml` header says `root = true`.
fn find_root_control(dir: &Utf8Path) -> Result<Option<(String, Vec<u8>)>, Error> {
    let default = dir.join(DEFAULT_FOLDER).join("control.toml");
    if let Ok(bytes) = std::fs::read(default.as_std_path()) {
        return Ok(Some((DEFAULT_FOLDER.to_string(), bytes)));
    }
    let rd = std::fs::read_dir(dir.as_std_path())
        .map_err(|e| Error::io(dir.as_str(), "read directory", &e, true))?;
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    for name in names {
        let p = dir.join(&name).join("control.toml");
        if let Ok(bytes) = std::fs::read(p.as_std_path())
            && parse_header_lenient(&bytes).is_ok_and(|h| h.root)
        {
            return Ok(Some((name, bytes)));
        }
    }
    Ok(None)
}

/// Discovery that tolerates "no root" (`Ok(None)`), for `init`.
pub fn discover_optional(req: &DiscoverRequest) -> Result<Option<Discovered>, Error> {
    let (dir, source) = if let Some(f) = &req.root_flag {
        (absolutize(f, &req.cwd), RootSource::Flag)
    } else if let Some(e) = &req.env_root {
        (absolutize(e, &req.cwd), RootSource::Env)
    } else {
        match search(&req.cwd)? {
            Some(found) => (found, RootSource::Search),
            None => return Ok(None),
        }
    };
    let dir = canonical(&dir)?;
    let Some((folder, bytes)) = find_root_control(&dir)? else {
        return Err(Error::NoRoot {
            searched_from: dir.to_string(),
        });
    };
    let rel = control_rel(&folder);
    // 1. Lenient header and pin first; nothing else is interpreted before the pin check (R-ARCH-013).
    let (header, pin) = parse_pin_lenient(&bytes).map_err(|f| Error::from(f.with_path(&rel)))?;
    if !header.root {
        return Err(Error::Usage(format!(
            "{dir} is not a codebase root: {rel} lacks `root = true`"
        )));
    }
    if let PinVerdict::Refused(mut refusal) =
        pin_check(&req.binary_version, SUPPORTED_EDITION, &pin)
    {
        refusal.diags.push(
            Diagnostic::new(
                codes::HN_C006,
                Severity::Error,
                Location::file(rel.clone()),
                format!("pin mismatch: {}", refusal.reason),
            )
            .with_hint(format!(
                "install the pinned binary: {}",
                refusal.install_line
            )),
        );
        return Err(Error::Pin(refusal));
    }
    // 2. Full parse of the root control file.
    let root_control = parse_control(&rel, &bytes, true).map_err(Error::from)?;
    let declared = root_control
        .settings
        .folder
        .clone()
        .unwrap_or_else(|| DEFAULT_FOLDER.to_string());
    if declared != folder {
        let d = Diagnostic::new(
            codes::HN_C005,
            Severity::Error,
            Location::at(rel.clone(), 1, 1),
            format!("folder name mismatch: [settings] folder is {declared:?} but the managerial folder is {folder:?}"),
        )
        .with_hint("rename the folder or the key; renaming is done by `fairlead upgrade`");
        return Err(Error::from(ConfigFailure {
            path: rel,
            diags: vec![d],
        }));
    }
    let cwd_abs = canonical(&absolutize(&req.cwd, &req.cwd)).unwrap_or_else(|_| req.cwd.clone());
    let cwd_rel = cwd_abs
        .strip_prefix(&dir)
        .ok()
        .and_then(|r| RepoPath::from_native_rel(r).ok())
        .unwrap_or_else(RepoPath::root);
    let root = Root {
        dir,
        folder,
        source,
        edition: pin.edition,
        pin,
        config_hash: hash_normalised(&bytes),
    };
    Ok(Some(Discovered {
        root,
        root_control,
        cwd_rel,
    }))
}

/// S1: flag, then `FAIRLEAD_ROOT`, then the upward search (R-CLI-004, D-31).
pub fn discover(req: &DiscoverRequest) -> Result<Discovered, Error> {
    discover_optional(req)?.ok_or_else(|| Error::NoRoot {
        searched_from: req.cwd.to_string(),
    })
}

#[cfg(test)]
mod root_tests {
    use super::*;
    use crate::error::{Error, ErrorClass};
    use crate::testutil::{CHILD_CONTROL, ROOT_CONTROL, utf8_tmp, write_file};

    fn req(cwd: &Utf8Path) -> DiscoverRequest {
        DiscoverRequest {
            root_flag: None,
            env_root: None,
            cwd: cwd.to_path_buf(),
            binary_version: Version::new(0, 1, 0),
            require_root: true,
        }
    }

    fn canon(p: &Utf8Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(dunce::canonicalize(p.as_std_path()).unwrap()).unwrap()
    }

    #[test]
    fn r_scope_004__search_upward_is_pure_and_nearest_first() {
        let mut seen = Vec::new();
        let found = search_upward(Utf8Path::new("/a/b/c"), |d| {
            seen.push(d.to_string());
            match d.as_str() {
                "/a/b" => RootProbe::Root,
                "/a" => RootProbe::Root,
                "/a/b/c" => RootProbe::NotRoot,
                _ => RootProbe::NoControl,
            }
        });
        assert_eq!(found, Some(Utf8PathBuf::from("/a/b")));
        assert_eq!(
            seen,
            ["/a/b/c", "/a/b"],
            "the search stops at the nearest root"
        );
        assert_eq!(
            search_upward(Utf8Path::new("/x/y"), |_| RootProbe::NotRoot),
            None
        );
    }

    #[test]
    fn r_scope_004__flag_then_env_then_search() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "b/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "a/sub/x.txt", "");
        let mut r = req(&t.join("a/sub"));
        let d = discover(&r).unwrap();
        assert_eq!(
            (d.root.source, d.root.dir.clone()),
            (RootSource::Search, canon(&t.join("a")))
        );
        assert_eq!(d.cwd_rel.as_str(), "sub");
        assert_eq!(d.root.folder, "fairlead");
        assert_eq!(d.root.edition, Edition(1));
        r.env_root = Some(t.join("b"));
        let d = discover(&r).unwrap();
        assert_eq!(
            (d.root.source, d.root.dir.clone()),
            (RootSource::Env, canon(&t.join("b")))
        );
        assert!(
            d.cwd_rel.is_root(),
            "a working directory outside the root maps to the root path"
        );
        r.root_flag = Some(t.join("a"));
        let d = discover(&r).unwrap();
        assert_eq!(
            (d.root.source, d.root.dir),
            (RootSource::Flag, canon(&t.join("a")))
        );
    }

    #[test]
    fn r_cli_004__ordinary_managerial_folders_are_skipped() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "repo/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "repo/pkg/fairlead/control.toml", CHILD_CONTROL);
        write_file(&t, "repo/pkg/src/a.rs", "");
        let d = discover(&req(&t.join("repo/pkg/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("repo")));
        assert_eq!(d.cwd_rel.as_str(), "pkg/src");
    }

    #[test]
    fn r_cli_004__nearest_root_true_wins_so_a_mounted_subroot_stops_the_search() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "vendor/lib/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "vendor/lib/src/x.rs", "");
        let d = discover(&req(&t.join("vendor/lib/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("vendor/lib")));
    }

    #[test]
    fn r_arch_010__the_vcs_top_level_is_never_a_root() {
        let (_g, t) = utf8_tmp();
        write_file(&t, ".git/HEAD", "ref: refs/heads/main\n");
        write_file(&t, "proj/src/a.rs", "");
        assert!(
            discover_optional(&req(&t.join("proj/src")))
                .unwrap()
                .is_none()
        );
        write_file(&t, "proj/fairlead/control.toml", ROOT_CONTROL);
        let d = discover(&req(&t.join("proj/src"))).unwrap();
        assert_eq!(d.root.dir, canon(&t.join("proj")));
    }

    #[test]
    fn r_cli_004__no_root_is_exit_2_and_never_invented() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "src/a.rs", "");
        let e = discover(&req(&t.join("src"))).unwrap_err();
        assert!(matches!(e, Error::NoRoot { .. }), "{e}");
        assert_eq!(e.class(), ErrorClass::Usage);
        assert!(e.to_string().contains("fairlead init"));
        let mut r = req(&t);
        r.require_root = false;
        assert!(discover_optional(&r).unwrap().is_none());
        r.root_flag = Some(t.join("src"));
        let e = discover(&r).unwrap_err();
        assert!(
            matches!(e, Error::NoRoot { .. }),
            "a flag directory without a root control file is NoRoot"
        );
        r.root_flag = Some(t.join("missing"));
        assert_eq!(
            discover(&r).unwrap_err().class(),
            ErrorClass::Usage,
            "NotFound on a user path is a usage error"
        );
    }

    #[test]
    fn r_arch_011__unparseable_control_on_the_path_fails_closed() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "repo/fairlead/control.toml", ROOT_CONTROL);
        write_file(&t, "repo/pkg/fairlead/control.toml", "[fairlead\n");
        write_file(&t, "repo/pkg/src/a.rs", "");
        let e = discover(&req(&t.join("repo/pkg/src"))).unwrap_err();
        assert!(
            matches!(e, Error::Config(_)),
            "must not fall through to the farther root: {e}"
        );
        assert_eq!(e.class(), ErrorClass::Usage);
    }

    #[test]
    fn r_arch_011__root_without_edition_or_with_wrong_types_is_hc001() {
        let (_g, t) = utf8_tmp();
        write_file(
            &t,
            "a/fairlead/control.toml",
            "[fairlead]\nroot = true\nscope = \"root\"\n",
        );
        let e = discover(&req(&t.join("a"))).unwrap_err();
        assert!(
            matches!(&e, Error::Config(f) if f.diags[0].code == crate::diag::codes::HN_C001),
            "{e}"
        );
        write_file(
            &t,
            "b/fairlead/control.toml",
            &format!("{ROOT_CONTROL}[scan]\nmax-file = \"big\"\n"),
        );
        let e = discover(&req(&t.join("b"))).unwrap_err();
        assert!(matches!(e, Error::Config(_)), "{e}");
    }

    #[test]
    fn r_arch_013__pin_is_checked_before_the_full_parse() {
        let (_g, t) = utf8_tmp();
        let bad_rest =
            "[settings]\nfairlead-version = \"99.0\"\n[scan]\nmax-file = \"not a number\"\n";
        write_file(
            &t,
            "a/fairlead/control.toml",
            &format!("{ROOT_CONTROL}{bad_rest}"),
        );
        let e = discover(&req(&t.join("a"))).unwrap_err();
        match &e {
            Error::Pin(r) => {
                assert_eq!(
                    r.install_line,
                    "cargo install fairlead --version 99.0.0 --locked"
                );
                assert_eq!(e.diagnostics().len(), 1);
                assert_eq!(e.diagnostics()[0].code, codes::HN_C006);
                assert_eq!(
                    e.diagnostics()[0].location.path.as_str(),
                    "fairlead/control.toml"
                );
            }
            other => panic!("expected the pin refusal first, got {other}"),
        }
        assert_eq!(e.class(), ErrorClass::Refused);
        write_file(
            &t,
            "b/fairlead/control.toml",
            &format!("{ROOT_CONTROL}{}", bad_rest.replace("99.0", "0.1")),
        );
        assert!(matches!(
            discover(&req(&t.join("b"))).unwrap_err(),
            Error::Config(_)
        ));
    }

    #[test]
    fn r_cli_005__other_edition_is_refused_and_a_newer_binary_proceeds() {
        let (_g, t) = utf8_tmp();
        write_file(
            &t,
            "a/fairlead/control.toml",
            "[fairlead]\nedition = 2\nroot = true\nscope = \"root\"\n[future]\nx = 1\n",
        );
        assert!(matches!(
            discover(&req(&t.join("a"))).unwrap_err(),
            Error::Pin(_)
        ));
        write_file(
            &t,
            "b/fairlead/control.toml",
            &format!("{ROOT_CONTROL}[settings]\nfairlead-version = \"0.0.9\"\n"),
        );
        let d = discover(&req(&t.join("b"))).unwrap();
        assert_eq!(d.root.pin.version, Some(Version::new(0, 0, 9)));
        assert!(matches!(
            pin_check(&Version::new(0, 1, 0), SUPPORTED_EDITION, &d.root.pin),
            PinVerdict::OkNewerBinary { .. }
        ));
    }

    #[test]
    fn r_scope_004__folder_mismatch_is_hc005_and_renamed_folders_are_found_by_flag() {
        let (_g, t) = utf8_tmp();
        write_file(
            &t,
            "a/fairlead/control.toml",
            &format!("{ROOT_CONTROL}[settings]\nfolder = \"gov\"\n"),
        );
        let e = discover(&req(&t.join("a"))).unwrap_err();
        assert!(
            matches!(&e, Error::Config(f) if f.diags[0].code == crate::diag::codes::HN_C005),
            "{e}"
        );
        write_file(
            &t,
            "b/gov/control.toml",
            &format!("{ROOT_CONTROL}[settings]\nfolder = \"gov\"\n"),
        );
        let mut r = req(&t.join("b"));
        r.root_flag = Some(t.join("b"));
        let d = discover(&r).unwrap();
        assert_eq!(d.root.folder, "gov");
        assert_eq!(d.root_control.path.as_str(), "gov/control.toml");
    }

    #[test]
    fn r_arch_077__root_config_hash_is_the_normalised_hash_of_the_root_control_bytes() {
        let (_g, t) = utf8_tmp();
        write_file(&t, "a/fairlead/control.toml", ROOT_CONTROL);
        write_file(
            &t,
            "b/fairlead/control.toml",
            &ROOT_CONTROL.replace('\n', "\r\n"),
        );
        let a = discover(&req(&t.join("a"))).unwrap();
        let b = discover(&req(&t.join("b"))).unwrap();
        assert_eq!(
            a.root.config_hash,
            crate::hash::hash_normalised(ROOT_CONTROL.as_bytes())
        );
        assert_eq!(a.root.config_hash, b.root.config_hash);
        assert_eq!(a.root_control.fairlead.scope.as_str(), "root");
    }
}
