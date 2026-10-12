use ledger_core::{limits, AccountId, Currency, LedgerError, Money};

#[test]
fn s1_limits() {
    assert_eq!(limits::MAX_AMOUNT_CENTS, 100_000_000);
    assert_eq!(limits::MAX_MEMO_LEN, 140);
    assert_eq!(limits::MAX_ACCOUNT_ID_LEN, 32);
    assert_eq!(limits::DEFAULT_CURRENCY, "EUR");
}

#[test]
fn s1_ids_currencies_money() {
    assert!(AccountId::parse("alice-1").is_ok());
    assert!(AccountId::parse("").is_err());
    assert!(AccountId::parse(&"a".repeat(33)).is_err());
    assert_eq!(Currency::parse("USD").unwrap(), Currency::Usd);
    assert!(Currency::parse("XYZ").is_err());
    assert_eq!(Currency::Gbp.code(), "GBP");
    assert_eq!(Money { cents: 1234, currency: Currency::Eur }.to_string(), "12.34 EUR");
}

#[test]
fn s1_errors_have_codes() {
    // The spec fixes no id format beyond its length limit; an empty id is invalid by any reading.
    let e: LedgerError = AccountId::parse("").unwrap_err();
    let code = e.code();
    assert!(code.len() == 4 && code.starts_with('E') && code[1..].chars().all(|c| c.is_ascii_digit()), "{code}");
}
