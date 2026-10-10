//! `fairlead upgrade` (R-EDN-030 to R-EDN-039): one edition at a time, in two steps. The plan
//! is the mechanical edit set (the control file's `edition`, carried last; every generated file
//! re-rendered by the new binary), the fields the new edition requires that nobody may invent
//! (`needs-human`), and a hash of that edit set. Applying needs a `ready` upgrade doc for the
//! same hash, approved with `fairlead approve <draft>`. Pure: the pipeline reads and writes.
use crate::discover::Edition;
use crate::edition::{EditionSpec, spec};
use crate::hash::{ContentHash, Fingerprint, base32};
use crate::path::RepoPath;

/// A value the new edition requires that no migration can derive (R-EDN-034).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NeedsHuman {
    pub path: RepoPath,
    pub field: String,
    pub reason: String,
}

/// R-EDN-039: exactly one edition per upgrade; a longer jump lists the steps to run instead.
pub fn one_step(from: Edition, to: Edition) -> Result<&'static EditionSpec, String> {
    if to.0 == from.0 + 1
        && let Some(s) = spec(to)
    {
        return Ok(s);
    }
    if to.0 <= from.0 {
        return Err(format!(
            "the codebase is on edition {}; this binary writes edition {}: nothing to upgrade",
            from.0, to.0
        ));
    }
    let steps: Vec<String> = (from.0..to.0)
        .map(|e| format!("{e} -> {}", e + 1))
        .collect();
    Err(format!(
        "an upgrade moves exactly one edition; run it stepwise, each step its own upgrade doc: {}",
        steps.join(", ")
    ))
}

/// The control file with `[fairlead] edition` set to `to`; every other byte kept.
pub fn rewrite_edition(text: &str, to: Edition) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| format!("{e}"))?;
    let t = doc
        .get_mut("fairlead")
        .and_then(|i| i.as_table_like_mut())
        .ok_or("the control file has no [fairlead] table")?;
    match t.get_mut("edition").and_then(|i| i.as_value_mut()) {
        Some(v) => {
            let decor = v.decor().clone();
            *v = toml_edit::Value::from(i64::from(to.0));
            *v.decor_mut() = decor;
        }
        None => {
            t.insert("edition", toml_edit::value(i64::from(to.0)));
        }
    }
    Ok(doc.to_string())
}

/// The fields `to` newly requires that the root control file does not set (R-EDN-034).
pub fn needs_human(path: &RepoPath, control: &toml::Table, to: &EditionSpec) -> Vec<NeedsHuman> {
    to.requires
        .iter()
        .filter(|(table, key)| {
            control
                .get(*table)
                .and_then(|t| t.as_table())
                .and_then(|t| t.get(*key))
                .is_none()
        })
        .map(|(table, key)| NeedsHuman {
            path: path.clone(),
            field: format!("{table}.{key}"),
            reason: format!(
                "edition {} requires it and no value can be derived; fill it, then re-plan",
                to.number
            ),
        })
        .collect()
}

/// `b3:` and 20 base32 characters of the write-set hash (R-EDN-037).
pub fn hash_text(h: &ContentHash) -> String {
    format!("b3:{}", base32(&h.0, 20))
}

/// The approval fingerprint of an upgrade doc: its draft ID and the plan hash it approves.
pub fn upgrade_fp(draft: &str, plan_hash: &str) -> Fingerprint {
    Fingerprint::from_hash(&crate::hash::hash_raw(
        format!("upgrade {draft} {plan_hash}").as_bytes(),
    ))
}

/// The draft upgrade doc (R-EDN-030): `kind = "upgrade"`, `impact = "break"`, the only stamp is
/// `created` (R-EDN-033), the edits listed for review.
#[allow(clippy::too_many_arguments)] // reason: one plain text template
pub fn draft_doc(
    draft: &str,
    folder: &str,
    from: Edition,
    to: Edition,
    created: &str,
    hash: &str,
    edits: &[String],
    needs: &[NeedsHuman],
) -> String {
    let mut s = format!(
        "+++\nid = \"{draft}\"\nkind = \"upgrade\"\nstatus = \"draft\"\nimpact = \"break\"\nscope = \"root\"\ntitle = \"Upgrade edition {f} to {t}\"\ncovers = [\"{folder}/**\"]\ncreated = \"{created}\"\nplan-hash = \"{hash}\"\nfrom = {f}\nto = {t}\n+++\n\n# Upgrade edition {f} to {t}\n\nPlan {hash}: {n} mechanical edits.\n\n",
        f = from.0,
        t = to.0,
        n = edits.len()
    );
    for e in edits {
        s.push_str(&format!("- {e}\n"));
    }
    if !needs.is_empty() {
        s.push_str("\nNeeds a human (the upgrade refuses until filled):\n\n");
        for n in needs {
            s.push_str(&format!(
                "- {} in {}: {}\n",
                n.field,
                n.path.as_str(),
                n.reason
            ));
        }
    }
    s.push_str(&format!(
        "\nNew rules: none in edition {}.\n\nTo apply: set `status = \"ready\"`, a human runs `fairlead approve {draft}`, then `fairlead upgrade --yes` again. Rollback is git: the pre-apply tree is HEAD.\n",
        to.0
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_edn_039__two_edition_jump_refused_with_steps() {
        let e = one_step(Edition(1), Edition(3)).unwrap_err();
        assert!(e.contains("1 -> 2, 2 -> 3"), "{e}");
        assert!(
            one_step(Edition(1), Edition(1))
                .unwrap_err()
                .contains("nothing to upgrade")
        );
    }

    #[test]
    fn r_edn_021__rewriting_the_edition_keeps_every_other_byte() {
        let text = "# keep me\n[fairlead]\nedition = 1 # pinned\nroot = true\nx-team = \"core\"\n\n[unknown.table]\nk = 1\n";
        let out = rewrite_edition(text, Edition(2)).unwrap();
        assert_eq!(
            out,
            text.replace("edition = 1 # pinned", "edition = 2 # pinned")
        );
    }

    proptest::proptest! {
        #[test]
        fn r_edn_021__rewrites_preserve_unknown_keys(
            keys in proptest::collection::btree_map("x-[a-z]{1,8}|zz[a-z]{1,6}", "[a-zA-Z0-9 ]{0,12}", 0..6)
        ) {
            let mut text = String::from("[fairlead]\nedition = 1\nroot = true\n");
            for (k, v) in &keys {
                text.push_str(&format!("{k} = {v:?}\n"));
            }
            text.push_str("[future]\nkey = 1\n");
            let out = rewrite_edition(&text, Edition(2)).unwrap();
            proptest::prop_assert_eq!(out, text.replace("edition = 1", "edition = 2"));
            let doc = format!("+++\n{}+++\nbody\n", text);
            let set = crate::graveyard::set_keys(&doc, |d| {
                d["state"] = toml_edit::value("revisited");
            })
            .unwrap();
            for (k, v) in &keys {
                let line = format!("{k} = {v:?}");
                proptest::prop_assert!(set.contains(&line), "{} lost", k);
            }
            proptest::prop_assert!(set.ends_with("+++\nbody\n"));
        }
    }

    #[test]
    fn r_edn_034__a_required_field_without_value_needs_a_human() {
        let p = RepoPath::new("fairlead/control.toml").unwrap();
        let spec = EditionSpec {
            number: 2,
            grammar: "t",
            header_key: "ed",
            requires: &[("settings", "purpose")],
        };
        let without: toml::Table = "[settings]\nfolder = \"fairlead\"\n".parse().unwrap();
        let with: toml::Table = "[settings]\npurpose = \"x\"\n".parse().unwrap();
        assert_eq!(
            needs_human(&p, &without, &spec)[0].field,
            "settings.purpose"
        );
        assert!(needs_human(&p, &with, &spec).is_empty());
    }
}
