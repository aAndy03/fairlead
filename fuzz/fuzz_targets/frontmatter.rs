#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Some((front, body)) = fairlead_syntax::frontmatter::split_frontmatter(&text) {
        assert!(front.end <= text.len() && body.end <= text.len(), "ranges stay inside the text");
    }
});
