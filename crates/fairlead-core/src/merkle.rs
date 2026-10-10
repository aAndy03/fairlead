//! Merkle digests (R-VER-016, D-23) and `gen/scope.lock` (R-VER-015). Pure.
//!
//! A scope digest hashes the sorted `path hash` entries of the scope's own files (content hashes
//! from the corpus, which the cache fills, so unchanged files are never re-read) followed by the
//! digests of its child scopes; a change rehashes only the path from the file to the root.
//! Generated files are left out so `scope.lock` never hashes itself.

use std::collections::BTreeMap;

use crate::classify::FileClass;
use crate::extract::Corpus;
use crate::generate::shard::policy_word;
use crate::hash::{base32, hash_raw};
use crate::registry::{FieldValue, Registry, ScopeId};
use crate::version::Version;

fn digest20(text: &str) -> String {
    base32(&hash_raw(text.as_bytes()).0, 20)
}

/// Every scope's Merkle digest, keyed by scope id.
pub fn scope_digests(reg: &Registry, corpus: &Corpus) -> BTreeMap<ScopeId, String> {
    let mut own: BTreeMap<ScopeId, String> = BTreeMap::new();
    for (path, rec) in &corpus.records {
        if rec.class == FileClass::Generated {
            continue;
        }
        if let Some(s) = reg.scope_of(path) {
            let e = own.entry(s.clone()).or_default();
            e.push_str(path.as_str());
            e.push(' ');
            e.push_str(&rec.hash.to_hex());
            e.push('\n');
        }
    }
    // Deepest first, so every child digest exists before its parent is hashed.
    let mut order: Vec<&ScopeId> = reg.scopes().map(|s| &s.id).collect();
    order.sort_by_key(|id| std::cmp::Reverse(reg.chain(id).len()));
    let mut out: BTreeMap<ScopeId, String> = BTreeMap::new();
    for id in order {
        let mut text = own.get(id).cloned().unwrap_or_default();
        let mut children: Vec<&ScopeId> = reg
            .scopes()
            .filter(|s| s.parent.as_ref() == Some(id))
            .map(|s| &s.id)
            .collect();
        children.sort();
        for c in children {
            text.push_str("child ");
            text.push_str(c.0.as_str());
            text.push(' ');
            text.push_str(out.get(c).map_or("", String::as_str));
            text.push('\n');
        }
        out.insert(id.clone(), digest20(&text));
    }
    out
}

fn field_text(v: &FieldValue) -> String {
    match v {
        FieldValue::Scalar(s) => s.clone(),
        FieldValue::List(l) => l.join(","),
    }
}

/// R-VER-015: the digest of only the effective view a scope inherits from its ancestors (items
/// whose home is another scope), rendered as sorted `id policy k=v...` lines.
pub fn ancestor_digest(reg: &Registry, scope: &ScopeId) -> String {
    let mut text = String::new();
    for (id, item) in &reg.view(scope).items {
        if &id.scope == scope {
            continue;
        }
        text.push_str(&format!(
            "{} policy={}",
            id.to_id(),
            policy_word(&item.policy)
        ));
        for (k, f) in &item.fields {
            text.push_str(&format!(" {k}={}", field_text(&f.value)));
        }
        text.push('\n');
    }
    digest20(&text)
}

/// The body of `gen/scope.lock`: `reconciled=<version>` and `ancestor-digest=<hash>`.
pub fn scope_lock_body(reconciled: Version, ancestor: &str) -> String {
    format!("ancestor-digest={ancestor}\nreconciled={reconciled}\n")
}

/// `(reconciled, ancestor-digest)` from a `scope.lock` on disk (header line skipped).
pub fn parse_scope_lock(text: &str) -> Option<(Version, String)> {
    let mut reconciled = None;
    let mut ancestor = None;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        if let Some(v) = line.strip_prefix("reconciled=") {
            reconciled = Version::parse(v.trim());
        } else if let Some(a) = line.strip_prefix("ancestor-digest=") {
            ancestor = Some(a.trim().to_owned());
        }
    }
    Some((reconciled?, ancestor?))
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::check::testkit::World;

    const CHILD: &str = "[fairlead]\nscope = \"web\"\n";

    fn digests(files: &[(&str, &str)]) -> BTreeMap<ScopeId, String> {
        let mut w = World::new().scope("apps/web", CHILD);
        for (p, t) in files {
            w = w.file(p, t);
        }
        let b = w.build();
        scope_digests(&b.registry, &b.corpus)
    }

    fn id(s: &str) -> ScopeId {
        ScopeId(fairlead_syntax::ids::ScopeSlug::parse(s).unwrap())
    }

    #[test]
    fn r_ver_016__changing_one_file_rehashes_only_its_path_to_root() {
        let a = digests(&[("apps/web/a.ts", "x\n"), ("lib/b.ts", "y\n")]);
        let child_edit = digests(&[("apps/web/a.ts", "x2\n"), ("lib/b.ts", "y\n")]);
        assert_ne!(a[&id("web")], child_edit[&id("web")]);
        assert_ne!(
            a[&id("root")],
            child_edit[&id("root")],
            "the root covers its child"
        );
        let root_edit = digests(&[("apps/web/a.ts", "x\n"), ("lib/b.ts", "y2\n")]);
        assert_eq!(
            a[&id("web")],
            root_edit[&id("web")],
            "a sibling path is untouched"
        );
        assert_ne!(a[&id("root")], root_edit[&id("root")]);
        assert_eq!(a, digests(&[("lib/b.ts", "y\n"), ("apps/web/a.ts", "x\n")]));
    }

    #[test]
    fn r_ver_015__scope_lock_round_trips() {
        let body = scope_lock_body(Version::new(1, 2, 0), "abc");
        assert_eq!(body, "ancestor-digest=abc\nreconciled=1.2.0\n");
        let with_header = format!("# scope web edition=1 body=x\n{body}");
        assert_eq!(
            parse_scope_lock(&with_header),
            Some((Version::new(1, 2, 0), "abc".to_owned()))
        );
        assert_eq!(parse_scope_lock("reconciled=1.2\n"), None);
    }

    #[test]
    fn r_ver_015__ancestor_digest_covers_only_the_inherited_view() {
        let rule = "\n[rules.no-x]\ncheck = \"pattern\"\nforbid = \"x\"\nmsg = \"m\"\nglob = [\"**\"]\nsev = \"error\"\nstage = \"enforce\"\nwhy = \"w\"\n";
        let plain = World::new().scope("apps/web", CHILD).build();
        let ruled = World::new().root(rule).scope("apps/web", CHILD).build();
        let web = id("web");
        assert_ne!(
            ancestor_digest(&plain.registry, &web),
            ancestor_digest(&ruled.registry, &web),
            "a root rule is part of the child's inherited view"
        );
        let own_tok = World::new()
            .scope(
                "apps/web",
                "[fairlead]\nscope = \"web\"\n[kinds.tok]\ntitle = \"t\"\nshape = \"doc\"\n",
            )
            .file(
                "apps/web/fairlead/contract/tokens/a.md",
                "+++\ntitle = \"a\"\ndirectives = ['def tok.a']\n+++\n",
            )
            .build();
        assert_eq!(
            ancestor_digest(&plain.registry, &web),
            ancestor_digest(&own_tok.registry, &web),
            "the scope's own items do not change its ancestor digest"
        );
    }
}
