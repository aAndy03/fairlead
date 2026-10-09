//! Control-file parsing (lenient header pass, typed schema, rules, approvals).
pub mod approvals;
pub mod header;
pub mod rules;
pub mod schema;

pub use header::{FairleadHeader, parse_header_lenient, parse_pin_lenient};
pub use rules::*;
pub use schema::*;

use crate::diag::Diagnostic;
use crate::path::RepoPath;

/// A control file (or approvals file) that cannot be used: the scope it governs is unresolved
/// (R-SYN-033, fail closed).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfigFailure {
    pub path: RepoPath,
    pub diags: Vec<Diagnostic>,
}

impl ConfigFailure {
    /// Re-label every location with the real file path (header parsing does not know it).
    pub fn with_path(mut self, path: &RepoPath) -> Self {
        self.path = path.clone();
        for d in &mut self.diags {
            d.location.path = path.clone();
        }
        self
    }
}

use crate::diag::{Location, Severity, codes};
use crate::discover::DEFAULT_FOLDER;
use crate::hash::{ContentHash, hash_normalised};
use schema::{Ctx, join_key};
use std::collections::BTreeMap;

/// One parsed `control.toml` (contract 4.6). `dir` is the directory that OWNS the managerial folder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ControlFile {
    pub path: RepoPath,
    pub dir: RepoPath,
    pub content_hash: ContentHash,
    pub fairlead: FairleadHeader,
    pub settings: Settings,
    pub scan: ScanConfig,
    pub comments: BTreeMap<String, Vec<String>>,
    pub agents: Vec<String>,
    pub kinds: BTreeMap<String, KindDecl>,
    pub file_kinds: BTreeMap<String, FileKindDecl>,
    pub layout: Layout,
    pub files: Vec<FileItemDecl>,
    pub vendors: Vec<VendorDecl>,
    pub ovrs: Vec<OvrDecl>,
    pub rules: BTreeMap<String, RuleDecl>,
    pub policies: PolicyTable,
    pub allows: Vec<AllowDecl>,
    pub reqs: Vec<ReqDecl>,
    pub conventions: Vec<ConventionDecl>,
    pub context: ContextConfig,
    pub baseline: BaselineConfig,
    pub secrets: SecretsConfig,
    pub exceptions: ExceptionsConfig,
    pub eligibility: EligibilityConfig,
    /// `HN-C002` (R-SYN-039): unknown keys and tables, kept for tools that rewrite the file.
    pub unknown: Vec<UnknownKey>,
    /// `HN-C003` (R-SYN-041, R-SCOPE-051/052/056): non-delegable keys in a child file; the keys are inert.
    pub violations: Vec<Diagnostic>,
}

impl ControlFile {
    /// The warnings and inert-key errors of this file: `HN-C002` per unknown key, then `violations`.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut out: Vec<Diagnostic> = self
            .unknown
            .iter()
            .map(|u| {
                Diagnostic::new(
                    codes::HN_C002,
                    Severity::Warn,
                    Location::at(self.path.clone(), u.line, 1),
                    format!("unknown control key or table `{}`", u.path),
                )
                .with_hint("remove it; keys of a later milestone warn as unknown in an older build")
            })
            .collect();
        out.extend(self.violations.iter().cloned());
        out
    }
}

/// Top-level tables a child may not hold (known 0.1 root-only tables).
const ROOT_ONLY_TABLES: [&str; 9] = [
    "comments",
    "agents",
    "layout",
    "policies",
    "context",
    "baseline",
    "secrets",
    "exceptions",
    "eligibility",
];
/// Known keys that only the root may set, per table.
const ROOT_ONLY_KEYS: [(&str, &[&str]); 3] = [
    ("fairlead", &["edition", "marker"]),
    (
        "settings",
        &[
            "folder",
            "fairlead-version",
            "fairlead-range",
            "index-values",
            "owners",
        ],
    ),
    ("scan", &["header-skip", "max-file", "max-directives"]),
];

fn violation(cx: &Ctx<'_>, dotted: &str) -> Diagnostic {
    let line = header::locate_key(cx.text, dotted);
    let mut d = Diagnostic::new(
        codes::HN_C003,
        Severity::Error,
        Location::at(cx.path.clone(), line, 1),
        format!("`{dotted}` is not delegable: only the root control file may set it (inert here)"),
    )
    .with_hint("fairlead new escalation (writes a [[req]] asking the ancestor)");
    d.related = Some(dotted.to_string());
    d
}

fn strip_non_delegable(cx: &Ctx<'_>, table: &mut toml::Table, out: &mut Vec<Diagnostic>) {
    for name in ROOT_ONLY_TABLES {
        if table.remove(name).is_some() {
            out.push(violation(cx, name));
        }
    }
    for (name, keys) in ROOT_ONLY_KEYS {
        if let Some(toml::Value::Table(t)) = table.get_mut(name) {
            for k in keys {
                if t.remove(*k).is_some() {
                    out.push(violation(cx, &join_key(name, k)));
                }
            }
        }
    }
}

fn sorted(mut diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    crate::diag::sort_diagnostics(&mut diags);
    diags
}

/// Pure whole-file parse. On failure the scope is unresolved (R-SYN-033). `is_root_dir` is true when
/// the file lives in the root directory's managerial folder.
pub fn parse_control(
    path: &RepoPath,
    bytes: &[u8],
    is_root_dir: bool,
) -> Result<ControlFile, ConfigFailure> {
    let (text, mut table) = header::parse_table(bytes).map_err(|f| f.with_path(path))?;
    let mut cx = Ctx::new(path, &text);
    let mut violations = Vec::new();
    if !is_root_dir {
        strip_non_delegable(&cx, &mut table, &mut violations);
    }
    let fairlead_tbl = match table.remove("fairlead") {
        Some(toml::Value::Table(h)) => h,
        _ => {
            return Err(header::failure(
                &text,
                "fairlead",
                "a control file must contain a [fairlead] header table",
            )
            .with_path(path));
        }
    };
    let fairlead =
        header::header_from_table(&text, &fairlead_tbl).map_err(|f| f.with_path(path))?;
    let extra: toml::Table = fairlead_tbl
        .into_iter()
        .filter(|(k, _)| !header::HEADER_KEYS.contains(&k.as_str()))
        .collect();
    cx.unknown_in("fairlead", &extra);
    if is_root_dir {
        if fairlead.edition.is_none() {
            cx.error(
                "fairlead",
                "the root control file requires `edition` in [fairlead] (C-34)",
            );
        }
        if !fairlead.root {
            cx.error_code(
                codes::HN_C004,
                "fairlead.root",
                "the root control file must set `root = true` in [fairlead]",
            );
        }
    }
    let settings = schema::parse_settings(&mut cx, &mut table);
    let scan = schema::parse_scan(&mut cx, &mut table);
    let comments = schema::parse_comments(&mut cx, &mut table);
    let agents = schema::parse_agents(&mut cx, &mut table);
    let kinds = schema::parse_kinds(&mut cx, &mut table);
    let file_kinds = schema::parse_file_kinds(&mut cx, &mut table);
    let layout = schema::parse_layout(&mut cx, &mut table);
    let files = schema::parse_files(&mut cx, &mut table);
    let vendors = schema::parse_vendors(&mut cx, &mut table);
    let ovrs = schema::parse_ovrs(&mut cx, &mut table);
    let rules = rules::parse_rules(&mut cx, &mut table);
    let policies = rules::parse_policies(&mut cx, &mut table);
    let allows = rules::parse_allows(&mut cx, &mut table);
    let reqs = rules::parse_reqs(&mut cx, &mut table);
    let conventions = rules::parse_conventions(&mut cx, &mut table);
    let context = rules::parse_context(&mut cx, &mut table);
    let mut baseline = rules::parse_baseline(&mut cx, &mut table);
    let secrets = rules::parse_secrets(&mut cx, &mut table);
    let exceptions = rules::parse_exceptions(&mut cx, &mut table);
    let eligibility = rules::parse_eligibility(&mut cx, &mut table);
    cx.unknown_in("", &table);

    if baseline.file.is_empty() {
        let folder = settings
            .folder
            .clone()
            .unwrap_or_else(|| DEFAULT_FOLDER.to_string());
        baseline.file = format!("{folder}/{}/baseline.txt", layout.r#gen);
    }
    if !cx.diags.is_empty() {
        return Err(ConfigFailure {
            path: path.clone(),
            diags: sorted(cx.diags),
        });
    }
    let dir = path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or_else(RepoPath::root);
    let mut unknown = cx.unknown;
    unknown.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ControlFile {
        path: path.clone(),
        dir,
        content_hash: hash_normalised(bytes),
        fairlead,
        settings,
        scan,
        comments,
        agents,
        kinds,
        file_kinds,
        layout,
        files,
        vendors,
        ovrs,
        rules,
        policies,
        allows,
        reqs,
        conventions,
        context,
        baseline,
        secrets,
        exceptions,
        eligibility,
        unknown,
        violations: sorted(violations),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::codes;
    use crate::discover::Edition;

    const ROOT: &str = r#"[fairlead]
edition = 1
root    = true
scope   = "root"
marker  = "fairlead"

[settings]
folder        = "fairlead"
fairlead-version = "0.1"
purpose       = "Sync app: desktop shell, web UI, core engines."
owners        = ["@platform"]

[scan]
exclude = ["target/", "node_modules/", "dist/", "**/*.gen.ts"]
skip = ["tests/fixtures/**"]

[agents]
targets = ["AGENTS.md", "CLAUDE.md"]

[kinds.tok]
title = "design token"
shape = "doc"
[kinds.cmp]
title = "component"
impl = 1
[kinds.cmp.keys]
layer = { type = "enum", values = ["atom", "molecule", "organism"] }

[rules.no-raw-hex]
check = "leak"
tokens = ["tok.*"]
sev = "error"
stage = "observe"
glob = ["**/*.css", "**/*.tsx"]
waivable = true
why = "colours come from tokens"

[policies]
"rules.*" = "sealed"
"rules.no-raw-hex" = "needs-exception"
[policies.waiver]
max-exp = "90d"
budget = 25

[[allow]]
for = ["web"]
target = "rules.no-raw-hex.stage"
why = "web is migrating"
exp = "2027-03-31"
"#;

    fn p(s: &str) -> RepoPath {
        RepoPath::new(s).unwrap()
    }

    fn root_file() -> ControlFile {
        parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap()
    }

    #[test]
    fn r_syn_038__root_control_parses_the_spec_example() {
        let c = root_file();
        assert_eq!(c.fairlead.scope.as_str(), "root");
        assert_eq!(
            (
                c.fairlead.edition,
                c.fairlead.root,
                c.fairlead.marker.as_deref()
            ),
            (Some(Edition(1)), true, Some("fairlead"))
        );
        assert_eq!(c.dir, RepoPath::root());
        assert_eq!(c.settings.owners.len(), 1);
        assert_eq!(c.scan.exclude.len(), 4);
        assert_eq!(c.agents, ["AGENTS.md", "CLAUDE.md"]);
        assert_eq!(c.kinds.len(), 2);
        assert!(
            matches!(&c.rules["no-raw-hex"].check, RuleCheckDecl::Leak { tokens } if tokens.len() == 1)
        );
        assert_eq!(c.allows[0].for_scopes, ["web"]);
        assert_eq!(
            c.baseline.file, "fairlead/gen/baseline.txt",
            "default follows folder and layout (R-FILE-020)"
        );
        assert!(c.unknown.is_empty() && c.violations.is_empty());
        assert!(c.diagnostics().is_empty());
    }

    #[test]
    fn r_syn_038__root_requires_edition_and_root_true() {
        let no_edition = "[fairlead]\nroot = true\nscope = \"root\"\n";
        let f =
            parse_control(&p("fairlead/control.toml"), no_edition.as_bytes(), true).unwrap_err();
        assert!(
            f.diags
                .iter()
                .any(|d| d.code == codes::HN_C001 && d.message.contains("edition"))
        );
        let not_root = "[fairlead]\nedition = 1\nscope = \"root\"\n";
        let f = parse_control(&p("fairlead/control.toml"), not_root.as_bytes(), true).unwrap_err();
        assert!(f.diags.iter().any(|d| d.code == codes::HN_C004));
        assert_eq!(f.path, p("fairlead/control.toml"));
        assert!(
            f.diags
                .iter()
                .all(|d| d.location.path == p("fairlead/control.toml"))
        );
    }

    #[test]
    fn r_syn_033__unparseable_or_mistyped_control_fails_closed() {
        for bad in [
            "[fairlead\n",
            "[fairlead]\nscope = \"a\"\n[settings]\npurpose = 5\n",
            "[fairlead]\nscope = \"a\"\n[scan]\nexclude = 5\n",
        ] {
            let f =
                parse_control(&p("web/fairlead/control.toml"), bad.as_bytes(), false).unwrap_err();
            assert!(
                f.diags
                    .iter()
                    .all(|d| d.code == codes::HN_C001 || d.code == codes::HN_P001),
                "{bad:?}"
            );
            assert!(!f.diags.is_empty());
        }
    }

    #[test]
    fn r_syn_039__unknown_tables_and_keys_warn_but_parse() {
        let text = format!(
            "{ROOT}\n[future]\nx = 1\n\n[tok.color]\nrole = \"brand\"\n\n[delegate]\n\"rules.*\" = \"tighten\"\n"
        );
        let c = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap();
        let paths: Vec<&str> = c.unknown.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, ["delegate", "future", "tok"]);
        let d = c.diagnostics();
        assert_eq!(d.len(), 3);
        assert!(
            d.iter()
                .all(|x| x.code == codes::HN_C002 && x.severity == Severity::Warn)
        );
    }

    #[test]
    fn r_syn_050__cmd_and_tok_tables_are_unknown_tables_not_declarations() {
        let text = "[fairlead]\nscope = \"web\"\n\n[cmd.sync-now]\nsym = \"x\"\n";
        let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
        assert_eq!(c.unknown[0].path, "cmd");
        assert!(c.kinds.is_empty());
    }

    #[test]
    fn r_syn_051__requires_is_not_a_settings_key() {
        let text = format!("{ROOT}\n[scope]\nportable = true\nrequires = [\"x\"]\n");
        let c = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap();
        assert!(
            c.unknown.iter().any(|u| u.path == "scope"),
            "`[scope]` is a 0.3 table: unknown in 0.1"
        );
        let text2 = ROOT.replace("fairlead-version = \"0.1\"", "requires = \"x\"");
        let c2 = parse_control(&p("fairlead/control.toml"), text2.as_bytes(), true).unwrap();
        assert!(c2.unknown.iter().any(|u| u.path == "settings.requires"));
    }

    const CHILD_BAD: &str = r#"[fairlead]
scope = "web"
marker = "other"
edition = 2

[settings]
purpose = "the web app"
folder = "gov"
fairlead-version = "9.9"
owners = ["@x"]

[scan]
exclude = ["dist/"]
max-file = 1

[comments]
rs = ["//"]

[policies]
"rules.*" = "open"

[agents]
targets = ["X.md"]
"#;

    #[test]
    fn r_syn_041__child_non_delegable_keys_are_hc003_and_inert() {
        let c =
            parse_control(&p("web/fairlead/control.toml"), CHILD_BAD.as_bytes(), false).unwrap();
        let mut keys: Vec<String> = c
            .violations
            .iter()
            .map(|d| d.related.clone().unwrap_or_default())
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "agents",
                "comments",
                "fairlead.edition",
                "fairlead.marker",
                "policies",
                "scan.max-file",
                "settings.fairlead-version",
                "settings.folder",
                "settings.owners"
            ]
        );
        assert!(
            c.violations
                .iter()
                .all(|d| d.code == codes::HN_C003 && d.hint.contains("fairlead new escalation"))
        );
        assert_eq!(
            c.fairlead.marker, None,
            "the stripped marker does not apply"
        );
        assert_eq!(c.settings.folder, None);
        assert!(c.settings.owners.is_empty());
        assert_eq!(
            c.settings.purpose.as_deref(),
            Some("the web app"),
            "delegable keys still apply"
        );
        assert_eq!(c.scan.exclude, ["dist/"]);
        assert_eq!(c.scan.max_file, 2_000_000);
        assert!(c.agents.is_empty() && c.comments.is_empty());
        assert_eq!(c.dir, p("web"));
    }

    #[test]
    fn r_agt_013__vocabulary_keys_change_only_in_the_root_file() {
        let root_ok = parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap();
        assert!(root_ok.violations.is_empty());
        let c =
            parse_control(&p("web/fairlead/control.toml"), CHILD_BAD.as_bytes(), false).unwrap();
        assert!(
            c.violations
                .iter()
                .any(|d| d.related.as_deref() == Some("fairlead.marker"))
        );
        assert!(
            c.violations
                .iter()
                .any(|d| d.related.as_deref() == Some("settings.fairlead-version"))
        );
    }

    #[test]
    fn r_scope_056__child_may_hold_facts_without_violations() {
        let text = r#"[fairlead]
scope = "web"

[settings]
purpose = "web"

[scan]
exclude = ["dist/"]
skip = ["fixtures/**"]

[kinds.wid]
title = "widget"

[file-kinds.sheet]
extractor = "css"
binding = "import"

[[file]]
id = "file.web-styles"
kind = "stylesheet"
path = "app/styles.css"
extends = "root:file.styles"

[[vendor]]
path = "components.json"
tool = "shadcn"

[[ovr]]
target = "rule.no-raw-hex"
why = "legacy"
owner = "@platform"
exp = "2026-12-31"

[[allow]]
for = ["web-admin"]
target = "rules.no-raw-hex.stage"
why = "w"
exp = "2027-01-01"

[[req]]
to = "root"
target = "rule.x"
why = "w"

[[convention]]
glob = "**/*.gen.ts"
class = "external-generated"

[rules.web-only]
check = "pattern"
forbid = "TODO"
msg = "no TODO"
"#;
        let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
        assert!(
            c.violations.is_empty() && c.unknown.is_empty(),
            "{:?} {:?}",
            c.violations,
            c.unknown
        );
        assert_eq!(
            (
                c.files.len(),
                c.vendors.len(),
                c.ovrs.len(),
                c.allows.len(),
                c.reqs.len(),
                c.conventions.len()
            ),
            (1, 1, 1, 1, 1, 1)
        );
        assert_eq!(
            (c.kinds.len(), c.file_kinds.len(), c.rules.len()),
            (1, 1, 1)
        );
    }

    #[test]
    fn r_scope_051_052__root_only_tables_in_a_child_are_violations() {
        for table in [
            "comments",
            "agents",
            "layout",
            "policies",
            "context",
            "baseline",
            "secrets",
            "exceptions",
            "eligibility",
        ] {
            let text = format!("[fairlead]\nscope = \"web\"\n\n[{table}]\n");
            let c = parse_control(&p("web/fairlead/control.toml"), text.as_bytes(), false).unwrap();
            assert_eq!(c.violations.len(), 1, "{table}");
            assert_eq!(c.violations[0].related.as_deref(), Some(table));
        }
    }

    #[test]
    fn r_scope_065__kinds_exc_fails_the_whole_file() {
        let text = format!("{ROOT}\n[kinds.exc]\ntitle = \"x\"\n");
        let f = parse_control(&p("fairlead/control.toml"), text.as_bytes(), true).unwrap_err();
        assert!(f.diags[0].message.contains("exc"));
    }

    #[test]
    fn r_arch_076__control_content_hash_ignores_line_endings() {
        let a = parse_control(&p("fairlead/control.toml"), ROOT.as_bytes(), true).unwrap();
        let crlf = ROOT.replace('\n', "\r\n");
        let b = parse_control(&p("fairlead/control.toml"), crlf.as_bytes(), true).unwrap();
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a, b);
    }

    #[test]
    fn r_file_020__baseline_default_follows_folder_and_layout() {
        let text = "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[settings]\nfolder = \"gov\"\n[layout]\ngen = \"generated\"\n";
        let c = parse_control(&p("gov/control.toml"), text.as_bytes(), true).unwrap();
        assert_eq!(c.baseline.file, "gov/generated/baseline.txt");
        assert_eq!(c.layout.r#gen, "generated");
    }
}
