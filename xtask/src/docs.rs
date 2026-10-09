//! `cargo xtask docs-check`: dangling R- and D- references in docs/ (R-ARCH-161) and
//! open 0.1 questions in the register (E10). Definitions are rule rows and bold rule lines.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn md_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "plans") {
                md_files(&p, out)?;
            }
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(())
}

/// Every `R-XXX-nnn` or `D-nn` token of the form the spec uses.
fn ids_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
        let rule = word
            .strip_prefix("R-")
            .and_then(|r| r.split_once('-'))
            .is_some_and(|(area, n)| {
                !area.is_empty()
                    && area.bytes().all(|b| b.is_ascii_uppercase())
                    && n.len() == 3
                    && n.bytes().all(|b| b.is_ascii_digit())
            });
        let decision = word.len() == 4
            && word.starts_with("D-")
            && word[2..].bytes().all(|b| b.is_ascii_digit());
        if rule || decision {
            out.push(word.to_string());
        }
    }
    out
}

pub fn dangling_ids(docs: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    md_files(docs, &mut files)?;
    let mut defined = BTreeSet::new();
    let mut referenced = BTreeSet::new();
    for f in files {
        for line in std::fs::read_to_string(&f)?.lines() {
            let is_def =
                line.starts_with("- **") || line.starts_with("| ") || line.starts_with("**");
            for id in ids_in(line) {
                if is_def {
                    defined.insert(id.clone());
                }
                referenced.insert(id);
            }
        }
    }
    Ok(referenced
        .difference(&defined)
        .filter(|id| !RETIRED.contains(&id.as_str()))
        .cloned()
        .collect())
}

/// Numbers docs/README.md names as never used ("D-08 and D-09 are not used").
const RETIRED: &[&str] = &["D-08", "D-09"];

/// Rows of the register whose milestone is exactly `0.1` and whose resolution column is
/// empty or starts with "Open".
pub fn open_0_1_questions(register: &Path) -> std::io::Result<Vec<String>> {
    let mut open = Vec::new();
    for line in std::fs::read_to_string(register)?.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() >= 7
            && cells[6] == "0.1"
            && (cells[4].is_empty() || cells[4].starts_with("Open"))
        {
            open.push(cells[1].to_string());
        }
    }
    Ok(open)
}

/// Both checks against `<root>/docs`; the error lists every defect.
pub fn check(root: &Path) -> Result<(), String> {
    let dangling = dangling_ids(&root.join("docs")).map_err(|e| e.to_string())?;
    let open =
        open_0_1_questions(&root.join("docs/70-open-questions.md")).map_err(|e| e.to_string())?;
    let mut problems = Vec::new();
    if !dangling.is_empty() {
        problems.push(format!("referenced but never defined: {dangling:?}"));
    }
    if !open.is_empty() {
        problems.push(format!("open 0.1 questions: {open:?}"));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
#[allow(non_snake_case)] // reason: test names start with the rule id
mod tests {
    use super::*;

    #[test]
    fn r_arch_161__docs_have_no_dangling_rule_ids() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let dangling = dangling_ids(&root.join("docs")).expect("docs readable");
        assert!(
            dangling.is_empty(),
            "referenced but never defined: {dangling:?}"
        );
    }

    #[test]
    fn e10__no_open_question_tagged_0_1() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let open =
            open_0_1_questions(&root.join("docs/70-open-questions.md")).expect("register readable");
        assert!(open.is_empty(), "open 0.1 questions: {open:?}");
    }

    #[test]
    fn r_arch_161__ids_in_finds_rule_and_decision_tokens_only() {
        assert_eq!(
            ids_in("see R-ARCH-161 and D-34, not R-X-1 or D-345"),
            vec!["R-ARCH-161".to_string(), "D-34".to_string()]
        );
    }
}
