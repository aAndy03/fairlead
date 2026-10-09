//! Token estimates (R-ARCH-056, R-CTX-043). `ceil(bytes / bytes-per-token)`, always labelled.
use crate::cache::Cache;
use crate::error::Error;
use crate::hash::{ContentHash, hash_raw};
use crate::render::sanitize::sanitize_data;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenEstimate {
    pub tokens: u32,
    pub exact: bool,
}

/// `ceil(bytes / bpt)`; a ratio of 0 is clamped to 1 so a bad control file cannot divide by zero.
pub fn estimate_bytes(bytes: usize, bytes_per_token: u32) -> TokenEstimate {
    let bpt = u64::from(bytes_per_token.max(1));
    let n = u64::try_from(bytes).unwrap_or(u64::MAX).div_ceil(bpt);
    TokenEstimate {
        tokens: u32::try_from(n).unwrap_or(u32::MAX),
        exact: false,
    }
}

/// Per-object estimate store. The cache key set already contains the control-file hash, so a changed
/// `bytes-per-token` invalidates every entry (contract 4.7 `CacheKey.config_hash`).
pub trait EstimateStore {
    fn get(&self, h: &ContentHash) -> Option<u32>;
    fn put(&self, h: &ContentHash, tokens: u32);
}

impl EstimateStore for Cache {
    fn get(&self, h: &ContentHash) -> Option<u32> {
        self.token_estimate(h)
    }
    fn put(&self, h: &ContentHash, tokens: u32) {
        self.put_token_estimate(h, tokens);
    }
}

/// Estimate one rendered object, consulting and filling the store so packing never re-counts.
pub fn estimate_text(store: &dyn EstimateStore, text: &str, bytes_per_token: u32) -> TokenEstimate {
    let key = hash_raw(text.as_bytes());
    if let Some(tokens) = store.get(&key) {
        return TokenEstimate {
            tokens,
            exact: false,
        };
    }
    let est = estimate_bytes(text.len(), bytes_per_token);
    store.put(&key, est.tokens);
    est
}

/// `--exact-tokens` (R-CTX-043). 0.1 ships no exact counter and no network stack (R-ARCH-123), so this
/// always fails cleanly with exit class Usage.
pub fn exact_tokens(_text: &str, tokenizer: Option<&str>) -> Result<TokenEstimate, Error> {
    Err(Error::Usage(match tokenizer {
        None => "--exact-tokens needs a `tokenizer` in [context] of the root control file; \
                 token counts are estimates (ceil(bytes / bytes-per-token))"
            .to_string(),
        Some(name) => format!(
            "--exact-tokens: tokenizer \"{}\" is not built into this binary (0.1 ships none)",
            sanitize_data(name, 40)
        ),
    }))
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;
    use crate::error::ErrorClass;

    struct Mem(RefCell<BTreeMap<[u8; 32], u32>>);
    impl EstimateStore for Mem {
        fn get(&self, h: &ContentHash) -> Option<u32> {
            self.0.borrow().get(&h.0).copied()
        }
        fn put(&self, h: &ContentHash, tokens: u32) {
            self.0.borrow_mut().insert(h.0, tokens);
        }
    }

    #[test]
    fn r_arch_056__estimate_is_ceil_of_bytes_over_ratio() {
        let cases = [
            (0, 4, 0),
            (1, 4, 1),
            (4, 4, 1),
            (5, 4, 2),
            (263, 4, 66),
            (10, 3, 4),
            (7, 1, 7),
        ];
        for (bytes, bpt, want) in cases {
            assert_eq!(estimate_bytes(bytes, bpt).tokens, want, "{bytes}/{bpt}");
        }
    }

    #[test]
    fn r_ctx_043__every_estimate_is_labelled_inexact() {
        assert!(!estimate_bytes(100, 4).exact);
    }

    #[test]
    fn r_ctx_043__zero_ratio_is_clamped_to_one_byte_per_token() {
        assert_eq!(estimate_bytes(9, 0).tokens, 9);
    }

    #[test]
    fn r_ctx_043__per_object_estimates_are_cached_and_not_recomputed() {
        let m = Mem(RefCell::new(BTreeMap::new()));
        let first = estimate_text(&m, "rule.no-raw-hex sev=error", 4);
        assert_eq!(first.tokens, 7);
        assert_eq!(m.0.borrow().len(), 1);
        // poison the stored value: a second call must return it without re-counting
        let key = hash_raw("rule.no-raw-hex sev=error".as_bytes());
        m.put(&key, 999);
        assert_eq!(
            estimate_text(&m, "rule.no-raw-hex sev=error", 4).tokens,
            999
        );
        assert_eq!(m.0.borrow().len(), 1);
    }

    #[test]
    fn r_ctx_043__exact_tokens_without_tokenizer_is_a_usage_error() {
        let e = exact_tokens("x", None).unwrap_err();
        assert!(matches!(e.class(), ErrorClass::Usage));
        assert!(e.to_string().contains("tokenizer"), "{e}");
    }

    #[test]
    fn r_ctx_043__exact_tokens_with_an_unbuilt_tokenizer_is_still_a_usage_error() {
        let e = exact_tokens("x", Some("cl100k")).unwrap_err();
        assert!(matches!(e.class(), ErrorClass::Usage));
        assert!(e.to_string().contains("not built into this binary"), "{e}");
    }
}
