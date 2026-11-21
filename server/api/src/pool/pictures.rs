//! A question's pictures: its own `image` and its picture choices' (`kind.Mc.answers[i].image`, and the same in every Multipart Mc part).
//! Each is a store file `media/<sha256>.<png|jpg|gif|webp>` and takes the same server path as the question's image: checked verified and the
//! author's, held, rewritten from the blob's record, linked to the publish (`item_blobs`) and privacy-scanned. A text-only question's
//! pictures are exactly its image, so everything keyed on them is unchanged for it.
use super::qjson::Json;
use super::sqlfrag::is_sha;
use serde_json::{Value, json};

/// The sha of a picture reference `{"file": "media/<sha>.<ext>", ...}`.
fn sha_of(file: Option<&str>) -> Option<String> {
    let (sha, ext) = file?.strip_prefix("media/")?.split_once('.')?;
    (is_sha(sha) && matches!(ext, "png" | "jpg" | "gif" | "webp")).then(|| sha.to_string())
}

fn push(out: &mut Vec<String>, s: Option<String>) {
    if let Some(s) = s
        && !out.contains(&s)
    {
        out.push(s);
    }
}

/// The answer lists of a question's Mc kind and of its Multipart Mc parts, in order.
fn answer_lists(q: &Value) -> Vec<&Vec<Value>> {
    let kind = q.get("kind");
    let mut out: Vec<&Vec<Value>> = kind.and_then(|k| k.get("Mc")).and_then(|m| m.get("answers")).and_then(Value::as_array).into_iter().collect();
    for p in kind.and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).and_then(Value::as_array).into_iter().flatten() {
        out.extend(p.get("kind").and_then(|k| k.get("Mc")).and_then(|m| m.get("answers")).and_then(Value::as_array));
    }
    out
}

fn answer_lists_j(q: &Json) -> Vec<&[Json]> {
    let kind = q.get("kind");
    let mut out: Vec<&[Json]> = kind.and_then(|k| k.get("Mc")).and_then(|m| m.get("answers")).map(Json::as_arr).into_iter().collect();
    for p in kind.and_then(|k| k.get("Multipart")).and_then(|m| m.get("parts")).map(Json::as_arr).unwrap_or(&[]) {
        out.extend(p.get("kind").and_then(|k| k.get("Mc")).and_then(|m| m.get("answers")).map(Json::as_arr));
    }
    out
}

/// Whether any choice of the question carries a picture or a figure (a picture choice).
pub fn has_choice_pictures(q: &Value) -> bool {
    let set = |v: Option<&Value>| v.is_some_and(|x| !x.is_null());
    answer_lists(q).into_iter().flatten().any(|a| set(a.get("image")) || set(a.get("figure")))
}

/// The question's own image sha.
pub fn question_sha(q: &Value) -> Option<String> {
    sha_of(q.get("image").and_then(|i| i.get("file")).and_then(Value::as_str))
}

/// Every picture sha of a question: its image first, then its picture choices in order; each once.
pub fn shas(q: &Value) -> Vec<String> {
    let mut out = Vec::new();
    push(&mut out, question_sha(q));
    for list in answer_lists(q) {
        for a in list {
            push(&mut out, sha_of(a.get("image").and_then(|i| i.get("file")).and_then(Value::as_str)));
        }
    }
    out
}

fn file_j(v: &Json) -> Option<&str> {
    v.get("image").and_then(|i| i.get("file")).and_then(Json::as_str)
}

/// [`question_sha`] for the pool's own JSON tree.
pub fn question_sha_j(q: &Json) -> Option<String> {
    sha_of(file_j(q))
}

/// [`shas`] for the pool's own JSON tree.
pub fn shas_j(q: &Json) -> Vec<String> {
    let mut out = Vec::new();
    push(&mut out, question_sha_j(q));
    for list in answer_lists_j(q) {
        for a in list {
            push(&mut out, sha_of(file_j(a)));
        }
    }
    out
}

/// What a question shows of its picture choices, for the key-free hashes: None for a question without any (so its hashes are what they
/// were before picture choices existed), else each choice's text and picture sha, sorted (served choice order is never part of them).
pub fn choice_pictures_j(q: &Json) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut any = false;
    for list in answer_lists_j(q) {
        for a in list {
            let sha = sha_of(file_j(a));
            any |= sha.is_some();
            out.push(format!("{}\u{0}{}", a.get("text").and_then(Json::as_str).unwrap_or(""), sha.unwrap_or_default()));
        }
    }
    out.sort();
    any.then_some(out)
}

/// Rewrites every picture reference of a question (its image and its choices') from the server's blob record: `media/<sha>.<ext of the
/// stored type>` with the stored width and height. A reference whose sha `info` does not know is left as it is.
pub fn rewrite(q: &mut Value, info: &dyn Fn(&str) -> Option<(&'static str, i64, i64)>) {
    let fix = |o: &mut serde_json::Map<String, Value>| {
        let Some(s) = sha_of(o.get("image").and_then(|i| i.get("file")).and_then(Value::as_str)) else { return };
        if let Some((ext, w, h)) = info(&s) {
            o.insert("image".into(), json!({ "file": format!("media/{s}.{ext}"), "width": w, "height": h }));
        }
    };
    let Some(o) = q.as_object_mut() else { return };
    fix(o);
    let Some(kind) = o.get_mut("kind").and_then(Value::as_object_mut) else { return };
    let fix_all = |list: &mut Vec<Value>| list.iter_mut().filter_map(Value::as_object_mut).for_each(fix);
    if let Some(a) = kind.get_mut("Mc").and_then(|m| m.get_mut("answers")).and_then(Value::as_array_mut) {
        fix_all(a);
    }
    if let Some(parts) = kind.get_mut("Multipart").and_then(|m| m.get_mut("parts")).and_then(Value::as_array_mut) {
        for p in parts {
            if let Some(a) = p.get_mut("kind").and_then(|k| k.get_mut("Mc")).and_then(|m| m.get_mut("answers")).and_then(Value::as_array_mut) {
                fix_all(a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha(c: char) -> String {
        c.to_string().repeat(64)
    }

    fn pictured() -> Value {
        json!({
            "image": { "file": format!("media/{}.png", sha('a')), "width": 1, "height": 1 },
            "kind": { "Mc": { "answers": [
                { "text": "", "correct": true, "locked": false, "image": { "file": format!("media/{}.webp", sha('b')), "width": 9, "height": 9 } },
                { "text": "x", "correct": false, "locked": false },
                { "text": "", "correct": false, "locked": false, "image": { "file": format!("media/{}.png", sha('a')), "width": 1, "height": 1 } },
            ] } }
        })
    }

    #[test]
    fn a_questions_pictures_are_its_image_then_its_choices_once_each() {
        assert_eq!(shas(&pictured()), [sha('a'), sha('b')]);
        let j = Json::parse(pictured().to_string().as_bytes()).unwrap();
        assert_eq!(shas_j(&j), [sha('a'), sha('b')]);
        let parts = json!({ "kind": { "Multipart": { "parts": [
            { "stem": "p", "kind": { "Tf": { "correct": true } } },
            { "stem": "q", "kind": { "Mc": { "answers": [{ "text": "", "image": { "file": format!("media/{}.gif", sha('c')), "width": 2, "height": 2 } }] } } },
        ] } } });
        assert_eq!(shas(&parts), [sha('c')]);
        assert_eq!(shas(&json!({ "kind": { "Mc": { "answers": [{ "text": "a" }, { "text": "b" }] } } })), Vec::<String>::new());
        assert_eq!(shas(&json!({ "image": { "file": "media/nothex.png" } })), Vec::<String>::new());
    }

    #[test]
    fn text_choices_leave_the_key_free_picture_facts_empty() {
        let text = Json::parse(br#"{"kind":{"Mc":{"answers":[{"text":"a"},{"text":"b"}]}}}"#).unwrap();
        assert_eq!(choice_pictures_j(&text), None);
        let j = Json::parse(pictured().to_string().as_bytes()).unwrap();
        let facts = choice_pictures_j(&j).unwrap();
        assert_eq!(facts.len(), 3);
        assert!(facts.windows(2).all(|w| w[0] <= w[1]), "sorted: the served order is not part of it");
    }

    #[test]
    fn rewrite_fixes_every_picture_from_the_blob_record() {
        let mut q = pictured();
        rewrite(&mut q, &|s| (s == sha('b')).then_some(("png", 40, 30)));
        let a = &q["kind"]["Mc"]["answers"];
        assert_eq!(a[0]["image"], json!({ "file": format!("media/{}.png", sha('b')), "width": 40, "height": 30 }));
        assert_eq!(q["image"]["file"], json!(format!("media/{}.png", sha('a'))), "an unknown sha is left alone");
        assert!(a[1].get("image").is_none(), "a text choice gains nothing");
    }
}
