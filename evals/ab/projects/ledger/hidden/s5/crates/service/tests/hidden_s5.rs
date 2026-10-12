use ledger_core::limits::MAX_MEMO_LEN;
use ledger_service::Ledger;

#[test]
fn s5_memo_length_is_enforced() {
    let mut l = Ledger::new();
    l.open("a", "A", None).unwrap();
    l.open("b", "B", None).unwrap();
    l.deposit("a", 1_000).unwrap();
    assert!(l.transfer("a", "b", 1, &"m".repeat(MAX_MEMO_LEN)).is_ok());
    let e = l.transfer("a", "b", 1, &"m".repeat(MAX_MEMO_LEN + 1)).unwrap_err();
    assert!(e.code().starts_with('E'), "{}", e.code());
    assert_eq!(l.balance("b").unwrap().cents, 1);
}
