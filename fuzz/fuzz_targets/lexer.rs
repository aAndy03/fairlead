#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut last = 0usize;
    for span in fairlead_syntax::lexer::comment_spans(&text, "rs") {
        assert!(span.start >= last && span.start <= span.end && span.end <= text.len(), "spans are ordered and in bounds");
        last = span.end;
    }
});
