//! `human` renderer (R-CHK-035, R-CHK-036, R-ARCH-103). Colour is applied only by this module.

use super::line::quote;
use super::sanitize::{FIX_CAP, INTENT_CAP, LINE_CAP, clean_data};
use super::terse::TerseRenderer;
use super::{
    OutputFormat, RenderOptions, Renderer, build_view, counts_line, excerpt_lines, finish,
    header_line, location_text, more_line, sev_word, tail_lines,
};
use crate::check::{CheckReport, Finding, FindingState};
use crate::diag::Severity;

pub struct HumanRenderer;

fn paint(word: &str, severity: Severity, color: bool) -> String {
    if !color {
        return word.to_string();
    }
    let code = match severity {
        Severity::Error => "1;31",
        Severity::Warn => "33",
        Severity::Info => "36",
    };
    format!("\u{1b}[{code}m{word}\u{1b}[0m")
}

fn intent_line(f: &Finding) -> Option<String> {
    let intent = f
        .intent
        .as_deref()
        .map(|i| clean_data(i, INTENT_CAP))
        .filter(|i| !i.is_empty());
    let rule = f.rule.as_ref().map(|r| format!("rule.{}", r.0.as_str()));
    let who = match (rule, intent) {
        (Some(r), Some(i)) => format!("{r} {}", quote(&i)),
        (Some(r), None) => r,
        (None, Some(i)) => quote(&i),
        (None, None) => return None,
    };
    Some(format!("  intent: {who} ({})", f.scope.0.as_str()))
}

impl Renderer for HumanRenderer {
    fn format(&self) -> OutputFormat {
        OutputFormat::Human
    }

    fn check(&self, report: &CheckReport, opts: &RenderOptions) -> String {
        if opts.for_agent {
            let terse = RenderOptions {
                format: OutputFormat::Terse,
                ..opts.clone()
            };
            return TerseRenderer.check(report, &terse);
        }
        let view = build_view(report, opts);
        let mut out: Vec<String> = Vec::new();
        if !opts.quiet {
            out.push(header_line(report));
        }
        out.push(counts_line(report));
        for g in &view.groups {
            let f = g.lead;
            let note = if matches!(f.state, FindingState::WaiverIneffective) {
                "  [waiver ineffective]"
            } else {
                ""
            };
            out.push(String::new());
            out.push(format!(
                "{} {}  {}{note}",
                paint(sev_word(f.severity), f.severity, opts.color),
                f.code.0,
                location_text(g)
            ));
            out.push(format!("  {}", clean_data(&f.message, LINE_CAP)));
            if g.count == 1 {
                out.extend(excerpt_lines(f.excerpt.as_deref(), "  "));
            }
            out.extend(intent_line(f));
            let fix = clean_data(&f.fix, FIX_CAP);
            if !fix.is_empty() {
                out.push(format!(
                    "  fix:    {fix}     explain: fairlead explain {}",
                    f.code.0
                ));
            }
        }
        if !view.groups.is_empty() {
            out.push(String::new());
        }
        out.extend(more_line(view.hidden, &view.hidden_codes));
        out.extend(tail_lines(report, opts));
        finish(out.join("\n"))
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::MaxFindings;
    use crate::diag::codes::{HN_F001, HN_R001, HN_T001};
    use crate::render::testkit::{finding, report, rule, scope, with};

    fn opts() -> RenderOptions {
        RenderOptions {
            format: OutputFormat::Human,
            for_agent: false,
            max: MaxFindings::Default,
            show_observe: false,
            color: false,
            quiet: false,
        }
    }

    fn one() -> CheckReport {
        report(vec![with(
            finding(
                HN_T001,
                Severity::Error,
                "apps/web/src/Panel.tsx",
                44,
                18,
                "literal #2563eb is tok.primary",
            ),
            |f| {
                f.rule = Some(rule("no-raw-hex"));
                f.scope = scope("web");
                f.intent = Some("colours come from tokens".to_string());
                f.fix = "use var(--primary)".to_string();
            },
        )])
    }

    #[test]
    fn r_arch_103_r_chk_035__block_layout_matches_the_spec_example() {
        let out = HumanRenderer.check(&one(), &opts());
        let want = "\
fairlead check 0.1.0 edition=1 as-of=2026-10-08 scopes=3 files=1204 rules=7
errors 1 warn 0 info 0 | baselined 0 | waived 0 | observe 0 | accepted 0

error HN-T001  apps/web/src/Panel.tsx:44:18
  literal #2563eb is tok.primary
  intent: rule.no-raw-hex \"colours come from tokens\" (web)
  fix:    use var(--primary)     explain: fairlead explain HN-T001

waivers: none | relaxations: none
exceptions: +0
";
        assert_eq!(out, want);
    }

    #[test]
    fn r_arch_103__the_excerpt_is_shown_with_a_caret() {
        let mut r = one();
        if let Some(f) = r.findings.first_mut() {
            f.excerpt = Some("#2563eb".into());
        }
        let out = HumanRenderer.check(&r, &opts());
        assert!(
            out.contains("  literal #2563eb is tok.primary\n  | #2563eb\n  | ^^^^^^^\n"),
            "{out}"
        );
    }

    #[test]
    fn r_cli_006__colour_only_on_the_severity_word_and_only_when_asked() {
        let plain = HumanRenderer.check(&one(), &opts());
        assert!(!plain.contains('\u{1b}'));
        let coloured = HumanRenderer.check(
            &one(),
            &RenderOptions {
                color: true,
                ..opts()
            },
        );
        assert!(coloured.contains("\u{1b}[1;31merror\u{1b}[0m HN-T001"));
        assert_eq!(coloured.matches('\u{1b}').count(), 2);
    }

    #[test]
    fn r_agt_017__escape_sequences_in_repository_text_are_stripped_even_with_colour() {
        let f = finding(
            HN_T001,
            Severity::Warn,
            "a.rs",
            1,
            1,
            "bad \u{1b}[31mred\u{1b}[0m \u{202e}text",
        );
        let colour = RenderOptions {
            color: true,
            ..opts()
        };
        let out = HumanRenderer.check(&report(vec![f]), &colour);
        assert!(out.contains("  bad red text\n"), "{out}");
        assert_eq!(
            out.matches('\u{1b}').count(),
            2,
            "only our own severity colour"
        );
    }

    #[test]
    fn r_chk_038__floor_findings_come_first_and_the_cap_is_20() {
        let mut findings: Vec<_> = (0..25)
            .map(|i| {
                finding(
                    HN_R001,
                    Severity::Error,
                    &format!("src/f{i:02}.rs"),
                    1,
                    1,
                    "x",
                )
            })
            .collect();
        findings.push(finding(
            HN_F001,
            Severity::Error,
            "z-secret.md",
            1,
            1,
            "secret-like value",
        ));
        let out = HumanRenderer.check(&report(findings), &opts());
        let blocks: Vec<&str> = out.lines().filter(|l| l.starts_with("error ")).collect();
        assert_eq!(blocks.len(), 20);
        assert!(
            blocks[0].starts_with("error HN-F001  z-secret.md"),
            "{}",
            blocks[0]
        );
        assert!(out.contains("+6 more -> fairlead check --rule HN-R001 --max 0"));
    }

    #[test]
    fn r_chk_039__for_agent_on_the_human_format_falls_back_to_terse() {
        let human = HumanRenderer.check(
            &one(),
            &RenderOptions {
                for_agent: true,
                ..opts()
            },
        );
        let terse = TerseRenderer.check(
            &one(),
            &RenderOptions {
                for_agent: true,
                format: OutputFormat::Terse,
                ..opts()
            },
        );
        assert_eq!(human, terse);
    }
}
