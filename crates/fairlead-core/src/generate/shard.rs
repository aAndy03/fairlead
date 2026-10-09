//! Registry shards `gen/reg/<kind>.reg` (R-FILE-003, R-FILE-008, R-FILE-022, R-FILE-030,
//! R-FILE-038). Pure: items in, sorted terse lines out. Lines hold no file paths.

use std::collections::BTreeMap;

use crate::config::{IndexValues, Policy};
use crate::hash::{base32, hash_raw};
use crate::registry::FieldValue;
use crate::render::line::TerseLine;
use crate::render::sanitize::clean_data;

/// A shard above this many lines is split by slug bucket (R-FILE-022).
pub const SHARD_LINE_LIMIT: usize = 500;

/// One value site of a token in the authored stylesheet; `src` is relative to the scope folder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TokSite {
    pub value: String,
    pub context: String,
    pub src: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShardItem {
    pub kind: String,
    pub name: String,
    pub policy: Policy,
    pub sum: Option<String>,
    pub fields: BTreeMap<String, FieldValue>,
    /// Non-empty only for token items found in the stylesheet (`css` field matched).
    pub sites: Vec<TokSite>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShardOut {
    pub file_name: String,
    pub body: String,
}

pub fn policy_word(p: &Policy) -> &'static str {
    match p {
        Policy::Open => "open",
        Policy::NeedsException => "needs-exception",
        Policy::Sealed => "sealed",
    }
}

/// `kind.name pol=<policy> [sum="..."] <field>=<value>...` with fields in key order.
pub fn item_line(item: &ShardItem) -> TerseLine {
    let mut line = TerseLine::new(&format!("{}.{}", item.kind, item.name)).with("pol", policy_word(&item.policy));
    if let Some(sum) = &item.sum {
        line = line.with("sum", &clean_data(sum, 160));
    }
    for (key, value) in &item.fields {
        let text = match value {
            FieldValue::Scalar(s) => clean_data(s, 240),
            FieldValue::List(l) => l.iter().map(|s| clean_data(s, 240)).collect::<Vec<_>>().join(","),
        };
        line = line.with(key, &text);
    }
    line
}

fn is_dark(context: &str) -> bool {
    let c = context.to_ascii_lowercase();
    c.contains(".dark") || c.contains("data-theme=\"dark\"") || c.contains("data-theme=dark") || c.contains("prefers-color-scheme: dark")
}

fn is_base_context(context: &str) -> bool {
    matches!(context.trim(), "" | ":root" | "html" | "*" | "@theme" | "@theme inline")
}

fn index_value(value: &str, mode: &IndexValues) -> String {
    match mode {
        IndexValues::Value => clean_data(value.trim(), 120),
        IndexValues::Hash => format!("h:{}", base32(&hash_raw(value.trim().as_bytes()).0, 8)),
    }
}

fn token_line(item: &ShardItem, mode: &IndexValues) -> TerseLine {
    let mut line = item_line(item);
    let light = item.sites.iter().find(|s| !is_dark(&s.context) && is_base_context(&s.context));
    let dark = item.sites.iter().find(|s| is_dark(&s.context));
    if let Some(s) = light {
        line = line.with("light", &index_value(&s.value, mode));
    }
    if let Some(s) = dark {
        line = line.with("dark", &index_value(&s.value, mode));
    }
    if let Some(s) = light.or(dark).or(item.sites.first()) {
        line = line.with("src", &clean_data(&s.src, 240));
    }
    line
}

fn join_lines(rows: &[(String, String)]) -> String {
    let mut out = String::new();
    for (_, line) in rows {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn split_bucket(kind: &str, rows: Vec<(String, String)>, depth: usize, out: &mut Vec<ShardOut>) {
    let mut groups: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for row in rows {
        let key: String = row.0.chars().take(depth).collect();
        groups.entry(key).or_default().push(row);
    }
    for (key, group) in groups {
        let can_split = group.iter().any(|r| r.0.chars().count() > depth);
        if group.len() > SHARD_LINE_LIMIT && can_split {
            split_bucket(kind, group, depth + 1, out);
        } else {
            out.push(ShardOut { file_name: format!("{kind}-{}.reg", key.replace('.', "_")), body: join_lines(&group) });
        }
    }
}

/// The shard files of one scope and one kind: none when empty, `<kind>.reg` up to 500 lines,
/// otherwise `<kind>-<bucket>.reg` split by slug prefix, files sorted by name.
pub fn shard_kind(kind: &str, items: &[ShardItem], mode: &IndexValues) -> Vec<ShardOut> {
    let mut rows: Vec<(String, String)> = items
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| {
            let line = if i.sites.is_empty() { item_line(i) } else { token_line(i, mode) };
            (i.name.clone(), line.render())
        })
        .collect();
    if rows.is_empty() {
        return Vec::new();
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    if rows.len() <= SHARD_LINE_LIMIT {
        return vec![ShardOut { file_name: format!("{kind}.reg"), body: join_lines(&rows) }];
    }
    let mut out = Vec::new();
    split_bucket(kind, rows, 1, &mut out);
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    out
}

/// One exception ledger fact of a scope (R-SCOPE-044). All strings are display text already.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ExcRow {
    pub ovr: String,
    pub target: String,
    pub home: String,
    pub varied_in: String,
    pub depth: u32,
    /// `None` when the exception is effective (like `st=active`, omitted, R-CTX-030).
    pub state: Option<String>,
    pub why: Option<String>,
    pub owner: Option<String>,
    pub exp: Option<String>,
}

pub fn exc_body(rows: &[ExcRow]) -> String {
    let mut lines: Vec<(String, String)> = rows
        .iter()
        .map(|r| {
            let id = format!("exc.{}", r.ovr);
            let mut l = TerseLine::new(&id)
                .with("tgt", &clean_data(&r.target, 120))
                .with("home", &r.home)
                .with("in", &r.varied_in)
                .with("depth", &r.depth.to_string());
            if let Some(s) = &r.state {
                l = l.with("st", s);
            }
            if let Some(w) = &r.why {
                l = l.with("why", &clean_data(w, 160));
            }
            if let Some(o) = &r.owner {
                l = l.with("owner", &clean_data(o, 60));
            }
            if let Some(e) = &r.exp {
                l = l.with("exp", e);
            }
            (id, l.render())
        })
        .collect();
    lines.sort_by(|a, b| a.0.cmp(&b.0));
    join_lines(&lines)
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
    use super::*;

    fn item(kind: &str, name: &str) -> ShardItem {
        ShardItem {
            kind: kind.to_string(),
            name: name.to_string(),
            policy: Policy::Open,
            sum: None,
            fields: BTreeMap::new(),
            sites: Vec::new(),
        }
    }

    fn scalar(s: &str) -> FieldValue {
        FieldValue::Scalar(s.to_string())
    }

    #[test]
    fn r_file_003__item_line_is_policy_then_sum_then_sorted_fields() {
        let mut it = item("rule", "no-raw-hex");
        it.policy = Policy::Sealed;
        it.sum = Some("tokens not hex".to_string());
        it.fields.insert("stage".to_string(), scalar("enforce"));
        it.fields.insert("sev".to_string(), scalar("error"));
        it.fields.insert("glob".to_string(), FieldValue::List(vec!["**/*.tsx".to_string(), "**/*.css".to_string()]));
        assert_eq!(
            item_line(&it).render(),
            "rule.no-raw-hex pol=sealed sum=\"tokens not hex\" glob=**/*.tsx,**/*.css sev=error stage=enforce"
        );
    }

    #[test]
    fn r_file_022__small_kind_is_one_file_sorted_bytewise_by_id() {
        let items = vec![item("tok", "zeta"), item("tok", "alpha"), item("tok", "mid")];
        let out = shard_kind("tok", &items, &IndexValues::Value);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_name, "tok.reg");
        assert_eq!(out[0].body, "tok.alpha pol=open\ntok.mid pol=open\ntok.zeta pol=open\n");
    }

    #[test]
    fn r_file_022__empty_kind_gets_no_shard() {
        assert!(shard_kind("tok", &[], &IndexValues::Value).is_empty());
        assert!(shard_kind("tok", &[item("rule", "x")], &IndexValues::Value).is_empty());
    }

    #[test]
    fn r_file_022__over_500_lines_splits_by_first_slug_character() {
        let items: Vec<ShardItem> =
            (0..1200).map(|i| item("tok", &format!("{}{:03}", ['a', 'b', 'c'][i / 400], i % 400))).collect();
        let out = shard_kind("tok", &items, &IndexValues::Value);
        let names: Vec<&str> = out.iter().map(|o| o.file_name.as_str()).collect();
        assert_eq!(names, vec!["tok-a.reg", "tok-b.reg", "tok-c.reg"]);
        assert!(out.iter().all(|o| o.body.lines().count() == 400));
    }

    #[test]
    fn r_file_022__a_bucket_that_is_still_too_big_splits_on_more_characters() {
        let items: Vec<ShardItem> = (0..600).map(|i| item("tok", &format!("a{i:04}"))).collect();
        let out = shard_kind("tok", &items, &IndexValues::Value);
        let names: Vec<&str> = out.iter().map(|o| o.file_name.as_str()).collect();
        assert_eq!(names, vec!["tok-a00.reg", "tok-a01.reg", "tok-a02.reg", "tok-a03.reg", "tok-a04.reg", "tok-a05.reg"]);
        assert_eq!(out.iter().map(|o| o.body.lines().count()).sum::<usize>(), 600);
        assert!(out.iter().all(|o| o.body.lines().count() <= SHARD_LINE_LIMIT));
    }

    #[test]
    fn r_file_008__output_depends_only_on_the_items_not_their_order() {
        let a = vec![item("tok", "b"), item("tok", "a"), item("tok", "c")];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(shard_kind("tok", &a, &IndexValues::Value), shard_kind("tok", &b, &IndexValues::Value));
    }

    #[test]
    fn r_agt_022__secrets_in_field_values_never_reach_a_shard() {
        let mut it = item("tok", "x");
        it.fields.insert("note".to_string(), scalar(&format!("key ghp_{}", "a1B2".repeat(9))));
        let line = item_line(&it).render();
        assert_eq!(line, "tok.x pol=open note=\"key [redacted:github-token]\"");
    }

    fn site(value: &str, context: &str) -> TokSite {
        TokSite { value: value.to_string(), context: context.to_string(), src: "app/globals.css".to_string() }
    }

    fn token() -> ShardItem {
        let mut it = item("tok", "radius-base");
        it.policy = Policy::NeedsException;
        it.fields.insert("css".to_string(), scalar("--radius"));
        it.sites = vec![site("0.5rem", ":root"), site("0.4rem", ".dark")];
        it
    }

    #[test]
    fn r_file_030__token_index_line_carries_light_dark_and_src() {
        let out = shard_kind("tok", &[token()], &IndexValues::Value);
        assert_eq!(
            out[0].body,
            "tok.radius-base pol=needs-exception css=--radius light=0.5rem dark=0.4rem src=app/globals.css\n"
        );
    }

    #[test]
    fn r_file_030__a_token_missing_from_the_css_keeps_the_plain_line() {
        let mut it = token();
        it.sites.clear();
        let out = shard_kind("tok", &[it], &IndexValues::Value);
        assert_eq!(out[0].body, "tok.radius-base pol=needs-exception css=--radius\n");
    }

    #[test]
    fn r_file_038__hash_mode_stores_value_hashes_not_values() {
        let out = shard_kind("tok", &[token()], &IndexValues::Hash);
        let h_light = base32(&hash_raw(b"0.5rem").0, 8);
        let h_dark = base32(&hash_raw(b"0.4rem").0, 8);
        assert_eq!(
            out[0].body,
            format!("tok.radius-base pol=needs-exception css=--radius light=h:{h_light} dark=h:{h_dark} src=app/globals.css\n")
        );
        assert!(!out[0].body.contains("0.5rem"));
    }

    fn exc(ovr: &str) -> ExcRow {
        ExcRow {
            ovr: ovr.to_string(),
            target: "root:rule.no-raw-hex".to_string(),
            home: "root".to_string(),
            varied_in: "web".to_string(),
            depth: 1,
            state: None,
            why: Some("legacy email template".to_string()),
            owner: Some("@design".to_string()),
            exp: Some("2026-12-31".to_string()),
        }
    }

    #[test]
    fn r_file_022__exc_reg_lines_sorted_by_ovr_id_with_state_only_when_not_effective() {
        let mut expired = exc("a1111111111b");
        expired.state = Some("expired".to_string());
        let body = exc_body(&[exc("k3m9q2x7a1bz"), expired]);
        assert_eq!(
            body,
            "exc.a1111111111b tgt=root:rule.no-raw-hex home=root in=web depth=1 st=expired why=\"legacy email template\" owner=@design exp=2026-12-31\n\
             exc.k3m9q2x7a1bz tgt=root:rule.no-raw-hex home=root in=web depth=1 why=\"legacy email template\" owner=@design exp=2026-12-31\n"
        );
        assert_eq!(exc_body(&[]), "");
    }
}
