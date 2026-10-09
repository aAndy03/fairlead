//! Pure directive extraction for fairlead (R-ARCH-002, R-ARCH-030).
//! No I/O, no clock, no `fairlead-core`. Every public function is total.
#![forbid(unsafe_code)]
// R-ARCH-032: extraction must not panic on any bytes, so slice indexing is denied outside tests.
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        non_snake_case
    )
)]

pub mod attach;
pub mod delimiters;
pub mod diag;
pub mod directive;
pub mod extract;
pub mod frontmatter;
pub mod ids;
pub mod lexer;
pub mod normalize;

/// Bumped when extraction behaviour changes in a way that must invalidate cached records.
pub const SYNTAX_VERSION: u32 = 1;
/// Directive grammar edition implemented by this crate (D-17, D-45).
pub const GRAMMAR_EDITION: u32 = 1;

pub use delimiters::{CommentTable, Leader};
pub use diag::{DiagCode, Severity, Span, SyntaxDiag, codes};
pub use directive::{
    Arg, ArgOp, ArgValue, Carrier, Directive, Extent, Verb, known_keys, render_directive_body,
};
pub use extract::{ExtractConfig, Extraction, ScanStatus, extract};
pub use frontmatter::{Frontmatter, split_frontmatter};
pub use normalize::lf_bytes;
