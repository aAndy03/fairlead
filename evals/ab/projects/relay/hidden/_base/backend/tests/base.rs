use relay_backend::{limits, notes::Store, reply};

#[test]
fn base_the_four_commands() {
    let mut s = Store::default();
    assert_eq!(reply(&mut s, "notes.create?title=Milk&body=2%20litres"), "ok id=1");
    assert!(reply(&mut s, "notes.list").contains("title=Milk"));
    assert!(reply(&mut s, "notes.get?id=1").starts_with("ok "));
    assert!(reply(&mut s, "notes.delete?id=1").starts_with("ok"));
    assert_eq!(reply(&mut s, "notes.get?id=1"), "err not-found");
}

#[test]
fn base_limits() {
    let mut s = Store::default();
    let long = "x".repeat(limits::MAX_TITLE_LEN + 1);
    assert_eq!(reply(&mut s, &format!("notes.create?title={long}")), "err title-too-long");
}
