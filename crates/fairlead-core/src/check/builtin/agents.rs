//! Chapter 40 checks: agent block, CODEOWNERS, owners, secrets (R-AGT-002/010/022..024, R-CHK-053).
use crate::{
    check::{
        CheckCx,
        builtin::{control_path, root_control},
        finding::DiagB,
        rules::Check,
    },
    diag::{Diagnostic, codes::*},
    registry::ScopeId,
    write::init::pointer_block,
};
use fairlead_syntax::diag::DiagCode;
use globset::{Glob, GlobSetBuilder};

pub struct AgentsCheck;
const CODES: [DiagCode; 4] = [HN_A001, HN_A002, HN_A004, HN_F001];

/// The managed block of `text` (LF), from its BEGIN marker through its END marker.
fn block_of(text: &str, marker: &str) -> Option<String> {
    let lf = text.replace("\r\n", "\n");
    let begin = lf.find(&format!("<!-- BEGIN {marker}"))?;
    let end_pat = format!("<!-- END {marker} -->");
    let end = lf.get(begin..)?.find(&end_pat)? + begin + end_pat.len();
    lf.get(begin..end).map(str::to_string)
}

/// Owners of the last CODEOWNERS line matching `path`; `None` = no line matches.
fn owners_for(codeowners: &str, path: &str) -> Option<Vec<String>> {
    let mut found = None;
    for line in codeowners.lines() {
        let mut it = line.split('#').next().unwrap_or("").split_whitespace();
        let Some(pat) = it.next() else { continue };
        let body = pat.trim_start_matches('/').trim_end_matches('/');
        let base = if pat.starts_with('/') || body.contains('/') {
            body.to_string()
        } else {
            format!("**/{body}")
        };
        let mut b = GlobSetBuilder::new();
        for g in [base.clone(), format!("{base}/**")] {
            if let Ok(g) = Glob::new(&g) {
                b.add(g);
            }
        }
        if b.build().is_ok_and(|s| s.is_match(path)) {
            found = Some(it.map(str::to_string).collect());
        }
    }
    found
}

impl Check for AgentsCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let (reg, f) = (cx.registry, cx.facts);
        let Some(root) = root_control(reg) else {
            return;
        };
        let marker = root
            .fairlead
            .marker
            .clone()
            .unwrap_or_else(|| "fairlead".to_string());
        let folder = root
            .settings
            .folder
            .clone()
            .unwrap_or_else(|| "fairlead".to_string());
        // F001 (floor): only hits recorded by the extractor; the value is never available here
        for (path, rec) in cx
            .corpus
            .records
            .iter()
            .filter(|(p, _)| reg.scope_of(p) == Some(scope))
        {
            let allowed = |fp: &str| root.secrets.allow.iter().any(|a| a.starts_with(fp));
            for h in rec.secrets.iter().filter(|h| !allowed(&h.fp_prefix)) {
                out.push(
                    DiagB::new(
                        HN_F001,
                        path,
                        h.span.line,
                        h.span.col,
                        format!("secret-like value ({}) found; the value is not shown", h.class),
                    )
                    .hint("declare secrets by env var name (value=env:NAME); false positives go in [secrets] allow")
                    .text(format!("{}:{}", h.class, h.fp_prefix))
                    .build(),
                );
            }
        }
        // A002: this scope's managerial folder must be owned in CODEOWNERS
        let cp = control_path(reg, scope);
        if let Some(cp) = &cp {
            let owned = f
                .codeowners
                .as_deref()
                .and_then(|t| owners_for(t, cp.as_str()))
                .is_some_and(|o| !o.is_empty());
            if !owned {
                let dir = cp
                    .parent()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default();
                out.push(
                    DiagB::new(
                        HN_A002,
                        cp,
                        0,
                        0,
                        format!("managerial folder {dir} is not covered by a CODEOWNERS entry"),
                    )
                    .hint(format!("add `/{dir}/ @owner` to CODEOWNERS"))
                    .text(dir)
                    .build(),
                );
            }
        }
        if scope != reg.root_scope() {
            return;
        }
        let expected = pointer_block(&marker, &folder);
        for (path, text) in &f.agent_files {
            if block_of(text, &marker).as_deref() != Some(expected.trim_end()) {
                out.push(
                    DiagB::new(
                        HN_A001,
                        path,
                        0,
                        0,
                        "agent file has no managed block, or it differs from the pinned text",
                    )
                    .hint("run fairlead init --yes to rewrite the block")
                    .text(path.as_str())
                    .build(),
                );
            }
        }
        if let (Some(co), Some(cp)) = (f.codeowners.as_deref(), &cp) {
            let listed = owners_for(co, cp.as_str()).unwrap_or_default();
            for o in root
                .settings
                .owners
                .iter()
                .filter(|o| !listed.contains(&o.handle))
            {
                out.push(
                    DiagB::new(
                        HN_A004,
                        cp,
                        0,
                        0,
                        format!(
                            "owner {} has no CODEOWNERS entry on the managerial folder",
                            o.handle
                        ),
                    )
                    .hint("add the handle to the CODEOWNERS line owning the folder")
                    .text(o.handle.clone())
                    .build(),
                );
            }
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
        extract::SecretHit,
        path::RepoPath,
    };
    use fairlead_syntax::diag::Span;
    use std::sync::Arc;

    fn agent(name: &'static str, text: String) -> impl FnOnce(&mut crate::facts::Facts) {
        move |f| {
            f.agent_files
                .insert(RepoPath::new(name).unwrap(), Arc::<str>::from(text));
        }
    }
    fn good() -> String {
        pointer_block("fairlead", "fairlead")
    }
    fn hit() -> SecretHit {
        SecretHit {
            class: "provider-key".into(),
            span: Span {
                start: 0,
                end: 4,
                line: 3,
                col: 5,
            },
            fp_prefix: "k3m9".into(),
        }
    }

    #[test]
    fn r_agt_002__missing_stale_and_good_blocks() {
        let stale = good().replace("Before changing code", "Whenever you like");
        let crlf = format!("intro\n{}outro\n", good()).replace('\n', "\r\n");
        let b = World::new()
            .facts(agent("AGENTS.md", "# Notes\n".into()))
            .facts(agent("CLAUDE.md", stale))
            .facts(agent("GEMINI.md", crlf))
            .build();
        let paths: Vec<_> = of(&b.run_check(&AgentsCheck), c::HN_A001)
            .iter()
            .map(|d| d.location.path.as_str().to_string())
            .collect();
        assert_eq!(paths, ["AGENTS.md", "CLAUDE.md"]);
    }

    #[test]
    fn r_agt_010__an_uncovered_managerial_folder_is_a002() {
        let uncovered = |co: Option<&str>| {
            let co = co.map(Arc::<str>::from);
            World::new()
                .scope("web", "[fairlead]\nscope = \"web\"\n")
                .facts(move |f| f.codeowners = co)
                .build()
                .run_check(&AgentsCheck)
        };
        assert_eq!(of(&uncovered(None), c::HN_A002).len(), 2);
        assert_eq!(of(&uncovered(Some("/docs/ @a\n")), c::HN_A002).len(), 2);
        let both = "/fairlead/ @platform\n**/fairlead/ @platform\n";
        assert!(of(&uncovered(Some(both)), c::HN_A002).is_empty());
        // the last match with no owner un-owns
        let unown = "**/fairlead/ @platform\n/web/fairlead/\n";
        assert_eq!(of(&uncovered(Some(unown)), c::HN_A002).len(), 1);
    }

    #[test]
    fn r_chk_053__an_owner_handle_without_a_codeowners_entry_is_a004() {
        let co = Some(Arc::<str>::from("/fairlead/ @someone-else\n"));
        let d = World::new()
            .facts(move |f| f.codeowners = co)
            .build()
            .run_check(&AgentsCheck);
        assert_eq!((of(&d, c::HN_A004).len(), of(&d, c::HN_A002).len()), (1, 0));
    }

    #[test]
    fn r_agt_022__r_agt_023__secret_hits_name_class_and_fingerprint_prefix_never_the_value() {
        let mut b = World::new().file("fairlead/docs/a.md", "# a\n").build();
        let p = RepoPath::new("fairlead/docs/a.md").unwrap();
        b.corpus.records.get_mut(&p).unwrap().secrets.push(hit());
        let ds = b.run_check(&AgentsCheck);
        let d = of(&ds, c::HN_F001);
        assert_eq!((d.len(), d[0].location.line, d[0].location.col), (1, 3, 5));
        assert!(d[0].message.contains("provider-key") && !d[0].message.contains("k3m9"));
        assert!(d[0].text.contains("k3m9"));
    }

    #[test]
    fn r_agt_024__a_fingerprint_in_secrets_allow_clears_the_hit() {
        let mut b = World::new()
            .root("[secrets]\nallow = [\"k3m9q2x7a1bz\"]\n")
            .file("fairlead/docs/a.md", "# a\n")
            .build();
        let p = RepoPath::new("fairlead/docs/a.md").unwrap();
        b.corpus.records.get_mut(&p).unwrap().secrets.push(hit());
        assert!(of(&b.run_check(&AgentsCheck), c::HN_F001).is_empty());
    }
}
