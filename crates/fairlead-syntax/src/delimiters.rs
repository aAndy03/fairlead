//! Per-extension comment leaders (R-SYN-006, R-SYN-044).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Leader {
    /// A leader that runs to the end of the line, e.g. `//`, `#`, `--`.
    Line(String),
    /// A delimited comment, e.g. `/* */`.
    Block { open: String, close: String },
    /// `<!-- -->`.
    Html,
}

impl Leader {
    /// Parses a `[comments]` value: `"//"`, `"/* */"`, `"<!-- -->"`, `"{- -}"`.
    pub fn from_spec(spec: &str) -> Option<Leader> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        if spec == "<!-- -->" {
            return Some(Leader::Html);
        }
        match spec.split_once(' ') {
            Some((open, close)) if !open.is_empty() && !close.trim().is_empty() => {
                Some(Leader::Block {
                    open: open.to_owned(),
                    close: close.trim().to_owned(),
                })
            }
            _ => Some(Leader::Line(spec.to_owned())),
        }
    }
}

/// Key: extension without the dot, lowercase.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommentTable(BTreeMap<String, Vec<Leader>>);

fn norm_ext(ext: &str) -> String {
    ext.trim_start_matches('.').to_ascii_lowercase()
}

impl CommentTable {
    /// The table of R-SYN-006.
    pub fn builtin() -> Self {
        let mut m: BTreeMap<String, Vec<Leader>> = BTreeMap::new();
        let slash = || {
            vec![
                Leader::Line("//".to_owned()),
                Leader::Block {
                    open: "/*".to_owned(),
                    close: "*/".to_owned(),
                },
            ]
        };
        for e in [
            "rs", "ts", "tsx", "js", "jsx", "mjs", "go", "java", "kt", "swift", "c", "h", "cc",
            "cpp", "cs", "scala", "zig",
        ] {
            m.insert(e.to_owned(), slash());
        }
        for e in ["css", "scss", "less"] {
            m.insert(
                e.to_owned(),
                vec![Leader::Block {
                    open: "/*".to_owned(),
                    close: "*/".to_owned(),
                }],
            );
        }
        for e in ["py", "sh", "rb", "toml", "yaml", "yml", "ps1", "gitignore"] {
            m.insert(e.to_owned(), vec![Leader::Line("#".to_owned())]);
        }
        for e in ["sql", "lua", "hs"] {
            m.insert(e.to_owned(), vec![Leader::Line("--".to_owned())]);
        }
        for e in ["html", "md", "xml", "svg"] {
            m.insert(e.to_owned(), vec![Leader::Html]);
        }
        m.insert("json".to_owned(), Vec::new()); // comment-free: [[attach]] from 0.2 (R-SYN-047)
        CommentTable(m)
    }

    /// `[comments]` of the root control file adds or replaces an entry.
    pub fn with_overrides(mut self, ext: &str, leaders: Vec<Leader>) -> Self {
        self.0.insert(norm_ext(ext), leaders);
        self
    }

    /// `Some(&[])` = known but comment-free; `None` = unknown extension (HN-I002).
    pub fn leaders_for(&self, ext: &str) -> Option<&[Leader]> {
        self.0.get(&norm_ext(ext)).map(Vec::as_slice)
    }

    /// Sorted iteration, used for the config fingerprint.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<Leader>)> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(s: &str) -> Leader {
        Leader::Line(s.to_owned())
    }
    fn block() -> Leader {
        Leader::Block {
            open: "/*".to_owned(),
            close: "*/".to_owned(),
        }
    }

    #[test]
    fn r_syn_006__builtin_table_matches_the_spec() {
        let t = CommentTable::builtin();
        for ext in [
            "rs", "ts", "tsx", "js", "jsx", "mjs", "go", "java", "kt", "swift", "c", "h", "cc",
            "cpp", "cs", "scala", "zig",
        ] {
            assert_eq!(
                t.leaders_for(ext),
                Some(&[line("//"), block()][..]),
                "{ext}"
            );
        }
        for ext in ["css", "scss", "less"] {
            assert_eq!(t.leaders_for(ext), Some(&[block()][..]), "{ext}");
        }
        for ext in ["py", "sh", "rb", "toml", "yaml", "yml", "ps1", "gitignore"] {
            assert_eq!(t.leaders_for(ext), Some(&[line("#")][..]), "{ext}");
        }
        for ext in ["sql", "lua", "hs"] {
            assert_eq!(t.leaders_for(ext), Some(&[line("--")][..]), "{ext}");
        }
        for ext in ["html", "md", "xml", "svg"] {
            assert_eq!(t.leaders_for(ext), Some(&[Leader::Html][..]), "{ext}");
        }
    }

    #[test]
    fn r_syn_006__json_has_no_leaders_and_unknown_is_none() {
        let t = CommentTable::builtin();
        assert_eq!(t.leaders_for("json"), Some(&[][..]));
        assert_eq!(
            t.leaders_for("xyz"),
            None,
            "unknown extension yields HN-I002 upstream"
        );
        assert_eq!(t.leaders_for(""), None);
    }

    #[test]
    fn r_syn_006__overrides_add_and_replace() {
        let t = CommentTable::builtin()
            .with_overrides("foo", vec![line(";")])
            .with_overrides("rs", vec![line("--")]);
        assert_eq!(t.leaders_for("foo"), Some(&[line(";")][..]));
        assert_eq!(t.leaders_for("rs"), Some(&[line("--")][..]));
        assert_eq!(
            t.leaders_for("ts"),
            Some(&[line("//"), block()][..]),
            "other entries keep their leaders"
        );
    }

    #[test]
    fn r_syn_044__lookup_ignores_leading_dot_and_case() {
        let t = CommentTable::builtin();
        assert_eq!(t.leaders_for(".RS"), t.leaders_for("rs"));
        assert_eq!(t.leaders_for("Gitignore"), t.leaders_for("gitignore"));
    }

    #[test]
    fn r_syn_006__from_spec_reads_control_file_strings() {
        assert_eq!(Leader::from_spec("//"), Some(line("//")));
        assert_eq!(Leader::from_spec("# "), Some(line("#")));
        assert_eq!(Leader::from_spec("/* */"), Some(block()));
        assert_eq!(Leader::from_spec("<!-- -->"), Some(Leader::Html));
        assert_eq!(
            Leader::from_spec("{- -}"),
            Some(Leader::Block {
                open: "{-".into(),
                close: "-}".into()
            })
        );
        assert_eq!(Leader::from_spec(""), None);
    }
}
