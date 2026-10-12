//! Path globs of the scorer rules: `**` crosses folders, `*` and `?` stay inside one.

pub fn to_regex(glob: &str) -> regex::Regex {
    let mut re = String::from("^");
    let chars: Vec<char> = glob.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if chars.get(i + 1) == Some(&'*') => {
                // `**/` matches zero or more folders; a trailing `**` matches the rest.
                if chars.get(i + 2) == Some(&'/') {
                    re.push_str("(?:.*/)?");
                    i += 3;
                } else {
                    re.push_str(".*");
                    i += 2;
                }
                continue;
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
        i += 1;
    }
    re.push('$');
    regex::Regex::new(&re).expect("a glob always compiles")
}

pub struct Set(Vec<regex::Regex>);

impl Set {
    pub fn new(globs: &[String]) -> Set {
        Set(globs.iter().map(|g| to_regex(g)).collect())
    }

    pub fn matches(&self, path: &str) -> bool {
        self.0.iter().any(|r| r.is_match(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(to_regex("store/**/*.ts").is_match("store/src/cart.ts"));
        assert!(to_regex("store/**/*.ts").is_match("store/a.ts"));
        assert!(!to_regex("store/**/*.ts").is_match("kit/a.ts"));
        assert!(to_regex("crates/cli/**").is_match("crates/cli/Cargo.toml"));
        assert!(!to_regex("frontend/src/*.ts").is_match("frontend/src/a/b.ts"));
    }
}
