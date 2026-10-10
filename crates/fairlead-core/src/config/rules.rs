//! `[rules.*]`, `[policies]`, `[[allow]]`, `[[req]]`, `[[convention]]` and the small root tables.
use crate::classify::FileClass;
use crate::config::schema::{
    BaselineConfig, ContextConfig, ConventionAction, ConventionDecl, Ctx, DocsFirstConfig,
    EligibilityConfig, ExceptionsConfig, Policy, SecretsConfig, Stage, VersionConfig, check_glob,
    clean_pattern, clean_rel_path, join_key, take_bool, take_date_string, take_string,
    take_string_list, take_table, take_table_array, take_u32,
};
use crate::diag::Severity;
use crate::hash::Fingerprint;
use fairlead_syntax::ids::{Id, ScopeSlug};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RuleCheckDecl {
    Leak { tokens: Vec<String> },
    Pattern { forbid: String, msg: String },
    Builtin,
}

/// `[rules.<name>]` (R-CHK-005).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuleDecl {
    pub name: String,
    pub check: RuleCheckDecl,
    pub sev: Option<Severity>,
    pub stage: Option<Stage>,
    pub glob: Vec<String>,
    pub exclude: Vec<String>,
    pub waivable: bool,
    pub why: Option<String>,
    pub since: Option<jiff::civil::Date>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WaiverPolicy {
    pub max_exp_days: u32,
    pub budget: u32,
    pub max_renewals: u32,
    pub approval_required: bool,
}

impl Default for WaiverPolicy {
    fn default() -> Self {
        Self {
            max_exp_days: 90,
            budget: 25,
            max_renewals: 2,
            approval_required: true,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldPolicy {
    pub target: Option<String>,
    pub fields: Vec<String>,
    pub policy: Policy,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PolicyTable {
    pub by_pattern: BTreeMap<String, Policy>,
    pub waiver: WaiverPolicy,
    pub fields: BTreeMap<String, FieldPolicy>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AllowDecl {
    pub target: String,
    pub field: Option<String>,
    pub for_scopes: Vec<String>,
    pub why: String,
    pub exp: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReqDecl {
    pub to: String,
    pub target: String,
    pub set: Option<String>,
    pub why: String,
    pub blocking: bool,
}

fn severity_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Severity> {
    match s {
        "info" => Some(Severity::Info),
        "warn" => Some(Severity::Warn),
        "error" => Some(Severity::Error),
        _ => {
            cx.error(key, format!("`sev` must be info, warn or error, got {s:?}"));
            None
        }
    }
}

fn stage_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Stage> {
    match s {
        "observe" => Some(Stage::Observe),
        "warn" => Some(Stage::Warn),
        "enforce" => Some(Stage::Enforce),
        _ => {
            cx.error(
                key,
                format!("`stage` must be observe, warn or enforce, got {s:?}"),
            );
            None
        }
    }
}

fn policy_of(cx: &mut Ctx<'_>, key: &str, s: &str) -> Option<Policy> {
    match s {
        "open" => Some(Policy::Open),
        "needs-exception" => Some(Policy::NeedsException),
        "sealed" => Some(Policy::Sealed),
        _ => {
            cx.error(
                key,
                format!("policy must be open, needs-exception or sealed, got {s:?}"),
            );
            None
        }
    }
}

pub(crate) fn parse_rules(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BTreeMap<String, RuleDecl> {
    let mut out = BTreeMap::new();
    let Some(rules) = take_table(cx, root, "", "rules") else {
        return out;
    };
    for (name, v) in rules {
        let key = join_key("rules", &name);
        if Id::parse(&format!("rule.{name}")).is_err() {
            cx.error(
                &key,
                format!("rule name {name:?} must be lowercase slugs separated by '.'"),
            );
            continue;
        }
        let Some(mut t) = v.as_table().cloned() else {
            cx.error(&key, format!("`{key}` must be a table"));
            continue;
        };
        let check = match take_string(cx, &mut t, &key, "check").as_deref() {
            None | Some("builtin") => Some(RuleCheckDecl::Builtin),
            Some("leak") => {
                let tokens = take_string_list(cx, &mut t, &key, "tokens").unwrap_or_default();
                if tokens.is_empty() {
                    cx.error(&key, "a `leak` rule needs a non-empty `tokens` list");
                    None
                } else {
                    Some(RuleCheckDecl::Leak { tokens })
                }
            }
            Some("pattern") => {
                let forbid = take_string(cx, &mut t, &key, "forbid");
                let msg = take_string(cx, &mut t, &key, "msg");
                match (forbid, msg) {
                    (Some(forbid), Some(msg)) => match regex::RegexBuilder::new(&forbid)
                        .size_limit(1_000_000)
                        .build()
                    {
                        Ok(_) => Some(RuleCheckDecl::Pattern { forbid, msg }),
                        Err(e) => {
                            cx.error(&key, format!("`forbid` is not a valid regex: {e}"));
                            None
                        }
                    },
                    _ => {
                        cx.error(&key, "a `pattern` rule needs `forbid` and `msg`");
                        None
                    }
                }
            }
            Some(other) => {
                cx.error(
                    &key,
                    format!("unknown check {other:?}; use \"leak\", \"pattern\" or \"builtin\""),
                );
                None
            }
        };
        let sev = take_string(cx, &mut t, &key, "sev").and_then(|s| severity_of(cx, &key, &s));
        let stage = take_string(cx, &mut t, &key, "stage").and_then(|s| stage_of(cx, &key, &s));
        let glob = take_string_list(cx, &mut t, &key, "glob")
            .unwrap_or_default()
            .iter()
            .filter_map(|p| clean_pattern(cx, &key, p))
            .collect();
        let exclude = take_string_list(cx, &mut t, &key, "exclude")
            .unwrap_or_default()
            .iter()
            .filter_map(|p| clean_pattern(cx, &key, p))
            .collect();
        let waivable = take_bool(cx, &mut t, &key, "waivable").unwrap_or(false);
        let why = take_string(cx, &mut t, &key, "why");
        let since = take_date_string(cx, &mut t, &key, "since")
            .and_then(|s| s.parse::<jiff::civil::Date>().ok());
        cx.unknown_in(&key, &t);
        if let Some(check) = check {
            out.insert(
                name.clone(),
                RuleDecl {
                    name,
                    check,
                    sev,
                    stage,
                    glob,
                    exclude,
                    waivable,
                    why,
                    since,
                },
            );
        }
    }
    out
}

fn parse_waiver(cx: &mut Ctx<'_>, mut w: toml::Table) -> WaiverPolicy {
    let mut out = WaiverPolicy::default();
    let bad_exp = "`policies.waiver.max-exp` must be a number of days such as 90 or \"90d\"";
    match w.remove("max-exp") {
        None => {}
        Some(toml::Value::Integer(n)) => match u32::try_from(n) {
            Ok(v) => out.max_exp_days = v,
            Err(_) => cx.error("policies.waiver.max-exp", bad_exp),
        },
        Some(toml::Value::String(s)) => {
            match s.strip_suffix('d').and_then(|d| d.parse::<u32>().ok()) {
                Some(v) => out.max_exp_days = v,
                None => cx.error("policies.waiver.max-exp", bad_exp),
            }
        }
        Some(_) => cx.error("policies.waiver.max-exp", bad_exp),
    }
    if let Some(v) = take_u32(cx, &mut w, "policies.waiver", "budget") {
        out.budget = v;
    }
    if let Some(v) = take_u32(cx, &mut w, "policies.waiver", "max-renewals") {
        out.max_renewals = v;
    }
    match w.remove("approval") {
        None => {}
        Some(toml::Value::Boolean(b)) => out.approval_required = b,
        Some(toml::Value::String(s)) if s == "required" => out.approval_required = true,
        Some(toml::Value::String(s)) if s == "none" => out.approval_required = false,
        Some(_) => cx.error(
            "policies.waiver.approval",
            "approval must be \"required\", \"none\" or a bool",
        ),
    }
    cx.unknown_in("policies.waiver", &w);
    out
}

pub(crate) fn parse_policies(cx: &mut Ctx<'_>, root: &mut toml::Table) -> PolicyTable {
    let mut out = PolicyTable::default();
    let Some(t) = take_table(cx, root, "", "policies") else {
        return out;
    };
    for (k, v) in t {
        let key = join_key("policies", &k);
        match v {
            toml::Value::Table(w) if k == "waiver" => out.waiver = parse_waiver(cx, w),
            toml::Value::String(s) => {
                if let Some(p) = policy_of(cx, &key, &s) {
                    out.by_pattern.insert(k, p);
                }
            }
            toml::Value::Table(mut ft) => {
                let target = take_string(cx, &mut ft, &key, "target");
                let fields = take_string_list(cx, &mut ft, &key, "fields").unwrap_or_default();
                let policy =
                    take_string(cx, &mut ft, &key, "policy").and_then(|s| policy_of(cx, &key, &s));
                cx.unknown_in(&key, &ft);
                match (policy, fields.is_empty()) {
                    (Some(policy), false) => {
                        out.fields.insert(
                            k,
                            FieldPolicy {
                                target,
                                fields,
                                policy,
                            },
                        );
                    }
                    _ => cx.error(
                        &key,
                        format!("`{key}` needs a non-empty `fields` list and a valid `policy`"),
                    ),
                }
            }
            _ => cx.error(&key, format!("`{key}` must be a policy string or a table")),
        }
    }
    out
}

fn scope_ids(cx: &mut Ctx<'_>, key: &str, raw: &[String]) -> Option<Vec<String>> {
    let mut ok = true;
    for s in raw {
        if ScopeSlug::parse(s).is_err() {
            cx.error(key, format!("{s:?} is not a valid scope id"));
            ok = false;
        }
    }
    ok.then(|| raw.to_vec())
}

pub(crate) fn parse_allows(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<AllowDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "allow") {
        let before = cx.diags.len();
        let target = take_string(cx, &mut t, "allow", "target");
        let field = take_string(cx, &mut t, "allow", "field");
        let for_raw = take_string_list(cx, &mut t, "allow", "for");
        let why = take_string(cx, &mut t, "allow", "why");
        let exp = take_date_string(cx, &mut t, "allow", "exp");
        cx.unknown_in("allow", &t);
        let for_scopes = for_raw.and_then(|l| scope_ids(cx, "allow.for", &l));
        match (target, for_scopes, why, exp) {
            (Some(target), Some(for_scopes), Some(why), Some(exp)) if !for_scopes.is_empty() => {
                out.push(AllowDecl {
                    target,
                    field,
                    for_scopes,
                    why,
                    exp,
                });
            }
            _ => {
                if cx.diags.len() == before {
                    cx.error(
                        "allow",
                        "an [[allow]] needs `target`, a non-empty `for`, `why` and `exp`",
                    );
                }
            }
        }
    }
    out
}

pub(crate) fn parse_reqs(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<ReqDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "req") {
        let before = cx.diags.len();
        let to = take_string(cx, &mut t, "req", "to");
        let target = take_string(cx, &mut t, "req", "target");
        let set = take_string(cx, &mut t, "req", "set");
        let why = take_string(cx, &mut t, "req", "why");
        let blocking = take_bool(cx, &mut t, "req", "blocking").unwrap_or(false);
        cx.unknown_in("req", &t);
        let to_ok = to
            .as_ref()
            .and_then(|s| scope_ids(cx, "req.to", std::slice::from_ref(s)));
        match (to, to_ok, target, why) {
            (Some(to), Some(_), Some(target), Some(why)) => out.push(ReqDecl {
                to,
                target,
                set,
                why,
                blocking,
            }),
            _ => {
                if cx.diags.len() == before {
                    cx.error("req", "a [[req]] needs `to`, `target` and `why`");
                }
            }
        }
    }
    out
}

pub(crate) fn parse_conventions(cx: &mut Ctx<'_>, root: &mut toml::Table) -> Vec<ConventionDecl> {
    let mut out = Vec::new();
    for mut t in take_table_array(cx, root, "", "convention") {
        let before = cx.diags.len();
        let glob = take_string(cx, &mut t, "convention", "glob")
            .and_then(|g| clean_rel_path(cx, "convention.glob", &g))
            .filter(|g| check_glob(cx, "convention.glob", g));
        let transparent = take_bool(cx, &mut t, "convention", "transparent").unwrap_or(false);
        let class = take_string(cx, &mut t, "convention", "class");
        cx.unknown_in("convention", &t);
        let class = match class.as_deref() {
            None => None,
            Some("external-generated") => Some(FileClass::ExternalGenerated),
            Some("vendor-owned") => Some(FileClass::VendorOwned),
            Some(other) => {
                cx.error("convention.class", format!("class {other:?} is not supported; use \"external-generated\" or \"vendor-owned\""));
                None
            }
        };
        let action = match (transparent, class) {
            (true, None) => Some(ConventionAction::Transparent),
            (false, Some(c)) => Some(ConventionAction::Class(c)),
            _ => None,
        };
        match (glob, action) {
            (Some(glob), Some(action)) => out.push(ConventionDecl { glob, action }),
            _ => {
                if cx.diags.len() == before {
                    cx.error("convention", "a [[convention]] needs `glob` and exactly one of `transparent = true` or `class`");
                }
            }
        }
    }
    out
}

pub(crate) fn parse_context(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ContextConfig {
    let mut out = ContextConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "context") {
        if let Some(v) = take_u32(cx, &mut t, "context", "budget-start") {
            out.budget_start = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "context", "budget-path") {
            out.budget_path = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "context", "bytes-per-token") {
            if v == 0 {
                cx.error(
                    "context.bytes-per-token",
                    "bytes-per-token must be at least 1",
                );
            } else {
                out.bytes_per_token = v;
            }
        }
        out.tokenizer = take_string(cx, &mut t, "context", "tokenizer");
        if let Some(mut w) = take_table(cx, &mut t, "context", "sev-weight") {
            for (key, slot) in [
                ("error", &mut out.sev_weight.error),
                ("warn", &mut out.sev_weight.warn),
                ("info", &mut out.sev_weight.info),
            ] {
                if let Some(v) = take_u32(cx, &mut w, "context.sev-weight", key) {
                    *slot = v;
                }
            }
            cx.unknown_in("context.sev-weight", &w);
        }
        cx.unknown_in("context", &t);
    }
    out
}

pub(crate) fn parse_baseline(cx: &mut Ctx<'_>, root: &mut toml::Table) -> BaselineConfig {
    let mut out = BaselineConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "baseline") {
        if let Some(f) = take_string(cx, &mut t, "baseline", "file")
            && let Some(clean) = clean_rel_path(cx, "baseline.file", &f)
        {
            out.file = clean;
        }
        cx.unknown_in("baseline", &t);
    }
    out
}

pub(crate) fn parse_secrets(cx: &mut Ctx<'_>, root: &mut toml::Table) -> SecretsConfig {
    let mut out = SecretsConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "secrets") {
        for fp in take_string_list(cx, &mut t, "secrets", "allow").unwrap_or_default() {
            match Fingerprint::parse(&fp) {
                Ok(_) => out.allow.push(fp),
                Err(e) => cx.error(
                    "secrets.allow",
                    format!("{fp:?} is not a 12-character fingerprint: {e}"),
                ),
            }
        }
        cx.unknown_in("secrets", &t);
    }
    out
}

pub(crate) fn parse_exceptions(cx: &mut Ctx<'_>, root: &mut toml::Table) -> ExceptionsConfig {
    let mut out = ExceptionsConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "exceptions") {
        out.max_days = take_u32(cx, &mut t, "exceptions", "max-days");
        if let Some(v) = take_u32(cx, &mut t, "exceptions", "max-live-per-rule") {
            out.max_live_per_rule = v;
        }
        out.max_minors = take_u32(cx, &mut t, "exceptions", "max-minors");
        cx.unknown_in("exceptions", &t);
    }
    out
}

pub(crate) fn parse_version(cx: &mut Ctx<'_>, root: &mut toml::Table) -> VersionConfig {
    let mut out = VersionConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "version") {
        if let Some(s) = take_string(cx, &mut t, "version", "start") {
            if crate::version::Version::parse(&s).is_some() {
                out.start = s;
            } else {
                cx.error(
                    "version.start",
                    format!("`version.start` must be MAJOR.MINOR.PATCH, not {s:?}"),
                );
            }
        }
        out.mirror = take_string(cx, &mut t, "version", "mirror");
        match take_string(cx, &mut t, "version", "seal-where").as_deref() {
            None | Some("integration") => {}
            Some("any") => out.seal_anywhere = true,
            Some(o) => cx.error(
                "version.seal-where",
                format!("`version.seal-where` is integration or any, not {o:?}"),
            ),
        }
        if let Some(b) = take_string(cx, &mut t, "version", "integration-branch") {
            out.integration_branch = b;
        }
        for (key, slot) in [
            ("ancestor-grace", &mut out.ancestor_grace),
            ("escalation-grace", &mut out.escalation_grace),
            ("sweep-after", &mut out.sweep_after),
        ] {
            if let Some(v) = take_u32(cx, &mut t, "version", key) {
                *slot = v;
            }
        }
        cx.unknown_in("version", &t);
    }
    out
}

pub(crate) fn parse_docs_first(cx: &mut Ctx<'_>, root: &mut toml::Table) -> DocsFirstConfig {
    let mut out = DocsFirstConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "docs-first") {
        out.level = match take_string(cx, &mut t, "docs-first", "level").as_deref() {
            None => None,
            Some("observe") => Some(Stage::Observe),
            Some("warn") => Some(Stage::Warn),
            Some("enforce") => Some(Stage::Enforce),
            Some(o) => {
                cx.error(
                    "docs-first.level",
                    format!("`docs-first.level` is observe, warn or enforce, not {o:?}"),
                );
                None
            }
        };
        out.exempt = take_string_list(cx, &mut t, "docs-first", "exempt").unwrap_or_default();
        out.hotfix = take_bool(cx, &mut t, "docs-first", "hotfix").unwrap_or(false);
        out.hotfix_window = take_u32(cx, &mut t, "docs-first", "hotfix-window");
        out.strict_order = take_bool(cx, &mut t, "docs-first", "strict-order").unwrap_or(false);
        cx.unknown_in("docs-first", &t);
    }
    out
}

pub(crate) fn parse_eligibility(cx: &mut Ctx<'_>, root: &mut toml::Table) -> EligibilityConfig {
    let mut out = EligibilityConfig::default();
    if let Some(mut t) = take_table(cx, root, "", "eligibility") {
        if let Some(v) = take_u32(cx, &mut t, "eligibility", "min-lines") {
            out.min_lines = v;
        }
        if let Some(v) = take_u32(cx, &mut t, "eligibility", "min-files") {
            out.min_files = v;
        }
        cx.unknown_in("eligibility", &t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::FileClass;
    use crate::config::schema::{SevWeight, UnknownKey};
    use crate::diag::codes;
    use crate::path::RepoPath;

    fn cx(text: &str) -> (Ctx<'_>, toml::Table) {
        let table: toml::Table = text.parse().unwrap();
        (
            Ctx::new(&RepoPath::new("fairlead/control.toml").unwrap(), text),
            table,
        )
    }

    #[test]
    fn r_chk_005__leak_rule_parses_with_all_keys() {
        let text = "[rules.no-raw-hex]\ncheck = \"leak\"\ntokens = [\"tok.*\"]\nsev = \"error\"\nstage = \"observe\"\nglob = [\"**/*.css\", \"**/*.tsx\"]\nexclude = [\"dist/\"]\nwaivable = true\nwhy = \"colours come from tokens\"\nsince = \"2026-10-01\"\n";
        let (mut c, mut t) = cx(text);
        let rules = parse_rules(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let r = &rules["no-raw-hex"];
        assert_eq!(
            r.check,
            RuleCheckDecl::Leak {
                tokens: vec!["tok.*".into()]
            }
        );
        assert_eq!(
            (r.sev, r.stage, r.waivable),
            (Some(Severity::Error), Some(Stage::Observe), true)
        );
        assert_eq!(r.glob, ["**/*.css", "**/*.tsx"]);
        assert_eq!(r.exclude, ["dist/"]);
        assert_eq!(r.why.as_deref(), Some("colours come from tokens"));
        assert_eq!(
            r.since.map(|d| d.to_string()).as_deref(),
            Some("2026-10-01")
        );
        assert_eq!(r.name, "no-raw-hex");
    }

    #[test]
    fn r_chk_005__pattern_rule_compiles_its_regex_at_load() {
        let (mut c, mut t) = cx(
            "[rules.no-unwrap]\ncheck = \"pattern\"\nforbid = '\\.unwrap\\(\\)'\nmsg = \"no unwrap\"\n",
        );
        let rules = parse_rules(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert!(
            matches!(&rules["no-unwrap"].check, RuleCheckDecl::Pattern { msg, .. } if msg == "no unwrap")
        );
        let (mut c2, mut t2) =
            cx("[rules.bad]\ncheck = \"pattern\"\nforbid = \"(unclosed\"\nmsg = \"m\"\n");
        let rules2 = parse_rules(&mut c2, &mut t2);
        assert!(rules2.is_empty());
        assert_eq!(c2.diags.len(), 1);
        assert!(
            c2.diags[0].message.contains("regex"),
            "{}",
            c2.diags[0].message
        );
    }

    #[test]
    fn r_chk_005__rule_errors_and_builtin_default() {
        let text = "[rules.a-leak]\ncheck = \"leak\"\n\n[rules.a-pattern]\ncheck = \"pattern\"\nforbid = \"x\"\n\n[rules.Bad_Name]\ncheck = \"leak\"\ntokens = [\"t\"]\n\n[rules.unknown-check]\ncheck = \"magic\"\n\n[rules.fine]\nsev = \"warn\"\nstage = \"enforce\"\n\n[rules.bad-sev]\nsev = \"loud\"\n";
        let (mut c, mut t) = cx(text);
        let rules = parse_rules(&mut c, &mut t);
        assert_eq!(c.diags.len(), 5, "{:?}", c.diags);
        assert_eq!(
            rules.keys().collect::<Vec<_>>(),
            ["bad-sev", "fine"].iter().collect::<Vec<_>>()
        );
        assert_eq!(rules["fine"].check, RuleCheckDecl::Builtin);
    }

    #[test]
    fn r_syn_053__field_level_policy_form() {
        let text = "[policies]\n\"rules.*\" = \"sealed\"\n\"rules.no-raw-hex\" = \"needs-exception\"\n\n[policies.\"cargo.dep-version\"]\ntarget = \"cargo\"\nfields = [\"dependencies.*.version\", \"workspace.dependencies.*.version\"]\npolicy = \"sealed\"\n\n[policies.waiver]\nmax-exp = \"60d\"\nbudget = 10\n";
        let (mut c, mut t) = cx(text);
        let p = parse_policies(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(p.by_pattern["rules.*"], Policy::Sealed);
        assert_eq!(p.by_pattern["rules.no-raw-hex"], Policy::NeedsException);
        let f = &p.fields["cargo.dep-version"];
        assert_eq!(
            (f.target.as_deref(), f.policy, f.fields.len()),
            (Some("cargo"), Policy::Sealed, 2)
        );
        assert!(
            !p.fields.contains_key("waiver"),
            "the label `waiver` is the waiver table, never a policy id"
        );
        assert_eq!(
            (
                p.waiver.max_exp_days,
                p.waiver.budget,
                p.waiver.max_renewals,
                p.waiver.approval_required
            ),
            (60, 10, 2, true)
        );
    }

    #[test]
    fn r_syn_053__policy_errors() {
        let (mut c, mut t) = cx(
            "[policies]\n\"a.b\" = \"maybe\"\n\n[policies.\"x.y\"]\npolicy = \"sealed\"\n\n[policies.\"z.w\"]\nfields = [\"a\"]\n\n[policies.waiver]\nmax-exp = \"soon\"\napproval = \"sometimes\"\n",
        );
        let _ = parse_policies(&mut c, &mut t);
        assert_eq!(c.diags.len(), 5, "{:?}", c.diags);
    }

    #[test]
    fn r_chk_005__waiver_defaults_match_the_contract() {
        let w = WaiverPolicy::default();
        assert_eq!(
            (
                w.max_exp_days,
                w.budget,
                w.max_renewals,
                w.approval_required
            ),
            (90, 25, 2, true)
        );
        let (mut c, mut t) = cx("[policies.waiver]\nmax-exp = 30\napproval = \"none\"\n");
        let p = parse_policies(&mut c, &mut t);
        assert_eq!(
            (p.waiver.max_exp_days, p.waiver.approval_required),
            (30, false)
        );
    }

    #[test]
    fn r_scope_018__convention_actions() {
        let text = "[[convention]]\nglob = \"app/(*)/**\"\ntransparent = true\n\n[[convention]]\nglob = \"**/*.gen.ts\"\nclass = \"external-generated\"\n\n[[convention]]\nglob = \"ui/**\"\nclass = \"vendor-owned\"\n";
        let (mut c, mut t) = cx(text);
        let v = parse_conventions(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(v[0].action, ConventionAction::Transparent);
        assert_eq!(
            v[1].action,
            ConventionAction::Class(FileClass::ExternalGenerated)
        );
        assert_eq!(v[2].action, ConventionAction::Class(FileClass::VendorOwned));
    }

    #[test]
    fn r_scope_018__convention_needs_exactly_one_action_and_own_is_unknown_in_0_1() {
        let text = "[[convention]]\nglob = \"a/**\"\n\n[[convention]]\nglob = \"b/**\"\ntransparent = true\nclass = \"external-generated\"\n\n[[convention]]\nglob = \"c/**\"\nclass = \"authored\"\n\n[[convention]]\nglob = \"d/**\"\ntransparent = true\nown = \"web\"\n";
        let (mut c, mut t) = cx(text);
        let v = parse_conventions(&mut c, &mut t);
        assert_eq!(c.diags.len(), 3);
        assert_eq!(v.len(), 1);
        assert_eq!(
            c.unknown,
            vec![UnknownKey {
                path: "convention.own".into(),
                line: 16
            }]
        );
    }

    #[test]
    fn r_scope_056__allow_and_req_tables() {
        let text = "[[allow]]\nfor = [\"web\", \"api\"]\ntarget = \"rules.no-raw-hex.stage\"\nfield = \"stage\"\nwhy = \"web is migrating\"\nexp = 2027-03-31\n\n[[req]]\nto = \"root\"\ntarget = \"rule.no-raw-hex\"\nset = \"warn\"\nwhy = \"need it\"\nblocking = true\n";
        let (mut c, mut t) = cx(text);
        let a = parse_allows(&mut c, &mut t);
        let r = parse_reqs(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(
            a,
            vec![AllowDecl {
                target: "rules.no-raw-hex.stage".into(),
                field: Some("stage".into()),
                for_scopes: vec!["web".into(), "api".into()],
                why: "web is migrating".into(),
                exp: "2027-03-31".into(),
            }]
        );
        assert_eq!(
            r,
            vec![ReqDecl {
                to: "root".into(),
                target: "rule.no-raw-hex".into(),
                set: Some("warn".into()),
                why: "need it".into(),
                blocking: true
            }]
        );
    }

    #[test]
    fn r_scope_056__allow_and_req_need_their_keys_and_valid_scope_ids() {
        let (mut c, mut t) = cx(
            "[[allow]]\ntarget = \"x\"\nwhy = \"w\"\nexp = \"2027-01-01\"\n\n[[allow]]\nfor = [\"Bad Scope\"]\ntarget = \"x\"\nwhy = \"w\"\nexp = \"2027-01-01\"\n\n[[req]]\nto = \"root\"\ntarget = \"x\"\n",
        );
        assert!(parse_allows(&mut c, &mut t).is_empty());
        assert!(parse_reqs(&mut c, &mut t).is_empty());
        assert_eq!(c.diags.len(), 3);
    }

    #[test]
    fn r_syn_039__context_table_parses_and_defaults() {
        let d = ContextConfig::default();
        assert_eq!(
            (d.budget_start, d.budget_path, d.bytes_per_token),
            (500, 700, 4)
        );
        assert_eq!(
            d.sev_weight,
            SevWeight {
                error: 4,
                warn: 2,
                info: 1
            }
        );
        let (mut c, mut t) = cx(
            "[context]\nbudget-start = 300\ntokenizer = \"chars4\"\nbytes-per-token = 3\n[context.sev-weight]\nerror = 9\n",
        );
        let x = parse_context(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(
            (x.budget_start, x.budget_path, x.bytes_per_token),
            (300, 700, 3)
        );
        assert_eq!(
            (
                x.tokenizer.as_deref(),
                x.sev_weight.error,
                x.sev_weight.warn
            ),
            (Some("chars4"), 9, 2)
        );
        let (mut c2, mut t2) = cx("[context]\nbytes-per-token = 0\n");
        let _ = parse_context(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
    }

    #[test]
    fn r_syn_039__baseline_secrets_exceptions_eligibility() {
        let text = "[baseline]\nfile = \"fairlead/gen/baseline.txt\"\n\n[secrets]\nallow = [\"0123456789ab\"]\n\n[exceptions]\nmax-days = 45\nmax-live-per-rule = 3\n\n[eligibility]\nmin-lines = 8\n";
        let (mut c, mut t) = cx(text);
        assert_eq!(
            parse_baseline(&mut c, &mut t).file,
            "fairlead/gen/baseline.txt"
        );
        assert_eq!(parse_secrets(&mut c, &mut t).allow, ["0123456789ab"]);
        let e = parse_exceptions(&mut c, &mut t);
        assert_eq!((e.max_days, e.max_live_per_rule), (Some(45), 3));
        let el = parse_eligibility(&mut c, &mut t);
        assert_eq!((el.min_lines, el.min_files), (8, 3));
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        let (mut c2, mut t2) = cx("[secrets]\nallow = [\"not-a-fingerprint\"]\n");
        let _ = parse_secrets(&mut c2, &mut t2);
        assert_eq!(c2.diags.len(), 1);
        assert_eq!(ExceptionsConfig::default().max_live_per_rule, 10);
        assert_eq!(codes::HN_C001.0, "HN-C001");
    }

    #[test]
    fn r_ver_034__version_and_docs_first_tables_parse() {
        let text = "[version]\nstart = \"1.0.0\"\nseal-where = \"any\"\nintegration-branch = \"trunk\"\nancestor-grace = 5\n\n[docs-first]\nlevel = \"enforce\"\nexempt = [\"dist/**\"]\nhotfix = true\nhotfix-window = 2\n";
        let (mut c, mut t) = cx(text);
        let v = parse_version(&mut c, &mut t);
        let d = parse_docs_first(&mut c, &mut t);
        assert!(c.diags.is_empty(), "{:?}", c.diags);
        assert_eq!(
            (
                v.start.as_str(),
                v.seal_anywhere,
                v.integration_branch.as_str(),
                v.ancestor_grace
            ),
            ("1.0.0", true, "trunk", 5)
        );
        assert_eq!(
            (d.level, d.exempt.len(), d.hotfix, d.hotfix_window),
            (Some(Stage::Enforce), 1, true, Some(2))
        );
        let (mut c, mut t) = cx("[version]\nstart = \"1.0\"\n");
        parse_version(&mut c, &mut t);
        assert_eq!(c.diags.len(), 1, "a two-part start is not a version");
    }
}
