//! The tuple every cache entry is valid for (Q-TOOL-1, R-ARCH-077).
use crate::hash::{ContentHash, base32};
use crate::time::AsOf;

/// `as_of` is `Some` only for entries whose result depends on the date (R-ARCH-090).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CacheKey {
    pub fairlead_version: String,
    pub grammar: u32,
    pub config_hash: ContentHash,
    pub as_of: Option<AsOf>,
}

impl CacheKey {
    /// 20 base32 characters of a blake3 over the tuple; the name of the cache directory.
    pub fn id(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"fairlead-cache-v1\0");
        h.update(self.fairlead_version.as_bytes());
        h.update(&[0]);
        h.update(&self.grammar.to_le_bytes());
        h.update(&self.config_hash.0);
        match self.as_of {
            Some(a) => {
                h.update(b"as-of\0");
                h.update(a.to_string().as_bytes());
            }
            None => {
                h.update(b"no-as-of\0");
            }
        }
        base32(h.finalize().as_bytes(), 20)
    }

    /// Readable description stored as `key.json` (diagnostic aid; validated on open).
    pub(crate) fn describe(&self) -> serde_json::Value {
        serde_json::json!({
            "fairlead_version": self.fairlead_version,
            "grammar": self.grammar,
            "config_hash": self.config_hash.to_hex(),
            "as_of": self.as_of.map(|a| a.to_string()),
        })
    }
}
