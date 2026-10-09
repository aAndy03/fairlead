//! Scope-tree checks against the last written membership shards (C-35, A-36).
use crate::{
    check::{
        CheckCx,
        builtin::{control_path, root_control},
        finding::DiagB,
        rules::{Check, RuleSet},
    },
    classify::FileClass,
    diag::{Diagnostic, codes::*},
    generate::members::parse_members,
    path::RepoPath,
    registry::{Registry, ScopeId, ScopeState},
};
use fairlead_syntax::diag::{DiagCode, Severity};
use std::collections::{BTreeMap, BTreeSet};

pub struct ScopesCheck;
const CODES: [DiagCode; 5] = [HN_N002, HN_C009, HN_M002, HN_M003, HN_V013];

/// One scope as a membership shard last recorded it.
struct Member {
    id: String,
    parent: Option<String>,
    dir: String,
}

fn norm_dir(d: &str) -> String {
    if d == "." {
        String::new()
    } else {
        d.to_string()
    }
}

/// The scope whose managerial folder holds `shard` (longest folder prefix).
fn shard_owner<'a>(reg: &'a Registry, shard: &RepoPath) -> Option<&'a ScopeId> {
    reg.scopes()
        .filter_map(|s| {
            let folder = s.control.as_ref()?.path.parent()?;
            shard
                .as_str()
                .starts_with(&format!("{}/", folder.as_str()))
                .then_some((folder.as_str().len(), &s.id))
        })
        .max_by_key(|(len, _)| *len)
        .map(|(_, id)| id)
}

/// Every shard's `self.` line and `scope.` lines, one row per scope id (a `self.` line wins).
fn recorded(reg: &Registry, cx: &CheckCx<'_>) -> Vec<(RepoPath, Member)> {
    let mut rows: BTreeMap<String, (bool, RepoPath, Member)> = BTreeMap::new();
    for (path, bytes) in &cx.facts.generated_on_disk {
        if !path.as_str().ends_with("gen/members") {
            continue;
        }
        let parsed = parse_members(&String::from_utf8_lossy(bytes));
        let owner = parsed
            .id
            .clone()
            .or_else(|| shard_owner(reg, path).map(|s| s.0.as_str().to_string()));
        if let (Some(id), Some(dir)) = (&parsed.id, &parsed.dir) {
            let parent = parsed.up.clone().filter(|u| u != "-");
            let m = Member {
                id: id.clone(),
                parent,
                dir: norm_dir(dir),
            };
            rows.insert(id.clone(), (true, path.clone(), m));
        }
        for (child, dir) in &parsed.children {
            if rows.get(child).is_some_and(|r| r.0) {
                continue;
            }
            let m = Member {
                id: child.clone(),
                parent: owner.clone(),
                dir: norm_dir(dir),
            };
            rows.insert(child.clone(), (false, path.clone(), m));
        }
    }
    rows.into_values().map(|(_, p, m)| (p, m)).collect()
}

/// Does moving under `new` lose a rule, or lower a severity or stage, that `old` had? Unknown old parent: yes.
fn loosens(rules: &RuleSet, reg: &Registry, old: Option<&str>, new: Option<&ScopeId>) -> bool {
    let Some(old_id) = reg
        .scopes()
        .find(|s| Some(s.id.0.as_str()) == old)
        .map(|s| &s.id)
    else {
        return true;
    };
    let new_rules = new.map(|p| rules.for_scope(p)).unwrap_or(&[]);
    rules.for_scope(old_id).iter().any(|r| {
        match new_rules.iter().find(|n| n.id == r.id && n.home == r.home) {
            None => true,
            Some(n) => n.sev < r.sev || n.stage < r.stage,
        }
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Check for ScopesCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let (reg, facts) = (cx.registry, cx.facts);
        if scope != reg.root_scope() {
            return;
        }
        // V013: absent from a sparse checkout is unknown, never green. Paths inside a scope that is
        // absent as a whole are reported once, by the scope line below.
        let absent_dirs: Vec<&RepoPath> = reg
            .scopes()
            .filter(|s| matches!(s.state, ScopeState::AbsentFromTree))
            .map(|s| &s.dir)
            .collect();
        for p in facts
            .git
            .sparse_absent
            .iter()
            .filter(|p| !absent_dirs.iter().any(|d| p.is_within(d)))
        {
            out.push(
                DiagB::new(
                    HN_V013,
                    p,
                    0,
                    0,
                    "path is absent from the sparse checkout; the verdict depends on it",
                )
                .hint("widen the sparse checkout or run check in CI")
                .text(p.as_str())
                .build(),
            );
        }
        for s in reg
            .scopes()
            .filter(|s| matches!(s.state, ScopeState::AbsentFromTree))
        {
            out.push(
                DiagB::new(
                    HN_V013,
                    &s.dir,
                    0,
                    0,
                    "scope is absent from the working tree",
                )
                .hint("widen the sparse checkout")
                .text(s.id.0.as_str())
                .build(),
            );
        }
        // membership shards on disk against the current tree
        for (shard, m) in &recorded(reg, cx) {
            let by_id = reg.scopes().find(|s| s.id.0.as_str() == m.id);
            let at_dir = reg.scopes().find(|s| s.dir.as_str() == m.dir);
            match (by_id, at_dir) {
                (Some(s), _) => {
                    let new_parent = s.parent.as_ref();
                    if new_parent.map(|p| p.0.as_str()) == m.parent.as_deref() {
                        continue;
                    }
                    let Some(cp) = control_path(reg, &s.id) else {
                        continue;
                    };
                    let bad = loosens(cx.rules, reg, m.parent.as_deref(), new_parent);
                    let msg = format!(
                        "scope {} moved from parent {} to {}",
                        m.id,
                        m.parent.as_deref().unwrap_or("-"),
                        new_parent.map_or("-", |p| p.0.as_str())
                    );
                    out.push(
                        DiagB::new(HN_M002, &cp, 0, 0, msg)
                            .severity(if bad { Severity::Error } else { Severity::Warn })
                            .hint(if bad {
                                "restore the old parent or have its owners approve the change"
                            } else {
                                "run fairlead sync to record the new parent"
                            })
                            .text(m.id.clone())
                            .build(),
                    );
                }
                (None, Some(d)) => {
                    let Some(cp) = control_path(reg, &d.id) else {
                        continue;
                    };
                    let msg = format!(
                        "scope id at {} changed from {} to {}",
                        if m.dir.is_empty() { "." } else { &m.dir },
                        m.id,
                        d.id.0.as_str()
                    );
                    out.push(
                        DiagB::new(HN_M003, &cp, 0, 0, msg)
                            .hint("restore the stored id; renames use replaced-by and a tombstone")
                            .text(m.id.clone())
                            .build(),
                    );
                }
                (None, None)
                    if !facts.git.sparse_absent.iter().any(|p| {
                        let p = p.as_str();
                        m.dir.starts_with(p) || p.starts_with(&format!("{}/", m.dir))
                    }) =>
                {
                    let msg = format!(
                        "registered scope {} ({}) is missing from the tree",
                        m.id, m.dir
                    );
                    out.push(
                        DiagB::new(HN_C009, shard, 0, 0, msg)
                            .hint("restore the folder, or demote the scope with a tombstone")
                            .text(m.id.clone())
                            .build(),
                    );
                }
                (None, None) => {}
            }
        }
        // N002: eligibility hints (info only)
        let Some(root) = root_control(reg) else {
            return;
        };
        let (min_lines, min_files) = (root.eligibility.min_lines, root.eligibility.min_files);
        let mut by_dir: BTreeMap<RepoPath, (u32, u32)> = BTreeMap::new();
        for (path, rec) in &cx.corpus.records {
            let Some(ex) = rec
                .extraction
                .as_ref()
                .filter(|e| !e.directives.is_empty() && rec.class == FileClass::Authored)
            else {
                continue;
            };
            let Some(dir) = path.parent() else { continue };
            if reg
                .scope_of(path)
                .and_then(|s| reg.scope(s))
                .is_some_and(|s| s.dir == dir)
            {
                continue; // already a scope
            }
            let e = by_dir.entry(dir).or_default();
            e.0 = e.0.saturating_add(count(ex.directives.len()));
            e.1 = e.1.saturating_add(1);
        }
        let scope_dirs: BTreeSet<&str> = reg.scopes().map(|s| s.dir.as_str()).collect();
        let hinted: BTreeSet<RepoPath> = by_dir
            .iter()
            .filter(|(_, (l, f))| *l >= min_lines && *f >= min_files)
            .map(|(d, _)| d.clone())
            .chain(
                facts
                    .manifest_hints
                    .workspace_members
                    .iter()
                    .filter(|m| !scope_dirs.contains(m.as_str()))
                    .cloned(),
            )
            .collect();
        for d in hinted {
            out.push(
                DiagB::new(HN_N002, &d, 0, 0, "folder is eligible to become a scope")
                    .hint("fairlead new scope <dir> promotes it, never automatically")
                    .text(d.as_str())
                    .build(),
            );
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
        facts::Facts,
    };
    use std::sync::Arc;

    fn shards(list: &[(&str, &str)]) -> impl FnOnce(&mut Facts) {
        let v: Vec<(RepoPath, Arc<[u8]>)> = list
            .iter()
            .map(|(p, t)| (RepoPath::new(p).unwrap(), Arc::from(t.as_bytes().to_vec())))
            .collect();
        move |f| f.generated_on_disk.extend(v)
    }
    const ROOT_SHARD: &str = "fairlead/gen/members";
    const STRICT: &str = "[fairlead]\nscope = \"core\"\n[rules.strict]\ncheck = \"pattern\"\nforbid = \"zzz\"\nmsg = \"no zzz\"\nsev = \"error\"\nstage = \"enforce\"\n";

    #[test]
    fn r_scope_011__folders_with_enough_file_local_lines_and_unscoped_members_are_eligible() {
        let src = "// fairlead: use tok.a\n// fairlead: use tok.b\nexport const a = 1;\n";
        let b = World::new()
            .root("[eligibility]\nmin-lines = 2\nmin-files = 2\n")
            .file("src/ui/a.ts", src)
            .file("src/ui/b.ts", src)
            .facts(|f| {
                f.manifest_hints
                    .workspace_members
                    .insert(RepoPath::new("crates/x").unwrap());
            })
            .build();
        let ds = b.run_check(&ScopesCheck);
        let paths: Vec<_> = of(&ds, c::HN_N002)
            .iter()
            .map(|d| d.location.path.as_str().to_string())
            .collect();
        assert_eq!(paths, ["crates/x", "src/ui"], "{ds:#?}");
    }

    #[test]
    fn r_scope_014__a_registered_scope_missing_from_the_tree_is_c009() {
        let b = World::new()
            .facts(shards(&[(ROOT_SHARD, "scope.web dir=web\n")]))
            .build();
        assert_eq!(of(&b.run_check(&ScopesCheck), c::HN_C009).len(), 1);
    }

    #[test]
    fn r_port_008__a_hand_edited_scope_id_is_m003() {
        let b = World::new()
            .scope("web", "[fairlead]\nscope = \"portal\"\n")
            .facts(shards(&[(ROOT_SHARD, "scope.web dir=web\n")]))
            .build();
        let ds = b.run_check(&ScopesCheck);
        assert_eq!(
            (of(&ds, c::HN_M003).len(), of(&ds, c::HN_C009).len()),
            (1, 0),
            "{ds:#?}"
        );
    }

    #[test]
    fn r_port_003__a_reparent_that_loosens_is_an_error_and_one_that_tightens_a_warning() {
        let loosen = World::new()
            .scope("core", STRICT)
            .scope("web", "[fairlead]\nscope = \"web\"\n")
            .facts(shards(&[
                (ROOT_SHARD, "scope.core dir=core\n"),
                ("core/fairlead/gen/members", "scope.web dir=web\n"),
            ]))
            .build();
        let d = loosen.run_check(&ScopesCheck);
        assert_eq!(of(&d, c::HN_M002)[0].severity, Severity::Error, "{d:#?}");
        let tighten = World::new()
            .scope("core", STRICT)
            .scope("core/web", "[fairlead]\nscope = \"web\"\n")
            .facts(shards(&[(
                ROOT_SHARD,
                "scope.core dir=core\nscope.web dir=core/web\n",
            )]))
            .build();
        let d = tighten.run_check(&ScopesCheck);
        assert_eq!(of(&d, c::HN_M002)[0].severity, Severity::Warn, "{d:#?}");
    }

    #[test]
    fn r_git_007__paths_absent_from_a_sparse_checkout_are_v013() {
        let b = World::new()
            .facts(|f| {
                f.git
                    .sparse_absent
                    .push(RepoPath::new("apps/legacy").unwrap())
            })
            .build();
        let ds = b.run_check(&ScopesCheck);
        let d = of(&ds, c::HN_V013);
        assert_eq!(d.len(), 1);
        assert_eq!(
            (d[0].location.path.as_str(), d[0].severity),
            ("apps/legacy", Severity::Warn)
        );
    }

    #[test]
    fn r_git_007__a_scope_whose_files_are_absent_is_one_v013_and_never_c009() {
        // git reports absent FILES; the registered scope that holds them is absent as a whole.
        let b = World::new()
            .facts(shards(&[(
                ROOT_SHARD,
                "scope.web dir=web
",
            )]))
            .facts(|f| {
                f.git
                    .sparse_absent
                    .push(RepoPath::new("web/fairlead/control.toml").unwrap());
                f.git
                    .sparse_absent
                    .push(RepoPath::new("web/src/app.ts").unwrap());
            })
            .build();
        let ds = b.run_check(&ScopesCheck);
        let v = of(&ds, c::HN_V013);
        assert_eq!(v.len(), 1, "{ds:#?}");
        assert_eq!(v[0].location.path.as_str(), "web");
        assert!(of(&ds, c::HN_C009).is_empty(), "{ds:#?}");
    }
}
