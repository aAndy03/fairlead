//! `fairlead context`: the pure brief builder (seam S9). No I/O, no clock, no writes besides the
//! disposable token-estimate cache (R-CTX-006).
pub mod brief;
pub mod digest;
pub mod estimate;
pub mod request;

pub use digest::effective_digest;
pub use estimate::{TokenEstimate, estimate_bytes};
pub use request::{
    Budget, ContextArgError, ContextInput, ContextRequest, ContextTarget, NO_MATCH_MESSAGE,
    missing_note, parse_target,
};

/// `usize` to `u32` without a lossy cast.
#[allow(dead_code, reason = "used by the P6 gatherers (T7 onwards)")]
pub(crate) fn n32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
pub(crate) mod testutil {
    use crate::config::{ContextConfig, SevWeight};

    pub fn cfg() -> ContextConfig {
        ContextConfig {
            budget_start: 500,
            budget_path: 700,
            bytes_per_token: 4,
            tokenizer: None,
            sev_weight: SevWeight {
                error: 4,
                warn: 2,
                info: 1,
            },
        }
    }
}
