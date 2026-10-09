#![no_main]
use fairlead_syntax::{extract, render_directive_body, CommentTable, ExtractConfig};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let cfg = ExtractConfig {
        marker: "fairlead".into(),
        comments: CommentTable::builtin(),
        header_skip: Vec::new(),
        max_file: 2_000_000,
        max_directives: 500,
    };
    let e = extract(data, "rs", &cfg);
    assert_eq!(e.candidates as usize, e.directives.len() + e.dropped as usize, "R-ARCH-031");
    for d in &e.directives {
        let body = render_directive_body(d);
        let again = extract(format!("// fairlead: {body}\n").as_bytes(), "rs", &cfg);
        assert!(
            again.directives.iter().any(|x| render_directive_body(x) == body),
            "parse-render-parse is stable: {body}"
        );
    }
});
