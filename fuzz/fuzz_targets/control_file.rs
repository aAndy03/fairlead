#![no_main]
use fairlead_core::config::{parse_control, parse_header_lenient};
use fairlead_core::path::RepoPath;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(path) = RepoPath::new("fairlead/control.toml") {
        let _ = parse_control(&path, data, true);
        let _ = parse_control(&path, data, false);
    }
    let _ = parse_header_lenient(data);
});
