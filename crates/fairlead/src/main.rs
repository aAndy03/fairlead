//! The `fairlead` binary. The 0.1 verb set arrives with P8; this bootstrap build compiles and says so.
#![allow(clippy::print_stdout, clippy::print_stderr)] // reason: only the fairlead binary prints (contract section 1)

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("fairlead: bootstrap build, no verb is implemented yet");
    ExitCode::from(2)
}
