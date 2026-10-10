//! Directive grammar (winnow) and the directive record types (R-SYN-001..015, 028, 029, 048).
use crate::diag::{DiagCode, Span, codes, did_you_mean};
use crate::ids::{Id, IdError};
use crate::normalize::nfc;
use serde::{Deserialize, Serialize};
use winnow::combinator::alt;
use winnow::prelude::*;
use winnow::token::take_while;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Verb {
    Def,
    Use,
    Ovr,
    Flag,
    Env,
    End,
}

pub const VERBS: [&str; 6] = ["def", "use", "ovr", "flag", "env", "end"];

impl Verb {
    pub fn as_str(self) -> &'static str {
        match self {
            Verb::Def => "def",
            Verb::Use => "use",
            Verb::Ovr => "ovr",
            Verb::Flag => "flag",
            Verb::Env => "env",
            Verb::End => "end",
        }
    }
    pub fn parse(s: &str) -> Option<Verb> {
        Some(match s {
            "def" => Verb::Def,
            "use" => Verb::Use,
            "ovr" => Verb::Ovr,
            "flag" => Verb::Flag,
            "env" => Verb::Env,
            "end" => Verb::End,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArgOp {
    Set,
    Append,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArgValue {
    Word(String),
    Quoted(String),
    List(Vec<String>),
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Arg {
    pub key: String,
    pub op: ArgOp,
    pub value: ArgValue,
    pub span: Span,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Carrier {
    LineComment,
    BlockComment,
    HtmlComment,
    Frontmatter,
}

/// R-SYN-016..019, 022. Lines are 1-based inclusive. `Doc` = frontmatter or heading binding.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Extent {
    File,
    Doc,
    Lines { first: u32, last: u32 },
}

/// One attached directive. Dropped/malformed directives never appear here (R-SYN-032, R-ARCH-031).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Directive {
    pub ordinal: u32,
    pub verb: Verb,
    pub must_understand: bool,
    pub ids: Vec<Id>,
    pub bare: Vec<String>,
    pub args: Vec<Arg>,
    pub note: Option<String>,
    pub carrier: Carrier,
    pub span: Span,
    pub extent: Extent,
    pub end_span: Option<Span>,
}

impl Directive {
    pub fn arg(&self, key: &str) -> Option<&Arg> {
        self.args.iter().find(|a| a.key == key)
    }
}

/// The explicit extent word of a directive (`+N` or `+end`), before attachment.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Explicit {
    Lines(u32),
    Region,
}

/// Result of parsing one directive body; offsets are byte offsets inside the body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedBody {
    pub must_understand: bool,
    pub verb: Verb,
    pub verb_at: usize,
    pub ids: Vec<Id>,
    pub bare: Vec<String>,
    pub args: Vec<Arg>,
    pub arg_pos: Vec<(usize, usize)>,
    pub note: Option<String>,
    pub explicit: Option<Explicit>,
}

/// A malformed body: code, byte range in the body, message, expected form, did-you-mean (R-SYN-032).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BodyError {
    pub code: DiagCode,
    pub at: usize,
    pub len: usize,
    pub message: String,
    pub expected: Option<String>,
    pub did_you_mean: Option<String>,
}

impl BodyError {
    pub fn new(code: DiagCode, at: usize, len: usize, message: impl Into<String>) -> BodyError {
        BodyError {
            code,
            at,
            len: len.max(1),
            message: message.into(),
            expected: None,
            did_you_mean: None,
        }
    }
    pub fn expected(mut self, e: impl Into<String>) -> BodyError {
        self.expected = Some(e.into());
        self
    }
    pub fn suggest(mut self, s: Option<String>) -> BodyError {
        self.did_you_mean = s;
        self
    }
}

type PResult<T> = winnow::ModalResult<T>; // Verify: `ModalResult` is the winnow >= 0.7 name (0.6: `PResult`).

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '-' | '_' | '.' | '#' | '/' | '@' | '%' | ':' | '*' | ',')
}
fn is_key(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
}
fn word<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., is_word).parse_next(i)
}
fn key<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., is_key).parse_next(i)
}
fn spaces<'a>(i: &mut &'a str) -> PResult<&'a str> {
    take_while(1.., ' ').parse_next(i)
}
fn assign(i: &mut &str) -> PResult<ArgOp> {
    alt(("+=".value(ArgOp::Append), "=".value(ArgOp::Set))).parse_next(i)
}

fn pos(total: usize, rest: &str) -> usize {
    total.saturating_sub(rest.len())
}

/// `s` starts at the opening quote. Ok((value, bytes consumed)) or Err((offset in `s`, reason)).
fn quoted(s: &str) -> Result<(String, usize), (usize, &'static str)> {
    let mut out = String::new();
    let mut it = s.char_indices();
    it.next();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Ok((out, i + 1)),
            '\n' => return Err((i, "newline inside quotes (use a continuation line)")),
            '\\' => match it.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some(_) => return Err((i, "unknown escape (only \\\" and \\\\ exist)")),
                None => return Err((s.len(), "unterminated quote")),
            },
            _ => out.push(c),
        }
    }
    Err((s.len(), "unterminated quote"))
}

enum RawVal {
    Word(String),
    Quoted(String),
}

enum Tok<'a> {
    Pair {
        key: &'a str,
        op: ArgOp,
        val: RawVal,
        at: usize,
        end: usize,
    },
    Word {
        text: &'a str,
        at: usize,
    },
    Extent {
        x: Explicit,
    },
}

struct Raw<'a> {
    bang: bool,
    verb: &'a str,
    verb_at: usize,
    toks: Vec<Tok<'a>>,
    note: Option<String>,
}

fn boundary(rest: &str, total: usize) -> Result<(), BodyError> {
    match rest.chars().next() {
        None | Some(' ') => Ok(()),
        Some(c) => Err(BodyError::new(
            codes::HN_S016,
            pos(total, rest),
            c.len_utf8(),
            format!("unexpected character `{c}`"),
        )
        .expected("lowercase ASCII words; text only inside \"quotes\" or after `--`")),
    }
}

fn parse_extent(rest: &mut &str, total: usize) -> Result<Explicit, BodyError> {
    let at = pos(total, rest);
    let r = rest.strip_prefix('+').unwrap_or(rest);
    let n = r.find(' ').unwrap_or(r.len());
    let w = r.get(..n).unwrap_or("");
    *rest = r.get(n..).unwrap_or("");
    if w == "end" {
        return Ok(Explicit::Region);
    }
    match w.parse::<u32>() {
        Ok(k) if k >= 1 && w.bytes().all(|b| b.is_ascii_digit()) => Ok(Explicit::Lines(k)),
        _ => Err(
            BodyError::new(codes::HN_S016, at, w.len() + 1, "bad extent")
                .expected("+N with N >= 1, or +end"),
        ),
    }
}

fn tokenize(body: &str) -> Result<Raw<'_>, BodyError> {
    let total = body.len();
    let mut rest = body;
    let bang = match rest.strip_prefix('!') {
        Some(r) => {
            rest = r;
            true
        }
        None => false,
    };
    let verb_at = pos(total, rest);
    let verb = match take_while::<_, _, winnow::error::ContextError>(1.., |c: char| {
        c.is_ascii_alphanumeric() || c == '-'
    })
    .parse_next(&mut rest)
    {
        Ok(v) => v,
        Err(_) => {
            return Err(BodyError::new(
                codes::HN_S016,
                verb_at,
                1,
                "expected a verb after the marker",
            )
            .expected("def | use | ovr | flag | env | end"));
        }
    };
    let mut toks = Vec::new();
    let mut note = None;
    while !rest.is_empty() {
        if spaces(&mut rest).is_err() {
            return Err(BodyError::new(
                codes::HN_S016,
                pos(total, rest),
                1,
                "expected a space between directive parts",
            ));
        }
        if rest.is_empty() {
            break;
        }
        if rest.starts_with("--")
            && rest
                .get(2..)
                .is_some_and(|t| t.is_empty() || t.starts_with(' '))
        {
            note = Some(nfc(rest.get(2..).unwrap_or("").trim_start()));
            break;
        }
        let at = pos(total, rest);
        if rest.starts_with('+') {
            let x = parse_extent(&mut rest, total)?;
            boundary(rest, total)?;
            toks.push(Tok::Extent { x });
            continue;
        }
        let mut probe = rest;
        let k = key(&mut probe);
        let op = assign(&mut probe);
        if let (Ok(k), Ok(op)) = (k, op) {
            let val = if probe.starts_with('"') {
                match quoted(probe) {
                    Ok((v, used)) => {
                        probe = probe.get(used..).unwrap_or("");
                        RawVal::Quoted(nfc(&v))
                    }
                    Err((o, why)) => return Err(BodyError::new(
                        codes::HN_S007,
                        pos(total, probe) + o,
                        1,
                        why,
                    )
                    .expected(
                        "\"text\" using only \\\" and \\\\ escapes, closed on the same line",
                    )),
                }
            } else {
                match word(&mut probe) {
                    Ok(w) => RawVal::Word(w.to_owned()),
                    Err(_) => {
                        return Err(BodyError::new(
                            codes::HN_S016,
                            pos(total, probe),
                            1,
                            format!("expected a value after `{k}=`"),
                        )
                        .expected("word, list a,b or \"quoted text\""));
                    }
                }
            };
            boundary(probe, total)?;
            rest = probe;
            toks.push(Tok::Pair {
                key: k,
                op,
                val,
                at,
                end: pos(total, rest),
            });
            continue;
        }
        match word(&mut rest) {
            Ok(text) => {
                boundary(rest, total)?;
                toks.push(Tok::Word { text, at });
            }
            Err(_) => {
                boundary(rest, total)?;
                return Err(BodyError::new(
                    codes::HN_S016,
                    pos(total, rest),
                    1,
                    "expected a word",
                ));
            }
        }
    }
    Ok(Raw {
        bang,
        verb,
        verb_at,
        toks,
        note,
    })
}

fn id_error(text: &str, at: usize, e: IdError) -> BodyError {
    if e == IdError::ReservedName {
        return BodyError::new(
            codes::HN_P001,
            at,
            text.len(),
            format!("`{text}` uses a Windows-reserved name"),
        )
        .expected("a name that is not con prn aux nul com1..9 lpt1..9");
    }
    BodyError::new(
        codes::HN_S002,
        at,
        text.len(),
        format!("invalid ID \"{text}\": {e}"),
    )
    .expected("[scope:]kind.name with a 2 to 8 letter lowercase kind")
}

fn to_value(v: RawVal, at: usize) -> Result<ArgValue, BodyError> {
    match v {
        RawVal::Quoted(q) => Ok(ArgValue::Quoted(q)),
        RawVal::Word(w) if w.contains(',') => {
            let items: Vec<String> = w.split(',').map(str::to_owned).collect();
            if items.iter().any(String::is_empty) {
                return Err(
                    BodyError::new(codes::HN_S016, at, w.len(), "empty list item")
                        .expected("word,word without spaces"),
                );
            }
            Ok(ArgValue::List(items))
        }
        RawVal::Word(w) => Ok(ArgValue::Word(w)),
    }
}

/// Parses one directive body (R-SYN-007 grammar). Pure and total.
pub fn parse_body(body: &str) -> Result<ParsedBody, BodyError> {
    let raw = tokenize(body)?;
    let Some(verb) = Verb::parse(raw.verb) else {
        return Err(BodyError::new(
            codes::HN_S001,
            raw.verb_at,
            raw.verb.len(),
            format!("unknown verb \"{}\"", raw.verb),
        )
        .expected("one of: def use ovr flag env end")
        .suggest(did_you_mean(raw.verb, &VERBS).map(str::to_owned)));
    };
    let mut p = ParsedBody {
        must_understand: raw.bang,
        verb,
        verb_at: raw.verb_at,
        ids: Vec::new(),
        bare: Vec::new(),
        args: Vec::new(),
        arg_pos: Vec::new(),
        note: raw.note,
        explicit: None,
    };
    for t in raw.toks {
        match t {
            Tok::Extent { x } => p.explicit = Some(x),
            Tok::Pair {
                key,
                op,
                val,
                at,
                end,
            } => {
                let value = to_value(val, at)?;
                p.args.push(Arg {
                    key: key.to_owned(),
                    op,
                    value,
                    span: Span::whole_file(),
                });
                p.arg_pos.push((at, end));
            }
            Tok::Word { text, at } => match verb {
                Verb::Def | Verb::Use | Verb::Ovr => p
                    .ids
                    .push(Id::parse(text).map_err(|e| id_error(text, at, e))?),
                Verb::Flag | Verb::Env | Verb::End => p.bare.push(text.to_owned()),
            },
        }
    }
    check_verb(verb, &mut p)?;
    Ok(p)
}

/// Per-verb arity and key rules; each verb's arm is filled by its own task.
fn check_verb(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    match verb {
        Verb::Def | Verb::Use => check_def_use(verb, p),
        Verb::Ovr => check_ovr(p),
        Verb::Flag | Verb::Env | Verb::End => check_flag_env_end(verb, p),
    }
}

fn check_def_use(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    let form = if verb == Verb::Def {
        "def ID [k=v..] [+N|+end]"
    } else {
        "use ID [ID..] [sym=..]"
    };
    let bad = |msg: &str| {
        BodyError::new(
            codes::HN_S016,
            p.verb_at,
            verb.as_str().len(),
            msg.to_owned(),
        )
        .expected(form)
    };
    if !p.bare.is_empty() {
        return Err(bad("unexpected bare word"));
    }
    match (verb, p.ids.len()) {
        (_, 0) => Err(bad(&format!("`{}` needs an ID", verb.as_str()))),
        (Verb::Def, n) if n > 1 => Err(bad("`def` declares exactly one ID")),
        _ => Ok(()),
    }
}

/// `YYYY-MM-DD` and a real Gregorian calendar date (R-SYN-012).
pub(crate) fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b.get(4) != Some(&b'-') || b.get(7) != Some(&b'-') {
        return false;
    }
    let num = |a: usize, z: usize| -> Option<u32> {
        let t = s.get(a..z)?;
        if t.bytes().all(|c| c.is_ascii_digit()) {
            t.parse().ok()
        } else {
            None
        }
    };
    let (Some(y), Some(m), Some(d)) = (num(0, 4), num(5, 7), num(8, 10)) else {
        return false;
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dim).contains(&d)
}

fn check_ovr(p: &mut ParsedBody) -> Result<(), BodyError> {
    let form = "ovr ID why=\"..\" [owner=@h] [exp=YYYY-MM-DD] [k=v..]";
    if !p.bare.is_empty() || p.ids.len() != 1 {
        return Err(
            BodyError::new(codes::HN_S016, p.verb_at, 3, "`ovr` names exactly one ID")
                .expected(form),
        );
    }
    for (a, &(at, end)) in p.args.iter().zip(p.arg_pos.iter()) {
        if a.key == "until" {
            return Err(BodyError::new(
                codes::HN_S012,
                at,
                a.key.len(),
                "`until=` needs seals (edition 1 has none)",
            )
            .expected("exp=YYYY-MM-DD"));
        }
        if a.key == "exp" {
            let date = match (&a.op, &a.value) {
                (ArgOp::Set, ArgValue::Word(w) | ArgValue::Quoted(w)) => Some(w.as_str()),
                _ => None,
            };
            if !date.is_some_and(is_iso_date) {
                return Err(BodyError::new(
                    codes::HN_S012,
                    at,
                    end.saturating_sub(at),
                    "`exp` must be an ISO date",
                )
                .expected("exp=YYYY-MM-DD"));
            }
        }
    }
    Ok(())
}

fn is_lower_word(w: &str, extra: &[u8]) -> bool {
    let b = w.as_bytes();
    b.first().is_some_and(u8::is_ascii_lowercase)
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-' || extra.contains(c)
        })
}

fn check_flag_env_end(verb: Verb, p: &mut ParsedBody) -> Result<(), BodyError> {
    let shape = |msg: &str, form: &str| {
        BodyError::new(
            codes::HN_S016,
            p.verb_at,
            verb.as_str().len(),
            msg.to_owned(),
        )
        .expected(form.to_owned())
    };
    match verb {
        Verb::Flag => {
            let words = std::mem::take(&mut p.bare);
            let mut it = words.into_iter();
            let Some(flag_word) = it.next() else {
                return Err(shape(
                    "`flag` needs a word",
                    "flag WORD [ID] [to=ID] [target=VERSION]",
                ));
            };
            if !is_lower_word(&flag_word, b"") || flag_word.len() > 32 {
                return Err(shape(
                    "flag words are lowercase `a-z0-9-`",
                    "flag WORD [ID]",
                ));
            }
            let rest: Vec<String> = it.collect();
            if rest.len() > 1 {
                return Err(shape(
                    "`flag` takes one word and at most one ID",
                    "flag WORD [ID]",
                ));
            }
            for t in &rest {
                p.ids.push(
                    Id::parse(t).map_err(|e| id_error(t, p.verb_at + 5 + flag_word.len(), e))?,
                );
            }
            p.bare = vec![flag_word];
            Ok(())
        }
        Verb::Env => {
            let mut names = Vec::new();
            for w in std::mem::take(&mut p.bare) {
                for n in w.split(',') {
                    if !is_lower_word(n, b"_") {
                        return Err(shape(
                            "environment names are lowercase `a-z0-9_-`",
                            "env a,b",
                        ));
                    }
                    names.push(n.to_owned());
                }
            }
            if names.is_empty() || !p.args.is_empty() {
                return Err(shape(
                    "`env` takes only environment names",
                    "env a,b [+N|+end]",
                ));
            }
            p.bare = names;
            Ok(())
        }
        _ => {
            if p.bare.is_empty() && p.args.is_empty() && p.explicit.is_none() {
                Ok(())
            } else {
                Err(shape("`end` takes no arguments", "end"))
            }
        }
    }
}

const DEF_KEYS: &[&str] = &[
    "st", "sum", "kw", "sym", "of", "policy", "value", "applies", "min-len", "home", "gen", "tool",
    "in", "mat", "bind", "vis",
];
const USE_KEYS: &[&str] = &["sym"];
const OVR_KEYS: &[&str] = &["why", "owner", "exp", "op", "base"];
const FLAG_KEYS: &[&str] = &["to", "target", "by-date", "since"];

/// R-SYN-015, R-SYN-013, R-SYN-048: built-in keys of edition 1 (kind-declared keys and `x-` keys are extra).
pub fn known_keys(verb: Verb) -> &'static [&'static str] {
    match verb {
        Verb::Def => DEF_KEYS,
        Verb::Use => USE_KEYS,
        Verb::Ovr => OVR_KEYS,
        Verb::Flag => FLAG_KEYS,
        Verb::Env | Verb::End => &[],
    }
}

/// `x-` user extension keys are always known and never warned (R-SYN-009).
pub fn is_extension_key(k: &str) -> bool {
    k.len() > 2 && k.starts_with("x-")
}

fn render_value(v: &ArgValue, out: &mut String) {
    match v {
        ArgValue::Word(w) => out.push_str(w),
        ArgValue::List(items) => out.push_str(&items.join(",")),
        ArgValue::Quoted(q) => {
            out.push('"');
            for c in q.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' | '\r' => out.push(' '),
                    _ => out.push(c),
                }
            }
            out.push('"');
        }
    }
}

/// Stable re-rendering used by the parse-render-parse fuzz invariant (R-ARCH-150 table).
pub fn render_directive_body(d: &Directive) -> String {
    let mut s = String::new();
    if d.must_understand {
        s.push('!');
    }
    s.push_str(d.verb.as_str());
    if d.verb == Verb::Env {
        if !d.bare.is_empty() {
            s.push(' ');
            s.push_str(&d.bare.join(","));
        }
    } else {
        for w in &d.bare {
            s.push(' ');
            s.push_str(w);
        }
    }
    for id in &d.ids {
        s.push(' ');
        s.push_str(&id.to_string());
    }
    for a in &d.args {
        s.push(' ');
        s.push_str(&a.key);
        s.push_str(if a.op == ArgOp::Append { "+=" } else { "=" });
        render_value(&a.value, &mut s);
    }
    if d.end_span.is_some() {
        s.push_str(" +end");
    }
    if let Some(n) = &d.note {
        s.push_str(" -- ");
        s.push_str(&n.replace(['\n', '\r'], " "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(body: &str) -> ParsedBody {
        parse_body(body).unwrap_or_else(|e| panic!("{body:?}: {e:?}"))
    }
    fn err(body: &str) -> BodyError {
        parse_body(body).expect_err(body)
    }

    #[test]
    fn r_syn_007__pairs_words_extent_and_note() {
        let p = ok("def cmp.sync-button sym=SyncButton layer=molecule +5 -- hello world");
        assert_eq!(p.verb, Verb::Def);
        assert_eq!(p.ids.len(), 1);
        assert_eq!(p.args.len(), 2);
        assert_eq!(p.args[0].key, "sym");
        assert_eq!(p.args[0].value, ArgValue::Word("SyncButton".into()));
        assert_eq!(p.explicit, Some(Explicit::Lines(5)));
        assert_eq!(p.note.as_deref(), Some("hello world"));
        assert_eq!(
            p.arg_pos[0],
            (20, 34),
            "byte range of `sym=SyncButton` inside the body"
        );
    }

    #[test]
    fn r_syn_013__plus_equals_is_the_append_marker() {
        let p = ok("ovr rule.x kw+=a,b why=\"z\"");
        assert_eq!(p.args[0].op, ArgOp::Append);
        assert_eq!(
            p.args[0].value,
            ArgValue::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(p.args[1].op, ArgOp::Set);
    }

    #[test]
    fn r_syn_007__value_forms_word_quoted_list() {
        let p =
            ok("use tt.b sym=\"x y\" params=id:string,limit:u32 c=#ff0000 o=@platform p=src/a*.rs");
        assert_eq!(p.args[0].value, ArgValue::Quoted("x y".into()));
        assert_eq!(
            p.args[1].value,
            ArgValue::List(vec!["id:string".into(), "limit:u32".into()])
        );
        assert_eq!(p.args[2].value, ArgValue::Word("#ff0000".into()));
        assert_eq!(p.args[3].value, ArgValue::Word("@platform".into()));
        assert_eq!(p.args[4].value, ArgValue::Word("src/a*.rs".into()));
    }

    #[test]
    fn r_syn_029__quote_escapes_are_only_quote_and_backslash() {
        let p = ok(r#"use tt.b why="say \"hi\" \\ done""#);
        assert_eq!(
            p.args[0].value,
            ArgValue::Quoted(r#"say "hi" \ done"#.into())
        );
    }

    #[test]
    fn r_syn_029__unterminated_quote_and_unknown_escape_are_s007() {
        let e = err("use tt.b why=\"abc");
        assert_eq!(e.code, codes::HN_S007);
        assert_eq!(
            e.at, 17,
            "points at the end of the text where the quote should close"
        );
        let e2 = err(r#"use tt.b why="a\nb""#);
        assert_eq!(e2.code, codes::HN_S007);
        assert_eq!(e2.at, 15, "points at the backslash");
        assert_eq!(
            err("use tt.b why=\"a\u{0}b\nc\"").code,
            codes::HN_S007,
            "no newline inside quotes"
        );
    }

    #[test]
    fn r_syn_008__note_is_data_and_never_parsed() {
        let p = ok("use tt.b -- def xx.y exp=oops \"unterminated");
        assert_eq!(p.note.as_deref(), Some("def xx.y exp=oops \"unterminated"));
        assert!(p.args.is_empty());
        assert_eq!(ok("use tt.b --").note.as_deref().unwrap_or(""), "");
    }

    #[test]
    fn r_syn_009__unknown_verb_is_s001_with_suggestion() {
        let e = err("frobnicate tt.b");
        assert_eq!(e.code, codes::HN_S001);
        assert_eq!(e.did_you_mean, None);
        let e2 = err("deff tt.b");
        assert_eq!(e2.did_you_mean.as_deref(), Some("def"));
        assert_eq!((e2.at, e2.len), (0, 4));
        assert_eq!(err("!usee tt.b").at, 1, "the bang is not part of the verb");
    }

    #[test]
    fn r_syn_009__bang_sits_directly_before_the_verb() {
        assert!(ok("!def tt.b").must_understand);
        assert!(!ok("def tt.b").must_understand);
        assert_eq!(err("def !tt.b").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__stray_characters_are_s016() {
        assert_eq!(err("def tt.b(").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=,").code, codes::HN_S016, "empty list item");
        assert_eq!(err("def tt.b +0").code, codes::HN_S016);
        assert_eq!(err("def tt.b +x").code, codes::HN_S016);
        assert_eq!(err("").code, codes::HN_S016);
        assert_eq!(err("def tt.b k=v=w").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__non_ascii_only_inside_quotes_and_note() {
        assert!(parse_body("use tt.b why=\"caf\u{e9}\" -- na\u{ef}ve").is_ok());
        assert_eq!(err("use tt.b k=caf\u{e9}").code, codes::HN_S016);
        assert_eq!(err("use caf\u{e9}.x").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__quoted_and_note_are_nfc_normalised() {
        let p = ok("use tt.b why=\"e\u{301}\" -- e\u{301}");
        assert_eq!(p.args[0].value, ArgValue::Quoted("\u{e9}".into()));
        assert_eq!(p.note.as_deref(), Some("\u{e9}"));
    }

    #[test]
    fn r_syn_007__parse_body_is_total() {
        for s in [
            "!",
            "!!def",
            "def \"",
            "def tt.b +",
            "def tt.b k+",
            "def tt.b k+=",
            "--",
            "def -- ",
            "\u{0}",
            "def tt.b \u{e9}",
            "= =",
            "def  tt.b",
        ] {
            let _ = parse_body(s);
        }
    }

    #[test]
    fn r_syn_007__def_takes_exactly_one_id() {
        assert_eq!(
            ok("def tok.primary applies=color,background-color min-len=4").ids[0].to_string(),
            "tok.primary"
        );
        let e = err("def");
        assert_eq!(e.code, codes::HN_S016);
        assert_eq!(e.expected.as_deref(), Some("def ID [k=v..] [+N|+end]"));
        assert_eq!(err("def tt.b uu.d").code, codes::HN_S016);
        assert_eq!(
            err("def sym=x").code,
            codes::HN_S016,
            "pairs alone are not an ID"
        );
    }

    #[test]
    fn r_syn_007__use_takes_one_or_more_ids() {
        let p = ok("use eng.sync-verify eng.sync-purify :tok.primary web:cmp.button sym=Foo");
        assert_eq!(p.ids.len(), 4);
        assert_eq!(p.ids[2].to_string(), ":tok.primary");
        assert_eq!(err("use").code, codes::HN_S016);
        assert_eq!(err("use sym=x").code, codes::HN_S016);
    }

    #[test]
    fn r_syn_007__bad_id_in_the_id_slot_is_s002_with_position() {
        let e = err("def comp.Sync-Button");
        assert_eq!(e.code, codes::HN_S002);
        assert_eq!(e.at, 4);
        let e2 = err("use cmp");
        assert_eq!(e2.code, codes::HN_S002);
        assert_eq!(
            err("def tauri-command.x").code,
            codes::HN_S002,
            "labels are not kinds (R-SYN-024)"
        );
    }

    #[test]
    fn r_syn_027__reserved_windows_name_in_an_id_is_hn_p001() {
        let e = err("def cmp.con");
        assert_eq!(e.code, codes::HN_P001);
        assert_eq!(err("use nul:cmp.x").code, codes::HN_P001);
    }

    #[test]
    fn r_syn_014__def_foreign_with_plus_end_opens_a_region() {
        let p = ok("def foreign.shadcn tool=shadcn +end");
        assert_eq!(p.ids[0].kind.as_str(), "foreign");
        assert_eq!(p.explicit, Some(Explicit::Region));
        assert_eq!(p.args[0].key, "tool");
    }

    #[test]
    fn r_file_042__foreign_region_bracket_parses_like_any_def() {
        let p = ok("def foreign.tailwind tool=tailwind +end -- written by the tool");
        assert_eq!(p.note.as_deref(), Some("written by the tool"));
    }

    #[test]
    fn r_syn_009__extension_keys_and_bang_parse_like_other_keys() {
        let p = ok("!def cmp.x x-owner=team x-note=\"a b\"");
        assert!(p.must_understand);
        assert_eq!(
            p.args.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(),
            vec!["x-owner", "x-note"]
        );
    }

    #[test]
    fn r_syn_012__exp_must_be_a_real_iso_date() {
        assert!(parse_body("ovr rule.x why=\"w\" owner=@a exp=2026-12-31").is_ok());
        assert!(
            parse_body("ovr rule.x why=\"w\" exp=\"2024-02-29\"").is_ok(),
            "leap day, quoted"
        );
        for bad in [
            "exp=2025-02-29",
            "exp=2026-13-01",
            "exp=2026-00-10",
            "exp=2026-1-5",
            "exp=1.9.0",
            "exp=tomorrow",
            "exp=2026-12-31,2027-01-01",
        ] {
            let e = err(&format!("ovr rule.x why=\"w\" {bad}"));
            assert_eq!(e.code, codes::HN_S012, "{bad}");
        }
    }

    #[test]
    fn r_syn_012__until_is_rejected_in_edition_one() {
        let e = err("ovr rule.x why=\"w\" until=1.9.0");
        assert_eq!(e.code, codes::HN_S012);
        assert_eq!(e.at, 19, "points at the key");
        assert!(e.message.contains("until"));
        assert_eq!(e.expected.as_deref(), Some("exp=YYYY-MM-DD"));
    }

    #[test]
    fn r_syn_013__ovr_keys_replace_or_append() {
        let p = ok("ovr tok.primary why=\"brand\" value=#ff0000 applies+=color,fill");
        assert_eq!(p.args.len(), 3);
        assert_eq!(
            (p.args[2].key.as_str(), p.args[2].op),
            ("applies", ArgOp::Append)
        );
    }

    #[test]
    fn r_syn_011__ovr_shape_is_one_id() {
        assert_eq!(err("ovr").code, codes::HN_S016);
        assert_eq!(err("ovr tt.b uu.d why=\"w\"").code, codes::HN_S016);
        assert!(
            parse_body("ovr rule.x").is_ok(),
            "missing why is resolve's HN-S016 (C-5), not extraction's"
        );
    }

    #[test]
    fn r_syn_012__iso_date_helper() {
        assert!(
            is_iso_date("2026-02-28") && is_iso_date("2000-02-29") && !is_iso_date("1900-02-29")
        );
        assert!(
            !is_iso_date("2026-04-31") && !is_iso_date("26-04-01") && !is_iso_date("2026/04/01")
        );
    }

    #[test]
    fn r_syn_048__flag_word_then_optional_id() {
        let p = ok("flag deprecate cmp.sync-menu to=cmp.sync-menu2 target=2.0.0 since=1.4.0");
        assert_eq!(p.verb, Verb::Flag);
        assert_eq!(p.bare, vec!["deprecate"]);
        assert_eq!(p.ids.len(), 1);
        assert_eq!(
            p.args.iter().map(|a| a.key.as_str()).collect::<Vec<_>>(),
            vec!["to", "target", "since"]
        );
        let q = ok("flag dep");
        assert!(q.ids.is_empty() && q.bare == vec!["dep"]);
    }

    #[test]
    fn r_syn_048__flag_shape_errors() {
        assert_eq!(err("flag").code, codes::HN_S016);
        assert_eq!(
            err("flag Deprecate").code,
            codes::HN_S016,
            "flag words are lowercase"
        );
        assert_eq!(err("flag dep cmp.x cmp.y").code, codes::HN_S016);
        assert_eq!(
            err("flag cmp.x").code,
            codes::HN_S016,
            "the first word is the flag word, not an ID"
        );
        assert_eq!(err("flag dep not-an-id").code, codes::HN_S002);
    }

    #[test]
    fn r_syn_048__until_on_flag_is_parsed_and_left_to_resolve() {
        let p = ok("flag deprecate cmp.x until=2.0.0");
        assert_eq!(
            p.args[0].key, "until",
            "HN-S011 is raised by resolve (C-5), error under !"
        );
    }

    #[test]
    fn r_syn_009__env_lists_names() {
        assert_eq!(ok("env dev").bare, vec!["dev"]);
        assert_eq!(ok("env dev,staging +end").bare, vec!["dev", "staging"]);
        assert_eq!(ok("env dev,staging +end").explicit, Some(Explicit::Region));
        assert_eq!(ok("env a b").bare, vec!["a", "b"]);
        assert_eq!(err("env").code, codes::HN_S016);
        assert_eq!(err("env Dev").code, codes::HN_S016);
        assert_eq!(err("env dev,,prod").code, codes::HN_S016);
        assert_eq!(err("env dev k=v").code, codes::HN_S016, "env takes no keys");
    }

    #[test]
    fn r_syn_019__end_takes_nothing_but_a_note() {
        let p = ok("end -- closes the shadcn block");
        assert_eq!(p.verb, Verb::End);
        assert_eq!(p.note.as_deref(), Some("closes the shadcn block"));
        assert_eq!(err("end cmp.x").code, codes::HN_S016);
        assert_eq!(err("end +3").code, codes::HN_S016);
        assert_eq!(err("end k=v").code, codes::HN_S016);
    }

    fn dir(body: &str) -> Directive {
        let p = ok(body);
        Directive {
            ordinal: 0,
            verb: p.verb,
            must_understand: p.must_understand,
            ids: p.ids,
            bare: p.bare,
            args: p.args,
            note: p.note,
            carrier: Carrier::LineComment,
            span: Span::whole_file(),
            extent: Extent::Lines { first: 1, last: 1 },
            end_span: if p.explicit == Some(Explicit::Region) {
                Some(Span::whole_file())
            } else {
                None
            },
        }
    }

    #[test]
    fn r_syn_015__known_keys_per_verb() {
        assert!(
            known_keys(Verb::Def).contains(&"gen") && known_keys(Verb::Def).contains(&"min-len")
        );
        assert!(
            known_keys(Verb::Def).contains(&"tool"),
            "def foreign tool= (R-SYN-014)"
        );
        assert_eq!(known_keys(Verb::Use), &["sym"]);
        assert_eq!(
            known_keys(Verb::Ovr),
            &["why", "owner", "exp", "op", "base"]
        );
        assert_eq!(
            known_keys(Verb::Flag),
            &["to", "target", "by-date", "since"]
        );
        assert!(known_keys(Verb::Env).is_empty() && known_keys(Verb::End).is_empty());
        assert!(!known_keys(Verb::Flag).contains(&"until"), "D-49");
        assert!(
            is_extension_key("x-owner") && !is_extension_key("xowner") && !is_extension_key("x-")
        );
    }

    #[test]
    fn r_arch_150__render_then_parse_is_stable() {
        for body in [
            "def cmp.sync-button sym=SyncButton layer=molecule",
            "!def foreign.shadcn tool=shadcn +end",
            "use eng.a eng.b sym=\"has \\\"q\\\" and \\\\\" -- note text",
            "ovr rule.x why=\"w\" owner=@a exp=2026-12-31 kw+=a,b",
            "flag deprecate cmp.x target=2.0.0",
            "env dev,staging",
            "end",
        ] {
            let d1 = dir(body);
            let r1 = render_directive_body(&d1);
            let d2 = dir(&r1);
            assert_eq!(render_directive_body(&d2), r1, "{body}");
            assert_eq!(
                (d1.verb, &d1.ids, &d1.bare, &d1.args, &d1.note),
                (d2.verb, &d2.ids, &d2.bare, &d2.args, &d2.note),
                "{body}"
            );
        }
    }

    #[test]
    fn r_arch_150__render_is_canonical() {
        assert_eq!(
            render_directive_body(&dir("def   tt.b   k=\"v\"   +end")),
            "def tt.b k=\"v\" +end"
        );
        assert_eq!(
            render_directive_body(&dir("flag dep cmp.x")),
            "flag dep cmp.x"
        );
        assert!(!render_directive_body(&dir("use tt.b")).contains('\n'));
    }

    #[test]
    fn r_syn_008__directive_arg_lookup() {
        let d = dir("def tt.b sym=X sym=Y");
        assert_eq!(
            d.arg("sym").map(|a| &a.value),
            Some(&ArgValue::Word("X".into())),
            "first wins"
        );
        assert!(d.arg("nope").is_none());
    }
}
