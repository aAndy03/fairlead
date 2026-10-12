use ledger_service::Ledger;

#[test]
fn base_deposit_transfer_balance_statement() {
    let mut l = Ledger::new();
    l.open("alice", "Alice", None).unwrap();
    l.open("bob", "Bob", None).unwrap();
    l.deposit("alice", 5_000).unwrap();
    l.transfer("alice", "bob", 1_200, "rent").unwrap();
    assert_eq!(l.balance("alice").unwrap().cents, 3_800);
    assert_eq!(l.balance("bob").unwrap().to_string(), "12.00 EUR");
    let st = l.statement("bob").unwrap();
    assert!(st.iter().any(|(_, c, m)| *c == 1_200 && m == "rent"), "{st:?}");
}

#[test]
fn base_errors_keep_their_codes() {
    let mut l = Ledger::new();
    l.open("a", "A", None).unwrap();
    assert_eq!(l.open("a", "A", None).unwrap_err().code(), "E003");
    assert_eq!(l.balance("nobody").unwrap_err().code(), "E002");
    assert_eq!(l.open("B!", "B", None).unwrap_err().code(), "E001");
    assert_eq!(l.open("c", "C", Some("XYZ")).unwrap_err().code(), "E004");
    l.open("u", "U", Some("USD")).unwrap();
    l.deposit("a", 100).unwrap();
    assert_eq!(l.transfer("a", "u", 1, "x").unwrap_err().code(), "E005");
    assert_eq!(l.deposit("a", 0).unwrap_err().code(), "E006");
    l.open("b", "B", None).unwrap();
    assert_eq!(l.transfer("a", "b", 101, "x").unwrap_err().code(), "E007");
}
