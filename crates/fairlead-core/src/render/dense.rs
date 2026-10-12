//! `context --dense` (R-CTX-034, 0.3): a dictionary-aliased form of the terse brief for L0
//! dumps of very large trees. Optional and never the default; the dictionary comes first.
//!
//! Form: a `#dense v1` line, a `#dict` line of `<n>=<token>` pairs, then the terse lines with
//! every dictionary token replaced by `~<n>`. A terse token that itself starts with `~` is
//! written with one more `~`, so `expand(compress(t)) == t` for every text.
use std::collections::BTreeMap;

const HEADER: &str = "#dense v1";

/// Tokens worth aliasing: repeated at least three times and at least six characters long.
fn dictionary(text: &str) -> Vec<String> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for line in text.lines() {
        for t in line.split(' ') {
            if t.chars().count() >= 6 && !t.contains('=') || t.chars().count() >= 8 {
                *count.entry(t).or_insert(0) += 1;
            }
        }
    }
    let mut v: Vec<(&str, usize)> = count
        .into_iter()
        .filter(|(t, n)| *n >= 3 && !t.is_empty())
        .collect();
    // most saved characters first, then bytewise: deterministic
    v.sort_by(|a, b| {
        (b.0.len() * b.1)
            .cmp(&(a.0.len() * a.1))
            .then_with(|| a.0.cmp(b.0))
    });
    v.into_iter().map(|(t, _)| t.to_owned()).collect()
}

/// The dense form of terse `text`.
pub fn compress(text: &str) -> String {
    let dict = dictionary(text);
    let index: BTreeMap<&str, usize> = dict
        .iter()
        .enumerate()
        .map(|(i, t)| (t.as_str(), i))
        .collect();
    let mut out = String::from(HEADER);
    out.push('\n');
    out.push_str("#dict");
    for (i, t) in dict.iter().enumerate() {
        out.push_str(&format!(" {i}={t}"));
    }
    out.push('\n');
    for line in text.split_inclusive('\n') {
        let (body, nl) = match line.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (line, ""),
        };
        let toks: Vec<String> = body
            .split(' ')
            .map(|t| match index.get(t) {
                Some(i) => format!("~{i}"),
                None if t.starts_with('~') => format!("~{t}"),
                None => t.to_owned(),
            })
            .collect();
        out.push_str(&toks.join(" "));
        out.push_str(nl);
    }
    out
}

/// The terse text a dense form stands for; `None` when it is not a dense form.
pub fn expand(dense: &str) -> Option<String> {
    let rest = dense.strip_prefix(HEADER)?.strip_prefix('\n')?;
    let (dict_line, body) = rest.split_once('\n').unwrap_or((rest, ""));
    let dict: BTreeMap<usize, &str> = dict_line
        .strip_prefix("#dict")?
        .split(' ')
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (i, t) = p.split_once('=')?;
            Some((i.parse().ok()?, t))
        })
        .collect();
    let mut out = String::new();
    for line in body.split_inclusive('\n') {
        let (b, nl) = match line.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (line, ""),
        };
        let toks: Vec<String> = b
            .split(' ')
            .map(|t| {
                if let Some(r) = t.strip_prefix("~~") {
                    format!("~{r}")
                } else if let Some(i) = t.strip_prefix('~').and_then(|n| n.parse::<usize>().ok()) {
                    dict.get(&i)
                        .map_or_else(|| t.to_owned(), |x| (*x).to_owned())
                } else {
                    t.to_owned()
                }
            })
            .collect();
        out.push_str(&toks.join(" "));
        out.push_str(nl);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_ctx_034__dense_round_trips_to_terse() {
        let terse = "@root d=abcd1234 ed=1\ncmp.button-primary layer=ui.atom uses=3\ncmp.button-primary-ghost layer=ui.atom uses=1\ncmp.button-primary layer=ui.atom\n~odd token cmp.button-primary\nrule.no-raw-hex sev=error rule.no-raw-hex rule.no-raw-hex\n";
        let d = compress(terse);
        assert!(
            d.starts_with("#dense v1\n#dict "),
            "the dictionary comes first: {d}"
        );
        assert!(d.contains("~0"), "{d}");
        assert_eq!(expand(&d).as_deref(), Some(terse));
        assert!(d.len() < terse.len() + 40, "{d}");
        assert_eq!(expand("not dense"), None);
        for t in ["", "a\n", "~~x ~1 y", "no newline at end"] {
            assert_eq!(expand(&compress(t)).as_deref(), Some(t), "{t:?}");
        }
    }
}
