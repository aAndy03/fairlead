//! Check engine (P5a). Pure: no filesystem, no git, no clock (R-ARCH-050, R-CHK-004).
pub(crate) mod apply;
pub mod baseline;
pub mod builtin;
pub mod codes;
pub mod finding;
pub mod fingerprint;
pub mod floor;
pub mod options;
pub mod rules;
pub mod summary;
