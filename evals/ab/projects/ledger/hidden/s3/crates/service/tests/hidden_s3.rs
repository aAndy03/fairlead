use ledger_service::Ledger;

#[test]
fn s3_accounts_deposits_withdrawals_transfers_statements() {
    let mut l = Ledger::new();
    l.open("alice", "Alice", None).unwrap();
    l.open("bob", "Bob", Some("EUR")).unwrap();
    l.deposit("alice", 5_000).unwrap();
    let t = l.transfer("alice", "bob", 1_200, "rent").unwrap();
    assert!(t > 0);
    assert_eq!(l.balance("alice").unwrap().cents, 3_800);
    assert_eq!(l.balance("bob").unwrap().to_string(), "12.00 EUR");
    assert_eq!(l.withdraw("bob", 1_200).unwrap().cents, 0, "the whole balance can be withdrawn");
    assert!(l.withdraw("bob", 1).is_err());
    assert!(!l.statement("alice").unwrap().is_empty());
}

#[test]
fn s3_currencies_and_funds() {
    let mut l = Ledger::new();
    l.open("a", "A", None).unwrap();
    l.open("u", "U", Some("USD")).unwrap();
    l.deposit("a", 100).unwrap();
    assert!(l.transfer("a", "u", 1, "x").is_err(), "one currency per transfer");
    l.open("b", "B", None).unwrap();
    assert!(l.transfer("a", "b", 101, "x").is_err(), "no overdraft");
    assert!(l.open("a", "Again", None).is_err());
}
