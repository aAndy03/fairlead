//! Merkle digest chain (14 "Digest chain", C-7). A leaf change never changes an ancestor digest;
//! intent flows down.
use crate::discover::Edition;
use crate::hash::{ContentHash, Digest, base32};
use crate::registry::{Registry, ScopeId};

/// Bumped only when the context byte format changes (DD-9).
pub const CONTEXT_GENERATOR: &str = "ctx1";

pub fn generator_version(edition: Edition) -> String {
    format!("{CONTEXT_GENERATOR}/ed{}", edition.0)
}

/// `H(parent effective digest, own authored digest, generator version)`: blake3, first 40 bits.
pub fn effective_digest(
    parent: Option<&Digest>,
    own: &ContentHash,
    generator_version: &str,
) -> Digest {
    let mut h = blake3::Hasher::new();
    h.update(b"fairlead.context.digest\0");
    match parent {
        Some(p) => {
            h.update(&[1]);
            h.update(p.as_str().as_bytes());
        }
        None => {
            h.update(&[0]);
        }
    }
    h.update(&own.0);
    h.update(&[0xff]);
    h.update(generator_version.as_bytes());
    let out = h.finalize();
    Digest::from_hash(&ContentHash(*out.as_bytes()))
}

/// Digests of the scope chain, root first, ending with `scope` itself.
pub fn chain_digests(reg: &Registry, scope: &ScopeId, generator: &str) -> Vec<(ScopeId, Digest)> {
    let mut out: Vec<(ScopeId, Digest)> = Vec::new();
    for id in reg.chain(scope) {
        let d = effective_digest(
            out.last().map(|(_, d)| d),
            &reg.view(id).authored_digest,
            generator,
        );
        out.push((id.clone(), d));
    }
    out
}

/// The footer `=d`: 8 base32 chars over the body lines of the brief.
pub fn brief_digest(body: &str) -> String {
    base32(blake3::hash(body.as_bytes()).as_bytes(), 8)
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    const ALPHABET: &str = "0123456789abcdefghjkmnpqrstvwxyz";

    fn h(n: u8) -> ContentHash {
        ContentHash([n; 32])
    }

    /// Digests of a chain whose authored hashes are `owns`, root first.
    fn chain(owns: &[u8], g: &str) -> Vec<Digest> {
        let mut out: Vec<Digest> = Vec::new();
        for n in owns {
            let d = effective_digest(out.last(), &h(*n), g);
            out.push(d);
        }
        out
    }

    #[test]
    fn r_ctx_050__digest_is_eight_lowercase_crockford_chars() {
        let d = effective_digest(None, &h(1), "ctx1/ed1");
        assert_eq!(d.as_str().len(), 8);
        assert!(
            d.as_str().chars().all(|c| ALPHABET.contains(c)),
            "{}",
            d.as_str()
        );
    }

    #[test]
    fn r_ctx_050__same_inputs_give_the_same_digest() {
        assert_eq!(chain(&[1, 2, 3], "ctx1/ed1"), chain(&[1, 2, 3], "ctx1/ed1"));
    }

    #[test]
    fn r_ctx_050__parent_digest_feeds_the_child() {
        let a = effective_digest(None, &h(1), "g");
        let b = effective_digest(None, &h(2), "g");
        assert_ne!(
            effective_digest(Some(&a), &h(9), "g"),
            effective_digest(Some(&b), &h(9), "g")
        );
        assert_ne!(
            effective_digest(Some(&a), &h(9), "g"),
            effective_digest(None, &h(9), "g")
        );
    }

    #[test]
    fn r_ctx_050__a_leaf_change_never_changes_an_ancestor_digest() {
        let before = chain(&[1, 2, 3], "g");
        let after = chain(&[1, 2, 4], "g");
        assert_eq!(before.get(..2), after.get(..2));
        assert_ne!(before.get(2), after.get(2));
    }

    #[test]
    fn r_ctx_050__an_ancestor_change_changes_every_descendant() {
        let before = chain(&[1, 2, 3], "g");
        let after = chain(&[9, 2, 3], "g");
        for (a, b) in before.iter().zip(after.iter()) {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn r_ctx_050__generator_or_edition_change_changes_every_digest() {
        let g1 = generator_version(Edition(1));
        let g2 = generator_version(Edition(2));
        assert_eq!(g1, "ctx1/ed1");
        for (a, b) in chain(&[1, 2, 3], &g1)
            .iter()
            .zip(chain(&[1, 2, 3], &g2).iter())
        {
            assert_ne!(a, b);
        }
    }

    #[test]
    fn r_ctx_050__brief_digest_is_eight_chars_and_content_sensitive() {
        let a = brief_digest("rule.a\n");
        assert_eq!(a.len(), 8);
        assert!(a.chars().all(|c| ALPHABET.contains(c)));
        assert_ne!(a, brief_digest("rule.b\n"));
    }
}
