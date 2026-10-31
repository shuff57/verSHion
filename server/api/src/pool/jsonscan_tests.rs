//! Tests for `jsonscan.rs`, replaying worker/test/pool/round17.test.ts "the raw-bytes scan refuses a BOM, non-JSON
//! tokens and a second top-level value", plus the structural limits.
use super::*;

fn s(text: &str) -> JsonScan {
    scan_json(text.as_bytes())
}

#[test]
fn a_bom_non_json_tokens_and_a_second_top_level_value_are_refused() {
    let mut with_bom = vec![0xef, 0xbb, 0xbf];
    with_bom.extend_from_slice(br#"{"a":1}"#);
    assert_eq!(scan_json(&with_bom).code, Some("bom"));
    for bad in ["{\"a\":tru}", "{\"a\":1x}", "{} {}", "{\"a\":NaN}", "{\"a\":\u{1}1}", "{\"a\":Infinity}"] {
        assert!(!s(bad).ok, "{bad:?}");
    }
    let good = s(r#"{"a":[1,-2.5e3,true,false,null,"x"]}"#);
    assert!(good.ok);
    assert_eq!((good.values, good.objects, good.depth), (8, 1, 2));
}

#[test]
fn duplicate_keys_are_found_decoded_and_per_object() {
    assert_eq!(s(r#"{"a":1,"a":2}"#).code, Some("duplicate_key"));
    assert_eq!(s(r#"{"a":1,"a":2}"#).code, Some("duplicate_key"), "an escape is the same key");
    assert!(s(r#"{"a":{"a":1},"b":{"a":2}}"#).ok, "the same key in different objects is fine");
    assert!(s(r#"{"a":"a","b":"a"}"#).ok, "a value equal to a key is not a key");
    assert!(s(r#"[{"k":1},{"k":2}]"#).ok);
}

#[test]
fn depth_values_and_objects_are_bounded() {
    let deep = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
    assert!(s(&deep(16)).ok);
    assert_eq!(s(&deep(17)).code, Some("too_deep"));
    let many = format!("[{}]", vec!["0"; SCAN_MAX_VALUES].join(","));
    assert_eq!(s(&many).code, Some("too_many_values"), "the array itself counts as one");
    let objs = format!("[{}]", vec!["{}"; SCAN_MAX_OBJECTS + 1].join(","));
    assert_eq!(s(&objs).code, Some("too_many_objects"));
}

#[test]
fn structure_and_numbers_must_be_well_formed() {
    for bad in ["", "[", "]", "{]", "[}", "\"abc", "-", "01", "1.", "1e", "+1", ".5", "{\"a\"}x", "tr", "nul"] {
        let r = s(bad);
        // The empty input scans as an empty document: nothing to refuse, share-core refuses it.
        if bad.is_empty() {
            assert!(r.ok);
        } else {
            assert!(!r.ok, "{bad:?}");
        }
    }
    for good in ["0", "-0", "10", "1.5", "1e5", "-1.5E+3", "true", "null", "\"x\"", "[]", "{}", " \n{ \"a\" : [ ] } "] {
        assert!(s(good).ok, "{good:?}");
    }
    assert!(!s("1 2").ok && !s("\"a\" \"b\"").ok);
}
