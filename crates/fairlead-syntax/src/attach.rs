//! Attachment of directives to code: file, trailing, standalone, explicit range, regions (R-SYN-016..023, 045, 046).
use crate::directive::{Explicit, Extent};
use crate::lexer::Lexed;
use crate::normalize::{Normalized, slice};
use core::ops::Range;
use std::collections::BTreeSet;

/// One parsed directive as `attach` sees it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Site {
    pub first_line: u32,
    pub last_line: u32,
    /// Code shares the comment's line (trailing form, R-SYN-017).
    pub inline: bool,
    pub explicit: Option<Explicit>,
    /// The verb is `end`.
    pub is_end: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Bound {
        extent: Extent,
        end_site: Option<usize>,
    },
    /// An `end` that closed a `+end` region; it is structure, not a directive record.
    ClosesRegion,
    NothingToBind,
    UnmatchedEnd,
    UnclosedRegion,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Kind {
    Blank,
    CommentOnly,
    Code,
}

fn blank_out(b: &mut [u8], r: Range<usize>) {
    for k in r {
        if let Some(x) = b.get_mut(k)
            && *x != b'\n'
        {
            *x = b' ';
        }
    }
}

/// Copy of the text with comment bytes (and string bytes when `strings`) replaced by spaces.
fn mask(n: &Normalized, lexed: &Lexed, strings: bool) -> String {
    let mut b = n.text.as_bytes().to_vec();
    for c in &lexed.comments {
        blank_out(&mut b, c.start..c.end);
    }
    if strings {
        for s in &lexed.strings {
            blank_out(&mut b, s.clone());
        }
    }
    String::from_utf8(b).unwrap_or_else(|e| {
        let mut raw = e.into_bytes();
        for x in raw.iter_mut().filter(|x| !x.is_ascii()) {
            *x = b' ';
        }
        String::from_utf8(raw).unwrap_or_default()
    })
}

/// `*` matches any run of characters; everything else is literal.
fn wildcard(pattern: &str, text: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let Some(mut rest) = text.strip_prefix(first) else {
        return false;
    };
    let all: Vec<&str> = parts.collect();
    let Some((last, mids)) = all.split_last() else {
        return rest.is_empty();
    };
    for m in mids {
        rest = match rest.find(m) {
            Some(i) => rest.get(i + m.len()..).unwrap_or(""),
            None => return false,
        };
    }
    rest.ends_with(last)
}

struct Ctx<'a> {
    n: &'a Normalized,
    ext: String,
    kinds: Vec<Kind>,
    /// Comments and strings masked (bracket matching, indentation).
    code: String,
    count: u32,
}

impl<'a> Ctx<'a> {
    fn new(n: &'a Normalized, lexed: &Lexed, ext: &str) -> Ctx<'a> {
        let no_comments = mask(n, lexed, false);
        let code = mask(n, lexed, true);
        let count = n.line_count();
        let kinds = (1..=count)
            .map(|l| {
                if n.line_text(l).trim().is_empty() {
                    Kind::Blank
                } else if slice(&no_comments, n.line_start(l), n.line_end(l))
                    .trim()
                    .is_empty()
                {
                    Kind::CommentOnly
                } else {
                    Kind::Code
                }
            })
            .collect();
        Ctx {
            n,
            ext: ext.trim_start_matches('.').to_ascii_lowercase(),
            kinds,
            code,
            count,
        }
    }

    fn kind(&self, line: u32) -> Kind {
        line.checked_sub(1)
            .and_then(|i| self.kinds.get(i as usize))
            .copied()
            .unwrap_or(Kind::Blank)
    }

    fn is_html_family(&self) -> bool {
        matches!(self.ext.as_str(), "html" | "xml" | "svg")
    }

    fn header_line(&self, l: u32, header_skip: &[String]) -> bool {
        let t = self.n.line_text(l).trim();
        t.is_empty()
            || (l == 1 && t.starts_with("#!"))
            || t.starts_with("<?php")
            || matches!(
                t.trim_end_matches(';'),
                "\"use client\"" | "'use client'" | "\"use strict\"" | "'use strict'"
            )
            || (self.is_html_family()
                && (t.to_ascii_lowercase().starts_with("<!doctype") || t.starts_with("<?xml")))
            || (self.kind(l) == Kind::CommentOnly && t.contains("@ts-check"))
            || header_skip.iter().any(|p| wildcard(p, t))
    }

    /// The first comment block, when it is followed by a blank line or EOF (R-SYN-016).
    fn file_block(&self, header_skip: &[String]) -> Option<(u32, u32)> {
        let mut l = 1;
        while l <= self.count && self.header_line(l, header_skip) {
            l += 1;
        }
        if l > self.count || self.kind(l) != Kind::CommentOnly {
            return None;
        }
        let start = l;
        while l < self.count && self.kind(l + 1) == Kind::CommentOnly {
            l += 1;
        }
        (l == self.count || self.kind(l + 1) == Kind::Blank).then_some((start, l))
    }

    fn code_line(&self, l: u32) -> &str {
        slice(&self.code, self.n.line_start(l), self.n.line_end(l))
    }

    /// First line at or after `from` that holds code (comments and strings masked).
    fn next_nonblank(&self, from: u32) -> Option<u32> {
        (from..=self.count).find(|&l| !self.code_line(l).trim().is_empty())
    }

    /// Attribute and decorator lines (R-SYN-045).
    fn is_attribute(&self, l: u32) -> bool {
        let t = self.code_line(l).trim();
        match self.ext.as_str() {
            "rs" => t.starts_with("#[") || t.starts_with("#!["),
            "cs" => t.starts_with('[') && t.ends_with(']'),
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "java" | "kt" | "scala" | "swift" | "py" => t
                .strip_prefix('@')
                .is_some_and(|r| r.chars().next().is_some_and(|c| c.is_ascii_alphabetic())),
            _ => false,
        }
    }

    /// Last line of an attribute that may span lines (bracket balanced, at most 50 lines).
    fn attr_end(&self, l: u32) -> u32 {
        let cap = l.saturating_add(50).min(self.count);
        let mut depth = 0i32;
        let mut k = l;
        while k <= cap {
            for c in self.code_line(k).bytes() {
                match c {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= 0 {
                return k;
            }
            k += 1;
        }
        cap
    }

    /// `(first, bind)`: the first skipped comment/attribute line directly above the code line (or the code line)
    /// and the code line itself. `dirs` are lines occupied by directives (they never start a run).
    fn bind_target(&self, after: u32, dirs: &BTreeSet<u32>) -> Option<(u32, u32)> {
        let mut run: Option<u32> = None;
        let mut l = after + 1;
        while l <= self.count {
            match self.kind(l) {
                Kind::Blank => run = None,
                Kind::CommentOnly => {
                    if !dirs.contains(&l) && run.is_none() {
                        run = Some(l);
                    }
                }
                Kind::Code => {
                    if self.is_attribute(l) {
                        run.get_or_insert(l);
                        l = self.attr_end(l);
                    } else {
                        return Some((run.unwrap_or(l), l));
                    }
                }
            }
            l += 1;
        }
        None
    }

    /// Last line of the item that starts on code line `b` (R-SYN-018, R-SYN-046).
    fn extent_last(&self, b: u32) -> u32 {
        let cap = b.saturating_add(MAX_EXTENT - 1).min(self.count);
        let (mut depth, mut opened, mut sig_checked) = (0i32, false, false);
        let mut l = b;
        while l <= cap {
            let mut semi = false;
            for c in self.code_line(l).bytes() {
                match c {
                    b'{' | b'(' | b'[' => {
                        depth += 1;
                        opened = true;
                    }
                    b'}' | b')' | b']' => depth = (depth - 1).max(0),
                    b';' if depth == 0 && opened => semi = true,
                    _ => {}
                }
            }
            if depth > 0 {
                l += 1;
                continue;
            }
            if !opened {
                return self.indent_extent(b, l, cap);
            }
            if semi || sig_checked {
                return l;
            }
            match self.next_nonblank(l + 1) {
                Some(nl) if nl <= cap && self.code_line(nl).trim_start().starts_with('{') => {
                    sig_checked = true;
                    l = nl;
                }
                _ => return self.indent_extent(b, l, cap),
            }
        }
        cap
    }

    /// Indented-block rule: item header on `b`, its last bracket-balanced line is `from`. A trailing `:` on `from`
    /// or a deeper-indented next line extends the extent over the indented block (R-SYN-018).
    fn indent_extent(&self, b: u32, from: u32, cap: u32) -> u32 {
        let base = indent_of(self.code_line(b));
        let colon = self.code_line(from).trim_end().ends_with(':');
        let deeper = self
            .next_nonblank(from + 1)
            .is_some_and(|nl| indent_of(self.code_line(nl)) > base);
        if !(colon || deeper) {
            return from;
        }
        let mut last = from;
        let mut l = from + 1;
        while l <= cap {
            let s = self.code_line(l);
            if s.trim().is_empty() {
                l += 1;
            } else if indent_of(s) > base {
                last = l;
                l += 1;
            } else {
                break;
            }
        }
        last
    }

    fn is_markdown(&self) -> bool {
        matches!(self.ext.as_str(), "md" | "markdown")
    }

    /// R-SYN-022: the block, fence or heading section that follows a body comment.
    fn md_bind(&self, s: &Site) -> Option<Extent> {
        if s.inline {
            return Some(Extent::Lines {
                first: s.first_line,
                last: s.first_line,
            });
        }
        let mut l = s.last_line + 1;
        while l <= self.count && self.kind(l) != Kind::Code {
            l += 1;
        }
        if l > self.count {
            return None;
        }
        let first_text = self.n.line_text(l);
        if let Some(level) = heading_level(first_text) {
            let mut last = l;
            for k in l + 1..=self.count {
                let t = self.n.line_text(k);
                if heading_level(t).is_some_and(|h| h <= level) {
                    break;
                }
                if !t.trim().is_empty() {
                    last = k;
                }
            }
            return Some(if level == 1 {
                Extent::Doc
            } else {
                Extent::Lines { first: l, last }
            });
        }
        if let Some((ch, len)) = fence_of(first_text) {
            let mut last = self.count;
            for k in l + 1..=self.count {
                let t = self.n.line_text(k).trim();
                if t.len() >= len && t.bytes().all(|b| b == ch) {
                    last = k;
                    break;
                }
            }
            return Some(Extent::Lines { first: l, last });
        }
        let mut last = l;
        while last < self.count && !self.n.line_text(last + 1).trim().is_empty() {
            last += 1;
        }
        Some(Extent::Lines { first: l, last })
    }
}

pub const MAX_EXTENT: u32 = 500;

fn indent_of(s: &str) -> usize {
    let mut w = 0;
    for c in s.chars() {
        match c {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => break,
        }
    }
    w
}

fn region_start(s: &Site) -> u32 {
    if s.inline {
        s.first_line
    } else {
        s.last_line + 1
    }
}

/// One outcome per site, in order (R-SYN-016..019, 021, 045, 046).
pub fn attach(
    n: &Normalized,
    lexed: &Lexed,
    ext: &str,
    header_skip: &[String],
    sites: &[Site],
) -> Vec<Outcome> {
    let cx = Ctx::new(n, lexed, ext);
    let block = if cx.is_markdown() {
        None
    } else {
        cx.file_block(header_skip)
    };
    let dirs: BTreeSet<u32> = sites
        .iter()
        .flat_map(|s| s.first_line..=s.last_line)
        .collect();
    let bound = |first: u32, last: u32, end_site: Option<usize>| Outcome::Bound {
        extent: Extent::Lines { first, last },
        end_site,
    };
    let mut out: Vec<Option<Outcome>> = vec![None; sites.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, s) in sites.iter().enumerate() {
        let result = if s.is_end {
            match stack.pop().and_then(|o| sites.get(o).map(|op| (o, op))) {
                None => Outcome::UnmatchedEnd,
                Some((o, op)) => {
                    let last = s.first_line.saturating_sub(1).max(op.last_line);
                    let first = region_start(op).min(last);
                    if let Some(slot) = out.get_mut(o) {
                        *slot = Some(bound(first, last, Some(i)));
                    }
                    Outcome::ClosesRegion
                }
            }
        } else if s.explicit == Some(Explicit::Region) {
            stack.push(i);
            continue;
        } else if let Some(Explicit::Lines(k)) = s.explicit {
            let target = if s.inline {
                Some((s.first_line, s.first_line))
            } else {
                cx.bind_target(s.last_line, &dirs)
            };
            match target {
                Some((first, _)) => bound(first, first.saturating_add(k - 1).min(cx.count), None),
                None => Outcome::NothingToBind,
            }
        } else if let (Some((a, b)), false) = (block, s.inline) {
            if s.first_line >= a && s.last_line <= b {
                Outcome::Bound {
                    extent: Extent::File,
                    end_site: None,
                }
            } else {
                standalone(&cx, s, &dirs)
            }
        } else if s.inline {
            bound(s.first_line, s.first_line, None)
        } else {
            standalone(&cx, s, &dirs)
        };
        if let Some(slot) = out.get_mut(i) {
            *slot = Some(result);
        }
    }
    for o in stack {
        if let Some(slot) = out.get_mut(o) {
            *slot = Some(Outcome::UnclosedRegion);
        }
    }
    out.into_iter()
        .map(|o| o.unwrap_or(Outcome::NothingToBind))
        .collect()
}

fn standalone(cx: &Ctx<'_>, s: &Site, dirs: &BTreeSet<u32>) -> Outcome {
    if cx.is_markdown() {
        return match cx.md_bind(s) {
            Some(extent) => Outcome::Bound {
                extent,
                end_site: None,
            },
            None => Outcome::NothingToBind,
        };
    }
    match cx.bind_target(s.last_line, dirs) {
        Some((first, b)) => Outcome::Bound {
            extent: Extent::Lines {
                first,
                last: cx.extent_last(b),
            },
            end_site: None,
        },
        None => Outcome::NothingToBind,
    }
}

fn heading_level(line: &str) -> Option<usize> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.bytes().take_while(|b| *b == b'#').count();
    let after = t.as_bytes().get(level).copied();
    ((1..=6).contains(&level) && matches!(after, None | Some(b' ') | Some(b'\t'))).then_some(level)
}

fn fence_of(line: &str) -> Option<(u8, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let ch = *t.as_bytes().first()?;
    let run = t.bytes().take_while(|b| *b == ch).count();
    ((ch == b'`' || ch == b'~') && run >= 3).then_some((ch, run))
}

/// Text with comment bytes blanked (strings kept): the haystack of the `sym=` presence check (R-SYN-020).
pub(crate) fn code_text(n: &Normalized, lexed: &Lexed) -> String {
    mask(n, lexed, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delimiters::CommentTable;
    use crate::directive::{Verb, parse_body};
    use crate::lexer::{lex, scan_markers};

    pub(crate) fn run_with(src: &str, ext: &str, header_skip: &[String]) -> Vec<Outcome> {
        let n = Normalized::new(src);
        let table = CommentTable::builtin();
        let leaders = table.leaders_for(ext).unwrap().to_vec();
        let lexed = lex(&n, ext, &leaders);
        let scan = scan_markers(&n, &lexed, "fairlead");
        let sites: Vec<Site> = scan
            .candidates
            .iter()
            .map(|c| {
                let p = parse_body(&c.body).unwrap();
                Site {
                    first_line: c.first_line,
                    last_line: c.last_line,
                    inline: c.inline,
                    explicit: p.explicit,
                    is_end: p.verb == Verb::End,
                }
            })
            .collect();
        attach(&n, &lexed, ext, header_skip, &sites)
    }
    pub(crate) fn run(src: &str, ext: &str) -> Vec<Outcome> {
        run_with(src, ext, &[])
    }
    pub(crate) fn file() -> Outcome {
        Outcome::Bound {
            extent: Extent::File,
            end_site: None,
        }
    }

    #[test]
    fn r_syn_016__file_form_after_license_banner() {
        let src = "/*\n * License\n */\n// fairlead: use tok.a\n\nfn main() {}\n";
        assert_eq!(run(src, "rs"), vec![file()]);
        assert_eq!(
            run(
                "// Copyright 2026\n// fairlead: def cmp.x\n\nfn main() {}\n",
                "rs"
            ),
            vec![file()]
        );
    }

    #[test]
    fn r_syn_016__skips_bom_shebang_use_client_and_ts_check() {
        assert_eq!(
            run("\u{feff}// fairlead: use tt.b\n\ncode\n", "rs"),
            vec![file()]
        );
        assert_eq!(
            run(
                "#!/usr/bin/env python\n# fairlead: use tt.b\n\nimport os\n",
                "py"
            ),
            vec![file()]
        );
        assert_eq!(
            run(
                "\"use client\";\n// fairlead: use tok.a\n\nexport {}\n",
                "tsx"
            ),
            vec![file()]
        );
        assert_eq!(
            run("'use strict'\n// fairlead: use tok.a\n\nx()\n", "js"),
            vec![file()]
        );
        assert_eq!(
            run("// @ts-check\n// fairlead: use tt.b\n\nx\n", "ts"),
            vec![file()]
        );
        assert_eq!(
            run("<?php\n// fairlead: use tt.b\n\nx();\n", "rs"),
            vec![file()],
            "<?php line is skipped regardless of host"
        );
        assert_eq!(
            run(
                "<?xml version=\"1.0\"?>\n<!doctype svg>\n<!-- fairlead: def asset.logo -->\n\n<svg/>\n",
                "svg"
            ),
            vec![file()]
        );
    }

    #[test]
    fn r_syn_016__header_skip_patterns_use_star_wildcards() {
        let skip = vec![
            "// eslint-disable*".to_owned(),
            "/* generated by * */".to_owned(),
        ];
        assert_eq!(
            run_with(
                "// eslint-disable-next-line\n// fairlead: use tt.b\n\ncode\n",
                "ts",
                &skip
            ),
            vec![file()]
        );
        assert_eq!(
            run_with(
                "/* generated by tool 1.2 */\n// fairlead: use tt.b\n\ncode\n",
                "ts",
                &skip
            ),
            vec![file()]
        );
        assert_eq!(
            run(
                "// eslint-disable-next-line\n// fairlead: use tt.b\n\ncode\n",
                "ts"
            ),
            vec![file()],
            "without the pattern both lines form the block"
        );
    }

    #[test]
    fn r_syn_016__eof_also_ends_the_block_and_directives_stack() {
        assert_eq!(run("// fairlead: use tt.b", "rs"), vec![file()]);
        assert_eq!(
            run(
                "// fairlead: def tt.b\n// fairlead: use uu.d\n\ncode\n",
                "rs"
            ),
            vec![file(), file()]
        );
    }

    #[test]
    fn r_syn_016__block_directly_followed_by_code_is_not_file_form() {
        let out = run("// fairlead: use tt.b\nfn f() {}\n", "rs");
        assert_ne!(
            out,
            vec![file()],
            "edge case 5: standalone form, not file form"
        );
    }

    #[test]
    fn r_syn_016__later_block_is_not_file_form() {
        let out = run("fn a() {}\n\n// fairlead: use tt.b\n\nfn b() {}\n", "rs");
        assert_ne!(out, vec![file()]);
    }

    pub(crate) fn lines(first: u32, last: u32) -> Outcome {
        Outcome::Bound {
            extent: Extent::Lines { first, last },
            end_site: None,
        }
    }

    #[test]
    fn r_syn_017__trailing_directive_binds_to_its_own_line() {
        assert_eq!(
            run("let x = 1; // fairlead: use tt.b\nlet y = 2;\n", "rs"),
            vec![lines(1, 1)]
        );
        assert_eq!(
            run("x = 1  # fairlead: use tt.b\ny = 2\n", "py"),
            vec![lines(1, 1)]
        );
        let jsx = "<Button>\n  {/* fairlead: use tok.surface */}\n  <Icon />\n</Button>\n";
        assert_eq!(
            run(jsx, "tsx"),
            vec![lines(2, 2)],
            "JSX comment sits between braces on its own line"
        );
    }

    #[test]
    fn r_syn_018__standalone_one_liner_and_blank_lines() {
        assert_eq!(
            run("fn a() {}\n// fairlead: use tt.b\nlet x = 1;\n", "rs"),
            vec![lines(3, 3)]
        );
        assert_eq!(
            run("fn a() {}\n\n// fairlead: use tt.b\n\nfn b() {}\n", "rs"),
            vec![lines(5, 5)]
        );
    }

    #[test]
    fn r_syn_018__unbalanced_opener_runs_to_the_matching_close() {
        let src = "fn pre() {}\n\n// fairlead: def cmp.x\nfn run() {\n    if x {\n        y();\n    }\n}\nfn after() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 8)]);
        let ts = "const a = 1;\n\n// fairlead: def cmp.b\nexport function B() {\n  return (\n    <div/>\n  );\n}\n";
        assert_eq!(run(ts, "tsx"), vec![lines(4, 8)]);
    }

    #[test]
    fn r_syn_018__colon_or_deeper_indent_takes_the_indented_block() {
        let py = "import os\n\n# fairlead: use tt.b\ndef f(x):\n    return x\n\ndef g(): pass\n";
        assert_eq!(run(py, "py"), vec![lines(4, 5)]);
        let yaml = "a: 1\n\n# fairlead: use tt.b\nservices:\n  web:\n    image: x\n\nother: 2\n";
        assert_eq!(run(yaml, "yaml"), vec![lines(4, 6)]);
        let toml = "[package]\nname = \"x\"\n\n# fairlead: use eng.sync-verify\nsync-verify = { path = \"../sync-verify\" }\n";
        assert_eq!(run(toml, "toml"), vec![lines(5, 5)]);
    }

    #[test]
    fn r_syn_018__consecutive_directives_stack_on_one_code_line() {
        let src = "x\n\n// fairlead: def tt.b\n// fairlead: use uu.d\nfn f() {\n}\n";
        assert_eq!(run(src, "rs"), vec![lines(5, 6), lines(5, 6)]);
    }

    #[test]
    fn r_syn_045__attribute_lines_are_skipped_and_included() {
        let src = "fn pre() {}\n\n// fairlead: def cmd.x\n#[tauri::command]\npub async fn sync_now(\n    a: i32,\n) -> Result<(), E>\n{\n    body();\n}\nfn post() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 10)]);
        let doc = "x\n\n// fairlead: def cmd.y\n/// Docs\n#[attr]\nfn y() {}\n";
        assert_eq!(run(doc, "rs"), vec![lines(4, 6)]);
        let dec = "x\n\n// fairlead: def cmp.z\n@Component({\n  a: 1,\n})\nexport class Z {\n}\n";
        assert_eq!(run(dec, "ts"), vec![lines(4, 8)]);
        let cs = "x;\n\n// fairlead: def cmd.w\n[HttpGet]\npublic void W() {\n}\n";
        assert_eq!(run(cs, "cs"), vec![lines(4, 6)]);
    }

    #[test]
    fn r_syn_046__multi_line_signature_binds_with_its_body() {
        let src =
            "x\n\n// fairlead: def fn.a\nfn a(\n    p: i32,\n) -> i32 {\n    p\n}\nfn b() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 8)]);
        let decl = "x\n\n// fairlead: def fn.c\nfn c(\n    p: i32,\n);\nfn d() {}\n";
        assert_eq!(
            run(decl, "rs"),
            vec![lines(4, 6)],
            "a `;` at depth 0 ends the item"
        );
    }

    #[test]
    fn r_syn_018__extent_is_capped_at_500_lines() {
        let mut src = String::from("x\n\n// fairlead: def tt.b\nfn big() {\n");
        for _ in 0..600 {
            src.push_str("    y();\n");
        }
        src.push_str("}\n");
        assert_eq!(run(&src, "rs"), vec![lines(4, 503)]);
    }

    #[test]
    fn r_syn_023__brackets_in_strings_and_comments_are_ignored() {
        let src = "x\n\n// fairlead: def tt.b\nfn f() {\n    let s = \"}\"; // }\n}\nfn g() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 6)]);
    }

    #[test]
    fn r_syn_019__plus_n_sets_the_extent_to_n_lines() {
        let src = "x\n\n// fairlead: use tt.b +3\nfn a() {\n  x();\n}\nfn b() {}\n";
        assert_eq!(run(src, "rs"), vec![lines(4, 6)]);
        assert_eq!(
            run(
                "let a = 1; // fairlead: use tt.b +2\nlet b = 2;\nlet c = 3;\n",
                "rs"
            ),
            vec![lines(1, 2)]
        );
        assert_eq!(
            run("x\n\n// fairlead: use tt.b +9\nfn a() {}\n", "rs"),
            vec![lines(4, 4)],
            "clamped to the last line"
        );
    }

    #[test]
    fn r_syn_019__plus_end_region_closes_at_matching_end() {
        let css = ":root { --primary: #2563eb; }\n/* fairlead: env dev +end */\n.debug-grid { outline: 1px solid red; }\n.more { x: y; }\n/* fairlead: end */\n.after {}\n";
        assert_eq!(
            run(css, "css"),
            vec![
                Outcome::Bound {
                    extent: Extent::Lines { first: 3, last: 4 },
                    end_site: Some(1)
                },
                Outcome::ClosesRegion
            ]
        );
    }

    #[test]
    fn r_syn_019__regions_nest_and_close_innermost_first() {
        let src = "x\n/* fairlead: env a +end */\nA\n/* fairlead: env b +end */\nB\n/* fairlead: end */\nC\n/* fairlead: end */\nD\n";
        let out = run(src, "css");
        assert_eq!(
            out[0],
            Outcome::Bound {
                extent: Extent::Lines { first: 3, last: 7 },
                end_site: Some(3)
            }
        );
        assert_eq!(
            out[1],
            Outcome::Bound {
                extent: Extent::Lines { first: 5, last: 5 },
                end_site: Some(2)
            }
        );
        assert_eq!(out[2], Outcome::ClosesRegion);
        assert_eq!(out[3], Outcome::ClosesRegion);
    }

    #[test]
    fn r_syn_019__foreign_region_in_css() {
        let src = "a {}\n/* fairlead: def foreign.shadcn tool=shadcn +end */\n.shadcn-block { color: #111; }\n/* fairlead: end */\n";
        let out = run(src, "css");
        assert_eq!(
            out[0],
            Outcome::Bound {
                extent: Extent::Lines { first: 3, last: 3 },
                end_site: Some(1)
            }
        );
    }

    #[test]
    fn r_syn_019__unmatched_end_and_unclosed_region_are_flagged() {
        assert_eq!(
            run("a {}\n/* fairlead: end */\n", "css"),
            vec![Outcome::UnmatchedEnd]
        );
        assert_eq!(
            run("a {}\n/* fairlead: env dev +end */\nb {}\n", "css"),
            vec![Outcome::UnclosedRegion]
        );
    }

    #[test]
    fn r_syn_019__a_directive_never_becomes_a_region_by_lookahead() {
        let out = run(
            "a {}\n/* fairlead: env dev */\nb { x: y; }\n/* fairlead: end */\n",
            "css",
        );
        assert_eq!(out[0], lines(3, 3), "no +end: ordinary standalone binding");
        assert_eq!(out[1], Outcome::UnmatchedEnd);
    }

    #[test]
    fn r_syn_019__empty_region_is_the_openers_last_line() {
        let out = run(
            "a {}\n/* fairlead: env dev +end */\n/* fairlead: end */\nb {}\n",
            "css",
        );
        assert_eq!(
            out[0],
            Outcome::Bound {
                extent: Extent::Lines { first: 2, last: 2 },
                end_site: Some(1)
            }
        );
    }

    #[test]
    fn r_syn_021__directive_with_nothing_after_it_binds_to_nothing() {
        assert_eq!(
            run("fn a() {}\n// fairlead: use tt.b\n", "rs"),
            vec![Outcome::NothingToBind]
        );
        assert_eq!(
            run("fn a() {}\n// fairlead: use tt.b\n\n\n", "rs"),
            vec![Outcome::NothingToBind]
        );
        assert_eq!(
            run("fn a() {}\n// fairlead: use tt.b\n// plain comment\n", "rs"),
            vec![Outcome::NothingToBind]
        );
    }

    #[test]
    fn r_syn_022__comment_before_a_paragraph_binds_the_block() {
        let src = "# T\n\n<!-- fairlead: use tt.b -->\nSome text\nmore text\n\nnext\n";
        assert_eq!(run(src, "md"), vec![lines(4, 5)]);
        let list = "<!-- fairlead: use tt.b -->\n- one\n- two\n\nafter\n";
        assert_eq!(run(list, "md"), vec![lines(2, 3)]);
        let table = "intro\n\n<!-- fairlead: use tt.b -->\n| a | b |\n|---|---|\n| 1 | 2 |\n";
        assert_eq!(run(table, "md"), vec![lines(4, 6)]);
    }

    #[test]
    fn r_syn_022__comment_before_a_code_fence_binds_the_fence() {
        let src = "<!-- fairlead: use tt.b -->\n```rs\ncode\n```\nafter\n";
        assert_eq!(run(src, "md"), vec![lines(2, 4)]);
        assert_eq!(
            run("<!-- fairlead: use tt.b -->\n~~~~\nopen forever\n", "md"),
            vec![lines(2, 3)]
        );
    }

    #[test]
    fn r_syn_022__comment_before_a_heading_binds_the_section() {
        let src = "<!-- fairlead: use tt.b -->\n## Sec\ntext\n\n### Sub\nmore\n\n## Next\nx\n";
        assert_eq!(run(src, "md"), vec![lines(2, 6)]);
        let title = "<!-- fairlead: use tok.primary -->\n# Colour\n\ntext\n";
        assert_eq!(
            run(title, "md"),
            vec![Outcome::Bound {
                extent: Extent::Doc,
                end_site: None
            }]
        );
    }

    #[test]
    fn r_syn_022__inline_comment_binds_its_line_and_no_target_binds_nothing() {
        assert_eq!(
            run("Text <!-- fairlead: use tt.b --> more\nnext\n", "md"),
            vec![lines(1, 1)]
        );
        assert_eq!(
            run("# T\n<!-- fairlead: use tt.b -->\n", "md"),
            vec![Outcome::NothingToBind]
        );
    }

    #[test]
    fn r_syn_016__markdown_has_no_file_form() {
        let out = run("<!-- fairlead: use tt.b -->\n\nBody text\n", "md");
        assert_eq!(out, vec![lines(3, 3)]);
    }
}
