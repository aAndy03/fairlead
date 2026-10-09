#![allow(non_snake_case)] // reason: test names start with the rule id (r_syn_016__case)
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
//! Property tests for `fairlead_syntax::extract` (R-ARCH-031/032/033, R-ARCH-150).
use fairlead_syntax::{Carrier, ExtractConfig, Extraction, extract, render_directive_body};
use proptest::prelude::*;

const FRAGMENTS: &[&str] = &[
    "// fairlead: use cmp.a",
    "// fairlead: def cmp.b sym=run layer=molecule",
    "// fairlead: ovr rule.x why=\"w\" owner=@a exp=2026-12-31 kw+=a,b",
    "// fairlead: ovr rule.x why=\"w\" until=1.9.0",
    "// fairlead: flag deprecate cmp.a target=2.0.0 -- note \u{e9}",
    "// fairlead: env dev,staging",
    "// fairlead: frobnicate x.y",
    "// fairlead: use Bad.Id",
    "// fairlead: use cmp.a why=\"unterminated",
    "// fairlead: def foreign.t tool=t +end",
    "// fairlead: use cmp.c +3",
    "/* fairlead: env dev +end */",
    "/* fairlead: end */",
    "// fairlead: end",
    "// fairlead| k=v",
    "// fairlead|  why=\"more\"",
    "/// fairlead: use doc.leak",
    "//! fairlead: use doc.leak",
    "// Fairlead: use near.miss",
    "// fairlead\\: escaped",
    "let s = \"// fairlead: use in.string\";",
    "let r = r#\"// fairlead: use in.raw\"#;",
    "let c = '\"'; // fairlead: use tok.trail",
    "#[attr]",
    "fn f() {",
    "    body();",
    "}",
    "fn g(",
    ") -> T",
    "",
    "",
    "\t",
    "/*",
    " * fairlead: use cmp.block",
    " */",
    "'",
    "\"",
    "`",
    "r#\"",
];

fn doc_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS.to_vec()), 0..40)
        .prop_map(|v| v.join("\n") + "\n")
}

const EXTS: &[&str] = &[
    "rs", "ts", "tsx", "md", "py", "css", "html", "sh", "toml", "json", "zzz", "",
];

fn shape(e: &Extraction) -> String {
    let mut s = format!("{:?} {} {}\n", e.status, e.candidates, e.dropped);
    for d in &e.directives {
        s.push_str(&format!(
            "{} {} {:?} {}:{} {:?}\n",
            d.ordinal,
            render_directive_body(d),
            d.extent,
            d.span.line,
            d.span.col,
            d.carrier
        ));
    }
    for d in &e.diags {
        s.push_str(&format!(
            "{} {:?} {}:{} {}\n",
            d.code.0, d.severity, d.span.line, d.span.col, d.message
        ));
    }
    s
}

fn line_of(bytes: &[u8], off: usize) -> u32 {
    let (mut line, mut i) = (1u32, 0usize);
    while i < off {
        match bytes[i] {
            b'\n' => line += 1,
            b'\r' => {
                line += 1;
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    line
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_extract_never_panics_and_accounts_for_every_candidate(
        bytes in prop::collection::vec(any::<u8>(), 0..1500),
        ext in prop::sample::select(EXTS.to_vec()),
    ) {
        let e = extract(&bytes, ext, &ExtractConfig::default());
        prop_assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize);
    }

    #[test]
    fn prop_structured_documents_account_for_every_candidate(
        doc in doc_strategy(),
        ext in prop::sample::select(EXTS.to_vec()),
    ) {
        let e = extract(doc.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize);
        let ords: Vec<u32> = e.directives.iter().map(|d| d.ordinal).collect();
        prop_assert_eq!(ords, (0..e.directives.len() as u32).collect::<Vec<_>>());
    }

    #[test]
    fn prop_crlf_and_lf_checkouts_are_equivalent(doc in doc_strategy(), ext in prop::sample::select(EXTS.to_vec())) {
        let lf = extract(doc.as_bytes(), ext, &ExtractConfig::default());
        let crlf_text = doc.replace('\n', "\r\n");
        let crlf = extract(crlf_text.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(shape(&lf), shape(&crlf));
        let cr_text = doc.replace('\n', "\r");
        let cr = extract(cr_text.as_bytes(), ext, &ExtractConfig::default());
        prop_assert_eq!(shape(&lf), shape(&cr));
    }

    #[test]
    fn prop_offsets_map_to_original_bytes(doc in doc_strategy()) {
        let crlf = doc.replace('\n', "\r\n");
        let bytes = crlf.as_bytes();
        let e = extract(bytes, "rs", &ExtractConfig::default());
        for d in &e.directives {
            let (s, t) = (d.span.start as usize, d.span.end as usize);
            prop_assert!(s <= t && t <= bytes.len());
            prop_assert_eq!(d.span.line, line_of(bytes, s));
            if d.carrier != Carrier::Frontmatter {
                prop_assert!(bytes[s..].starts_with(b"fairlead:"), "directive span must start at the marker");
            }
        }
        for g in &e.diags {
            prop_assert!(g.span.start <= g.span.end && g.span.end as usize <= bytes.len());
            if g.span.line > 0 {
                prop_assert_eq!(g.span.line, line_of(bytes, g.span.start as usize));
            }
        }
    }

    #[test]
    fn prop_render_then_parse_is_stable(doc in doc_strategy()) {
        let e = extract(doc.as_bytes(), "rs", &ExtractConfig::default());
        for d in &e.directives {
            let body = render_directive_body(d);
            let tail = if d.end_span.is_some() { "// fairlead: end\n" } else { "" };
            let again = format!("x\n\n// fairlead: {body}\nfn x() {{}}\n{tail}");
            let e2 = extract(again.as_bytes(), "rs", &ExtractConfig::default());
            prop_assert_eq!(e2.directives.len(), 1, "{} => {:?}", body, e2.diags);
            prop_assert_eq!(render_directive_body(&e2.directives[0]), body);
        }
    }
}
