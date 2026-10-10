//! `[[attach]]` (R-SYN-047): directives declared in a control file for files that cannot carry
//! comments (JSON, images, generated configs). Each binds in file form to every matching file,
//! exactly as if it stood in the file's first comment block. Applied after extraction, so the
//! cached per-file extraction stays a pure function of the file's bytes.
use std::sync::Arc;

use fairlead_syntax::diag::Span;
use fairlead_syntax::directive::{Carrier, Directive, Extent, parse_body};

use crate::config::ControlSet;
use crate::extract::Corpus;

pub fn apply_attach(corpus: &mut Corpus, controls: &ControlSet) {
    for c in controls.ok_files() {
        let dir = c.dir.as_str().to_owned();
        for a in &c.attach {
            let Ok(glob) = globset::Glob::new(&a.path) else {
                continue;
            };
            let m = glob.compile_matcher();
            let bodies: Vec<_> = a
                .directives
                .iter()
                .filter_map(|d| parse_body(d).ok())
                .collect();
            for (path, rec) in corpus.records.iter_mut() {
                let rel = if dir.is_empty() {
                    Some(path.as_str())
                } else {
                    path.as_str()
                        .strip_prefix(dir.as_str())
                        .and_then(|r| r.strip_prefix('/'))
                };
                if !rel.is_some_and(|r| m.is_match(r)) {
                    continue;
                }
                let Some(ex) = rec.extraction.as_ref() else {
                    continue;
                };
                let mut ex = (**ex).clone();
                for b in &bodies {
                    let ordinal = u32::try_from(ex.directives.len()).unwrap_or(u32::MAX);
                    ex.directives.push(Directive {
                        ordinal,
                        verb: b.verb,
                        must_understand: b.must_understand,
                        ids: b.ids.clone(),
                        bare: b.bare.clone(),
                        args: b.args.clone(),
                        note: b.note.clone(),
                        carrier: Carrier::LineComment,
                        span: Span {
                            start: 0,
                            end: 0,
                            line: 0,
                            col: 0,
                        },
                        extent: Extent::File,
                        end_span: None,
                    });
                    ex.candidates += 1;
                }
                rec.extraction = Some(Arc::new(ex));
            }
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use crate::check::builtin::refs::RefsCheck;
    use crate::check::testkit::{World, of};
    use crate::config::parse_control;
    use crate::diag::codes as c;
    use crate::path::RepoPath;

    fn world(directive: &str) -> crate::check::testkit::Built {
        World::new()
            .root(&format!(
                "[kinds.cmp]\nimpl = 1\n\n[[attach]]\npath = \"config/*.json\"\ndirectives = [\"{directive}\"]\n"
            ))
            .file("fairlead/docs/cmp.md", "+++\ndirectives = ['def cmp.x']\n+++\n# C\n")
            .file("config/a.json", "{}\n")
            .build()
    }

    #[test]
    fn r_syn_047__attach_binds_file_form() {
        let b = world("use cmp.x");
        assert!(
            b.registry
                .edges()
                .iter()
                .any(|e| e.from.path.as_str() == "config/a.json" && e.resolved.is_some()),
            "the JSON file uses cmp.x"
        );
        let bad = world("use cmp.missing");
        let ds = bad.run_check(&RefsCheck);
        let r = of(&ds, c::HN_R001);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].location.path.as_str(), "config/a.json");
    }

    #[test]
    fn r_syn_047__end_and_regions_rejected() {
        for d in ["end", "use cmp.x +2", "env dev +end"] {
            let text = format!(
                "[fairlead]\nedition = 1\nroot = true\nscope = \"root\"\n[[attach]]\npath = \"a.json\"\ndirectives = [\"{d}\"]\n"
            );
            let r = parse_control(
                &RepoPath::new("fairlead/control.toml").unwrap(),
                text.as_bytes(),
                true,
            );
            assert!(r.is_err(), "{d} must be rejected");
        }
    }
}
