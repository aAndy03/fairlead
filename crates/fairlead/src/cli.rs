//! clap derive tree: the fixed verb set (R-CLI-001), common flags (R-CLI-006) and the
//! [0.1] arguments of each verb from the flag table of 15-cli.md. Parsing only; no behaviour here.

use crate::commands::{self, CommandResult};
use crate::output::{write_internal_error, write_text};
use camino::Utf8PathBuf;
use clap::builder::TypedValueParser as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use fairlead_core::error::Error;
use fairlead_core::outcome::ExitCode;
use fairlead_core::render::OutputFormat;
use fairlead_core::time::AsOf;

/// 0.2.1-T8 (D-62): the field test learned the topics and exit codes only by failing.
pub const TOP_HELP: &str = "Start:      fairlead init --dry-run, then fairlead init --yes; fairlead check; fairlead context <path>
Learn:      fairlead explain syntax | kinds | approve | pr-template | docs-first | authority | <a diagnostic code from check>
Exit codes: 0 ok; 1 findings that fail the run; 2 usage, configuration or syntax error;
            3 stale generated files (run fairlead sync); 4 refused by a gate or a policy;
            5 internal error or cannot determine. When several apply: 5, 2, 4, 1, 3, 0.";

/// 0.2.1-T8: the `--field` keys `new` accepts per kind (the lists `write/new.rs` validates).
pub const NEW_HELP: &str = "--field keys by kind (a key starting x- is always accepted):
  rule         check (pattern|leak), forbid (a regex), msg, glob and exclude (comma lists of globs),
               sev (error|warn|info), waivable (true|false), why, tokens (check=leak: design-token
               values leaking into CSS or TSX; it is not a secret scanner)
  scope        purpose (with --at <folder>)
  escalation   to (an ancestor scope), set, why, blocking
  change       impact (break|add|fix), covers (comma list of globs or IDs), hotfix, revisits, revisit-why
  handoff and declared kinds
               st, sum, kw, sym, of, policy, value, applies, min-len, home, css, and the keys of the
               kind's [kinds.K] table (fairlead explain kinds)
new checks the result before writing: a value the control file would reject writes nothing.";

#[derive(Parser, Debug)]
#[command(
    name = "fairlead",
    version,
    about = "Repository-local rules and context for agents",
    arg_required_else_help = true,
    after_help = TOP_HELP
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags of R-CLI-006 and the flag table that apply to every verb. All are `global`, so they parse before or after the verb.
#[derive(Args, Debug, Clone, Default)]
pub struct GlobalArgs {
    /// Codebase root; wins over FAIRLEAD_ROOT and the upward search (R-CLI-004).
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<Utf8PathBuf>,
    /// Run as if started in DIR; the root search starts there (R-CLI-006).
    #[arg(short = 'C', global = true, value_name = "DIR")]
    pub cwd: Option<Utf8PathBuf>,
    /// Output format: human, terse or json; sarif for check only (R-CLI-006).
    #[arg(long, global = true, value_name = "FORMAT", value_parser = format_parser())]
    pub format: Option<OutputFormat>,
    /// Suppress informational output; errors still print.
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Colour: auto (TTY and NO_COLOR unset) or never (R-CLI-006).
    #[arg(
        long,
        global = true,
        value_enum,
        default_value = "auto",
        value_name = "WHEN"
    )]
    pub color: ColorArg,
    /// Date for exp= and date expiry; default UTC today (D-37, R-CHK-014).
    #[arg(long = "as-of", global = true, value_name = "YYYY-MM-DD", value_parser = parse_as_of)]
    pub as_of: Option<AsOf>,
    /// Ignore the .fairlead/ cache; output stays byte-identical (R-CHK-013).
    #[arg(long = "no-cache", global = true)]
    pub no_cache: bool,
    /// Worker threads; FAIRLEAD_THREADS sets the same value and the flag wins (D-54).
    #[arg(long, global = true, value_name = "N")]
    pub threads: Option<usize>,
}

/// `--color` values (R-CLI-006).
#[derive(ValueEnum, Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ColorArg {
    #[default]
    Auto,
    Never,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Scaffold the control file, agent block, ignore lines and baseline; plan first (R-CLI-012).
    Init(InitArgs),
    /// Scan and report drift; never writes (R-CLI-021).
    Check(CheckArgs),
    /// Write generated files, the baseline with --baseline, and the cache (R-CLI-025).
    Sync(SyncArgs),
    /// Scaffold one authored item; never overwrites (R-CLI-028).
    #[command(after_help = NEW_HELP)]
    New(NewArgs),
    /// Budgeted brief for an agent: a path, an ID, or the session start (R-CLI-032).
    Context(ContextArgs),
    /// Unbudgeted depth on a file, an ID, a code or a topic (R-CLI-035).
    Explain(ExplainArgs),
    /// Record or list human approvals; terminal-gated (R-CLI-037).
    Approve(ApproveArgs),
    /// Relocate files, folders and scopes; rewrites fairlead's own text only (R-CLI-040).
    Move(MoveArgs),
    /// Close the open version: seal ready change docs, write the version file last (R-CLI-041).
    Seal(SealArgs),
    /// Move a codebase one edition forward through an approved, plan-hashed transaction (R-CLI-042).
    Upgrade(UpgradeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct InitArgs {
    /// Print the plan and write nothing; exit 0 (R-CLI-002).
    #[arg(long)]
    pub dry_run: bool,
    /// Apply the printed plan; without it init exits 4 (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Ecosystem preset: none, rust or ts (R-CLI-012).
    #[arg(long, value_enum, default_value = "none")]
    pub preset: PresetArg,
    /// Skip the baseline (R-CLI-019).
    #[arg(long = "no-baseline")]
    pub no_baseline: bool,
    /// Install git and agent hooks, merged and never default-on (R-CLI-020).
    #[arg(long)]
    pub hooks: bool,
    /// Add the CODEOWNERS line for the managerial folder (R-CLI-015).
    #[arg(long)]
    pub codeowners: bool,
    /// Instruction file that receives the managed block; repeatable (R-CLI-012).
    #[arg(long = "agents", value_name = "FILE")]
    pub agents: Vec<String>,
}

/// `--preset` values (R-CLI-012). Mapped to the planner's `Preset` in Task 11.
#[derive(ValueEnum, Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PresetArg {
    #[default]
    #[value(name = "none")]
    NoPreset,
    Rust,
    Ts,
}

#[derive(Args, Debug, Clone)]
pub struct CheckArgs {
    /// Report filter: only these paths are reported; global checks stay whole-registry (R-CLI-021).
    #[arg(value_name = "PATH")]
    pub paths: Vec<String>,
    /// Terse output, no excerpts, 2 KB cap (R-CHK-039).
    #[arg(long = "for-agent")]
    pub for_agent: bool,
    /// Cap printed findings; 0 prints all (R-CHK-038).
    #[arg(long, value_name = "N")]
    pub max: Option<u32>,
    /// Only findings with this code (R-CHK-038).
    #[arg(long, value_name = "CODE")]
    pub rule: Option<String>,
    /// Print observe-stage findings (R-CLI-021).
    #[arg(long = "show-observe")]
    pub show_observe: bool,
    /// Any non-baselined warning fails the run (R-CHK-040).
    #[arg(long = "deny-warnings")]
    pub deny_warnings: bool,
    /// Zero declarations exits 1 instead of 0 (R-CHK-003).
    #[arg(long = "require-declarations")]
    pub require_declarations: bool,
    /// Report only newline-separated paths; `-` reads stdin (R-CLI-022).
    #[arg(long = "files-from", value_name = "FILE")]
    pub files_from: Option<String>,
    /// Ignore the baseline (R-CLI-021).
    #[arg(long = "no-baseline")]
    pub no_baseline: bool,
    /// Stop at the deadline: integer plus ms, s or m (C-14).
    #[arg(long, value_name = "DURATION")]
    pub budget: Option<String>,
    /// Changed files only: working tree and index against HEAD (D-43).
    #[arg(long)]
    pub changed: bool,
    /// Integration-branch mode: with authority.signed-records, verify the signatures of authority
    /// records with `git verify-commit` (R-LC-040).
    #[arg(long)]
    pub ci: bool,
    /// Report only staged paths (R-CLI-022).
    #[arg(long)]
    pub staged: bool,
    /// Coverage base and changed set: the files changed since REF (R-GIT-041).
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// List what a reviewer must look at since REV: lowered pins, ovr changes, rules, instructions.
    #[arg(long, value_name = "REV")]
    pub since: Option<String>,
    /// Warn when local sealed version files differ from REF (R-GIT-042).
    #[arg(long, value_name = "REF")]
    pub against: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct SyncArgs {
    /// Print the plan and write nothing; exit 0 (R-CLI-025).
    #[arg(long)]
    pub dry_run: bool,
    /// Accepted and ignored: sync applies directly (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Write the baseline; prunes unless --accept-new is given (R-CHK-030).
    #[arg(long)]
    pub baseline: bool,
    /// Add current findings to the baseline; needs --baseline and --why (R-CHK-032).
    #[arg(long = "accept-new", requires = "baseline")]
    pub accept_new: bool,
    /// Reason recorded with accepted findings (R-CHK-032).
    #[arg(long, value_name = "TEXT", requires = "accept_new")]
    pub why: Option<String>,
    /// Accept changed vendor-owned hashes (R-FILE-040).
    #[arg(long = "accept-vendor")]
    pub accept_vendor: bool,
    /// Overwrite a hand-edited generated file; the edit is quarantined first (R-CLI-026).
    #[arg(long)]
    pub force: bool,
    /// Write only that scope's generated files (R-SCOPE-063).
    #[arg(long, value_name = "ID")]
    pub scope: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct NewArgs {
    /// Kind of the item: a declared kind, or scope, rule, escalation or handoff.
    pub kind: String,
    /// Slug name, at most 64 characters (R-CLI-029).
    pub name: String,
    /// Scope that receives the item (R-CLI-028).
    #[arg(long, value_name = "ID")]
    pub scope: Option<String>,
    /// Scaffold field K=V; repeatable (R-CLI-028).
    #[arg(long = "field", value_name = "K=V")]
    pub fields: Vec<String>,
    /// Write the scaffold to stdout only (R-CLI-028).
    #[arg(long)]
    pub print: bool,
    /// Print the plan and write nothing; exit 0 (R-CLI-002).
    #[arg(long)]
    pub dry_run: bool,
    /// Accepted and ignored: new applies directly (R-CLI-043).
    #[arg(long)]
    pub yes: bool,
    /// Folder for `new scope` (R-CLI-030).
    #[arg(long, value_name = "PATH")]
    pub at: Option<String>,
    /// Lift identical file-local lines into one scope-level item (R-SCOPE-013).
    #[arg(long)]
    pub hoist: bool,
    /// `new change --hotfix`: open a backfill debt (needs `docs-first.hotfix = true`, R-VER-047).
    #[arg(long)]
    pub hotfix: bool,
    /// `new change <draft id or path> --reid`: give a colliding draft a new ID (R-VER-042).
    #[arg(long)]
    pub reid: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ContextArgs {
    /// A path (wins), an ID (explain lookup), or nothing for the session-start brief (R-CLI-032).
    pub target: Option<String>,
    /// Token budget; 0 is unlimited (R-CTX-044).
    #[arg(long, value_name = "N")]
    pub budget: Option<u32>,
    /// Pin one scope (R-CLI-032).
    #[arg(long, value_name = "S")]
    pub scope: Option<String>,
    /// Expand inherited rules (R-CTX-022).
    #[arg(long)]
    pub full: bool,
    /// Index depth for a whole-tree L0 (R-CTX-071).
    #[arg(long, value_name = "N")]
    pub depth: Option<u32>,
    /// Print only the effective digest (R-CTX-033).
    #[arg(long)]
    pub digest: bool,
    /// Count with the configured tokenizer; exit 2 if none (R-CTX-043).
    #[arg(long = "exact-tokens")]
    pub exact_tokens: bool,
    /// Only what changed since a version, a git revision or a digest (R-CTX-051).
    #[arg(long, value_name = "VERSION|REV|DIGEST")]
    pub since: Option<String>,
}

#[derive(Args, Debug, Clone)]
pub struct ExplainArgs {
    /// A file path, an ID, a diagnostic code or a topic (R-CLI-035).
    pub target: String,
}

#[derive(Args, Debug, Clone)]
pub struct ApproveArgs {
    /// A 12-character fingerprint, or the ID of an item with a pending ovr (C-20).
    pub target: Option<String>,
    /// Approver handle recorded on the approval (R-CLI-037).
    #[arg(long = "as", value_name = "@HANDLE")]
    pub as_handle: Option<String>,
    /// List recorded approvals; read-only (R-CLI-037).
    #[arg(long)]
    pub list: bool,
    /// Revoke an approval by fingerprint (R-CLI-037).
    #[arg(long, value_name = "FP")]
    pub revoke: Option<String>,
}

/// `seal` (R-CLI-041). Without `--yes` the plan is printed and nothing is written (R-CLI-002).
#[derive(Args, Debug, Clone, Default)]
pub struct SealArgs {
    /// Print the plan and write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Apply the plan.
    #[arg(long)]
    pub yes: bool,
    /// Seal although files outside the seal write-set changed; recorded as `dirty = true`.
    #[arg(long)]
    pub allow_dirty: bool,
    /// Seal on any branch (`version.seal-where = "any"`), for solo work (R-VER-055).
    #[arg(long)]
    pub anywhere: bool,
    /// Commit exactly the seal's files with the user's `git` (trailer `Fairlead-Version`).
    #[arg(long)]
    pub commit: bool,
    /// Tag the seal commit `fairlead/<version>`; needs `--commit`.
    #[arg(long, requires = "commit")]
    pub tag: bool,
    /// Roll an interrupted seal back and stop (any writer would roll it forward instead).
    #[arg(long, conflicts_with_all = ["yes", "commit"])]
    pub abort: bool,
    /// Renumber your own unintegrated seal after the integration line (R-GIT-031).
    #[arg(long)]
    pub reseal: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct MoveArgs {
    /// The file, folder or scope directory to move.
    #[arg(value_name = "SRC")]
    pub src: String,
    /// Where it goes; omitted for --detach, --demote and --delete.
    #[arg(value_name = "DST")]
    pub dst: Option<String>,
    /// Print the plan and write nothing; exit 0.
    #[arg(long)]
    pub dry_run: bool,
    /// Apply the plan.
    #[arg(long)]
    pub yes: bool,
    /// Apply although files of the write-set differ from HEAD (R-GIT-062).
    #[arg(long)]
    pub allow_dirty: bool,
    /// Merge into an existing scope or folder; colliding IDs are renamed through aliases.
    #[arg(long)]
    pub merge: bool,
    /// Detach a scope from its ancestors without moving it (R-CLI-047).
    #[arg(long)]
    pub detach: bool,
    /// Turn a scope back into a folder of its parent.
    #[arg(long)]
    pub demote: bool,
    /// Delete a scope; its items are tombstoned (R-PORT-010).
    #[arg(long)]
    pub delete: bool,
    /// With --delete: override the zero-live-references gate (needs an approve record).
    #[arg(long)]
    pub force: bool,
    /// Arrives in 0.3.
    #[arg(long)]
    pub cut: bool,
    /// Arrives in 0.3.
    #[arg(long)]
    pub copy: bool,
    /// Arrives in 0.3.
    #[arg(long)]
    pub attach: bool,
    /// Arrives in 0.3.
    #[arg(long)]
    pub adopt: bool,
}

#[derive(Args, Debug, Clone, Default)]
pub struct UpgradeArgs {
    /// Apply: the first run writes the draft upgrade doc; an approved doc applies the plan.
    #[arg(long)]
    pub yes: bool,
    /// Apply although files of the write-set differ from HEAD (R-EDN-031).
    #[arg(long)]
    pub allow_dirty: bool,
    /// Retrieve edition migration data over the network (no remote data exists yet, Q-2).
    #[arg(long)]
    pub fetch: bool,
    /// Upgrade one sealed sub-root alone (R-EDN-043).
    #[arg(long, value_name = "SCOPE")]
    pub scope: Option<String>,
    /// Rename the directive marker (a rename step; the edition is unchanged, R-EDN-007).
    #[arg(long, value_name = "MARKER")]
    pub rename_marker: Option<String>,
    /// Rename the managerial folder (a rename step; the edition is unchanged, R-EDN-007).
    #[arg(long, value_name = "FOLDER")]
    pub rename_folder: Option<String>,
}

fn format_parser() -> impl clap::builder::TypedValueParser<Value = OutputFormat> {
    clap::builder::PossibleValuesParser::new(["human", "terse", "json", "sarif"])
        .map(|word: String| format_from_word(&word))
}

fn format_from_word(word: &str) -> OutputFormat {
    match word {
        "terse" => OutputFormat::Terse,
        "json" => OutputFormat::Json,
        "sarif" => OutputFormat::Sarif,
        _ => OutputFormat::Human,
    }
}

fn parse_as_of(text: &str) -> Result<AsOf, String> {
    AsOf::parse(text).map_err(|error| error.to_string())
}

/// One invocation, start to finish: parse, dispatch, write, exit. clap prints its own help and version text.
pub fn run_from_env() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => dispatch(cli),
        Err(error) => {
            let code = if error.use_stderr() {
                ExitCode::Usage
            } else {
                ExitCode::Ok
            };
            // clap writes its own text with LF line endings on every platform; a failed print has no other channel.
            let _ = error.print();
            code
        }
    }
}

pub fn dispatch(cli: Cli) -> ExitCode {
    let global = cli.global;
    let result = match cli.command {
        None => CommandResult::failed(&Error::Usage(
            "no verb given; run `fairlead --help`".to_string(),
        )),
        Some(Command::Init(args)) => commands::init::run(&global, &args),
        Some(Command::Check(args)) => commands::check::run(&global, &args),
        Some(Command::Sync(args)) => commands::sync::run(&global, &args),
        Some(Command::New(args)) => commands::new::run(&global, &args),
        Some(Command::Context(args)) => commands::context::run(&global, &args),
        Some(Command::Explain(args)) => commands::explain::run(&global, &args),
        Some(Command::Approve(args)) => commands::approve::run(&global, &args),
        Some(Command::Move(args)) => commands::mv::run(&global, &args),
        Some(Command::Seal(args)) => commands::seal::run(&global, &args),
        Some(Command::Upgrade(args)) => commands::upgrade::run(&global, &args),
    };
    emit(&result)
}

/// The one place stdout and stderr are written. A failed write is exit 5; success is never reported after it (R-ARCH-101).
pub fn emit(result: &CommandResult) -> ExitCode {
    match write_text(&result.stdout, &result.stderr) {
        Ok(()) => result.exit(),
        Err(error) => {
            write_internal_error(&error.to_string());
            ExitCode::Internal
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("fairlead").chain(args.iter().copied()))
    }

    #[test]
    fn r_cli_001__verb_set_is_exactly_the_ten_verbs() {
        let mut names: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "approve", "check", "context", "explain", "init", "move", "new", "seal", "sync",
                "upgrade"
            ]
        );
    }

    #[test]
    fn r_cli_001__reserved_verbs_parse_so_they_can_refuse() {
        assert!(matches!(
            parse(&["move", "a", "b"]).unwrap().command,
            Some(Command::Move(_))
        ));
        assert!(matches!(
            parse(&["upgrade"]).unwrap().command,
            Some(Command::Upgrade(_))
        ));
    }

    #[test]
    fn r_cli_006__global_flags_parse_before_and_after_the_verb() {
        let before = parse(&["--quiet", "--no-cache", "check"]).unwrap();
        let after = parse(&["check", "--no-cache", "--color", "never", "--quiet"]).unwrap();
        assert!(before.global.quiet && before.global.no_cache);
        assert!(after.global.quiet && after.global.no_cache);
        assert_eq!(after.global.color, ColorArg::Never);
    }

    #[test]
    fn r_cli_006__format_accepts_the_four_words_only() {
        assert!(parse(&["check", "--format", "sarif"]).is_ok());
        assert!(parse(&["check", "--format", "yaml"]).is_err());
    }

    #[test]
    fn r_cli_006__version_is_a_display_request_not_an_error() {
        let err = parse(&["--version"]).err().unwrap();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }

    #[test]
    fn r_cli_006__threads_zero_parses_and_is_rejected_by_the_glue() {
        assert_eq!(
            parse(&["--threads", "0", "check"]).unwrap().global.threads,
            Some(0)
        );
    }

    #[test]
    fn r_cli_002__writers_accept_dry_run() {
        assert!(parse(&["init", "--dry-run"]).is_ok());
        assert!(parse(&["sync", "--dry-run"]).is_ok());
        assert!(parse(&["new", "rule", "no-raw-hex", "--dry-run"]).is_ok());
    }

    #[test]
    fn r_cli_021__check_flags_parse_with_budget_max_and_changed() {
        let cli = parse(&[
            "check",
            "apps/web",
            "--max",
            "0",
            "--budget",
            "90s",
            "--changed",
            "--for-agent",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Check(args)) => {
                assert_eq!(args.paths, vec!["apps/web".to_string()]);
                assert_eq!(args.max, Some(0));
                assert_eq!(args.budget.as_deref(), Some("90s"));
                assert!(args.changed && args.for_agent);
            }
            _ => panic!("expected the check verb"),
        }
    }

    #[test]
    fn r_cli_032__context_target_is_optional() {
        match parse(&["context"]).unwrap().command {
            Some(Command::Context(args)) => assert!(args.target.is_none()),
            _ => panic!("expected the context verb"),
        }
        match parse(&["context", "apps/web/src"]).unwrap().command {
            Some(Command::Context(args)) => {
                assert_eq!(args.target.as_deref(), Some("apps/web/src"))
            }
            _ => panic!("expected the context verb"),
        }
    }

    #[test]
    fn r_cli_043__sync_and_new_accept_yes_as_a_no_op() {
        assert!(parse(&["sync", "--yes"]).is_ok());
        assert!(parse(&["new", "rule", "x", "--yes"]).is_ok());
    }

    #[test]
    fn r_cli_045__approve_is_a_verb_and_new_approval_is_only_data() {
        assert!(matches!(
            parse(&["approve", "abcd1234efgh"]).unwrap().command,
            Some(Command::Approve(_))
        ));
        // `new approval` parses (the kind is data); the planner refuses it in Task 12.
        assert!(matches!(
            parse(&["new", "approval", "x"]).unwrap().command,
            Some(Command::New(_))
        ));
    }

    #[test]
    fn r_cli_003__a_missing_verb_is_a_usage_error() {
        assert_eq!(
            dispatch(Cli {
                global: GlobalArgs::default(),
                command: None
            }),
            ExitCode::Usage
        );
    }

    #[test]
    fn r_cli_001__a_later_flag_dispatches_to_exit_2() {
        let cli = Cli {
            global: GlobalArgs::default(),
            command: Some(Command::Move(MoveArgs {
                src: "a".into(),
                cut: true,
                ..MoveArgs::default()
            })),
        };
        assert_eq!(dispatch(cli), ExitCode::Usage);
    }
}
