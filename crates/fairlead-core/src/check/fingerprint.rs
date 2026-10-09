//! Line-independent finding identity (R-CHK-031, R-ARCH-053).
use crate::{check::finding::RuleId, hash::Fingerprint, path::RepoPath, registry::ScopeId};
use fairlead_syntax::diag::DiagCode;

pub struct FingerprintInput<'a> {
    pub code: DiagCode,
    pub rule: Option<&'a RuleId>,
    pub scope: &'a ScopeId,
    pub path: &'a RepoPath,
    pub text: &'a str,
    pub occurrence: u32,
}

pub fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// blake3 over length-framed (code, rule, scope, path, collapsed text, occurrence), first 60 bits, 12 base32 chars.
pub fn fingerprint(i: &FingerprintInput<'_>) -> Fingerprint {
    let rule = i.rule.map(|r| r.0.as_str().to_string()).unwrap_or_default(); // Verify: Name::as_str
    let text = collapse_whitespace(i.text);
    let mut h = blake3::Hasher::new();
    {
        let mut put = |tag: &[u8], v: &[u8]| {
            h.update(tag);
            h.update(&(v.len() as u64).to_le_bytes());
            h.update(v);
        };
        put(b"code", i.code.0.as_bytes());
        put(b"rule", rule.as_bytes());
        put(b"scope", i.scope.0.as_str().as_bytes());
        put(b"path", i.path.as_str().as_bytes());
        put(b"text", text.as_bytes());
        put(b"occ", &i.occurrence.to_le_bytes());
    }
    Fingerprint::from_digest(h.finalize().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{check::finding::rule_id, diag::codes as c};
    use fairlead_syntax::ids::ScopeSlug;

    fn fp(
        code: DiagCode,
        rule: Option<&RuleId>,
        scope: &str,
        path: &str,
        text: &str,
        occ: u32,
    ) -> String {
        let scope = ScopeId(ScopeSlug::parse(scope).unwrap());
        let path = RepoPath::new(path).unwrap();
        fingerprint(&FingerprintInput {
            code,
            rule,
            scope: &scope,
            path: &path,
            text,
            occurrence: occ,
        })
        .as_str()
        .to_string()
    }

    #[test]
    fn r_chk_031__twelve_base32_chars_and_stable() {
        let a = fp(c::HN_T001, None, "web", "a.tsx", "#2563eb", 0);
        assert_eq!(a.len(), 12);
        assert!(
            a.chars()
                .all(|ch| "0123456789abcdefghjkmnpqrstvwxyz".contains(ch))
        );
        assert_eq!(a, fp(c::HN_T001, None, "web", "a.tsx", "#2563eb", 0));
    }

    #[test]
    fn r_chk_031__whitespace_is_collapsed_in_the_offending_text() {
        assert_eq!(collapse_whitespace("a  b\n\t c "), "a b c");
        assert_eq!(
            fp(c::HN_T001, None, "web", "a", "a  b", 0),
            fp(c::HN_T001, None, "web", "a", "a b", 0)
        );
    }

    #[test]
    fn r_chk_031__every_component_changes_the_fingerprint() {
        let r = rule_id("no-raw-hex").unwrap();
        let base = fp(c::HN_U001, Some(&r), "web", "a", "t", 0);
        for other in [
            fp(c::HN_T001, Some(&r), "web", "a", "t", 0),
            fp(c::HN_U001, None, "web", "a", "t", 0),
            fp(c::HN_U001, Some(&r), "ui", "a", "t", 0),
            fp(c::HN_U001, Some(&r), "web", "b", "t", 0),
            fp(c::HN_U001, Some(&r), "web", "a", "u", 0),
            fp(c::HN_U001, Some(&r), "web", "a", "t", 1),
        ] {
            assert_ne!(base, other);
        }
    }

    #[test]
    fn r_arch_053__field_boundaries_cannot_be_confused() {
        assert_ne!(
            fp(c::HN_T001, None, "web", "ab", "c", 0),
            fp(c::HN_T001, None, "web", "a", "bc", 0)
        );
    }
}
