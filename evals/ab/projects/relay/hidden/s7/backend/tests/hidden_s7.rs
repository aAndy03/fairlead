use relay_backend::{notes::Store, reply};

/// The records of a list payload (`ok id=1&title=a;id=2&title=b`) as (id, title) pairs, decoded.
fn records(r: &str) -> Vec<(String, String)> {
    let payload = r.strip_prefix("ok").unwrap_or("").trim();
    payload
        .split(';')
        .filter(|x| !x.is_empty())
        .map(|rec| {
            let mut id = String::new();
            let mut title = String::new();
            for f in rec.split('&') {
                let (k, v) = f.split_once('=').unwrap_or((f, ""));
                if k == "id" {
                    id = decode(v);
                }
                if k == "title" {
                    title = decode(v);
                }
            }
            (id, title)
        })
        .collect()
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 + 1 && i + 2 <= b.len() - 1 {
            if let Ok(x) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(x);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn create(s: &mut Store, title: &str) -> String {
    let r = reply(s, &format!("notes.create?title={}", enc(title)));
    assert!(r.starts_with("ok"), "{r}");
    r.split("id=").nth(1).unwrap_or("").split(['&', ';', ' ']).next().unwrap_or("").to_owned()
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[test]
fn s7_search_by_a_word_in_title_or_body() {
    let mut s = Store::default();
    // Compare ids: the spec fixes the records of notes.list only.
    let id_of = |r: String| r.split("id=").nth(1).unwrap_or("").split(['&', ';', ' ']).next().unwrap_or("").to_owned();
    let g = id_of(reply(&mut s, "notes.create?title=Groceries&body=milk%20and%20bread"));
    let m = id_of(reply(&mut s, "notes.create?title=Milk%20run&body=tomorrow"));
    reply(&mut s, "notes.create?title=Taxes&body=april");
    let mut hits: Vec<String> = records(&reply(&mut s, "notes.search?q=milk")).into_iter().map(|r| r.0).collect();
    hits.sort();
    let mut want = vec![g, m];
    want.sort();
    assert_eq!(hits, want);
    assert!(records(&reply(&mut s, "notes.search?q=zebra")).is_empty());
}
