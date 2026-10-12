//! Request types and argument classification (R-CTX-002..004, R-CLI-032, C-13).
use fairlead_syntax::ids::Id;

use crate::cache::Cache;
use crate::check::CheckReport;
use crate::config::ContextConfig;
use crate::error::Error;
use crate::extract::Corpus;
use crate::path::RepoPath;
use crate::registry::{Registry, ScopeId};
use crate::render::sanitize::sanitize_data;
use crate::time::AsOf;

/// `--budget N`; `0` is `Unlimited` (R-CTX-044).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    Default,
    Tokens(u32),
    Unlimited,
}

impl Budget {
    /// CLI value to `Budget`: absent is the default, `0` removes the cap.
    pub fn from_arg(n: Option<u32>) -> Budget {
        match n {
            None => Budget::Default,
            Some(0) => Budget::Unlimited,
            Some(n) => Budget::Tokens(n),
        }
    }

    /// The cap in estimated tokens; `None` means no cap.
    pub fn resolve(self, target: &ContextTarget, cfg: &ContextConfig) -> Option<u32> {
        match self {
            Budget::Unlimited | Budget::Tokens(0) => None,
            Budget::Tokens(n) => Some(n),
            Budget::Default => Some(match target {
                ContextTarget::SessionStart => cfg.budget_start,
                ContextTarget::Path(_) | ContextTarget::IdLookup(_) => cfg.budget_path,
            }),
        }
    }
}

/// IdLookup is forwarded to `explain` by the CLI (R-CTX-004); `build` stays total anyway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextTarget {
    SessionStart,
    Path(RepoPath),
    IdLookup(Id),
}

/// Both exit 2 (R-CTX-003, R-CTX-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextArgError {
    NoMatchTaskQuery,
    OutsideRoot,
}

/// `context --since` (R-CTX-023, R-CTX-051 to R-CTX-054).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Since {
    /// A version or git revision: the objects whose generated lines changed since then.
    Changed {
        label: String,
        ids: std::collections::BTreeSet<String>,
    },
    /// A digest equal to the current one: nothing changed.
    Same { label: String },
    /// A digest this machine cannot place: the full slice, marked `since=unknown`.
    Unknown,
}

#[derive(Clone, Debug)]
pub struct ContextRequest {
    pub target: ContextTarget,
    pub budget: Budget,
    pub scope_pin: Option<ScopeId>,
    pub full: bool,
    pub depth: Option<u32>,
    pub digest_only: bool,
    pub exact_tokens: bool,
    pub as_of: AsOf,
    pub since: Option<Since>,
    /// R-CTX-061 (0.3): the lines of a task query (match line first, then hits), already fenced.
    pub query: Vec<String>,
}

pub struct ContextInput<'a> {
    pub registry: &'a Registry,
    pub corpus: &'a Corpus,
    pub report: &'a CheckReport,
    pub config: &'a ContextConfig,
    pub request: &'a ContextRequest,
    pub cache: &'a Cache,
}

/// The classification of free text: neither a path nor an ID. The CLI runs it as a task query
/// (R-CTX-005, 0.3); callers of `parse_target` alone see a usage error.
pub const NO_MATCH_MESSAGE: &str = "not a path or an ID: run it as a task query";

impl ContextArgError {
    /// Both variants are class Usage (exit 2).
    pub fn to_error(self, arg: &str) -> Error {
        match self {
            ContextArgError::NoMatchTaskQuery => Error::Usage(NO_MATCH_MESSAGE.to_string()),
            ContextArgError::OutsideRoot => Error::PathOutsideRoot {
                path: sanitize_data(arg, 200),
            },
        }
    }
}

/// Pure classification (C-13, A-51). `root_rel` turns the raw argument into a root-relative path or
/// says it escapes the root; `exists` is the only filesystem question asked.
pub fn parse_target(
    arg: Option<&str>,
    exists: &dyn Fn(&RepoPath) -> bool,
    root_rel: &dyn Fn(&str) -> Result<RepoPath, ContextArgError>,
) -> Result<ContextTarget, ContextArgError> {
    let Some(arg) = arg.map(str::trim) else {
        return Ok(ContextTarget::SessionStart);
    };
    if arg.is_empty() {
        return Err(ContextArgError::NoMatchTaskQuery);
    }
    let rel = root_rel(arg);
    if let Ok(p) = &rel
        && exists(p)
    {
        return Ok(ContextTarget::Path(p.clone()));
    }
    if let Ok(id) = Id::parse(arg) {
        return Ok(ContextTarget::IdLookup(id));
    }
    if arg.chars().any(char::is_whitespace) {
        return Err(ContextArgError::NoMatchTaskQuery);
    }
    let p = rel?;
    if arg.contains('/') || arg.contains('\\') {
        return Ok(nearest_existing(&p, exists));
    }
    Err(ContextArgError::NoMatchTaskQuery)
}

/// A path that is gone resolves to its nearest existing ancestor; if none exists the root brief is shown.
fn nearest_existing(p: &RepoPath, exists: &dyn Fn(&RepoPath) -> bool) -> ContextTarget {
    let mut cur = p.parent();
    while let Some(c) = cur {
        if exists(&c) {
            return ContextTarget::Path(c);
        }
        cur = c.parent();
    }
    ContextTarget::SessionStart
}

/// The R-CTX-003 note. The bin prints it on stderr so stdout stays a byte-stable brief (CI-7).
pub fn missing_note(requested: &RepoPath, resolved: &ContextTarget) -> Option<String> {
    let req = sanitize_data(requested.as_str(), 120);
    match resolved {
        ContextTarget::Path(p) if p != requested => Some(format!(
            "note: path {req} not found; showing nearest existing ancestor {}",
            sanitize_data(p.as_str(), 120)
        )),
        ContextTarget::SessionStart => {
            Some(format!("note: path {req} not found; showing the root"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::context::testutil::cfg;

    fn dir() -> ContextTarget {
        ContextTarget::Path(RepoPath::new("apps/web").unwrap())
    }

    #[test]
    fn r_ctx_044__default_budget_is_500_for_session_start_and_700_for_a_path() {
        let c = cfg();
        assert_eq!(
            Budget::Default.resolve(&ContextTarget::SessionStart, &c),
            Some(500)
        );
        assert_eq!(Budget::Default.resolve(&dir(), &c), Some(700));
    }

    #[test]
    fn r_ctx_044__budget_zero_removes_the_cap() {
        assert_eq!(Budget::from_arg(Some(0)), Budget::Unlimited);
        assert_eq!(Budget::Unlimited.resolve(&dir(), &cfg()), None);
        assert_eq!(Budget::Tokens(0).resolve(&dir(), &cfg()), None);
    }

    #[test]
    fn r_ctx_044__explicit_budget_wins_and_absent_means_default() {
        assert_eq!(Budget::from_arg(Some(120)), Budget::Tokens(120));
        assert_eq!(Budget::from_arg(None), Budget::Default);
        assert_eq!(Budget::Tokens(120).resolve(&dir(), &cfg()), Some(120));
    }

    #[test]
    fn r_ctx_044__control_file_overrides_the_defaults() {
        let mut c = cfg();
        c.budget_start = 321;
        c.budget_path = 654;
        assert_eq!(
            Budget::Default.resolve(&ContextTarget::SessionStart, &c),
            Some(321)
        );
        assert_eq!(Budget::Default.resolve(&dir(), &c), Some(654));
    }

    fn exists(p: &RepoPath) -> bool {
        [
            "apps",
            "apps/web",
            "apps/web/src",
            "apps/web/src/Panel.tsx",
            "cmp.button",
            "README.md",
        ]
        .contains(&p.as_str())
    }

    fn root_rel(s: &str) -> Result<RepoPath, ContextArgError> {
        let s = s.replace('\\', "/");
        if s.starts_with("../") || s == ".." || s.starts_with('/') {
            return Err(ContextArgError::OutsideRoot);
        }
        RepoPath::new(&s).map_err(|_| ContextArgError::OutsideRoot)
    }

    fn parse(arg: Option<&str>) -> Result<ContextTarget, ContextArgError> {
        parse_target(arg, &exists, &root_rel)
    }

    fn path(s: &str) -> ContextTarget {
        ContextTarget::Path(RepoPath::new(s).unwrap())
    }

    #[test]
    fn r_ctx_001__no_argument_is_the_session_start_brief() {
        assert_eq!(parse(None), Ok(ContextTarget::SessionStart));
    }

    #[test]
    fn r_ctx_002__existing_file_or_directory_is_a_path_query() {
        assert_eq!(
            parse(Some("apps/web/src/Panel.tsx")),
            Ok(path("apps/web/src/Panel.tsx"))
        );
        assert_eq!(parse(Some("apps/web")), Ok(path("apps/web")));
        assert_eq!(parse(Some("apps\\web")), Ok(path("apps/web")));
    }

    #[test]
    fn r_cli_032__existing_path_wins_over_an_id_shaped_argument() {
        assert_eq!(parse(Some("cmp.button")), Ok(path("cmp.button")));
    }

    #[test]
    fn r_ctx_004__id_grammar_is_an_entity_lookup_forwarded_to_explain() {
        assert!(matches!(
            parse(Some("tok.primary")),
            Ok(ContextTarget::IdLookup(_))
        ));
        assert!(matches!(
            parse(Some("web:cmp.sync-button")),
            Ok(ContextTarget::IdLookup(_))
        ));
    }

    #[test]
    fn r_ctx_004__free_text_exits_2_and_never_guesses() {
        for t in [
            "add a progress bar",
            "refactor",
            "fix the sync.progress bug",
            "",
        ] {
            assert_eq!(
                parse(Some(t)),
                Err(ContextArgError::NoMatchTaskQuery),
                "{t}"
            );
        }
        assert_eq!(
            NO_MATCH_MESSAGE,
            "not a path or an ID: run it as a task query"
        );
    }

    #[test]
    fn r_ctx_003__missing_path_resolves_to_nearest_existing_ancestor() {
        assert_eq!(
            parse(Some("apps/web/src/Gone.tsx")),
            Ok(path("apps/web/src"))
        );
        assert_eq!(parse(Some("apps/web/old/dir/x.rs")), Ok(path("apps/web")));
        assert_eq!(parse(Some("nowhere/x.rs")), Ok(ContextTarget::SessionStart));
    }

    #[test]
    fn r_ctx_003__path_outside_the_root_is_exit_2() {
        use crate::error::ErrorClass;
        assert_eq!(
            parse(Some("../etc/passwd")),
            Err(ContextArgError::OutsideRoot)
        );
        assert!(matches!(
            ContextArgError::OutsideRoot.to_error("../x").class(),
            ErrorClass::Usage
        ));
        assert!(matches!(
            ContextArgError::NoMatchTaskQuery.to_error("x").class(),
            ErrorClass::Usage
        ));
    }

    #[test]
    fn r_cli_033__a_missing_path_resolves_to_its_ancestor_and_an_outside_path_exits_2() {
        assert_eq!(parse(Some("apps/web/gone/x.rs")), Ok(path("apps/web")));
        assert_eq!(parse(Some("../outside")), Err(ContextArgError::OutsideRoot));
    }

    #[test]
    fn r_ctx_003__missing_path_gets_a_note_naming_the_ancestor() {
        let want = RepoPath::new("apps/web/src/Gone.tsx").unwrap();
        let got = parse(Some("apps/web/src/Gone.tsx")).unwrap();
        assert_eq!(
            missing_note(&want, &got).as_deref(),
            Some(
                "note: path apps/web/src/Gone.tsx not found; showing nearest existing ancestor apps/web/src"
            )
        );
        let there = RepoPath::new("apps/web").unwrap();
        assert_eq!(missing_note(&there, &path("apps/web")), None);
    }
}
