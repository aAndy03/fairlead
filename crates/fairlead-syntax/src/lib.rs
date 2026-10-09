//! `fairlead-syntax`: the pure directive grammar of fairlead (contract sections 3 and 4.1 to 4.2).
//!
//! No I/O, no git, no threads, and no dependency on `fairlead-core` (R-ARCH-002).
#![forbid(unsafe_code)]
// R-ARCH-032: extraction must not panic on any bytes, so slice indexing is denied outside tests.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

/// Version of the syntax implementation (contract section 3, `lib`).
pub const SYNTAX_VERSION: u32 = 1;

/// Grammar edition of this build (D-17, D-45): an integer, 1 in 0.1.
pub const GRAMMAR_EDITION: u32 = 1;
