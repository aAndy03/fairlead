//! Output formats (R-CHK-035..039, R-ARCH-052, R-ARCH-054). Pure: a report in, a string out.
pub mod dense;
pub mod human;
pub mod json;
pub mod line;
pub mod sanitize;
pub mod sarif;
pub mod terse;
#[cfg(test)]
pub(crate) mod testkit;

use std::collections::{BTreeMap, BTreeSet};

use crate::check::floor::is_floor;
use crate::check::{CheckReport, Finding, FindingState, MaxFindings};
use crate::config::Stage;
use crate::context::ContextOutput;
use crate::diag::codes::HN_N001;
use crate::diag::{DiagCode, Diagnostic, Severity};
use crate::explain::ExplainOutput;
use sanitize::{LINE_CAP, clean_data};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Human,
    Terse,
    Json,
    Sarif,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RenderOptions {
    pub format: OutputFormat,
    pub for_agent: bool,
    pub max: MaxFindings,
    pub show_observe: bool,
    pub color: bool,
    pub quiet: bool,
}

pub trait Renderer {
    fn format(&self) -> OutputFormat;
    /// LF-only, repo-relative `/` paths, no timestamps (R-ARCH-052). Truncation and grouping
    /// per R-CHK-038/039 happen here.
    fn check(&self, report: &CheckReport, opts: &RenderOptions) -> String;
    /// Terse and Json only; `None` otherwise (C-28).
    fn context(&self, out: &ContextOutput) -> Option<String>;
    fn explain(&self, out: &ExplainOutput) -> Option<String>;
}

pub fn renderer(format: OutputFormat) -> &'static dyn Renderer {
    static HUMAN: human::HumanRenderer = human::HumanRenderer;
    static TERSE: terse::TerseRenderer = terse::TerseRenderer;
    static JSON: json::JsonRenderer = json::JsonRenderer;
    static SARIF: sarif::SarifRenderer = sarif::SarifRenderer;
    match format {
        OutputFormat::Human => &HUMAN,
        OutputFormat::Terse => &TERSE,
        OutputFormat::Json => &JSON,
        OutputFormat::Sarif => &SARIF,
    }
}

/// Last safety net for text built by P6/P7: redact secrets, LF-only, one final newline.
pub(crate) fn finish_text(text: &str) -> String {
    finish(crate::secrets::redact(text))
}

pub(crate) fn sev_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

pub(crate) fn stage_word(s: &Stage) -> &'static str {
    match s {
        Stage::Observe => "observe",
        Stage::Warn => "warn",
        Stage::Enforce => "enforce",
    }
}

pub(crate) fn state_word(s: &FindingState) -> &'static str {
    match s {
        FindingState::New => "new",
        FindingState::Baselined => "baselined",
        FindingState::Waived => "waived",
        FindingState::WaiverIneffective => "waiver-ineffective",
    }
}

/// Listing rule of the text formats (R-CHK-035 states, stage x severity table of 13).
/// Baselined and waived findings are only counted; observe-stage ones need `--show-observe`;
/// `--quiet` drops info findings except the "nothing verified" notice (R-CHK-003).
pub(crate) fn is_listed(f: &Finding, opts: &RenderOptions) -> bool {
    if matches!(f.state, FindingState::Baselined | FindingState::Waived) {
        return false;
    }
    if matches!(f.stage, Stage::Observe) && !opts.show_observe {
        return false;
    }
    !(opts.quiet && f.severity == Severity::Info && f.code != HN_N001)
}

/// Findings of one code, severity and message in one file, shown as one entry (R-CHK-038).
pub(crate) struct Group<'a> {
    pub lead: &'a Finding,
    pub lines: Vec<u32>,
    pub count: usize,
}

pub(crate) struct View<'a> {
    pub groups: Vec<Group<'a>>,
    /// Findings in entries cut by the cap.
    pub hidden: usize,
    pub hidden_codes: BTreeSet<DiagCode>,
}

fn display_rank(f: &Finding) -> (u8, u8) {
    let severity = match f.severity {
        Severity::Error => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    };
    (u8::from(!is_floor(f.code)), severity)
}

fn cap_of(max: &MaxFindings) -> usize {
    match max {
        MaxFindings::Default => 20,
        MaxFindings::Limit(0) | MaxFindings::All => usize::MAX,
        MaxFindings::Limit(n) => usize::try_from(*n).unwrap_or(usize::MAX),
    }
}

type GroupKey = (DiagCode, String, Severity, bool, String);

/// R-CHK-038: floor first, then severity, then path (the total order of `Finding` breaks ties);
/// repeats collapse; the cap counts entries (A-49).
pub(crate) fn build_view<'a>(report: &'a CheckReport, opts: &RenderOptions) -> View<'a> {
    let mut listed: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|f| is_listed(f, opts))
        .collect();
    listed.sort_by(|a, b| display_rank(a).cmp(&display_rank(b)).then_with(|| a.cmp(b)));
    let mut groups: Vec<Group<'a>> = Vec::new();
    let mut index: BTreeMap<GroupKey, usize> = BTreeMap::new();
    for f in listed {
        let key = (
            f.code,
            f.location.path.as_str().to_string(),
            f.severity,
            matches!(f.state, FindingState::WaiverIneffective),
            f.message.clone(),
        );
        match index.get(&key).copied() {
            Some(i) => {
                if let Some(g) = groups.get_mut(i) {
                    g.count += 1;
                    g.lines.push(f.location.line);
                }
            }
            None => {
                index.insert(key, groups.len());
                groups.push(Group {
                    lead: f,
                    lines: vec![f.location.line],
                    count: 1,
                });
            }
        }
    }
    let cap = cap_of(&opts.max);
    let mut hidden = 0;
    let mut hidden_codes = BTreeSet::new();
    if groups.len() > cap {
        for g in groups.split_off(cap) {
            hidden += g.count;
            hidden_codes.insert(g.lead.code);
        }
    }
    View {
        groups,
        hidden,
        hidden_codes,
    }
}

/// `+37 more -> fairlead check --rule HN-T001 --max 0` (fixed text; a rule filter only when every
/// hidden finding has one code).
pub(crate) fn more_line(hidden: usize, codes: &BTreeSet<DiagCode>) -> Option<String> {
    if hidden == 0 {
        return None;
    }
    let rule = match codes.iter().next() {
        Some(c) if codes.len() == 1 => format!(" --rule {}", c.0),
        _ => String::new(),
    };
    Some(format!("+{hidden} more -> fairlead check{rule} --max 0"))
}

pub(crate) fn header_line(report: &CheckReport) -> String {
    let h = &report.header;
    let mut s = format!(
        "fairlead check {} edition={} as-of={} scopes={} files={} rules={}",
        clean_data(&h.fairlead_version, 40),
        h.edition.0,
        h.as_of.0,
        h.scopes,
        h.files,
        h.rules
    );
    if report.partial {
        s.push_str(" partial=true");
    }
    s
}

/// 0.2.1-T6 (D-62): live errors whose stage is below `enforce` count in `errors N` but do not
/// fail the run; the field test read `errors 1` with exit 0 as a bug. `None` when every live
/// error fails (or none exists).
pub(crate) fn soft_error_note(report: &CheckReport) -> Option<String> {
    let soft = report
        .findings
        .iter()
        .filter(|f| {
            matches!(f.state, FindingState::New | FindingState::WaiverIneffective)
                && f.severity == Severity::Error
                && f.stage == Stage::Warn
        })
        .count();
    match soft {
        0 => None,
        1 => Some(
            "1 error is at stage warn and does not fail the run (stage enforce fails it)".into(),
        ),
        n => Some(format!(
            "{n} errors are at stage warn and do not fail the run (stage enforce fails them)"
        )),
    }
}

pub(crate) fn counts_line(report: &CheckReport) -> String {
    let s = &report.summary;
    format!(
        "errors {} warn {} info {} | baselined {} | waived {} | observe {} | accepted {}",
        s.errors, s.warns, s.infos, s.baselined, s.waived, s.observe, s.accepted
    )
}

/// Lines printed after the findings on every run: waiver summary and relaxations (R-CHK-028,
/// R-AGT-011), exception delta (R-SCOPE-048), CODEOWNERS gap (R-AGT-034), open requests
/// (R-AGT-033), unknown scopes (R-CHK-057), partial run (R-CLI-023).
pub(crate) fn tail_lines(report: &CheckReport, opts: &RenderOptions) -> Vec<String> {
    let s = &report.summary;
    let mut out = Vec::new();
    let waivers = if s.waivers.is_empty() {
        "none".to_string()
    } else {
        s.waivers
            .iter()
            .map(|(scope, w)| {
                let soon = if w.expiring_30d > 0 {
                    format!(" ({} expires <=30d)", w.expiring_30d)
                } else {
                    String::new()
                };
                format!("{} {}/{}{soon}", scope.0.as_str(), w.active, w.budget)
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut relaxations: Vec<String> = s
        .relaxations
        .iter()
        .map(|r| clean_data(r, LINE_CAP))
        .collect();
    relaxations.sort();
    let relaxations = if relaxations.is_empty() {
        "none".to_string()
    } else {
        relaxations.join(", ")
    };
    if !(opts.quiet && waivers == "none" && relaxations == "none") {
        out.push(format!("waivers: {waivers} | relaxations: {relaxations}"));
    }
    out.push(format!("exceptions: {:+}", s.exceptions_delta));
    if s.codeowners_uncovered > 0 {
        out.push(format!(
            "codeowners: {} managerial folders uncovered",
            s.codeowners_uncovered
        ));
    }
    if s.requests_open > 0 {
        out.push(match s.oldest_request_days {
            Some(d) => format!("requests: {} open, oldest {d}d", s.requests_open),
            None => format!("requests: {} open", s.requests_open),
        });
    }
    if let Some((bound, governed)) = s.hier_coverage {
        out.push(format!(
            "hier coverage: {bound}/{governed} governed files bound ({}%)",
            (u64::from(bound) * 100)
                .checked_div(u64::from(governed))
                .unwrap_or(100)
        ));
    }
    if !report.unknown_scopes.is_empty() {
        let names: Vec<&str> = report.unknown_scopes.iter().map(|s| s.0.as_str()).collect();
        out.push(format!(
            "unknown scopes: {} (sparse or unresolved; the verdict depends on them)",
            names.join(", ")
        ));
    }
    if report.partial {
        out.push("partial=true: --budget deadline reached; result is incomplete".to_string());
    }
    out
}

fn place(path: &str, line: u32, col: u32) -> String {
    let path = clean_data(path, LINE_CAP);
    match (line, col) {
        (0, _) => path,
        (l, 0) => format!("{path}:{l}"),
        (l, c) => format!("{path}:{l}:{c}"),
    }
}

/// `path`, `path:line` or `path:line:col`; a collapsed group lists its lines (`path:44,81,90`).
pub(crate) fn location_text(g: &Group<'_>) -> String {
    let loc = &g.lead.location;
    if g.count == 1 {
        return place(loc.path.as_str(), loc.line, loc.col);
    }
    let path = clean_data(loc.path.as_str(), LINE_CAP);
    let mut lines: Vec<u32> = g.lines.iter().copied().filter(|l| *l > 0).collect();
    lines.sort_unstable();
    lines.dedup();
    if lines.is_empty() {
        path
    } else {
        let list = lines
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("{path}:{list}")
    }
}

/// R-ARCH-103 (A-16): the offending text and a caret line under it, indented by `indent`.
pub(crate) fn excerpt_lines(excerpt: Option<&str>, indent: &str) -> Vec<String> {
    let Some(text) = excerpt
        .map(|e| clean_data(e, LINE_CAP))
        .filter(|e| !e.is_empty())
    else {
        return Vec::new();
    };
    let width = text.chars().count();
    vec![
        format!("{indent}| {text}"),
        format!("{indent}| {}", "^".repeat(width)),
    ]
}

/// Diagnostics carried by an error (`Error::diagnostics()`), one block each, with the excerpt
/// and caret of R-ARCH-103 (A-20). Sorted as `sort_diagnostics` sorts.
pub fn diagnostic_lines(diags: &[Diagnostic]) -> String {
    let mut sorted: Vec<&Diagnostic> = diags.iter().collect();
    sorted.sort_by(|a, b| {
        (
            &a.location.path,
            a.location.line,
            a.location.col,
            a.code,
            &a.message,
        )
            .cmp(&(
                &b.location.path,
                b.location.line,
                b.location.col,
                b.code,
                &b.message,
            ))
    });
    let mut out = Vec::new();
    for d in sorted {
        let loc = &d.location;
        out.push(format!(
            "{} {}  {}",
            sev_word(d.severity),
            d.code.0,
            place(loc.path.as_str(), loc.line, loc.col)
        ));
        let mut message = clean_data(&d.message, LINE_CAP);
        if let Some(e) = d.expected.as_deref().filter(|e| !e.is_empty()) {
            message.push_str(&format!(" (expected {})", clean_data(e, LINE_CAP)));
        }
        out.push(format!("  {message}"));
        out.extend(excerpt_lines(
            d.text.lines().next().filter(|_| loc.line > 0),
            "  ",
        ));
        let fix = if d.hint.is_empty() {
            d.did_you_mean
                .as_deref()
                .map(|x| format!("did you mean {x}"))
                .unwrap_or_default()
        } else {
            d.hint.clone()
        };
        let fix = clean_data(&fix, sanitize::FIX_CAP);
        if !fix.is_empty() {
            out.push(format!("  fix:    {fix}"));
        }
    }
    finish(out.join("\n"))
}

/// Machine formats list every finding; only an explicit `--max N` (N > 0) cuts, and the cut is
/// counted (A-49).
pub(crate) fn machine_slice<'a>(
    report: &'a CheckReport,
    opts: &RenderOptions,
) -> (Vec<&'a Finding>, usize) {
    let n = match opts.max {
        MaxFindings::Limit(n) if n > 0 => usize::try_from(n).unwrap_or(usize::MAX),
        _ => usize::MAX,
    };
    // R-CHK-011: the total order of `Finding`, whatever order the report arrived in.
    let mut all: Vec<&Finding> = report.findings.iter().collect();
    all.sort();
    let omitted = all.len().saturating_sub(n);
    all.truncate(n);
    (all, omitted)
}

/// LF-only, exactly one final newline (R-ARCH-052).
pub(crate) fn finish(mut s: String) -> String {
    if s.contains('\r') {
        s = s.replace("\r\n", "\n").replace('\r', "\n");
    }
    let keep = s.trim_end_matches('\n').len();
    s.truncate(keep);
    s.push('\n');
    s
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::testkit::{finding, report, with};
    use super::*;
    use crate::check::WaiverCount;
    use crate::diag::codes::{HN_F001, HN_N001, HN_R001, HN_T001, HN_X004};

    fn opts() -> RenderOptions {
        RenderOptions {
            format: OutputFormat::Terse,
            for_agent: false,
            max: MaxFindings::Default,
            show_observe: false,
            color: false,
            quiet: false,
        }
    }
    fn paths(r: &CheckReport, o: &RenderOptions) -> Vec<String> {
        build_view(r, o)
            .groups
            .iter()
            .map(|g| g.lead.location.path.as_str().to_string())
            .collect()
    }
    fn thirty(msg: &str) -> CheckReport {
        report(
            (0..30)
                .map(|i| {
                    finding(
                        HN_R001,
                        Severity::Error,
                        &format!("src/f{i:02}.rs"),
                        1,
                        1,
                        msg,
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn r_chk_038__default_cap_is_20_groups_and_the_rest_is_counted() {
        let r = thirty("unknown id");
        let view = build_view(&r, &opts());
        assert_eq!(view.groups.len(), 20);
        assert_eq!(view.hidden, 10);
        assert_eq!(
            more_line(view.hidden, &view.hidden_codes).as_deref(),
            Some("+10 more -> fairlead check --rule HN-R001 --max 0")
        );
    }

    #[test]
    fn r_chk_038__max_limit_and_all_override_the_default() {
        let r = thirty("x");
        let with_max = |m| RenderOptions { max: m, ..opts() };
        assert_eq!(
            build_view(&r, &with_max(MaxFindings::Limit(5)))
                .groups
                .len(),
            5
        );
        assert_eq!(build_view(&r, &with_max(MaxFindings::All)).groups.len(), 30);
        assert_eq!(build_view(&r, &with_max(MaxFindings::Limit(0))).hidden, 0);
    }

    #[test]
    fn r_chk_038__hidden_findings_of_several_codes_suggest_no_rule_filter() {
        let codes: BTreeSet<DiagCode> = [HN_R001, HN_T001].into_iter().collect();
        assert_eq!(
            more_line(3, &codes).as_deref(),
            Some("+3 more -> fairlead check --max 0")
        );
        assert_eq!(more_line(0, &codes), None);
    }

    #[test]
    fn r_chk_038__order_is_floor_first_then_severity_then_path() {
        let r = report(vec![
            finding(HN_X004, Severity::Warn, "a.rs", 1, 1, "w"),
            finding(HN_R001, Severity::Error, "b.rs", 1, 1, "e"),
            finding(HN_F001, Severity::Error, "z.rs", 1, 1, "floor"),
            finding(HN_R001, Severity::Error, "a.rs", 9, 1, "e2"),
        ]);
        assert_eq!(paths(&r, &opts()), vec!["z.rs", "a.rs", "b.rs", "a.rs"]);
    }

    #[test]
    fn r_chk_038__repeated_findings_of_one_code_in_one_file_collapse() {
        let p = "apps/web/src/Panel.tsx";
        let msg = "literal #2563eb is tok.primary";
        let r = report(vec![
            finding(HN_T001, Severity::Error, p, 44, 18, msg),
            finding(HN_T001, Severity::Error, p, 90, 3, msg),
            finding(HN_T001, Severity::Error, p, 81, 7, msg),
            finding(
                HN_T001,
                Severity::Error,
                p,
                12,
                7,
                "literal #ff0000 is tok.danger",
            ),
        ]);
        let view = build_view(&r, &opts());
        let places: Vec<String> = view.groups.iter().map(location_text).collect();
        assert_eq!(
            places,
            vec![
                "apps/web/src/Panel.tsx:12:7",
                "apps/web/src/Panel.tsx:44,81,90"
            ]
        );
        assert_eq!(view.hidden, 0);
    }

    #[test]
    fn r_chk_035__baselined_waived_and_observe_findings_are_not_listed_unless_asked() {
        let r = report(vec![
            with(finding(HN_R001, Severity::Error, "a.rs", 1, 1, "b"), |f| {
                f.state = FindingState::Baselined
            }),
            with(finding(HN_R001, Severity::Error, "b.rs", 1, 1, "w"), |f| {
                f.state = FindingState::Waived
            }),
            with(finding(HN_T001, Severity::Error, "c.rs", 1, 1, "o"), |f| {
                f.stage = Stage::Observe
            }),
            with(
                finding(HN_R001, Severity::Error, "d.rs", 1, 1, "inef"),
                |f| f.state = FindingState::WaiverIneffective,
            ),
        ]);
        assert_eq!(paths(&r, &opts()), vec!["d.rs"]);
        let observe = RenderOptions {
            show_observe: true,
            ..opts()
        };
        assert_eq!(paths(&r, &observe), vec!["c.rs", "d.rs"]);
    }

    #[test]
    fn r_chk_003__the_nothing_verified_notice_survives_quiet() {
        let r = report(vec![
            finding(
                HN_N001,
                Severity::Info,
                "fairlead/control.toml",
                0,
                0,
                "0 rules declared, 0 IDs: nothing verified",
            ),
            finding(HN_X004, Severity::Info, "a.rs", 1, 1, "other info"),
        ]);
        let quiet = RenderOptions {
            quiet: true,
            ..opts()
        };
        let codes: Vec<&str> = build_view(&r, &quiet)
            .groups
            .iter()
            .map(|g| g.lead.code.0)
            .collect();
        assert_eq!(codes, vec!["HN-N001"]);
    }

    #[test]
    fn r_chk_028__the_waiver_summary_is_printed_on_every_run() {
        let mut r = report(vec![]);
        assert_eq!(
            tail_lines(&r, &opts()),
            vec!["waivers: none | relaxations: none", "exceptions: +0"]
        );
        r.summary.waivers.insert(
            testkit::scope("web"),
            WaiverCount {
                active: 3,
                expiring_30d: 1,
                budget: 25,
                renewals: 0,
            },
        );
        assert_eq!(
            tail_lines(&r, &opts())[0],
            "waivers: web 3/25 (1 expires <=30d) | relaxations: none"
        );
    }

    #[test]
    fn r_agt_011__relaxations_are_listed_on_every_run() {
        let mut r = report(vec![]);
        r.summary.relaxations =
            vec!["rules.no-raw-hex stage=observe in web (declared enforce)".into()];
        assert_eq!(
            tail_lines(&r, &opts())[0],
            "waivers: none | relaxations: rules.no-raw-hex stage=observe in web (declared enforce)"
        );
    }

    #[test]
    fn r_scope_048__the_exception_delta_is_always_shown() {
        let mut r = report(vec![]);
        r.summary.exceptions_delta = -2;
        assert!(tail_lines(&r, &opts()).contains(&"exceptions: -2".to_string()));
    }

    #[test]
    fn r_agt_034__the_codeowners_gap_is_loud_while_any_folder_is_uncovered() {
        let mut r = report(vec![]);
        r.summary.codeowners_uncovered = 2;
        let quiet = RenderOptions {
            quiet: true,
            ..opts()
        };
        assert!(
            tail_lines(&r, &quiet).contains(&"codeowners: 2 managerial folders uncovered".into())
        );
    }

    #[test]
    fn r_agt_033__open_requests_and_the_oldest_age_are_summarised_never_failing() {
        let mut r = report(vec![]);
        r.summary.requests_open = 4;
        r.summary.oldest_request_days = Some(9);
        assert!(tail_lines(&r, &opts()).contains(&"requests: 4 open, oldest 9d".to_string()));
    }

    #[test]
    fn r_chk_057__unknown_scopes_are_named_so_green_is_never_silent() {
        let mut r = report(vec![]);
        r.unknown_scopes = vec![testkit::scope("api")];
        let want = "unknown scopes: api (sparse or unresolved; the verdict depends on them)";
        assert!(tail_lines(&r, &opts()).contains(&want.to_string()));
    }

    #[test]
    fn r_cli_023__a_partial_run_is_marked_in_the_tail() {
        let mut r = report(vec![]);
        r.partial = true;
        let want = "partial=true: --budget deadline reached; result is incomplete";
        assert!(tail_lines(&r, &opts()).contains(&want.to_string()));
    }

    #[test]
    fn r_arch_090__the_header_prints_the_as_of_date_and_no_other_time() {
        let r = report(vec![]);
        assert_eq!(
            header_line(&r),
            "fairlead check 0.1.0 edition=1 as-of=2026-10-08 scopes=3 files=1204 rules=7"
        );
    }

    #[test]
    fn r_cli_023__the_header_marks_a_partial_run() {
        let mut r = report(vec![]);
        r.partial = true;
        assert!(header_line(&r).ends_with(" partial=true"));
    }

    #[test]
    fn r_arch_052__finish_gives_lf_only_and_one_final_newline() {
        assert_eq!(finish("a\r\nb\rc\n\n\n".to_string()), "a\nb\nc\n");
        assert_eq!(finish(String::new()), "\n");
    }

    #[test]
    fn r_arch_103__diagnostic_lines_show_the_excerpt_with_a_caret() {
        use crate::check::finding::DiagB;
        let p = crate::path::RepoPath::new("fairlead/control.toml").unwrap();
        let d = DiagB::new(HN_R001, &p, 3, 9, "unknown id cmp.sync-buton")
            .text("cmp.sync-buton")
            .dym("cmp.sync-button")
            .build();
        assert_eq!(
            diagnostic_lines(&[d]),
            "error HN-R001  fairlead/control.toml:3:9\n  unknown id cmp.sync-buton\n  | cmp.sync-buton\n  | ^^^^^^^^^^^^^^\n  fix:    did you mean cmp.sync-button\n"
        );
    }

    fn all_formats() -> [OutputFormat; 4] {
        [
            OutputFormat::Human,
            OutputFormat::Terse,
            OutputFormat::Json,
            OutputFormat::Sarif,
        ]
    }
    fn opts_for(f: OutputFormat) -> RenderOptions {
        RenderOptions {
            format: f,
            ..opts()
        }
    }
    fn sample() -> CheckReport {
        report(vec![
            finding(
                HN_T001,
                Severity::Error,
                "apps/web/src/Panel.tsx",
                44,
                18,
                "literal #2563eb",
            ),
            finding(
                HN_R001,
                Severity::Error,
                "apps/web/src/Sync.tsx",
                7,
                3,
                "unknown id",
            ),
            finding(
                HN_X004,
                Severity::Warn,
                "apps/web/src/Legacy.tsx",
                12,
                0,
                "stale ovr",
            ),
        ])
    }

    #[test]
    fn r_arch_054__all_four_formats_are_available() {
        for f in all_formats() {
            assert_eq!(renderer(f).format(), f);
        }
    }

    #[test]
    fn r_agt_023__context_and_explain_text_is_redacted_and_lf_only() {
        let text = format!("a\r\nkey ghp_{}\r\n\r\n", "a1B2".repeat(9));
        assert_eq!(finish_text(&text), "a\nkey [redacted:github-token]\n");
    }

    #[test]
    fn r_chk_011__every_format_is_byte_stable_order_independent_and_machine_free() {
        let a = sample();
        let mut b = a.clone();
        b.findings.reverse();
        for f in all_formats() {
            let o = opts_for(f);
            let out = renderer(f).check(&a, &o);
            assert_eq!(out, renderer(f).check(&a, &o), "{f:?} twice");
            assert_eq!(out, renderer(f).check(&b, &o), "{f:?} input order");
            assert!(
                !out.contains('\r') && out.ends_with('\n') && !out.ends_with("\n\n"),
                "{f:?}"
            );
            assert!(
                !out.contains(":\\\\") && !out.contains("/Users/") && !out.contains("/home/"),
                "{f:?} leaks a machine path"
            );
        }
    }

    #[test]
    fn r_chk_001__every_format_prints_the_diagnostics_and_a_summary() {
        let r = sample();
        for f in [OutputFormat::Human, OutputFormat::Terse] {
            let out = renderer(f).check(&r, &opts_for(f));
            assert!(
                out.contains("errors 2 warn 1 info 0")
                    && out.contains("HN-T001")
                    && out.contains("exceptions: +0"),
                "{f:?}"
            );
        }
        let json = renderer(OutputFormat::Json).check(&r, &opts_for(OutputFormat::Json));
        assert!(json.contains("\"errors\": 2"));
        let sarif = renderer(OutputFormat::Sarif).check(&r, &opts_for(OutputFormat::Sarif));
        assert!(sarif.contains("\"ruleId\": \"HN-T001\""));
    }

    #[test]
    fn r_cli_035__explain_output_renders_terse_and_json_only() {
        use crate::explain::{ExplainOutput, ExplainSection, ExplainTarget};
        let out = ExplainOutput {
            target: ExplainTarget::Topic("syntax".into()),
            sections: vec![ExplainSection {
                title: "syntax".into(),
                prose: vec!["six verbs".into()],
                lines: vec![crate::render::line::TerseLine::new("def").with("k", "v")],
            }],
        };
        assert_eq!(
            renderer(OutputFormat::Terse).explain(&out).as_deref(),
            Some("syntax:\nsix verbs\n  def k=v\n")
        );
        let json = renderer(OutputFormat::Json).explain(&out).unwrap();
        assert!(json.contains("\"target\": \"syntax\""), "{json}");
        assert!(renderer(OutputFormat::Human).explain(&out).is_none());
        assert!(renderer(OutputFormat::Sarif).explain(&out).is_none());
    }
}
