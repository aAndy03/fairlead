//! Syntax diagnostics, line classes and scaffold placeholders (R-ARCH-031, R-FILE-011, C-31).
use fairlead_syntax::diag::{DiagCode, SyntaxDiag};
use fairlead_syntax::directive::Verb;
use fairlead_syntax::ids::PLACEHOLDER;

use crate::check::CheckCx;
use crate::check::finding::DiagB;
use crate::check::rules::Check;
use crate::classify::FileClass;
use crate::diag::Diagnostic;
use crate::diag::codes::*;
use crate::path::RepoPath;
use crate::registry::ScopeId;

pub struct SyntaxCheck;

const CODES: [DiagCode; 13] = [
    HN_S001, HN_S004, HN_S005, HN_S006, HN_S007, HN_S008, HN_S010, HN_S012, HN_S013, HN_S014,
    HN_S015, HN_I002, HN_C011,
];

fn lift(path: &RepoPath, sd: &SyntaxDiag) -> Diagnostic {
    let mut b = DiagB::new(sd.code, path, sd.span.line, sd.span.col, sd.message.clone())
        .severity(sd.severity)
        .hint(sd.hint.clone())
        .text(sd.message.clone());
    if let Some(d) = &sd.did_you_mean {
        b = b.dym(d.clone());
    }
    let mut d = b.build();
    d.expected = sd.expected.clone();
    d
}

fn verb_allowed(class: FileClass, v: Verb) -> bool {
    match class {
        FileClass::Generated => false,
        FileClass::Ledger => matches!(v, Verb::Def | Verb::Ovr | Verb::Flag),
        _ => true,
    }
}

fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Def => "def",
        Verb::Use => "use",
        Verb::Ovr => "ovr",
        Verb::Flag => "flag",
        Verb::Env => "env",
        Verb::End => "end",
    }
}

fn has_placeholder(t: &toml::Table) -> bool {
    t.values().any(|v| match v {
        toml::Value::String(s) => s == PLACEHOLDER,
        toml::Value::Table(inner) => has_placeholder(inner),
        _ => false,
    })
}

impl Check for SyntaxCheck {
    fn codes(&self) -> &'static [DiagCode] {
        &CODES
    }

    fn run(&self, cx: &CheckCx<'_>, scope: &ScopeId, out: &mut Vec<Diagnostic>) {
        for (path, rec) in &cx.corpus.records {
            if cx.registry.scope_of(path) != Some(scope) {
                continue;
            }
            let Some(ex) = rec.extraction.as_ref() else {
                continue;
            };
            // A-56: skip notices (HN-I001/I003) come from `notices.rs` only.
            out.extend(
                ex.diags
                    .iter()
                    .filter(|sd| sd.code != HN_I001 && sd.code != HN_I003)
                    .map(|sd| lift(path, sd)),
            );
            for d in ex
                .directives
                .iter()
                .filter(|d| !verb_allowed(rec.class, d.verb))
            {
                out.push(
                    DiagB::new(
                        HN_C011,
                        path,
                        d.span.line,
                        d.span.col,
                        format!(
                            "line class `{}` is not allowed in a {:?} file",
                            verb_name(d.verb),
                            rec.class
                        ),
                    )
                    .hint(
                        "move the line to an authored file; generated files are rewritten by sync",
                    )
                    .text(verb_name(d.verb))
                    .build(),
                );
            }
            if let Some(fm) = ex
                .frontmatter
                .as_ref()
                .filter(|f| has_placeholder(&f.table))
            {
                out.push(
                    DiagB::new(
                        HN_S004,
                        path,
                        fm.span.line,
                        fm.span.col,
                        "mandatory frontmatter slot still holds the scaffold placeholder",
                    )
                    .hint("replace the TODO slot with real content")
                    .text(PLACEHOLDER)
                    .build(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::testkit::{World, of};
    use crate::diag::codes as c;
    use fairlead_syntax::diag::Severity;

    #[test]
    fn r_arch_031__syntax_diagnostics_are_lifted_with_path_position_and_severity() {
        let b = World::new()
            .file("src/a.ts", "// fairlead: frob cmp.x\nexport const a = 1;\n")
            .build();
        let ds = b.run_check(&SyntaxCheck);
        let s = of(&ds, c::HN_S001);
        assert_eq!(s.len(), 1);
        assert_eq!(
            (
                s[0].location.path.as_str(),
                s[0].location.line,
                s[0].severity
            ),
            ("src/a.ts", 1, Severity::Error)
        );
    }

    #[test]
    fn r_file_011__r_file_008__a_directive_in_a_generated_file_is_a_class_line_error() {
        let b = World::new()
            .file(
                "fairlead/gen/reg/tok.ts",
                "// fairlead: use tok.a\nexport const x = 1;\n",
            )
            .build();
        let ds = b.run_check(&SyntaxCheck);
        let s = of(&ds, c::HN_C011);
        assert_eq!(s.len(), 1);
        assert!(s[0].message.contains("`use`"));
    }

    #[test]
    fn r_file_011__an_authored_directive_is_fine() {
        let b = World::new()
            .file("src/a.ts", "// fairlead: use tok.a\nexport const x = 1;\n")
            .build();
        assert!(of(&b.run_check(&SyntaxCheck), c::HN_C011).is_empty());
    }

    #[test]
    fn r_cli_031__scaffold_placeholder_in_frontmatter_is_a_missing_key() {
        let doc = "+++\ntitle = \"TODO(fairlead)\"\n+++\n# Doc\n";
        let b = World::new().file("fairlead/docs/a.md", doc).build();
        assert_eq!(of(&b.run_check(&SyntaxCheck), c::HN_S004).len(), 1);
    }
}
