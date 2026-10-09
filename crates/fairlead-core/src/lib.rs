#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        non_snake_case
    )
)]
//! Core library of fairlead. P3 owns the foundation modules declared below; later phases append
//! their own `pub mod` lines. Module list and signatures: plans/0.1/01-contract.md Sections 3 and 4.

#[cfg(test)]
pub(crate) mod testutil;

pub mod classify;
pub mod config;
pub mod diag;
pub mod discover;
pub mod error;
pub mod hash;
pub mod outcome;
pub mod path;
pub mod time;
