//! One command per line:
//!
//! ```text
//! open <id> <name> [currency]     deposit <id> <cents>     withdraw <id> <cents>
//! transfer <from> <to> <cents> [memo words...]     balance <id>     statement <id>
//! ```
//! Blank lines and lines starting with `#` are ignored.
use ledger_service::Ledger;

fn cents(s: &str) -> Result<i64, String> {
    s.parse::<i64>().map_err(|_| format!("not a whole number of cents: {s}"))
}

pub fn run_line(ledger: &mut Ledger, line: &str) -> Result<Option<String>, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let Some((&cmd, rest)) = words.split_first() else {
        return Ok(None);
    };
    if cmd.starts_with('#') {
        return Ok(None);
    }
    let err = |e: ledger_core::LedgerError| e.to_string();
    match (cmd, rest) {
        ("open", [id, name]) => ledger.open(id, name, None).map(|i| Some(format!("opened {i}"))).map_err(err),
        ("open", [id, name, cur]) => ledger.open(id, name, Some(cur)).map(|i| Some(format!("opened {i}"))).map_err(err),
        ("deposit", [id, c]) => ledger.deposit(id, cents(c)?).map(|m| Some(format!("{id}: {m}"))).map_err(err),
        ("withdraw", [id, c]) => ledger.withdraw(id, cents(c)?).map(|m| Some(format!("{id}: {m}"))).map_err(err),
        ("transfer", [from, to, c, memo @ ..]) => ledger
            .transfer(from, to, cents(c)?, &memo.join(" "))
            .map(|t| Some(format!("transaction {t}")))
            .map_err(err),
        ("balance", [id]) => ledger.balance(id).map(|m| Some(format!("{id}: {m}"))).map_err(err),
        ("statement", [id]) => ledger
            .statement(id)
            .map(|rows| {
                Some(
                    rows.iter()
                        .map(|(t, c, memo)| format!("{t} {c:+} {memo}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })
            .map_err(err),
        _ => Err(format!("unknown command or wrong arguments: {line}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script() {
        let mut l = Ledger::new();
        let script = "open alice Alice\nopen bob Bob\ndeposit alice 5000\ntransfer alice bob 1200 rent for may\nbalance alice\nbalance bob";
        let out: Vec<String> = script.lines().filter_map(|x| run_line(&mut l, x).unwrap()).collect();
        assert_eq!(out.last().unwrap(), "bob: 12.00 EUR");
        assert_eq!(out[4], "alice: 38.00 EUR");
    }

    #[test]
    fn errors_carry_their_code() {
        let mut l = Ledger::new();
        assert!(run_line(&mut l, "balance nobody").unwrap_err().starts_with("E002"));
        assert!(run_line(&mut l, "fly away").is_err());
    }
}
