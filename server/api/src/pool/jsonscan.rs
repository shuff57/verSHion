//! A scan of the RAW publish bytes before any parse. A JSON parse silently keeps the last of duplicate keys and recurses
//! without a depth limit, so the server's own read of a payload could see something other than what share-core
//! validates, or exhaust memory or the stack first. The scan applies share-core's structural limits (depth, values,
//! objects, duplicate keys) to the bytes; a payload that fails goes to share-core unparsed, whose refusal is the
//! answer. Port of worker/src/pool/jsonscan.ts.
use std::collections::HashSet;

#[derive(Debug, PartialEq, Eq)]
pub struct JsonScan {
    pub ok: bool,
    /// share-core's code for the first problem (too_deep, duplicate_key, too_many_values, too_many_objects, invalid, bom).
    pub code: Option<&'static str>,
    pub values: usize,
    pub objects: usize,
    pub depth: usize,
}

/// share-core's defaults (limits.rs): the server never accepts more than share-core would.
pub const SCAN_MAX_DEPTH: usize = 16;
pub const SCAN_MAX_VALUES: usize = 200_000;
pub const SCAN_MAX_OBJECTS: usize = 60_000;
/// Heap per parsed value (an object slot, a number or a short string), measured order of magnitude.
pub const PARSE_BYTES_PER_VALUE: usize = 96;

fn ws(b: u8) -> bool {
    matches!(b, 0x20 | 0x09 | 0x0a | 0x0d)
}
fn token_start(b: u8) -> bool {
    b == b'-' || b.is_ascii_digit() || matches!(b, b't' | b'f' | b'n')
}
fn token_byte(b: u8) -> bool {
    matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b't' | b'r' | b'u' | b'f' | b'a' | b'l' | b's' | b'n') || b.is_ascii_digit()
}

/// `^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$`
fn is_number(t: &[u8]) -> bool {
    let mut i = 0;
    if t.get(i) == Some(&b'-') {
        i += 1;
    }
    match t.get(i) {
        Some(b'0') => i += 1,
        Some(c) if c.is_ascii_digit() => {
            while t.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => return false,
    }
    if t.get(i) == Some(&b'.') {
        i += 1;
        let s = i;
        while t.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == s {
            return false;
        }
    }
    if matches!(t.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(t.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let s = i;
        while t.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == s {
            return false;
        }
    }
    i == t.len()
}

struct Open {
    obj: bool,
    keys: Option<HashSet<String>>,
    want_key: bool,
}

/// Scans `b` (UTF-8 JSON) without building it. Linear time; memory: the keys of the open objects only.
pub fn scan_json(b: &[u8]) -> JsonScan {
    let (mut values, mut objects, mut max_depth) = (0usize, 0usize, 0usize);
    let mut stack: Vec<Open> = Vec::new();
    macro_rules! fail {
        ($c:expr) => {
            return JsonScan { ok: false, code: Some($c), values, objects, depth: max_depth }
        };
    }
    let n = b.len();
    let mut i = 0;
    // A byte-order mark (or anything but JSON at the top) is refused here: a parse that drops a BOM must never accept
    // bytes share-core refuses.
    if n >= 3 && b[0] == 0xef && b[1] == 0xbb && b[2] == 0xbf {
        fail!("bom");
    }
    let mut tops = 0;
    while i < n {
        let ch = b[i];
        if ws(ch) || ch == b':' {
            i += 1;
            continue;
        }
        if ch == b',' {
            if let Some(top) = stack.last_mut() {
                if top.obj {
                    top.want_key = true;
                }
            }
            i += 1;
            continue;
        }
        if ch == b'"' {
            let start = i;
            i += 1;
            while i < n && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            if i >= n {
                fail!("invalid");
            }
            i += 1;
            if let Some(top) = stack.last_mut() {
                if top.obj && top.want_key {
                    // A key: compared decoded, as share-core compares them ("a" and "a" are one key).
                    let raw = String::from_utf8_lossy(&b[start..i]);
                    let Ok(key) = serde_json::from_str::<String>(&raw) else { fail!("invalid") };
                    if !top.keys.as_mut().expect("objects hold keys").insert(key) {
                        fail!("duplicate_key");
                    }
                    top.want_key = false;
                    continue;
                }
            } else {
                tops += 1;
                if tops > 1 {
                    fail!("invalid");
                }
            }
            values += 1;
            if values > SCAN_MAX_VALUES {
                fail!("too_many_values");
            }
            continue;
        }
        if ch == b'{' || ch == b'[' {
            if stack.is_empty() {
                tops += 1;
                if tops > 1 {
                    fail!("invalid");
                }
            }
            values += 1;
            if values > SCAN_MAX_VALUES {
                fail!("too_many_values");
            }
            let obj = ch == b'{';
            if obj {
                objects += 1;
                if objects > SCAN_MAX_OBJECTS {
                    fail!("too_many_objects");
                }
            }
            stack.push(Open { obj, keys: obj.then(HashSet::new), want_key: obj });
            max_depth = max_depth.max(stack.len());
            if stack.len() > SCAN_MAX_DEPTH {
                fail!("too_deep");
            }
            i += 1;
            continue;
        }
        if ch == b'}' || ch == b']' {
            match stack.last() {
                Some(top) if top.obj == (ch == b'}') => {
                    stack.pop();
                }
                _ => fail!("invalid"),
            }
            i += 1;
            continue;
        }
        // A number, true, false or null: one value up to the next delimiter, made only of the bytes those tokens use
        // (anything else, e.g. a stray letter or a control byte, is refused).
        if !token_start(ch) {
            fail!("invalid");
        }
        values += 1;
        if values > SCAN_MAX_VALUES {
            fail!("too_many_values");
        }
        let t0 = i;
        while i < n && !ws(b[i]) && !matches!(b[i], b',' | b'}' | b']' | b':') {
            if !token_byte(b[i]) {
                fail!("invalid");
            }
            i += 1;
        }
        let tok = &b[t0..i.min(t0 + 40)];
        if !(tok == b"true" || tok == b"false" || tok == b"null" || is_number(tok)) {
            fail!("invalid");
        }
        if stack.is_empty() {
            tops += 1;
            if tops > 1 {
                fail!("invalid");
            }
        }
    }
    if !stack.is_empty() {
        fail!("invalid");
    }
    JsonScan { ok: true, code: None, values, objects, depth: max_depth }
}

#[cfg(test)]
#[path = "jsonscan_tests.rs"]
mod tests;
