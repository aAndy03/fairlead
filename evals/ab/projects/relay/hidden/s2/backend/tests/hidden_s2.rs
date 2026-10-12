use relay_backend::{limits, notes::Store, reply};

#[test]
fn s2_the_four_commands_on_the_wire() {
    let mut s = Store::default();
    let created = reply(&mut s, "notes.create?title=Milk&body=two%20litres");
    assert!(created.starts_with("ok ") && created.contains("id="), "{created}");
    let list = reply(&mut s, "notes.list");
    assert!(list.starts_with("ok ") && list.contains("title=Milk"), "{list}");
    let got = reply(&mut s, "notes.get?id=1");
    assert!(got.contains("title=Milk") && got.contains("body=two%20litres"), "{got}");
    assert!(reply(&mut s, "notes.delete?id=1").starts_with("ok"));
    assert_eq!(reply(&mut s, "notes.get?id=1"), "err not-found");
}

#[test]
fn s2_errors() {
    let mut s = Store::default();
    assert_eq!(reply(&mut s, "notes.create?title=%20"), "err empty-title");
    let long = "x".repeat(limits::MAX_TITLE_LEN + 1);
    assert_eq!(reply(&mut s, &format!("notes.create?title={long}")), "err title-too-long");
    assert_eq!(reply(&mut s, "notes.get?id=abc"), "err bad-id");
    assert_eq!(reply(&mut s, "notes.get"), "err missing-arg-id");
    assert_eq!(limits::MAX_BODY_LEN, 2000);
}
