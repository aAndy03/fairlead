//! `fairlead explain` (R-CLI-035). Pure; text compiled in, works offline.
pub mod topics;

use crate::check::{CheckReport, all_specs, is_active, spec};
use crate::error::Error;
use crate::path::RepoPath;
use crate::registry::Registry;
use crate::render::line::{TerseLine, TerseValue};
use crate::render::sanitize::sanitize_data;
use fairlead_syntax::diag::{DiagCode, Severity};
use fairlead_syntax::ids::Id;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExplainTarget {
    File(RepoPath),
    Id(Id),
    Code(DiagCode),
    Topic(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExplainSection {
    pub title: String,
    pub prose: Vec<String>,
    pub lines: Vec<TerseLine>,
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExplainOutput {
    pub target: ExplainTarget,
    pub sections: Vec<ExplainSection>,
}

impl ExplainTarget {
    /// The argument as the user would type it.
    pub fn text(&self) -> String {
        match self {
            ExplainTarget::File(p) => p.as_str().to_string(),
            ExplainTarget::Id(id) => id.to_string(),
            ExplainTarget::Code(c) => c.0.to_string(),
            ExplainTarget::Topic(t) => t.clone(),
        }
    }
}

impl ExplainOutput {
    /// A-20: terse text. Each section is `<title>:`, then its prose, then its `ID k=v` lines indented.
    pub fn render_terse(&self) -> String {
        let mut out = String::new();
        for s in &self.sections {
            out.push_str(&s.title);
            out.push_str(":\n");
            for p in &s.prose {
                out.push_str(p);
                out.push('\n');
            }
            for l in &s.lines {
                out.push_str("  ");
                out.push_str(&l.render());
                out.push('\n');
            }
        }
        out
    }

    /// A-20: the same sections as a JSON value (lines rendered in their terse form).
    pub fn to_json_value(&self) -> serde_json::Value {
        let sections: Vec<serde_json::Value> = self
            .sections
            .iter()
            .map(|s| {
                serde_json::json!({
                    "title": s.title,
                    "prose": s.prose,
                    "lines": s.lines.iter().map(TerseLine::render).collect::<Vec<_>>(),
                })
            })
            .collect();
        serde_json::json!({ "target": self.target.text(), "sections": sections })
    }
}

pub fn parse_explain_target(
    arg: &str,
    exists: &dyn Fn(&RepoPath) -> bool,
) -> Result<ExplainTarget, Error> {
    if topics::TOPICS.contains(&arg) || arg == "pr-template" {
        return Ok(ExplainTarget::Topic(arg.to_owned()));
    }
    if arg.starts_with("HN-") {
        return all_specs()
            .iter()
            .find(|s| s.code.0 == arg)
            .map(|s| ExplainTarget::Code(s.code))
            .ok_or_else(|| {
                Error::Usage(format!(
                    "unknown diagnostic code {}",
                    sanitize_data(arg, 40)
                ))
            });
    }
    if let Ok(p) = RepoPath::new(arg)
        && exists(&p)
    {
        return Ok(ExplainTarget::File(p));
    }
    if let Ok(id) = Id::parse(arg) {
        return Ok(ExplainTarget::Id(id));
    }
    Err(Error::Usage(format!(
        "{:?} is not a topic, code, existing path or ID; topics: syntax, kinds, approve",
        sanitize_data(arg, 60)
    )))
}

pub(crate) fn val(v: &str) -> TerseValue {
    let s = sanitize_data(v, 240);
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:/@*".contains(c))
    {
        TerseValue::Word(s)
    } else {
        TerseValue::Quoted(s)
    }
}
pub(crate) fn tl(id: &str, pairs: &[(&str, &str)]) -> TerseLine {
    TerseLine {
        id: sanitize_data(id, 240),
        pairs: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), val(v)))
            .collect(),
    }
}
pub(crate) fn section(title: &str, prose: Vec<String>, lines: Vec<TerseLine>) -> ExplainSection {
    ExplainSection {
        title: title.to_owned(),
        prose,
        lines,
    }
}

fn code_sections(c: DiagCode) -> Result<Vec<ExplainSection>, Error> {
    let s = spec(c).ok_or_else(|| Error::Usage(format!("unknown diagnostic code {}", c.0)))?;
    let mut meaning = vec![format!("{} ({}): {}", s.code.0, s.mnemonic, s.title)];
    if !is_active(c) {
        meaning.push("inactive in 0.1; the code is reserved and its number is stable".into());
    }
    let sev = match s.default_severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    };
    let stage = format!("{:?}", s.default_stage).to_lowercase();
    let yn = |b: bool| if b { "yes" } else { "no" };
    let facts = tl(
        s.code.0,
        &[
            ("sev", sev),
            ("stage", &stage),
            ("floor", yn(s.floor)),
            ("waivable", yn(s.waivable)),
        ],
    );
    let (why, example, fix) = match topics::card(c.0) {
        Some(k) => (k.why, k.example, k.fix),
        None => (
            "See 13-checks-and-diagnostics.md for the rule behind this code.",
            "n/a",
            "Follow the one-line fix hint printed with the finding.",
        ),
    };
    Ok(vec![
        section("meaning", meaning, vec![facts]),
        section("why", vec![why.into()], vec![]),
        section("example", vec![example.into()], vec![]),
        section("fix", vec![fix.into()], vec![]),
    ])
}

pub fn explain(
    target: &ExplainTarget,
    reg: &Registry,
    report: Option<&CheckReport>,
) -> Result<ExplainOutput, Error> {
    let sections = match target {
        ExplainTarget::Topic(t) => topics::topic_sections(t, reg)?,
        ExplainTarget::Code(c) => code_sections(*c)?,
        ExplainTarget::File(p) => file_sections(p, reg, report)?,
        ExplainTarget::Id(id) => id_sections(id, reg)?,
    };
    Ok(ExplainOutput {
        target: target.clone(),
        sections,
    })
}

use crate::check::rules::RuleSet;
use crate::config::Policy;
use crate::registry::{FieldValue, LedgerEntry, Lookup, Origin};

fn policy_word(p: Policy) -> &'static str {
    match p {
        Policy::Open => "open",
        Policy::NeedsException => "needs-exception",
        Policy::Sealed => "sealed",
    }
}
fn field_text(v: &FieldValue) -> String {
    match v {
        FieldValue::Scalar(s) => s.clone(),
        FieldValue::List(l) => l.join(","),
    }
}

/// R-SCOPE-042: one record, two readings.
pub(crate) fn relativity_lines(e: &LedgerEntry) -> [TerseLine; 2] {
    let id = e.target.to_id().to_string();
    let exp = e.exp.map(|d| d.to_string()).unwrap_or_default();
    [
        tl(
            &id,
            &[
                ("reading", "inside"),
                ("scope", e.varied_in.0.as_str()),
                ("effective", "the patched value is the principle here"),
                ("from", e.ovr.0.as_str()),
            ],
        ),
        tl(
            &id,
            &[
                ("reading", "outside"),
                ("varied-in", e.varied_in.0.as_str()),
                ("home", e.home.0.as_str()),
                ("depth", &e.depth.to_string()),
                ("owner", e.owner.as_deref().unwrap_or("")),
                ("exp", &exp),
                ("why", e.why.as_deref().unwrap_or("")),
            ],
        ),
    ]
}

fn file_sections(
    path: &RepoPath,
    reg: &Registry,
    report: Option<&CheckReport>,
) -> Result<Vec<ExplainSection>, Error> {
    let scope = reg
        .scope_of(path)
        .cloned()
        .ok_or_else(|| Error::Usage(format!("{} is outside every scope", path.as_str())))?;
    let chain: Vec<TerseLine> = reg
        .chain(&scope)
        .into_iter()
        .filter_map(|s| reg.scope(s))
        .map(|s| {
            tl(
                s.id.0.as_str(),
                &[("depth", &s.depth.to_string()), ("dir", s.dir.as_str())],
            )
        })
        .collect();
    let rules = RuleSet::collect(reg);
    let view = reg.view(&scope);
    let (mut applicable, mut values) = (Vec::new(), Vec::new());
    for r in rules
        .for_scope(&scope)
        .iter()
        .filter(|r| rules.applies(r, path, reg))
    {
        let rid = r.id.to_string(); // Verify: RuleId: Display = "rule.<name>"
        applicable.push(tl(
            &rid,
            &[
                ("sev", &format!("{:?}", r.sev).to_lowercase()),
                ("stage", &format!("{:?}", r.stage).to_lowercase()),
                ("home", r.home.0.as_str()),
            ],
        ));
        if let Some(item) = view
            .items
            .values()
            .find(|i| i.id.to_id().kind_name() == rid)
        {
            for (f, ef) in &item.fields {
                let applied = ef
                    .provenance
                    .applied
                    .iter()
                    .map(|o| o.0.as_str().to_owned())
                    .collect::<Vec<_>>()
                    .join(",");
                values.push(tl(
                    &format!("{rid}.{f}"),
                    &[
                        ("value", &field_text(&ef.value)),
                        ("home", ef.provenance.home.0.as_str()),
                        ("applied", &applied),
                    ],
                ));
            }
        }
    }
    let mut out = vec![
        section("scope chain", vec![], chain),
        section("applicable rules", vec![], applicable),
        section("effective values", vec![], values),
    ];
    if let Some(rep) = report {
        let found: Vec<TerseLine> = rep
            .findings
            .iter()
            .filter(|f| f.location.path == *path)
            .take(20)
            .map(|f| {
                tl(
                    f.code.0,
                    &[("line", &f.location.line.to_string()), ("msg", &f.message)],
                )
            })
            .collect();
        out.push(section("findings", vec![], found));
    }
    Ok(out)
}

fn id_sections(id: &Id, reg: &Registry) -> Result<Vec<ExplainSection>, Error> {
    let item_id = match reg.lookup(reg.root_scope(), id) {
        Lookup::Found(i) => i,
        Lookup::Ambiguous(c) => {
            let list = c
                .iter()
                .map(|i| i.to_id().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Error::Usage(format!(
                "{id} is ambiguous; qualify it as one of: {list}"
            )));
        }
        Lookup::Missing { nearest } => {
            let hint = if nearest.is_empty() {
                String::new()
            } else {
                format!(
                    "; did you mean: {}",
                    nearest
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            return Err(Error::Usage(sanitize_data(
                &format!("no declaration of {id}{hint}"),
                240,
            )));
        }
    };
    let item = reg
        .item(&item_id)
        .ok_or_else(|| Error::Internal("lookup returned an unknown item".into()))?;
    let origin = match &item.origin {
        Origin::ControlTable { control } => control.as_str().to_owned(),
        Origin::DocDef { path, span } => format!("{}:{}", path.as_str(), span.line),
    };
    let qid = item_id.to_id().to_string();
    let decl = tl(
        &qid,
        &[
            ("home", item_id.scope.0.as_str()),
            ("policy", policy_word(item.policy)),
            ("origin", &origin),
            ("sum", item.sum.as_deref().unwrap_or("")),
        ],
    );
    let sites: Vec<TerseLine> = reg
        .sites_of(&item_id)
        .iter()
        .map(|s| {
            tl(
                s.path.as_str(),
                &[
                    ("line", &s.span.line.to_string()),
                    ("scope", s.scope.0.as_str()),
                ],
            )
        })
        .collect();
    let uses: Vec<TerseLine> = reg
        .edges()
        .iter()
        .filter(|e| e.resolved.as_ref() == Some(&item_id))
        .map(|e| {
            tl(
                e.from.path.as_str(),
                &[("line", &e.from.span.line.to_string())],
            )
        })
        .collect();
    let readings: Vec<TerseLine> = reg
        .ledger(&item_id.scope)
        .entries
        .iter()
        .filter(|e| e.target == item_id)
        .flat_map(relativity_lines)
        .collect();
    let shadow: Vec<String> = reg
        .diagnostics()
        .iter()
        .filter(|d| {
            d.related.as_deref() == Some(qid.as_str())
                || d.related.as_deref() == Some(&id.to_string())
        })
        .map(|d| format!("{} {}", d.code.0, d.message))
        .collect();
    Ok(vec![
        section("declaration", vec![], vec![decl]),
        section("sites", vec![], sites),
        section("uses", vec![], uses),
        section("readings", vec![], readings),
        section("shadowing", shadow, vec![]),
    ])
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::{all_specs, is_active};
    use crate::write::testutil::*;

    fn target(arg: &str, exists: bool) -> Result<ExplainTarget, Error> {
        parse_explain_target(arg, &|_| exists)
    }
    fn env() -> Registry {
        registry(
            &[
                ("fairlead/control.toml", ROOT_CONTROL),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        )
        .1
    }
    fn text(o: &ExplainOutput) -> String {
        o.sections
            .iter()
            .flat_map(|s| {
                s.prose
                    .iter()
                    .cloned()
                    .chain(s.lines.iter().map(|l| l.render()))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn r_cli_035__dispatch_is_topic_then_code_then_path_then_id_else_usage() {
        assert_eq!(
            target("syntax", true).unwrap(),
            ExplainTarget::Topic("syntax".into()),
            "a topic beats an existing file"
        );
        assert_eq!(
            target("pr-template", false).unwrap(),
            ExplainTarget::Topic("pr-template".into())
        );
        assert!(
            matches!(target("HN-T001", false).unwrap(), ExplainTarget::Code(c) if c.0 == "HN-T001")
        );
        assert!(
            matches!(target("HN-Z999", true), Err(Error::Usage(m)) if m.contains("unknown diagnostic code"))
        );
        assert!(
            matches!(target("src/lib.rs", true).unwrap(), ExplainTarget::File(p) if p.as_str() == "src/lib.rs")
        );
        assert!(
            matches!(
                target("rule.no-raw-hex", true).unwrap(),
                ExplainTarget::File(_)
            ),
            "an existing path wins over the ID grammar"
        );
        assert!(matches!(
            target("rule.no-raw-hex", false).unwrap(),
            ExplainTarget::Id(_)
        ));
        assert!(matches!(
            target("web:cmp.button", false).unwrap(),
            ExplainTarget::Id(_)
        ));
        assert!(
            matches!(target("!! nonsense", false), Err(Error::Usage(m)) if m.contains("syntax, kinds, approve"))
        );
    }

    #[test]
    fn r_cli_035__syntax_card_names_the_six_verbs_and_fits_800_tokens() {
        let c = topics::SYNTAX_CARD;
        for verb in ["def", "use", "ovr", "flag", "env", "end"] {
            assert!(c.contains(&format!("\n  {verb} ")), "{verb}");
        }
        assert!(c.len().div_ceil(4) <= 800, "{} bytes", c.len());
        let o = explain(&ExplainTarget::Topic("syntax".into()), &env(), None).unwrap();
        assert!(text(&o).contains("fairlead syntax card (edition 1)"));
    }

    #[test]
    fn r_agt_026__explain_approve_states_the_limit_and_that_it_is_not_authentication() {
        let t = text(&explain(&ExplainTarget::Topic("approve".into()), &env(), None).unwrap());
        assert!(
            t.contains("pseudo-terminal")
                && t.contains("CODEOWNERS")
                && t.contains("not authentication"),
            "{t}"
        );
    }

    #[test]
    fn r_cli_039__approve_is_a_convenience_record_and_the_review_gate_is_codeowners() {
        assert!(
            topics::APPROVE_TEXT.contains("convenience record")
                && topics::APPROVE_TEXT.contains("CODEOWNERS")
        );
    }

    #[test]
    fn r_agt_025__explain_hn_f001_says_history_needs_a_human_rewrite_and_never_echoes_values() {
        let t = text(&explain(&ExplainTarget::Code(spec_code("HN-F001")), &env(), None).unwrap());
        assert!(
            t.contains("history") && t.contains("rotation") && t.contains("env:"),
            "{t}"
        );
    }

    fn spec_code(s: &str) -> fairlead_syntax::diag::DiagCode {
        all_specs().iter().find(|x| x.code.0 == s).unwrap().code
    }

    #[test]
    fn r_cli_035__every_known_code_explains_with_four_sections_and_inactive_ones_say_so() {
        let reg = env();
        for s in all_specs() {
            let o = explain(&ExplainTarget::Code(s.code), &reg, None).unwrap();
            let titles: Vec<&str> = o.sections.iter().map(|x| x.title.as_str()).collect();
            assert_eq!(titles, ["meaning", "why", "example", "fix"], "{}", s.code.0);
            assert!(text(&o).contains(s.title), "{}", s.code.0);
            assert_eq!(
                text(&o).contains("inactive in 0.1"),
                !is_active(s.code),
                "{}",
                s.code.0
            );
        }
    }

    #[test]
    fn r_cli_035__kinds_topic_is_computed_from_the_registry_and_pr_template_waits_for_0_2() {
        let t = text(&explain(&ExplainTarget::Topic("kinds".into()), &env(), None).unwrap());
        assert!(
            t.contains("adr") && t.contains("cmp") && t.contains("handoff"),
            "{t}"
        );
        assert!(
            matches!(explain(&ExplainTarget::Topic("pr-template".into()), &env(), None), Err(Error::Usage(m)) if m.contains("0.2"))
        );
    }

    use crate::registry::{LedgerEntry, OvrId, OvrState, ScopeId};

    const RULE: &str = "\n[rules.no-raw-hex]\ncheck = \"pattern\"\nforbid = \"#[0-9a-f]{6}\"\nmsg = \"use tokens\"\nsev = \"error\"\nstage = \"enforce\"\nglob = [\"**/*.tsx\"]\nwhy = \"colours come from tokens\"\n";
    fn env_rule() -> Registry {
        registry(
            &[
                ("fairlead/control.toml", &format!("{ROOT_CONTROL}{RULE}")),
                ("apps/web/fairlead/control.toml", WEB_CONTROL),
            ],
            "2026-10-09",
        )
        .1
    }
    fn word(l: &TerseLine, k: &str) -> String {
        match &l.pairs.iter().find(|(key, _)| key == k).unwrap().1 {
            TerseValue::Word(w) | TerseValue::Quoted(w) => w.clone(),
        }
    }

    #[test]
    fn r_cli_035__file_explain_prints_the_chain_the_applicable_rules_and_value_origins() {
        let o = explain(
            &ExplainTarget::File(rp("apps/web/src/a.tsx")),
            &env_rule(),
            None,
        )
        .unwrap();
        let titles: Vec<&str> = o.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["scope chain", "applicable rules", "effective values"]
        );
        let chain: Vec<&str> = o.sections[0].lines.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(chain, ["root", "web"], "root first, nearest last");
        assert_eq!(word(&o.sections[0].lines[1], "depth"), "1");
        assert!(
            o.sections[1]
                .lines
                .iter()
                .any(|l| l.id == "rule.no-raw-hex")
        );
        let sev = o.sections[2]
            .lines
            .iter()
            .find(|l| l.id == "rule.no-raw-hex.sev")
            .unwrap();
        assert_eq!(
            (word(sev, "value").as_str(), word(sev, "home").as_str()),
            ("error", "root")
        );
        let css = explain(
            &ExplainTarget::File(rp("apps/web/src/a.css")),
            &env_rule(),
            None,
        )
        .unwrap();
        assert!(
            css.sections[1].lines.is_empty(),
            "the glob does not match a .css file"
        );
    }

    #[test]
    fn r_cli_035__id_explain_prints_declaration_policy_and_home_and_unknown_ids_get_suggestions() {
        let reg = env_rule();
        let id = fairlead_syntax::ids::Id::parse("rule.no-raw-hex").unwrap();
        let o = explain(&ExplainTarget::Id(id), &reg, None).unwrap();
        let decl = &o.sections[0];
        assert_eq!(decl.title, "declaration");
        assert_eq!(word(&decl.lines[0], "home"), "root");
        assert!(
            o.sections.iter().any(|s| s.title == "sites")
                && o.sections.iter().any(|s| s.title == "readings")
        );
        let missing = fairlead_syntax::ids::Id::parse("rule.no-raw-hx").unwrap();
        match explain(&ExplainTarget::Id(missing), &reg, None) {
            Err(Error::Usage(m)) => assert!(
                m.contains("no declaration") && m.contains("rule.no-raw-hex"),
                "{m}"
            ),
            other => panic!("{:?}", other.map(|o| o.sections.len())),
        }
    }

    #[test]
    fn r_scope_042__one_record_has_an_inside_and_an_outside_reading_with_distance() {
        let e = LedgerEntry {
            ovr: OvrId(crate::hash::Fingerprint::parse("aaaaaaaaaaaa").unwrap()),
            target: item_id("root", "rule.no-raw-hex"),
            home: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("root").unwrap()),
            varied_in: ScopeId(fairlead_syntax::ids::ScopeSlug::parse("web").unwrap()),
            depth: 1,
            why: Some("chart lib".into()),
            owner: Some("@lead".into()),
            exp: Some(jiff::civil::date(2026, 12, 31)),
            state: OvrState::Effective,
        };
        let [inside, outside] = relativity_lines(&e);
        assert_eq!(
            (
                word(&inside, "reading").as_str(),
                word(&inside, "scope").as_str()
            ),
            ("inside", "web")
        );
        assert!(word(&inside, "effective").contains("principle"));
        assert_eq!(
            (
                word(&outside, "reading").as_str(),
                word(&outside, "varied-in").as_str(),
                word(&outside, "depth").as_str()
            ),
            ("outside", "web", "1")
        );
        assert_eq!(
            (
                word(&outside, "owner").as_str(),
                word(&outside, "exp").as_str()
            ),
            ("@lead", "2026-12-31")
        );
    }

    #[test]
    fn r_agt_017__repository_text_in_explain_output_is_sanitised() {
        let hostile = "ok\u{1b}[31m\u{202e}evil\nline";
        let l = tl("x", &[("why", hostile)]);
        let rendered = l.render();
        assert!(
            !rendered.contains('\u{1b}')
                && !rendered.contains('\u{202e}')
                && !rendered.contains('\n'),
            "{rendered:?}"
        );
    }
}
