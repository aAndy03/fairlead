//! Generated files against what `sync` would write (R-CHK-023, R-CHK-024).
use crate::{
    check::{
        CheckCx,
        builtin::{baseline_rel, edition_of},
        finding::{DiagB, clip},
        rules::Check,
    },
    diag::{Diagnostic, codes::*},
    generate::{HeaderVerdict, first_differing_lines, verify_header},
    registry::ScopeId,
};
use fairlead_syntax::{diag::DiagCode, normalize::lf_bytes};

pub struct GeneratedCheck;
const CODES: [DiagCode; 3] = [HN_G001, HN_G002, HN_B002];

impl Check for GeneratedCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }
    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        let reg = cx.registry;
        if scope != reg.root_scope() {
            return;
        } // global: one pass over the whole set
        let baseline_file = baseline_rel(reg);
        for (path, have) in &cx.facts.generated_on_disk {
            let bytes: &[u8] = have;
            if verify_header(bytes) == HeaderVerdict::HandEdited {
                out.push(if path.as_str() == baseline_file {
                    DiagB::new(HN_B002, path, 0, 0, "baseline hash mismatch (hand-edited)")
                        .hint("git checkout the file; grow it with sync --baseline --accept-new --why").text(path.as_str()).build()
                } else {
                    DiagB::new(HN_G002, path, 0, 0, "generated file was edited by hand (header hash does not match the body)")
                        .hint("edit the source, then run fairlead sync --force").text(path.as_str()).build()
                });
                continue;
            }
            let Some(file) = cx.generated.files.get(path) else {
                continue;
            };
            let want = file.render(edition_of(reg));
            let have_lf = lf_bytes(bytes);
            if &*have_lf != want.as_slice() {
                // The header hash differs whenever the body does, so only the bodies are diffed.
                let body = |b: &[u8]| {
                    b.iter()
                        .position(|c| *c == 10)
                        .map_or(Vec::new(), |i| b[i + 1..].to_vec())
                };
                let lines = first_differing_lines(&body(&want), &body(&have_lf), 3)
                    .iter()
                    .map(|l| clip(l, 60))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let lines = if lines.is_empty() {
                    "header (edition) differs".to_string()
                } else {
                    lines
                };
                out.push(
                    DiagB::new(
                        HN_G001,
                        path,
                        0,
                        0,
                        format!("generated file differs from sync output: {lines}"),
                    )
                    .hint("run fairlead sync")
                    .text(path.as_str())
                    .build(),
                );
            }
        }
        for path in cx
            .generated
            .files
            .keys()
            .filter(|p| !cx.facts.generated_on_disk.contains_key(*p))
        {
            out.push(
                DiagB::new(HN_G001, path, 0, 0, "generated file is missing")
                    .hint("run fairlead sync")
                    .text(path.as_str())
                    .build(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        check::testkit::{World, of},
        diag::codes as c,
        generate::{GenKind, GeneratedFile, GeneratedSet},
        path::RepoPath,
    };
    use std::{collections::BTreeMap, sync::Arc};

    fn gen_set(b: &crate::check::testkit::Built, path: &str, body: &str) -> GeneratedSet {
        let p = RepoPath::new(path).unwrap();
        let f = GeneratedFile {
            path: p.clone(),
            scope: b.registry.root_scope().clone(),
            kind: GenKind::Members,
            body: body.to_string(),
        };
        GeneratedSet {
            files: BTreeMap::from([(p, f)]),
        }
    }
    fn disk(body: &str) -> Vec<u8> {
        let scope =
            crate::registry::ScopeId(fairlead_syntax::ids::ScopeSlug::parse("root").unwrap());
        let f = GeneratedFile {
            path: RepoPath::new("fairlead/gen/members").unwrap(),
            scope,
            kind: GenKind::Members,
            body: body.to_string(),
        };
        f.render(crate::discover::Edition(1))
    }
    fn crlf(b: &[u8]) -> Vec<u8> {
        b.iter()
            .flat_map(|c| if *c == 10 { vec![13, 10] } else { vec![*c] })
            .collect()
    }
    fn on_disk(path: &str, bytes: &[u8]) -> impl FnOnce(&mut crate::facts::Facts) {
        let (p, v) = (
            RepoPath::new(path).unwrap(),
            Arc::<[u8]>::from(bytes.to_vec()),
        );
        move |f| {
            f.generated_on_disk.insert(p, v);
        }
    }

    #[test]
    fn r_chk_023__differing_bytes_are_g001_with_the_first_differing_lines() {
        let b = World::new()
            .facts(on_disk("fairlead/gen/members", &disk("l1\nX\nY\nl4\n")))
            .build();
        let g = gen_set(&b, "fairlead/gen/members", "l1\nl2\nl3\nl4\n");
        let ds = of(&b.run_check_with(&GeneratedCheck, &g), c::HN_G001)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(ds.len(), 1);
        assert!(
            ds[0].message.contains("-X")
                && ds[0].message.contains("-Y")
                && ds[0].message.contains("+l2")
        );
        assert_eq!(ds[0].hint, "run fairlead sync");
    }

    #[test]
    fn r_chk_023__a_missing_generated_file_is_stale() {
        let b = World::new().build();
        let g = gen_set(&b, "fairlead/gen/members", "l1\n");
        assert_eq!(
            of(&b.run_check_with(&GeneratedCheck, &g), c::HN_G001).len(),
            1
        );
    }

    #[test]
    fn r_chk_012__a_crlf_checkout_of_identical_content_is_not_stale() {
        let b = World::new()
            .facts(on_disk("fairlead/gen/members", &crlf(&disk("l1\nl2\n"))))
            .build();
        let g = gen_set(&b, "fairlead/gen/members", "l1\nl2\n");
        let ds = b.run_check_with(&GeneratedCheck, &g);
        assert!(
            ds.is_empty(),
            "{:?}",
            ds.iter()
                .map(|d| (d.code.0, d.message.clone()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn r_file_007__r_chk_024__hand_edited_shard_is_g002_and_hand_edited_baseline_is_b002() {
        // bytes produced by P5b's renderer, then tampered after the header
        let b0 = World::new().build();
        let g = gen_set(&b0, "fairlead/gen/members", "l1\nl2\n");
        let mut bytes = g
            .files
            .values()
            .next()
            .unwrap()
            .render(crate::discover::Edition(1));
        bytes.extend_from_slice(b"hand edit\n");
        let b = World::new()
            .facts(on_disk("fairlead/gen/members", &bytes))
            .facts(on_disk("fairlead/gen/baseline.txt", &bytes))
            .build();
        let ds = b.run_check_with(&GeneratedCheck, &g);
        assert_eq!(of(&ds, c::HN_G002).len(), 1);
        assert_eq!(of(&ds, c::HN_B002).len(), 1);
    }
}
