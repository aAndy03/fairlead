//! The notes store.
use std::collections::BTreeMap;

use crate::limits;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
}

#[derive(Default)]
pub struct Store {
    notes: BTreeMap<u64, Note>,
    next: u64,
}

impl Store {
    pub fn create(&mut self, title: &str, body: &str) -> Result<Note, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("empty-title".into());
        }
        if title.chars().count() > limits::MAX_TITLE_LEN {
            return Err("title-too-long".into());
        }
        if body.chars().count() > limits::MAX_BODY_LEN {
            return Err("body-too-long".into());
        }
        self.next += 1;
        let note = Note { id: self.next, title: title.to_owned(), body: body.to_owned() };
        self.notes.insert(note.id, note.clone());
        Ok(note)
    }

    pub fn get(&self, id: u64) -> Option<&Note> {
        self.notes.get(&id)
    }

    pub fn delete(&mut self, id: u64) -> bool {
        self.notes.remove(&id).is_some()
    }

    /// Every note, oldest first.
    pub fn list(&self) -> Vec<&Note> {
        self.notes.values().collect()
    }
}
