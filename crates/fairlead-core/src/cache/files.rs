//! The file table: one `FileRow` per path (50-architecture Section 6).
use crate::hash::ContentHash;
use crate::path::RepoPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct FileRow {
    pub size: u64,
    #[serde(with = "i128_string")]
    pub mtime_ns: i128,
    pub hash: ContentHash,
}

mod i128_string {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &i128, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i128, D::Error> {
        let s = String::deserialize(d)?;
        s.parse::<i128>().map_err(serde::de::Error::custom)
    }
}

pub(crate) type FileTable = BTreeMap<RepoPath, FileRow>;
