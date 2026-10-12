//! The wire format of contract/commands.md: a request is `<command>?<query>`, where the query is
//! `key=value` pairs joined by `&` and percent-encoded; a reply is `ok <payload>` or `err <code>`.
use std::collections::BTreeMap;

pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Splits a request line into its command and arguments.
pub fn parse_request(line: &str) -> (String, BTreeMap<String, String>) {
    let (cmd, query) = line.split_once('?').unwrap_or((line, ""));
    let args = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect();
    (cmd.trim().to_owned(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = "Groceries & more: 5% off!";
        assert_eq!(decode(&encode(s)), s);
        let (cmd, args) = parse_request(&format!("notes.create?title={}&body=x", encode(s)));
        assert_eq!(cmd, "notes.create");
        assert_eq!(args["title"], s);
    }
}
