//! GET /items (worker/src/pool/read.ts `list`): browse the public pool (or the caller's own items), newest first, with
//! filters, a word / tag shortlist and a signed cursor. HTTP-free: the route passes the raw query-string values.
//!
//! Visibility, holds, removal, author standing, the filters and the LIMIT are ONE statement. Pages are keyed on the
//! immutable creation time (a republish never moves an item across a cursor).
use super::ctx::{PoolCtx, Res, bad};
use super::itemview::kinds_of;
use super::license::is_license;
use super::licensecheck::license_check_of;
use super::poolkeys::PoolKeys;
use super::sqlfrag::visible_to;
use super::textutil::{fold, search_key};
use rusqlite::params;
use serde_json::{Value, json};

/// Publishes the word index may hand one browse query: the bound on a search's walk.
pub const SEARCH_CANDIDATES: usize = 5000;
/// Word-index rows one browse query looks at before the shortlist.
pub const SEARCH_EXAMINED: usize = 50_000;

const KINDS: [(&str, i64); 6] = [("Mc", 1), ("Tf", 2), ("Matching", 4), ("Blank", 8), ("Short", 16), ("Multipart", 32)];

/// The raw query-string values of a browse request.
#[derive(Default, Clone, Debug)]
pub struct ListQuery {
    pub scope: Option<String>,
    pub limit: Option<String>,
    pub subject: Option<String>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub standard: Option<String>,
    pub q: Option<String>,
    pub license: Option<String>,
    pub cursor: Option<String>,
}

fn utf16(s: &str) -> usize {
    s.encode_utf16().count()
}
/// JS `Number(s)` for the strings a limit can plausibly be.
fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() { 0.0 } else { t.parse::<f64>().unwrap_or(f64::NAN) }
}
fn between(v: &Option<String>, max: usize) -> bool {
    v.as_ref().is_none_or(|s| !s.is_empty() && utf16(s) <= max)
}

pub fn list(x: &PoolCtx, keys: &PoolKeys, me: &str, q: &ListQuery, candidates: usize) -> Res<Value> {
    let scope = q.scope.as_deref().unwrap_or("public");
    if scope != "public" && scope != "mine" {
        return Err(bad("scope").into());
    }
    let limit = q.limit.as_deref().map(js_number).unwrap_or(20.0);
    if !(limit.is_finite() && limit.fract() == 0.0 && limit >= 1.0) {
        return Err(bad("limit").into());
    }
    let lim = limit.min(50.0) as usize;
    if !between(&q.subject, 100) {
        return Err(bad("subject").into());
    }
    let bit = match q.kind.as_deref() {
        None => 0,
        Some(k) => KINDS.iter().find(|(n, _)| *n == k).map(|(_, b)| *b).ok_or_else(|| bad("kind"))?,
    };
    if !between(&q.tag, 64) {
        return Err(bad("tag").into());
    }
    let tag = q.tag.as_deref().map(|t| t.trim().to_lowercase());
    if !between(&q.standard, 64) {
        return Err(bad("standard").into());
    }
    let standard = q.standard.as_deref().map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase());
    if !between(&q.q, 100) {
        return Err(bad("q").into());
    }
    // The start of any word of the title, case- and accent-insensitive: the query folds like the stored column
    // (search_key); it also matches the start of a tag.
    let q_key = q.q.as_deref().map(search_key);
    if q_key.as_deref() == Some("") {
        return Err(bad("q").into());
    }
    let mut q_words: Vec<String> = Vec::new();
    for w in q_key.as_deref().unwrap_or("").split(' ').filter(|w| !w.is_empty()) {
        if !q_words.iter().any(|x| x == w) {
            q_words.push(w.to_string());
        }
    }
    q_words.truncate(6);
    let license = q.license.as_deref();
    if license.is_some_and(|l| !is_license(l)) {
        return Err(bad("license").into());
    }
    let cur = match q.cursor.as_deref() {
        Some(c) => Some(keys.read_cursor(c).ok_or_else(|| bad("cursor"))?),
        None => None,
    };
    x.require_daily(me, "browse", 1)?;

    let mine = scope == "mine";
    let (pub_col, author_guard) = if mine {
        ("current_publish_id", "ci.author_id = ?1")
    } else {
        ("public_publish_id", "ci.visibility = 'public' AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = ci.id)")
    };
    let mut wh = vec![if mine {
        "i.author_id = ?1 AND i.removed_at IS NULL AND i.current_publish_id IS NOT NULL".to_string()
    } else {
        format!("i.visibility = 'public' AND i.removed_at IS NULL AND {}", visible_to("?1"))
    }];
    wh.extend([
        "(?2 IS NULL OR i.subject_fold = ?2)".to_string(),
        "(?3 = 0 OR (i.kinds_mask & ?3) <> 0)".to_string(),
        "(?4 IS NULL OR i.created_at < ?4 OR (i.created_at = ?4 AND i.id < ?5))".to_string(),
        "(?7 IS NULL OR EXISTS (SELECT 1 FROM item_tags t WHERE t.publish_id = i.current_publish_id AND (t.tag = ?7 OR t.fold = ?11)))".to_string(),
        "(?8 IS NULL OR EXISTS (SELECT 1 FROM item_standards s WHERE s.publish_id = i.current_publish_id AND s.code = ?8))".to_string(),
        // q: every query word starts a word of the title, the subject, a tag, or the SERVED question text (served_words,
        // keyed on the publish non-authors are served; the full form is never indexed). The walk starts from the word
        // index (the longest query word, at most SEARCH_CANDIDATES publishes), never from the whole item list.
        format!(
            "(json_array_length(?12) = 0 OR NOT EXISTS (SELECT 1 FROM json_each(?12) qw WHERE NOT EXISTS (
            SELECT 1 FROM served_words sw WHERE sw.publish_id = {}
               AND sw.word >= qw.value AND sw.word < qw.value || char(1114111))))",
            if mine { "i.current_publish_id" } else { "i.public_publish_id" }
        ),
        "(?13 IS NULL OR i.license = ?13)".to_string(),
    ]);
    let driver = q_words.iter().fold("", |a, b| if b.len() > a.len() { b } else { a });
    // The shortlist keeps only publishes that can be shown (the served publish of a public, live, unheld item; for
    // "mine", the author's current ones), newest first, so neither the prefix nor unlisted, held or old publishes decide
    // which candidates are kept. At most SEARCH_EXAMINED index rows are looked at.
    let shortlist = |source: &str| {
        format!(
            "(SELECT ci.{pub_col} AS publish_id
            FROM (SELECT DISTINCT publish_id FROM {source} LIMIT {SEARCH_EXAMINED}) x
            JOIN items ci ON ci.{pub_col} = x.publish_id
           WHERE ci.removed_at IS NULL AND ci.current_publish_id IS NOT NULL
             AND {author_guard}
           ORDER BY ci.created_at DESC, ci.id DESC LIMIT {candidates}) cand
         CROSS JOIN items i ON i.{pub_col} = cand.publish_id JOIN users u ON u.id = i.author_id"
        )
    };
    let from = if !q_words.is_empty() {
        shortlist("served_words WHERE word >= ?9 AND word < ?9 || char(1114111)")
    } else if tag.is_some() {
        shortlist("item_tags WHERE fold = ?11")
    } else {
        "items i JOIN users u ON u.id = i.author_id".to_string()
    };
    let sql = format!(
        "SELECT i.id, i.kind, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              CASE WHEN i.author_id = ?1 THEN i.version ELSE i.public_version END AS version, i.hide_key, i.license,
              i.has_images, CASE WHEN i.author_id = ?1 THEN i.committed_at ELSE COALESCE(i.public_at, i.committed_at) END AS committed_at,
              i.created_at, i.visibility, i.author_id, u.display_name AS author_name
         FROM {from}
        WHERE {}
        ORDER BY i.created_at DESC, i.id DESC LIMIT ?6",
        wh.join(" AND ")
    );
    struct Row {
        id: String,
        kind: String,
        title: String,
        subject: String,
        course: String,
        grade: String,
        kinds_mask: i64,
        question_count: i64,
        version: i64,
        hide_key: i64,
        license: String,
        has_images: i64,
        committed_at: i64,
        created_at: i64,
        visibility: String,
        author_id: String,
        author_name: Option<String>,
    }
    let mut st = x.conn.prepare(&sql)?;
    let rows: Vec<Row> = st
        .query_map(
            params![
                me,
                q.subject.as_deref().map(fold),
                bit,
                cur.as_ref().map(|c| c.0),
                cur.as_ref().map(|c| c.1.as_str()).unwrap_or(""),
                (lim + 1) as i64,
                tag,
                standard,
                driver,
                Option::<String>::None,
                q.tag.as_deref().map(search_key),
                serde_json::to_string(&q_words).unwrap(),
                license
            ],
            |r| {
                Ok(Row {
                    id: r.get("id")?,
                    kind: r.get("kind")?,
                    title: r.get("title")?,
                    subject: r.get("subject")?,
                    course: r.get("course")?,
                    grade: r.get("grade")?,
                    kinds_mask: r.get("kinds_mask")?,
                    question_count: r.get("question_count")?,
                    version: r.get("version")?,
                    hide_key: r.get("hide_key")?,
                    license: r.get("license")?,
                    has_images: r.get("has_images")?,
                    committed_at: r.get("committed_at")?,
                    created_at: r.get("created_at")?,
                    visibility: r.get("visibility")?,
                    author_id: r.get("author_id")?,
                    author_name: r.get("author_name")?,
                })
            },
        )?
        .collect::<Result<_, _>>()?;
    let page = &rows[..rows.len().min(lim)];
    // Each result carries the reader-visible licence marker, as a read would.
    let ids: Vec<String> = page.iter().map(|r| r.id.clone()).collect();
    let checks = license_check_of(x.conn, &ids)?;
    let next = if rows.len() > lim { page.last().map(|l| keys.sign_cursor(l.created_at, &l.id)) } else { None };
    let items: Vec<Value> = page
        .iter()
        .map(|r| {
            let mut v = json!({
                "id": r.id, "kind": r.kind, "title": r.title, "subject": r.subject, "course": r.course, "grade": r.grade,
                "kinds": kinds_of(r.kinds_mask), "question_count": r.question_count, "version": r.version, "hide_key": r.hide_key == 1,
                "license": r.license, "has_images": r.has_images == 1,
                "author_name": r.author_name.clone().unwrap_or_else(|| "deleted teacher".into()), "committed_at": r.committed_at,
            });
            if let Some(c) = checks.get(&r.id) {
                v["license_check"] = serde_json::to_value(c).unwrap();
            }
            if r.author_id == me {
                v["visibility"] = json!(r.visibility);
                v["is_author"] = json!(true);
            }
            v
        })
        .collect();
    Ok(json!({ "items": items, "next_cursor": next }))
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
