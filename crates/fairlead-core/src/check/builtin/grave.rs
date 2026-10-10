//! Graveyard matches (R-LC-033, R-LC-036): a pending change doc that matches an entry without
//! `revisits=`, and a new or changed file that matches an entry's glob or fingerprint or a
//! tombstone's `paths`, are `H-GRAVE-MATCH` (HN-L002), under a new ID too.
use fairlead_syntax::diag::DiagCode;

use crate::check::builtin::root_control;
use crate::check::{CheckCx, finding::DiagB, rules::Check};
use crate::diag::{Diagnostic, codes::*};
use crate::docs::{ChangeDoc, collect_change_docs, covers_path};
use crate::facts::Coverage;
use crate::graveyard::{Entry, GrvState, collect, fp_match, path_hits, text_hits};
use crate::lifecycle::doc_ids;
use crate::path::RepoPath;
use crate::registry::{Registry, ScopeId};

pub struct GraveCheck;
const CODES: [DiagCode; 1] = [HN_L002];

/// Entries that apply at `scope`: declared in it or an ancestor, not yet revisited (R-LC-030).
pub(crate) fn effective<'e>(
    reg: &Registry,
    entries: &'e [Entry],
    scope: &ScopeId,
) -> Vec<&'e Entry> {
    let chain = reg.chain(scope);
    entries
        .iter()
        .filter(|e| e.state != GrvState::Revisited)
        .filter(|e| reg.scope_of(&e.path).is_some_and(|s| chain.contains(&s)))
        .collect()
}

pub(crate) fn revisits(d: &ChangeDoc) -> Vec<String> {
    d.extra
        .get("revisits")
        .map(|v| doc_ids(v))
        .unwrap_or_default()
}

fn scope_dir(reg: &Registry, p: &RepoPath) -> String {
    reg.scope_of(p)
        .and_then(|s| reg.scope(s))
        .map(|s| s.dir.as_str().to_owned())
        .unwrap_or_default()
}

fn rel(dir: &str, p: &RepoPath) -> Option<String> {
    if dir.is_empty() || dir == "." {
        Some(p.as_str().to_owned())
    } else {
        p.as_str()
            .strip_prefix(&format!("{dir}/"))
            .map(str::to_owned)
    }
}

fn tomb_path_hit(pattern: &str, p: &str) -> bool {
    p == pattern
        || p.starts_with(&format!("{}/", pattern.trim_end_matches('/')))
        || globset::Glob::new(pattern).is_ok_and(|g| g.compile_matcher().is_match(p))
}

impl Check for GraveCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        let (entries, _) = collect(cx.corpus);
        let here = effective(reg, &entries, scope);
        let chain = reg.chain(scope);
        let tomb_paths: Vec<(&str, &str)> = reg
            .retired()
            .iter()
            .filter(|t| t.is_item() && t.scope.as_ref().is_some_and(|s| chain.contains(&s)))
            .flat_map(|t| t.paths.iter().map(move |p| (t.id.as_str(), p.as_str())))
            .collect();
        if here.is_empty() && tomb_paths.is_empty() {
            return;
        }
        let (ids_pct, sh_pct) = root_control(reg).map_or((60, 50), |r| {
            (r.graveyard.ids_percent, r.graveyard.shingles_percent)
        });
        let (docs, _) = collect_change_docs(cx.corpus);
        for d in docs
            .iter()
            .filter(|d| d.status.is_pending() && reg.scope_of(&d.path) == Some(scope))
        {
            let text = [
                d.title.as_deref().unwrap_or(""),
                d.extra.get("what").map_or("", String::as_str),
            ]
            .join(" ");
            for e in &here {
                let hits = text_hits(e, &text);
                if hits.is_empty() || revisits(d).contains(&e.id) {
                    continue;
                }
                let terms: Vec<&str> = hits.iter().map(|t| t.text()).collect();
                out.push(
                    DiagB::new(
                        HN_L002,
                        &d.path,
                        1,
                        1,
                        format!(
                            "{} matches the graveyard entry {} ({})",
                            d.id,
                            e.id,
                            terms.join(", ")
                        ),
                    )
                    .hint(format!(
                        "add revisits = \"{}\" with a reason, or drop the proposal; {} was {}: {}",
                        e.id,
                        e.id,
                        e.state.as_str(),
                        e.why
                    ))
                    .related(e.id.clone())
                    .text(e.id.clone())
                    .build(),
                );
            }
        }
        let changed: Vec<&RepoPath> = match (&cx.facts.git.coverage, &cx.facts.git.changed) {
            (Some(Coverage::Changed { files, .. }), _) => files.iter().collect(),
            (_, Some(c)) => c.0.iter().collect(),
            _ => Vec::new(),
        };
        let folder = root_control(reg)
            .and_then(|c| c.settings.folder.clone())
            .unwrap_or_else(|| "fairlead".into());
        for f in changed {
            if reg.scope_of(f) != Some(scope) || f.as_str().split('/').any(|s| s == folder) {
                continue;
            }
            let Some(rec) = cx.corpus.records.get(f) else {
                continue;
            };
            for e in &here {
                let dir = scope_dir(reg, &e.path);
                let mut why: Vec<String> = rel(&dir, f)
                    .map(|r| {
                        path_hits(e, &r)
                            .iter()
                            .map(|t| format!("glob {}", t.text()))
                            .collect()
                    })
                    .unwrap_or_default();
                if let (Some(efp), Some(ffp)) = (&e.fingerprint, &rec.grave)
                    && let Some(m) = fp_match(efp, ffp, ids_pct, sh_pct)
                {
                    why.push(m);
                }
                if why.is_empty() {
                    continue;
                }
                let silenced = docs.iter().any(|d| {
                    revisits(d).contains(&e.id) && covers_path(d, &scope_dir(reg, &d.path), f)
                });
                if silenced {
                    continue;
                }
                out.push(
                    DiagB::new(HN_L002, f, 1, 1, format!("similar to {} ({})", e.id, why.join("; ")))
                        .hint(format!("add revisits = \"{}\" with a reason to the covering change doc, or remove the file", e.id))
                        .related(e.id.clone())
                        .text(e.id.clone())
                        .build(),
                );
            }
            for (id, p) in &tomb_paths {
                if tomb_path_hit(p, f.as_str()) {
                    out.push(
                        DiagB::new(HN_L002, f, 1, 1, format!("re-introduces {p}, removed with {id}"))
                            .hint("the path was removed on purpose; a revive doc brings it back under a new ID")
                            .related((*id).to_string())
                            .text((*id).to_string())
                            .build(),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;
    use crate::facts::ChangedSet;

    const ENTRY: &str = "+++\nid = \"grv.dark-toggle-v1\"\nstate = \"rejected\"\nat = \"1.4.0\"\nwhat = \"Per-component dark-mode toggles\"\nwhy = \"Token drift\"\nmatch = [\"dark mode\", \"ui/**/*Toggle*\"]\n+++\n# Dark toggle\n";

    fn draft(title: &str, extra: &str) -> String {
        format!(
            "+++\nid = \"d-00000001\"\nkind = \"change\"\nstatus = \"draft\"\ntitle = \"{title}\"\ncovers = [\"ui/**\"]\n{extra}+++\n"
        )
    }

    fn changed(files: &[&str]) -> impl FnOnce(&mut crate::facts::Facts) {
        let set: std::collections::BTreeSet<RepoPath> =
            files.iter().map(|f| RepoPath::new(f).unwrap()).collect();
        move |f| f.git.changed = Some(ChangedSet(set))
    }

    #[test]
    fn r_lc_033__pending_doc_matching_an_entry_needs_revisits() {
        let w = |extra: &str| {
            World::new()
                .file("fairlead/docs/graveyard/dark.md", ENTRY)
                .file(
                    "fairlead/docs/changes/d-00000001-x.md",
                    &draft("Add a dark mode switch", extra),
                )
                .build()
                .run_check(&GraveCheck)
        };
        let hit = w("");
        assert_eq!(of(&hit, c::HN_L002).len(), 1);
        assert!(
            of(&hit, c::HN_L002)[0]
                .message
                .contains("grv.dark-toggle-v1 (dark mode)")
        );
        assert!(of(&w("revisits = \"grv.dark-toggle-v1\"\n"), c::HN_L002).is_empty());
    }

    #[test]
    fn r_lc_033__changed_file_matching_a_glob_is_grave_match() {
        let ds = World::new()
            .file("fairlead/docs/graveyard/dark.md", ENTRY)
            .file("ui/header/NightToggle.tsx", "export const A = 1;\n")
            .facts(changed(&["ui/header/NightToggle.tsx"]))
            .build()
            .run_check(&GraveCheck);
        let l = of(&ds, c::HN_L002);
        assert_eq!(l.len(), 1);
        assert!(
            l[0].message.contains("glob ui/**/*Toggle*"),
            "{}",
            l[0].message
        );
    }

    #[test]
    fn r_lc_036__revisits_on_the_covering_doc_silences_the_file() {
        let ds = World::new()
            .file("fairlead/docs/graveyard/dark.md", ENTRY)
            .file(
                "fairlead/docs/changes/d-00000001-x.md",
                &draft("Theme work", "revisits = \"grv.dark-toggle-v1\"\n"),
            )
            .file("ui/header/NightToggle.tsx", "export const A = 1;\n")
            .facts(changed(&["ui/header/NightToggle.tsx"]))
            .build()
            .run_check(&GraveCheck);
        assert!(of(&ds, c::HN_L002).is_empty(), "{ds:?}");
    }

    #[test]
    fn r_lc_033__fingerprint_catches_a_rename_under_new_paths() {
        let material = "export function ThemeToggle() {\n  const { dark, setDark } = useTheme();\n  document.documentElement.dataset.theme = dark ? 'dark' : 'light';\n  localStorage.setItem('theme', dark ? 'dark' : 'light');\n  return setDark(!dark);\n}\n";
        let fp = crate::graveyard::fingerprint(&[("ui/ThemeToggle.tsx".into(), material.into())]);
        let entry = format!(
            "+++\nid = \"grv.t\"\nstate = \"rejected\"\nmatch = [\"theme toggle\"]\nfingerprint = {}\n+++\n",
            fp.toml_inline()
        );
        let renamed = material.replace("ThemeToggle", "NightSwitch");
        let ds = World::new()
            .root("[graveyard]\n")
            .file("fairlead/docs/graveyard/t.md", &entry)
            .file("src/night/NightSwitch.tsx", &renamed)
            .facts(changed(&["src/night/NightSwitch.tsx"]))
            .build()
            .run_check(&GraveCheck);
        let l = of(&ds, c::HN_L002);
        assert_eq!(l.len(), 1, "{ds:?}");
        assert!(
            l[0].message.contains("similar to grv.t (shingles"),
            "{}",
            l[0].message
        );
    }

    #[test]
    fn r_lc_033__tombstone_paths_catch_reintroduction() {
        let ds = World::new()
            .file("src/legacy/diff.rs", "fn d() {}\n")
            .facts(|f| {
                f.tombstones.insert(
                    RepoPath::new("fairlead/tombstones/2026.toml").unwrap(),
                    "[[tomb]]\nid = \"engine.legacy-diff\"\nremoved = \"0.5.0\"\nwhy = \"quadratic\"\npaths = [\"src/legacy\"]\n".into(),
                );
            })
            .facts(changed(&["src/legacy/diff.rs"]))
            .build()
            .run_check(&GraveCheck);
        assert!(
            of(&ds, c::HN_L002)[0]
                .message
                .contains("removed with engine.legacy-diff")
        );
    }
}
