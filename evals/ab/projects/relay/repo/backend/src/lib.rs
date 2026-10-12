pub mod commands;
pub mod limits;
pub mod notes;
pub mod wire;

/// One request line in, one reply line out.
pub fn reply(store: &mut notes::Store, line: &str) -> String {
    let (cmd, args) = wire::parse_request(line);
    match commands::handle(store, &cmd, &args) {
        Ok(payload) => format!("ok {payload}"),
        Err(e) => format!("err {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_get_delete() {
        let mut s = notes::Store::default();
        assert_eq!(reply(&mut s, "notes.create?title=Milk&body=2%20litres"), "ok id=1");
        assert_eq!(reply(&mut s, "notes.list"), "ok id=1&title=Milk");
        assert_eq!(reply(&mut s, "notes.get?id=1"), "ok id=1&title=Milk&body=2%20litres");
        assert_eq!(reply(&mut s, "notes.delete?id=1"), "ok ");
        assert_eq!(reply(&mut s, "notes.get?id=1"), "err not-found");
    }

    #[test]
    fn titles_are_limited() {
        let mut s = notes::Store::default();
        let long = "x".repeat(limits::MAX_TITLE_LEN + 1);
        assert_eq!(reply(&mut s, &format!("notes.create?title={long}")), "err title-too-long");
        assert_eq!(reply(&mut s, "notes.create?title=%20"), "err empty-title");
    }
}
