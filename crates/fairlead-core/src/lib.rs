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

pub mod cache;
pub mod check;
pub mod classify;
pub mod config;
pub mod context;
pub mod diag;
pub mod discover;
pub mod docs;
pub mod error;
pub mod explain;
pub mod extract;
pub mod facts;
pub mod generate;
pub mod git;
pub mod hash;
pub mod hist;
pub mod manifest;
pub mod outcome;
pub mod path;
pub mod pipeline;
pub mod registry;
pub mod render;
pub mod secrets;
pub mod time;
pub mod version;
pub mod walk;
pub mod write;
