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
fn s6_tags_and_listing_by_tag() {
    let mut s = Store::default();
    // The spec fixes the records of notes.list only; compare ids, which every record carries.
    let id_of = |r: String| r.split("id=").nth(1).unwrap_or("").split(['&', ';', ' ']).next().unwrap_or("").to_owned();
    let a = id_of(reply(&mut s, "notes.create?title=A&tags=home%2Cwork"));
    let b = id_of(reply(&mut s, "notes.create?title=B&tags=work"));
    reply(&mut s, "notes.create?title=C");
    let mut work: Vec<String> = records(&reply(&mut s, "notes.by-tag?tag=work")).into_iter().map(|r| r.0).collect();
    work.sort();
    let mut want = vec![a.clone(), b];
    want.sort();
    assert_eq!(work, want);
    let home: Vec<String> = records(&reply(&mut s, "notes.by-tag?tag=home")).into_iter().map(|r| r.0).collect();
    assert_eq!(home, vec![a]);
}
