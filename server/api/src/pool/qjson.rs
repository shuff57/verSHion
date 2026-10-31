//! A JSON value that behaves like JavaScript's: objects keep insertion order (integer-like keys first, ascending, as
//! `Object.keys` lists them), numbers are IEEE doubles printed the way `JSON.stringify` prints them, and strings sort by
//! UTF-16 code unit. The pool's identity hashes are `HMAC(JSON.stringify(...))` of question content and are STORED, so the
//! bytes fed to the HMAC must be the Worker's bytes, not merely equivalent JSON.
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Insertion order; a repeated key keeps its first position and takes the last value, as `JSON.parse` does.
    Obj(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Json, E> {
                Ok(Json::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Num(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Num(v as f64))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Json, E> {
                Ok(Json::Num(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::Str(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<Json, E> {
                Ok(Json::Str(v))
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_none<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(Json::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Json, A::Error> {
                let mut v: Vec<(String, Json)> = Vec::new();
                while let Some((k, x)) = a.next_entry::<String, Json>()? {
                    match v.iter_mut().find(|(e, _)| *e == k) {
                        Some(slot) => slot.1 = x,
                        None => v.push((k, x)),
                    }
                }
                Ok(Json::Obj(v))
            }
        }
        d.deserialize_any(V)
    }
}

impl Json {
    pub fn parse(bytes: &[u8]) -> Option<Json> {
        serde_json::from_slice(bytes).ok()
    }
    pub fn obj(entries: Vec<(&str, Json)>) -> Json {
        Json::Obj(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
    pub fn s(v: &str) -> Json {
        Json::Str(v.to_string())
    }
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(e, _)| e == k).map(|(_, x)| x),
            _ => None,
        }
    }
    /// `x.k ?? null`-style read: absent and null are both None.
    pub fn field(&self, k: &str) -> Option<&Json> {
        self.get(k).filter(|v| !matches!(v, Json::Null))
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self { Some(s) } else { None }
    }
    pub fn as_arr(&self) -> &[Json] {
        if let Json::Arr(v) = self { v } else { &[] }
    }
    /// `Object.keys(x)[0]`.
    pub fn first_key(&self) -> Option<&str> {
        match self {
            Json::Obj(v) => ordered_keys(v).first().map(|i| v[*i].0.as_str()),
            _ => None,
        }
    }
    /// JavaScript truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Num(n) => *n != 0.0 && !n.is_nan(),
            Json::Str(s) => !s.is_empty(),
            _ => true,
        }
    }
    /// `JSON.stringify(self)` for a value that is not undefined.
    pub fn stringify(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Num(n) => out.push_str(&js_number(*n)),
            Json::Str(s) => write_str(s, out),
            Json::Arr(v) => {
                out.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    x.write(out);
                }
                out.push(']');
            }
            Json::Obj(v) => {
                out.push('{');
                let mut first = true;
                for i in ordered_keys(v) {
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    write_str(&v[i].0, out);
                    out.push(':');
                    v[i].1.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// The order `Object.keys` lists: canonical array indices ascending, then the rest as inserted.
fn ordered_keys(v: &[(String, Json)]) -> Vec<usize> {
    let idx = |k: &str| -> Option<u64> {
        let ok = !k.is_empty() && k.len() <= 10 && k.bytes().all(|b| b.is_ascii_digit()) && (k == "0" || !k.starts_with('0'));
        ok.then(|| k.parse::<u64>().ok()).flatten().filter(|n| *n < u32::MAX as u64)
    };
    let mut ints: Vec<(u64, usize)> = v.iter().enumerate().filter_map(|(i, (k, _))| idx(k).map(|n| (n, i))).collect();
    ints.sort();
    let mut out: Vec<usize> = ints.into_iter().map(|(_, i)| i).collect();
    out.extend((0..v.len()).filter(|i| idx(&v[*i].0).is_none()));
    out
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `Number.prototype.toString` for a double: integers without a fraction, exponent form from 1e21 and below 1e-6.
pub fn js_number(n: f64) -> String {
    if n.is_nan() || n.is_infinite() {
        return "null".into(); // JSON.stringify
    }
    if n == 0.0 {
        return "0".into();
    }
    let a = n.abs();
    if (1e-6..1e21).contains(&a) {
        // Rust's Display prints the shortest representation that round-trips, in positional notation.
        return format!("{n}");
    }
    // Exponent form: d.ddde+X, shortest digits.
    let e = format!("{n:e}"); // e.g. 1e21, 1.5e-7
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    format!("{mant}e{}{}", if exp < 0 { "-" } else { "+" }, exp.abs())
}

/// JavaScript's default string order: by UTF-16 code unit.
pub fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}
pub fn sort_utf16(v: &mut [String]) {
    v.sort_by(|a, b| cmp_utf16(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_the_way_javascript_prints_them() {
        for (n, want) in [(2.0, "2"), (1.5, "1.5"), (-0.0, "0"), (1e21, "1e+21"), (1e20, "100000000000000000000"), (0.000001, "0.000001"), (0.0000001, "1e-7"), (1.5e-7, "1.5e-7"), (123456789.125, "123456789.125"), (-3.0, "-3"), (f64::NAN, "null")] {
            assert_eq!(js_number(n), want, "{n}");
        }
    }

    #[test]
    fn objects_keep_insertion_order_with_integer_keys_first_and_a_repeated_key_keeps_its_place() {
        let j = Json::parse(br#"{"b":1,"10":2,"a":3,"2":4,"b":9,"01":5}"#).unwrap();
        assert_eq!(j.stringify(), r#"{"2":4,"10":2,"b":9,"a":3,"01":5}"#);
        assert_eq!(j.first_key(), Some("2"));
    }

    #[test]
    fn strings_escape_like_json_stringify_and_sort_by_utf16() {
        assert_eq!(Json::s("a\"b\\c\n\u{1}\u{7f}é\u{2028}").stringify(), "\"a\\\"b\\\\c\\n\\u0001\u{7f}é\u{2028}\"");
        // U+FF21 (BMP, above the surrogates) sorts AFTER an astral character by UTF-16 units, BEFORE it by code point.
        let mut v = vec!["\u{FF21}".to_string(), "\u{1F600}".to_string(), "a".to_string()];
        sort_utf16(&mut v);
        assert_eq!(v, vec!["a".to_string(), "\u{1F600}".to_string(), "\u{FF21}".to_string()]);
    }

    #[test]
    fn truthiness_follows_javascript() {
        assert!(!Json::Null.truthy() && !Json::Num(0.0).truthy() && !Json::s("").truthy() && !Json::Bool(false).truthy());
        assert!(Json::Num(1.0).truthy() && Json::s("0").truthy() && Json::Arr(vec![]).truthy() && Json::Obj(vec![]).truthy());
    }
}
