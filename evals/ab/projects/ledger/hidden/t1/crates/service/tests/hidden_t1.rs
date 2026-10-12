use ledger_service::Ledger;

#[test]
fn t1_the_whole_balance_can_be_withdrawn() {
    let mut l = Ledger::new();
    l.open("a", "A", None).unwrap();
    l.deposit("a", 2_500).unwrap();
    assert_eq!(l.withdraw("a", 2_500).unwrap().cents, 0);
    assert_eq!(l.balance("a").unwrap().cents, 0);
    assert_eq!(l.withdraw("a", 1).unwrap_err().code(), "E007", "still no overdraft");
}
