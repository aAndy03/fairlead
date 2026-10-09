//! blake3 everywhere (D-50). Base32 is the lowercase Crockford alphabet, hand-implemented (C-7).
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

fn sym(index: u32) -> char {
    usize::try_from(index)
        .ok()
        .and_then(|i| ALPHABET.get(i))
        .copied()
        .map(char::from)
        .unwrap_or('0')
}

fn in_alphabet(c: char) -> bool {
    c.is_ascii() && ALPHABET.contains(&(c as u8))
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum HashParseError {
    Length { want: usize, got: usize },
    BadChar { at: usize },
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashParseError::Length { want, got } => {
                write!(f, "expected {want} characters, got {got}")
            }
            HashParseError::BadChar { at } => write!(f, "invalid character at position {at}"),
        }
    }
}

impl std::error::Error for HashParseError {}

/// 32-byte blake3 digest. Serialises as 64 lowercase hex characters.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            s.push(char::from_digit(u32::from(b & 15), 16).unwrap_or('0'));
        }
        s
    }

    pub fn from_hex(s: &str) -> Result<Self, HashParseError> {
        if s.len() != 64 {
            return Err(HashParseError::Length {
                want: 64,
                got: s.len(),
            });
        }
        let mut out = [0u8; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            let pair = s
                .get(i * 2..i * 2 + 2)
                .ok_or(HashParseError::BadChar { at: i * 2 })?;
            *slot =
                u8::from_str_radix(pair, 16).map_err(|_| HashParseError::BadChar { at: i * 2 })?;
        }
        Ok(ContentHash(out))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ContentHash::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Hash after BOM strip and CRLF/CR to LF, so `core.autocrlf` never changes a hash (R-ARCH-076).
pub fn hash_normalised(bytes: &[u8]) -> ContentHash {
    let lf = fairlead_syntax::normalize::lf_bytes(bytes);
    ContentHash(*blake3::hash(&lf).as_bytes())
}

/// Hash of the bytes as they are. For non-UTF-8 files (R-PORT-016).
pub fn hash_raw(bytes: &[u8]) -> ContentHash {
    ContentHash(*blake3::hash(bytes).as_bytes())
}

/// Normalised hash for UTF-8 text, raw hash otherwise. The one entry point for file content.
pub fn hash_content(bytes: &[u8]) -> ContentHash {
    if std::str::from_utf8(bytes).is_ok() {
        hash_normalised(bytes)
    } else {
        hash_raw(bytes)
    }
}

/// Order-sensitive combination of hashes (config hashes, cache keys).
pub fn combine_hashes(parts: &[ContentHash]) -> ContentHash {
    let mut h = blake3::Hasher::new();
    for p in parts {
        h.update(&p.0);
    }
    ContentHash(*h.finalize().as_bytes())
}

/// First `chars` symbols of the lowercase Crockford base32 of `bytes`, most significant bit first.
/// Missing input bits are zero-padded, so the result always has exactly `chars` symbols.
pub fn base32(bytes: &[u8], chars: usize) -> String {
    let mut out = String::with_capacity(chars);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    let mut iter = bytes.iter();
    for _ in 0..chars {
        while nbits < 5 {
            match iter.next() {
                Some(b) => {
                    acc = (acc << 8) | u32::from(*b);
                    nbits += 8;
                }
                None => {
                    acc <<= 5;
                    nbits += 5;
                }
            }
        }
        nbits -= 5;
        out.push(sym((acc >> nbits) & 31));
        acc &= (1u32 << nbits) - 1;
    }
    out
}

macro_rules! short_id {
    ($(#[$m:meta])* $name:ident, $len:expr) => {
        $(#[$m])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub const LEN: usize = $len;
            pub fn from_hash(h: &ContentHash) -> Self {
                Self(base32(&h.0, Self::LEN))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

short_id!(
    /// 12 characters = first 60 bits (R-CHK-031); also the approval key (R-CLI-037).
    Fingerprint, 12
);
short_id!(
    /// 8 characters = first 40 bits (context digest, R-CTX-020).
    Digest, 8
);
short_id!(
    /// 20 characters = first 100 bits; generated-file header body hash (C-17).
    BodyHash, 20
);

impl std::str::FromStr for Fingerprint {
    type Err = HashParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Fingerprint::parse(s)
    }
}

impl Fingerprint {
    /// First 12 base32 symbols of a raw 32-byte digest (amendment A-01).
    pub fn from_digest(d: &[u8; 32]) -> Self {
        Self::from_hash(&ContentHash(*d))
    }

    /// Parse the 12-character text form (approvals, baseline lines).
    pub fn parse(s: &str) -> Result<Self, HashParseError> {
        let n = s.chars().count();
        if n != Self::LEN {
            return Err(HashParseError::Length {
                want: Self::LEN,
                got: n,
            });
        }
        if let Some((at, _)) = s.chars().enumerate().find(|(_, c)| !in_alphabet(*c)) {
            return Err(HashParseError::BadChar { at });
        }
        Ok(Self(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_chk_031__base32_known_vectors() {
        // 10101 01011 => 'n' (21) 'b' (11); lowercase Crockford alphabet (C-7).
        assert_eq!(base32(&[0b1010_1010, 0b1100_0000], 2), "nb");
        assert_eq!(base32(&[0xff, 0xff], 3), "zzz");
        assert_eq!(base32(&[0x00], 1), "0");
        assert_eq!(base32(&[], 4), "0000", "missing bits pad with zeros");
        assert_eq!(base32(&[0xff; 32], 12), "zzzzzzzzzzzz");
    }

    #[test]
    fn r_chk_031__fingerprint_digest_bodyhash_lengths() {
        let h = hash_raw(b"abc");
        assert_eq!(Fingerprint::from_hash(&h).as_str().len(), 12);
        assert_eq!(Digest::from_hash(&h).as_str().len(), 8);
        assert_eq!(BodyHash::from_hash(&h).as_str().len(), 20);
        // The short forms share one bit stream, so each is a prefix of the longer ones.
        assert!(
            BodyHash::from_hash(&h)
                .as_str()
                .starts_with(Fingerprint::from_hash(&h).as_str())
        );
        assert!(
            Fingerprint::from_hash(&h)
                .as_str()
                .starts_with(Digest::from_hash(&h).as_str())
        );
    }

    #[test]
    fn r_chk_031__fingerprint_parse_validates_alphabet_and_length() {
        assert!(Fingerprint::parse("0123456789ab").is_ok());
        assert_eq!(
            Fingerprint::parse("0123456789a"),
            Err(HashParseError::Length { want: 12, got: 11 })
        );
        // 'i' is not in the Crockford alphabet; uppercase is outside the lowercase alphabet.
        assert_eq!(
            Fingerprint::parse("0123456789ai"),
            Err(HashParseError::BadChar { at: 11 })
        );
        assert_eq!(
            Fingerprint::parse("0123456789AB"),
            Err(HashParseError::BadChar { at: 10 })
        );
    }

    #[test]
    fn r_arch_076__crlf_lf_and_bom_hash_equal() {
        let lf = hash_normalised(b"a\nb\n");
        assert_eq!(lf, hash_normalised(b"a\r\nb\r\n"));
        assert_eq!(lf, hash_normalised(b"a\rb\r"));
        assert_eq!(lf, hash_normalised(b"\xEF\xBB\xBFa\nb\n"));
        assert_ne!(lf, hash_raw(b"a\r\nb\r\n"));
    }

    #[test]
    fn r_file_024__known_blake3_vector_and_hex_roundtrip() {
        let h = hash_raw(b""); // blake3 of the empty input
        assert_eq!(
            h.to_hex(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(ContentHash::from_hex(&h.to_hex()), Ok(h));
        assert_eq!(
            ContentHash::from_hex("zz"),
            Err(HashParseError::Length { want: 64, got: 2 })
        );
    }

    #[test]
    fn r_port_016__non_utf8_is_hashed_over_raw_bytes() {
        let latin = b"caf\xE9\r\n"; // not UTF-8
        assert_eq!(hash_content(latin), hash_raw(latin));
        let utf8 = b"cafe\r\n";
        assert_eq!(hash_content(utf8), hash_normalised(utf8));
    }

    #[test]
    fn r_chk_031__content_hash_serde_is_hex_string() {
        let h = hash_raw(b"x");
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, format!("\"{}\"", h.to_hex()));
        assert_eq!(serde_json::from_str::<ContentHash>(&json).unwrap(), h);
    }

    #[test]
    fn r_arch_060__combine_hashes_is_order_sensitive_and_deterministic() {
        let a = hash_raw(b"a");
        let b = hash_raw(b"b");
        assert_eq!(combine_hashes(&[a, b]), combine_hashes(&[a, b]));
        assert_ne!(combine_hashes(&[a, b]), combine_hashes(&[b, a]));
    }
}
