//! Disposable JSON cache in `<root>/.fairlead/` (50-architecture Section 6). Never a source of truth.
mod files;
mod key;

pub use files::FileRow;
pub use key::CacheKey;

use crate::discover::Root;
use crate::extract::Extras;
use crate::hash::ContentHash;
use crate::path::RepoPath;
use camino::Utf8PathBuf;
use fairlead_syntax::extract::Extraction;
use files::FileTable;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug)]
pub struct Cache {
    dir: Option<Utf8PathBuf>,
    rows: Mutex<FileTable>,
}

fn read_json<T: DeserializeOwned>(path: &Utf8PathBuf) -> Option<T> {
    let bytes = std::fs::read(path.as_std_path()).ok()?;
    match serde_json::from_slice::<T>(&bytes) {
        Ok(v) => Some(v),
        Err(_) => {
            let _ = std::fs::remove_file(path.as_std_path()); // corrupt: discard and rebuild
            None
        }
    }
}

/// Temporary file in the same directory, then rename (R-ARCH-074). Every failure is ignored.
fn write_atomic<T: Serialize>(path: &Utf8PathBuf, value: &T) {
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent.as_std_path()).is_err() {
        return;
    }
    let Ok(bytes) = serde_json::to_vec(value) else {
        return;
    };
    let Ok(mut tmp) = tempfile::NamedTempFile::new_in(parent.as_std_path()) else {
        return;
    };
    if std::io::Write::write_all(&mut tmp, &bytes).is_err() {
        return;
    }
    let _ = tmp.persist(path.as_std_path());
}

impl Cache {
    /// Infallible: an unusable location yields an inactive cache.
    pub fn open(root: &Root, enabled: bool, key: CacheKey) -> Cache {
        let none = || Cache {
            dir: None,
            rows: Mutex::new(FileTable::new()),
        };
        if !enabled {
            return none();
        }
        let dir = root.dir.join(".fairlead").join("cache").join(key.id());
        let described = key.describe();
        let key_file = dir.join("key.json");
        if let Some(existing) = read_json::<serde_json::Value>(&key_file)
            && existing != described
        {
            let _ = std::fs::remove_dir_all(dir.as_std_path()); // version skew: discard
        }
        if std::fs::create_dir_all(dir.as_std_path()).is_err() {
            return none();
        }
        write_atomic(&key_file, &described);
        let rows = read_json::<FileTable>(&dir.join("files.json")).unwrap_or_default();
        Cache {
            dir: Some(dir),
            rows: Mutex::new(rows),
        }
    }

    pub fn is_active(&self) -> bool {
        self.dir.is_some()
    }

    pub fn dir(&self) -> Option<&Utf8PathBuf> {
        self.dir.as_ref()
    }

    fn shard(&self, kind: &str, h: &ContentHash) -> Option<Utf8PathBuf> {
        let hex = h.to_hex();
        let first = hex.get(..2)?.to_string();
        Some(
            self.dir
                .as_ref()?
                .join(kind)
                .join(first)
                .join(format!("{hex}.json")),
        )
    }

    pub fn get_extraction(&self, h: &ContentHash) -> Option<Arc<Extraction>> {
        read_json::<Extraction>(&self.shard("x", h)?).map(Arc::new)
    }

    pub fn put_extraction(&self, h: &ContentHash, e: &Extraction) {
        if let Some(p) = self.shard("x", h) {
            write_atomic(&p, e);
        }
    }

    pub fn get_extras(&self, h: &ContentHash, plan: &str) -> Option<Extras> {
        read_json::<Extras>(&self.shard(&format!("e/{plan}"), h)?)
    }

    pub fn put_extras(&self, h: &ContentHash, plan: &str, extras: &Extras) {
        if let Some(p) = self.shard(&format!("e/{plan}"), h) {
            write_atomic(&p, extras);
        }
    }

    pub fn file_row(&self, p: &RepoPath) -> Option<FileRow> {
        self.rows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(p)
            .cloned()
    }

    /// Merge rows into the in-memory table (always) and rewrite `files.json` (when active).
    pub fn put_file_rows(&self, rows: &[(RepoPath, FileRow)]) {
        let mut table = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        for (p, r) in rows {
            table.insert(p.clone(), r.clone());
        }
        if let Some(dir) = &self.dir {
            write_atomic(&dir.join("files.json"), &*table);
        }
    }

    pub fn token_estimate(&self, h: &ContentHash) -> Option<u32> {
        read_json::<u32>(&self.shard("t", h)?)
    }

    pub fn put_token_estimate(&self, h: &ContentHash, tokens: u32) {
        if let Some(p) = self.shard("t", h) {
            write_atomic(&p, &tokens);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{CssTokenSite, Extras};
    use crate::hash::hash_raw;
    use crate::testutil::{test_root, utf8_tmp};
    use crate::time::AsOf;
    use fairlead_syntax::delimiters::CommentTable;
    use fairlead_syntax::diag::Span;
    use fairlead_syntax::extract::ExtractConfig;

    fn key(version: &str, grammar: u32, cfg: &[u8], as_of: Option<&str>) -> CacheKey {
        CacheKey {
            fairlead_version: version.into(),
            grammar,
            config_hash: hash_raw(cfg),
            as_of: as_of.map(|s| AsOf::parse(s).unwrap()),
        }
    }

    fn sample_extraction() -> Extraction {
        let cfg = ExtractConfig {
            marker: "fairlead".into(),
            comments: CommentTable::builtin(),
            header_skip: vec![],
            max_file: 2_000_000,
            max_directives: 500,
        };
        fairlead_syntax::extract::extract(b"// fairlead: def cmp.a\nfn a() {}\n", "rs", &cfg)
    }

    #[test]
    fn r_arch_070__cache_lives_under_dot_fairlead_and_disabled_touches_nothing() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let c = Cache::open(&root, true, key("0.1.0", 1, b"c", None));
        assert!(c.is_active());
        assert!(
            c.dir().unwrap().starts_with(t.join(".fairlead")),
            "{:?}",
            c.dir()
        );
        let (_g2, t2) = utf8_tmp();
        let off = Cache::open(&test_root(&t2), false, key("0.1.0", 1, b"c", None));
        assert!(!off.is_active());
        assert!(
            !t2.join(".fairlead").exists(),
            "a disabled cache creates nothing"
        );
        let h = hash_raw(b"x");
        off.put_extraction(&h, &sample_extraction());
        assert!(off.get_extraction(&h).is_none());
    }

    #[test]
    fn r_arch_073__extraction_roundtrips_as_readable_json() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"file");
        let e = sample_extraction();
        assert!(c.get_extraction(&h).is_none());
        c.put_extraction(&h, &e);
        assert_eq!(*c.get_extraction(&h).unwrap(), e);
        let hex = h.to_hex();
        let path = c
            .dir()
            .unwrap()
            .join("x")
            .join(&hex[..2])
            .join(format!("{hex}.json"));
        let text = std::fs::read_to_string(path.as_std_path()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            v.get("directives").is_some(),
            "plain JSON, no binary format: {text}"
        );
        assert!(text.contains('\n') || text.len() < 4000);
    }

    #[test]
    fn r_arch_072__corrupt_entries_are_discarded_not_errors() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"file");
        c.put_extraction(&h, &sample_extraction());
        let hex = h.to_hex();
        let path = c
            .dir()
            .unwrap()
            .join("x")
            .join(&hex[..2])
            .join(format!("{hex}.json"));
        std::fs::write(path.as_std_path(), b"{ not json").unwrap();
        assert!(c.get_extraction(&h).is_none());
        assert!(
            !path.exists(),
            "the corrupt entry is removed so it is rebuilt"
        );
        c.put_extraction(&h, &sample_extraction());
        assert!(c.get_extraction(&h).is_some());
    }

    #[test]
    fn r_arch_072__unwritable_location_degrades_to_uncached_operation() {
        let (_g, t) = utf8_tmp();
        std::fs::write(
            t.join(".fairlead").as_std_path(),
            b"i am a file, not a directory",
        )
        .unwrap();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        assert!(!c.is_active());
        let h = hash_raw(b"x");
        c.put_extraction(&h, &sample_extraction());
        assert!(c.get_extraction(&h).is_none());
        c.put_file_rows(&[(
            RepoPath::new("a.rs").unwrap(),
            FileRow {
                size: 1,
                mtime_ns: 2,
                hash: h,
            },
        )]);
        assert!(
            c.file_row(&RepoPath::new("a.rs").unwrap()).is_some(),
            "rows still work in memory for this run"
        );
    }

    #[test]
    fn r_arch_072__corrupt_file_table_is_rebuilt_empty() {
        let (_g, t) = utf8_tmp();
        let k = key("0.1.0", 1, b"c", None);
        let c = Cache::open(&test_root(&t), true, k.clone());
        std::fs::write(c.dir().unwrap().join("files.json").as_std_path(), b"[[[").unwrap();
        let again = Cache::open(&test_root(&t), true, k);
        assert!(again.is_active());
        assert!(again.file_row(&RepoPath::new("a.rs").unwrap()).is_none());
    }

    #[test]
    fn r_arch_074__writes_are_atomic_and_concurrent_writers_are_harmless() {
        let (_g, t) = utf8_tmp();
        let c = std::sync::Arc::new(Cache::open(
            &test_root(&t),
            true,
            key("0.1.0", 1, b"c", None),
        ));
        let h = hash_raw(b"same");
        let e = sample_extraction();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (c, e) = (std::sync::Arc::clone(&c), e.clone());
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        c.put_extraction(&h, &e);
                        if let Some(got) = c.get_extraction(&h) {
                            assert_eq!(*got, e, "readers only ever see complete files");
                        }
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let hex = h.to_hex();
        let dir = c.dir().unwrap().join("x").join(&hex[..2]);
        let names: Vec<String> = std::fs::read_dir(dir.as_std_path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [format!("{hex}.json")],
            "no temporary files are left behind"
        );
    }

    #[test]
    fn r_arch_077__entries_are_valid_only_for_the_exact_key_tuple() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let h = hash_raw(b"f");
        let base = key("0.1.0", 1, b"c", None);
        Cache::open(&root, true, base.clone()).put_extraction(&h, &sample_extraction());
        assert!(
            Cache::open(&root, true, base.clone())
                .get_extraction(&h)
                .is_some()
        );
        for other in [
            key("0.1.1", 1, b"c", None),
            key("0.1.0", 2, b"c", None),
            key("0.1.0", 1, b"d", None),
            key("0.1.0", 1, b"c", Some("2026-10-09")),
        ] {
            assert_ne!(other.id(), base.id());
            assert!(Cache::open(&root, true, other).get_extraction(&h).is_none());
        }
        assert_ne!(
            key("0.1.0", 1, b"c", Some("2026-10-09")).id(),
            key("0.1.0", 1, b"c", Some("2026-10-10")).id()
        );
        assert_eq!(
            key("0.1.0", 1, b"c", Some("2026-10-09")).id(),
            key("0.1.0", 1, b"c", Some("2026-10-09")).id()
        );
        assert_eq!(base.id().len(), 20);
    }

    #[test]
    fn r_arch_071__file_rows_roundtrip_through_disk_including_huge_mtimes() {
        let (_g, t) = utf8_tmp();
        let root = test_root(&t);
        let k = key("0.1.0", 1, b"c", None);
        let rows = vec![
            (
                RepoPath::new("a.rs").unwrap(),
                FileRow {
                    size: 3,
                    mtime_ns: 1_700_000_000_123_456_789,
                    hash: hash_raw(b"a"),
                },
            ),
            (
                RepoPath::new("b/c.rs").unwrap(),
                FileRow {
                    size: 0,
                    mtime_ns: -5,
                    hash: hash_raw(b"b"),
                },
            ),
            (
                RepoPath::new("d.rs").unwrap(),
                FileRow {
                    size: 9,
                    mtime_ns: i128::MAX / 2,
                    hash: hash_raw(b"d"),
                },
            ),
        ];
        Cache::open(&root, true, k.clone()).put_file_rows(&rows);
        let again = Cache::open(&root, true, k);
        for (p, row) in &rows {
            assert_eq!(again.file_row(p).as_ref(), Some(row));
        }
        assert!(again.file_row(&RepoPath::new("zzz").unwrap()).is_none());
    }

    #[test]
    fn r_arch_071__token_estimates_and_extras_are_keyed_by_hash_and_plan() {
        let (_g, t) = utf8_tmp();
        let c = Cache::open(&test_root(&t), true, key("0.1.0", 1, b"c", None));
        let h = hash_raw(b"f");
        assert_eq!(c.token_estimate(&h), None);
        c.put_token_estimate(&h, 42);
        assert_eq!(c.token_estimate(&h), Some(42));
        let extras = Extras {
            css_tokens: vec![CssTokenSite {
                name: "--a".into(),
                value: "1".into(),
                context: ":root".into(),
                span: Span {
                    start: 0,
                    end: 3,
                    line: 1,
                    col: 1,
                },
                in_foreign: false,
            }],
            ..Extras::default()
        };
        c.put_extras(&h, "plan-a", &extras);
        assert_eq!(c.get_extras(&h, "plan-a"), Some(extras));
        assert_eq!(c.get_extras(&h, "plan-b"), None);
    }
}
