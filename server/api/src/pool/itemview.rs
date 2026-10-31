//! The item and question views of worker/src/pool/read.ts: `loadVisible`, `servedForm`, `itemView`, `questionsFor`. One rule
//! decides what a caller is served (`served_form`): the author gets the full form; anyone else gets the stripped form of a
//! hide_key publish and the full form otherwise. `full_sha256` / `full_content` / `full_key` never leave this module for a
//! non-author of a hide_key publish.
use super::chain::ChainLink;
use super::licensecheck::{LicenseCheck, display_chains, license_check, stand_ins_of, stored_allowed};
use super::sqlfrag::visible_to;
use super::textutil::{CHAIN_MAX, collapse_chain};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

const KINDS: [(&str, i64); 6] = [("Mc", 1), ("Tf", 2), ("Matching", 4), ("Blank", 8), ("Short", 16), ("Multipart", 32)];
pub fn kinds_of(mask: i64) -> Vec<&'static str> {
    KINDS.iter().filter(|(_, b)| mask & b != 0).map(|(k, _)| *k).collect()
}

#[derive(Clone, Debug)]
pub struct ItemRow {
    pub id: String,
    pub author_id: String,
    pub kind: String,
    pub visibility: String,
    pub version: i64,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub kinds_mask: i64,
    pub question_count: i64,
    pub license: String,
    pub has_images: i64,
    pub committed_at: i64,
    pub current_publish_id: String,
    pub full_key: String,
    pub stripped_key: Option<String>,
    pub p_hide: i64,
    pub p_license: String,
    pub p_version: i64,
    pub p_public_version: i64,
    pub public_at: Option<i64>,
    pub public_publish_id: String,
    pub schema_version: i64,
    pub author_name: Option<String>,
}

/// The item and its current publish, iff the caller may see it (one statement).
pub fn load_visible(conn: &Connection, id: &str, me: &str) -> rusqlite::Result<Option<ItemRow>> {
    let sql = format!(
        "SELECT i.id, i.author_id, i.kind, i.visibility, i.version, i.title, i.subject, i.course, i.grade, i.kinds_mask, i.question_count,
              i.license, i.has_images, i.committed_at, i.current_publish_id,
              p.full_key, p.stripped_key, p.hide_key AS p_hide, p.license AS p_license, p.version AS p_version, p.schema_version,
              p.public_version AS p_public_version, i.public_at, COALESCE(i.public_publish_id, i.current_publish_id) AS public_publish_id,
              u.display_name AS author_name
         FROM items i JOIN users u ON u.id = i.author_id JOIN item_publishes p ON p.publish_id = i.current_publish_id
        WHERE i.id = ?1 AND {}",
        visible_to("?2")
    );
    conn.query_row(&sql, params![id, me], |r| {
        Ok(ItemRow {
            id: r.get("id")?,
            author_id: r.get("author_id")?,
            kind: r.get("kind")?,
            visibility: r.get("visibility")?,
            version: r.get("version")?,
            title: r.get("title")?,
            subject: r.get("subject")?,
            course: r.get("course")?,
            grade: r.get("grade")?,
            kinds_mask: r.get("kinds_mask")?,
            question_count: r.get("question_count")?,
            license: r.get("license")?,
            has_images: r.get("has_images")?,
            committed_at: r.get("committed_at")?,
            current_publish_id: r.get("current_publish_id")?,
            full_key: r.get("full_key")?,
            stripped_key: r.get("stripped_key")?,
            p_hide: r.get("p_hide")?,
            p_license: r.get("p_license")?,
            p_version: r.get("p_version")?,
            p_public_version: r.get("p_public_version")?,
            public_at: r.get("public_at")?,
            public_publish_id: r.get("public_publish_id")?,
            schema_version: r.get("schema_version")?,
            author_name: r.get("author_name")?,
        })
    })
    .optional()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Full,
    Stripped,
}
impl Form {
    pub fn as_str(self) -> &'static str {
        match self {
            Form::Full => "full",
            Form::Stripped => "stripped",
        }
    }
}

/// THE served-form rule.
pub fn served_form(author_id: &str, p_hide: i64, me: &str) -> Form {
    if author_id != me && p_hide == 1 { Form::Stripped } else { Form::Full }
}

pub fn item_view(r: &ItemRow, me: &str, form: Form) -> Value {
    let is_author = r.author_id == me;
    let mut v = json!({
        "id": r.id,
        "kind": r.kind,
        "title": r.title,
        "subject": r.subject,
        "course": r.course,
        "grade": r.grade,
        "kinds": kinds_of(r.kinds_mask),
        "question_count": r.question_count,
        // Non-authors see the public version: it does not move for a change they are not served.
        "version": if is_author { r.p_version } else { r.p_public_version },
        // Non-authors only ever see the public publish id: a hidden-only republish moves nothing.
        "publish_id": if is_author { &r.current_publish_id } else { &r.public_publish_id },
        "license": r.p_license,
        "hide_key": r.p_hide == 1,
        "has_images": r.has_images == 1,
        "author_name": r.author_name.clone().unwrap_or_else(|| "deleted teacher".into()),
        "committed_at": if is_author { r.committed_at } else { r.public_at.unwrap_or(r.committed_at) },
        "form": form.as_str(),
        "is_author": is_author,
    });
    if is_author {
        v["visibility"] = json!(r.visibility);
    }
    v
}

/// Per-question provenance for the served form (never the full hash of a stripped serve).
pub fn questions_for(conn: &Connection, publish_id: &str, form: Form, license: &str) -> rusqlite::Result<(Vec<Value>, LicenseCheck)> {
    struct Q {
        qid: String,
        pos: i64,
        match_hash: String,
        full: String,
        stripped: Option<String>,
        chain_json: String,
        allowed_json: Option<String>,
    }
    let mut st = conn.prepare("SELECT qid, pos, match_hash, full_sha256, stripped_sha256, chain_json, allowed_json FROM item_questions WHERE publish_id = ?1 ORDER BY pos")?;
    let rows: Vec<Q> = st
        .query_map([publish_id], |r| Ok(Q { qid: r.get(0)?, pos: r.get(1)?, match_hash: r.get(2)?, full: r.get(3)?, stripped: r.get(4)?, chain_json: r.get(5)?, allowed_json: r.get(6)? }))?
        .collect::<Result<_, _>>()?;
    // Displayed chains: void links dropped, rightful stand-ins, deleted names.
    let chains: Vec<Vec<ChainLink>> = rows.iter().map(|q| serde_json::from_str(&q.chain_json).unwrap_or_default()).collect();
    let shown = display_chains(conn, &chains)?;
    // The reader-visible marker, from the displayed chains alone.
    let stored: Vec<_> = rows.iter().map(|q| stored_allowed(q.allowed_json.as_deref())).collect();
    let check = license_check(license, &shown, &stored, &stand_ins_of(conn, &stored)?);
    let questions = rows
        .iter()
        .zip(&shown)
        .map(|(q, chain)| {
            json!({
                "pos": q.pos,
                "qid": q.qid,
                "served_sha256": if form == Form::Stripped { json!(q.stripped) } else { json!(q.full) },
                "match_hash": q.match_hash,
                // The stored credits are complete; at most CHAIN_MAX are shown, with "and N earlier contributors".
                "chain": collapse_chain(chain, CHAIN_MAX),
            })
        })
        .collect();
    Ok((questions, check))
}

#[cfg(test)]
#[path = "itemview_tests.rs"]
mod tests;
