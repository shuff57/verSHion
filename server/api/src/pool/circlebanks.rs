//! Circle bank lookups and reads (worker/src/pool/circles.ts "bank reads"): the bank row, who may read it, which versions are kept, the
//! recorded origins, and the head of a bank read (`bank_read`: everything `getBank` decides before it streams the body, so the route
//! layer only delivers the stored object: the full text, a 304/HEAD, or share-core's stripped form for a viewer).
use super::circlebase::*;
use super::circlesql::*;
use super::ctx::{Fail, PoolCtx, PoolError, Res, not_found};
use super::sqlfrag::{hex, is_id};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone)]
pub struct BankRow {
    pub id: String,
    pub circle_id: String,
    pub author_id: String,
    pub title: String,
    pub subject: String,
    pub course: String,
    pub grade: String,
    pub license: String,
    pub head_version: i64,
    pub head_version_id: Option<String>,
    pub question_count: i64,
    pub has_images: bool,
    pub anonymized: bool,
    pub hide_key: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub removed_at: Option<i64>,
    pub removed_reason: Option<String>,
    pub purge_after: Option<i64>,
    pub held: bool,
    /// A report's or an admin's hold (not only a student-data hold a send raised).
    pub blocking: bool,
    /// The author is in good standing (or erased with the bank kept anonymised).
    pub author_ok: bool,
}

fn bank_cols() -> String {
    format!(
        "b.id, b.circle_id, b.author_id, b.title, b.subject, b.course, b.grade, b.license, b.head_version, b.head_version_id, b.question_count, b.has_images, b.anonymized, b.hide_key,
  b.created_at, b.updated_at, b.removed_at, b.removed_reason, b.purge_after, EXISTS (SELECT 1 FROM circle_holds h WHERE h.bank_id = b.id) AS held, {} AS blocking,
  {} AS author_ok",
        blocking_hold_sql("b.id"),
        author_ok_sql("b")
    )
}

fn bank_of_row(r: &rusqlite::Row) -> rusqlite::Result<BankRow> {
    Ok(BankRow {
        id: r.get(0)?,
        circle_id: r.get(1)?,
        author_id: r.get(2)?,
        title: r.get(3)?,
        subject: r.get(4)?,
        course: r.get(5)?,
        grade: r.get(6)?,
        license: r.get(7)?,
        head_version: r.get(8)?,
        head_version_id: r.get(9)?,
        question_count: r.get(10)?,
        has_images: r.get::<_, i64>(11)? == 1,
        anonymized: r.get::<_, i64>(12)? == 1,
        hide_key: r.get::<_, i64>(13)? == 1,
        created_at: r.get(14)?,
        updated_at: r.get(15)?,
        removed_at: r.get(16)?,
        removed_reason: r.get(17)?,
        purge_after: r.get(18)?,
        held: r.get::<_, i64>(19)? == 1,
        blocking: r.get::<_, i64>(20)? == 1,
        author_ok: r.get::<_, i64>(21)? == 1,
    })
}

/// The bank of circle `cid`, iff the caller is an active member (a counted 404 otherwise). Not yet checked: open, removed, held.
pub fn bank_or_miss(x: &PoolCtx, cid: &str, bid: &str, me: &str) -> Res<(Membership, BankRow)> {
    let m = circle_or_miss(x, cid, me)?;
    let b = if is_id(bid) {
        x.conn.query_row(&format!("SELECT {} FROM circle_banks b WHERE b.id = ?1 AND b.circle_id = ?2 AND b.purged_at IS NULL AND b.head_version > 0", bank_cols()), params![bid, cid], bank_of_row).optional()?
    } else {
        None
    };
    match b {
        Some(b) => Ok((m, b)),
        None => {
            x.count_miss(me, "item_miss")?;
            Err(not_found().into())
        }
    }
}

/// A member may READ the bank: the circle open, the bank not removed, not held (unless theirs).
pub fn require_readable(x: &PoolCtx, m: &Membership, b: &BankRow, me: &str) -> Res<()> {
    require_open(m)?;
    if b.removed_at.is_some() {
        let undo = matches!(b.removed_reason.as_deref(), Some("author_removed") | Some("owner_removed")).then_some(b.purge_after).flatten();
        return Err(PoolError::new(409, "bank_removed").with("reason", b.removed_reason.clone().map(Value::from).unwrap_or(Value::Null)).with("undo_until", undo.map(Value::from).unwrap_or(Value::Null)).into());
    }
    // The owner reads and acts on a bank held only for student data, as its author does; the withheld version itself stays unreadable.
    if (b.held && b.author_id != me && !(m.role == "owner" && !b.blocking)) || !b.author_ok {
        x.count_miss(me, "item_miss")?;
        return Err(not_found().into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kept {
    pub oldest_kept: Option<i64>,
    pub pinned_versions: Vec<i64>,
}
impl Kept {
    /// The two fields as they are merged into responses.
    pub fn fields(&self) -> [(&'static str, Value); 2] {
        [("oldest_kept", self.oldest_kept.map(Value::from).unwrap_or(Value::Null)), ("pinned_versions", json!(self.pinned_versions))]
    }
}
/// Which versions of a bank are kept in full, as members need it: `oldest_kept` (where the contiguous run of readable kept versions
/// that ends at the head starts) and `pinned_versions` (older versions kept outside that run; at most 50 listed).
pub fn kept_info(x: &PoolCtx, bid: &str, head: i64) -> rusqlite::Result<Kept> {
    let kept: BTreeSet<i64> = x
        .conn
        .prepare("SELECT version FROM circle_versions INDEXED BY circle_versions_bank WHERE bank_id = ?1 AND status IN ('current', 'previous') AND full_key <> '' AND withheld_at IS NULL")?
        .query_map([bid], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let Some(&max) = kept.iter().next_back() else { return Ok(Kept { oldest_kept: None, pinned_versions: vec![] }) };
    let mut start = head.min(max);
    while !kept.contains(&start) {
        start -= 1;
    }
    while kept.contains(&(start - 1)) {
        start -= 1;
    }
    Ok(Kept { oldest_kept: Some(start), pinned_versions: kept.iter().copied().filter(|v| *v < start).take(50).collect() })
}
/// `404 version_not_retained {oldest_kept, pinned_versions}`.
pub fn not_retained(x: &PoolCtx, bid: &str, head: i64) -> rusqlite::Result<PoolError> {
    let k = kept_info(x, bid, head)?;
    let mut e = PoolError::new(404, "version_not_retained");
    for (key, v) in k.fields() {
        e = e.with(key, v);
    }
    Ok(e.with("message", "This version is no longer kept (a bank keeps its last 20 versions in full).".into()))
}
/// `404 version_withheld`: the words name no reason, so members cannot tell student data from a copyright block.
pub fn withheld_error(hide_off_releases: Option<bool>) -> PoolError {
    let e = PoolError::new(404, "version_withheld");
    let e = match hide_off_releases {
        Some(b) => e.with("hide_off_releases", b.into()),
        None => e,
    };
    e.with("message", "This version is withheld until an administrator has reviewed it. Only an administrator can release it.".into())
}
pub fn keys_hidden_unavailable() -> PoolError {
    PoolError::new(409, "keys_hidden_unavailable")
        .with("message", "The answer keys of this bank are hidden from viewers, and this version cannot be shown without them. Ask the bank's author or the circle's owner.".into())
}

/// A version's recorded origins (public material, checked at send) with their authors' names read NOW.
pub fn origins_for_read(x: &PoolCtx, json_text: Option<&str>) -> rusqlite::Result<Vec<Value>> {
    let Some(rows) = json_text.and_then(|j| serde_json::from_str::<Value>(j).ok()).and_then(|v| v.as_array().cloned()).filter(|a| !a.is_empty()) else { return Ok(vec![]) };
    let mut ids: Vec<&str> = Vec::new();
    for r in &rows {
        if let Some(i) = r.get("item_id").and_then(Value::as_str) {
            if !ids.contains(&i) {
                ids.push(i);
            }
        }
    }
    let mut names: HashMap<String, String> = HashMap::new();
    let mut st = x.conn.prepare("SELECT i.id, i.anonymized, u.display_name, u.deleted_at FROM json_each(?1) j CROSS JOIN items i ON i.id = j.value LEFT JOIN users u ON u.id = i.author_id")?;
    let mut q = st.query([serde_json::to_string(&ids).unwrap()])?;
    while let Some(r) = q.next()? {
        let anon = r.get::<_, i64>(1)? == 1;
        names.insert(r.get(0)?, if anon { DELETED_NAME.into() } else { name_of(r.get::<_, Option<String>>(2)?.as_deref(), r.get(3)?) });
    }
    Ok(rows
        .iter()
        .map(|r| {
            let item = r.get("item_id").and_then(Value::as_str).unwrap_or("");
            json!({ "pos": r.get("pos").cloned().unwrap_or(Value::Null), "item_id": r.get("item_id").cloned().unwrap_or(Value::Null),
                "qid": r.get("qid").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null), "version": r.get("version").filter(|v| !v.is_null()).cloned().unwrap_or(Value::Null),
                "license": r.get("license").cloned().unwrap_or(Value::Null), "author_name": names.get(item).cloned().unwrap_or_else(|| DELETED_NAME.into()) })
        })
        .collect())
}

/// What `getBank` decides before it streams: the head JSON (sent before the payload), its ETag and the stored object to deliver.
#[derive(Debug, Clone)]
pub struct BankRead {
    /// The head object as JSON text (the body is this with `"payload":<stored object>` appended).
    pub head: String,
    pub etag: String,
    pub full_key: String,
    /// A viewer of a bank that hides its keys: the object must be stripped with `strip_opts` before it is served.
    pub hidden: bool,
    pub strip_opts: Option<String>,
}

/// GET / HEAD /circles/:cid/banks/:bid (and `/versions/:n` when `version` is set): the checks, the read charge and the head.
/// `hide_off_releases(me, bid, version_id)` answers whether turning key-hiding off would release a withheld version (circlehide).
pub fn bank_read(x: &PoolCtx, me: &str, cid: &str, bid: &str, version: Option<i64>, hide_off_releases: &dyn Fn(&str, &str, &str) -> bool) -> Res<BankRead> {
    let (m, b) = bank_or_miss(x, cid, bid, me)?;
    require_readable(x, &m, &b, me)?;
    if let Some(v) = version {
        if v < 1 || v > b.head_version {
            x.count_miss(me, "item_miss")?;
            return Err(not_found().into());
        }
    }
    x.require_daily(me, "item_reads", 1)?;
    struct V {
        id: String,
        version: i64,
        full_key: String,
        committed_at: i64,
        license: String,
        origins: Option<String>,
        message: Option<String>,
        rollback_of: Option<i64>,
        merged_from: Option<i64>,
        withheld_at: Option<i64>,
        strip_opts: Option<String>,
        by: String,
    }
    let cols = "v.id, v.version, v.full_key, v.committed_at, v.license, v.origins, v.message, v.rollback_of, v.merged_from, v.withheld_at, v.strip_opts, u.display_name, u.deleted_at";
    let map = |r: &rusqlite::Row| -> rusqlite::Result<V> {
        Ok(V {
            id: r.get(0)?,
            version: r.get(1)?,
            full_key: r.get(2)?,
            committed_at: r.get(3)?,
            license: r.get(4)?,
            origins: r.get(5)?,
            message: r.get(6)?,
            rollback_of: r.get(7)?,
            merged_from: r.get(8)?,
            withheld_at: r.get(9)?,
            strip_opts: r.get(10)?,
            by: name_of(r.get::<_, Option<String>>(11)?.as_deref(), r.get(12)?),
        })
    };
    let v: Option<V> = match version {
        None => x
            .conn
            .query_row(&format!("SELECT {cols} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.id = ?1 AND v.bank_id = ?2 AND v.status = 'current'"), params![b.head_version_id, bid], map)
            .optional()?,
        Some(n) => x
            .conn
            .query_row(
                &format!("SELECT {cols} FROM circle_versions v LEFT JOIN users u ON u.id = v.committer_id WHERE v.bank_id = ?1 AND v.version = ?2 AND v.status IN ('current', 'previous') AND v.full_key <> ''"),
                params![bid, n],
                map,
            )
            .optional()?,
    };
    let v = match (v, version) {
        (Some(v), _) => v,
        (None, Some(n)) => {
            // A withheld version pruned is not "past the last 20": its words say what it is.
            let gone: Option<Option<i64>> = x.conn.query_row("SELECT withheld_at FROM circle_versions WHERE bank_id = ?1 AND version = ?2", params![bid, n], |r| r.get(0)).optional()?;
            if let Some(Some(_)) = gone {
                let k = kept_info(x, bid, b.head_version)?;
                let mut e = PoolError::new(404, "version_not_retained");
                for (key, val) in k.fields() {
                    e = e.with(key, val);
                }
                return Err(e
                    .with("withheld", true.into())
                    .with("review_copy", "hashes_only".into())
                    .with("message", "This version was withheld for an administrator's review and its text is no longer kept: only a record of what matched is kept, for the administrator.".into())
                    .into());
            }
            return Err(not_retained(x, bid, b.head_version)?.into());
        }
        (None, None) => return Err(not_found().into()),
    };
    // A version that matched confirmed student data is served to no member (its author included) until an administrator releases it.
    if v.withheld_at.is_some() {
        let may_hide = m.role == "owner" || (b.author_id == me && m.role == "editor");
        let extra = if may_hide { Some(b.hide_key && hide_off_releases(me, bid, &v.id)) } else { None };
        return Err(withheld_error(extra).into());
    }
    // A VIEWER of a bank that hides its keys is served the stripped form; owners and editors the full text. Viewers read only a version
    // whose served form was checked (recorded strip decisions).
    let hidden = b.hide_key && m.role == "viewer";
    if hidden && v.strip_opts.is_none() {
        return Err(keys_hidden_unavailable().into());
    }
    let author_name = if b.anonymized {
        DELETED_NAME.to_string()
    } else {
        let u: Option<(Option<String>, Option<i64>)> = x.conn.query_row("SELECT display_name, deleted_at FROM users WHERE id = ?1", [&b.author_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        match u {
            Some((d, del)) => name_of(d.as_deref(), del),
            None => name_of(None, Some(1)),
        }
    };
    let mut bank = serde_json::Map::new();
    for (k, val) in [
        ("id", json!(b.id)),
        ("title", json!(b.title)),
        ("subject", json!(b.subject)),
        ("course", json!(b.course)),
        ("grade", json!(b.grade)),
        ("license", json!(b.license)),
        ("version_license", json!(v.license)),
        ("author_name", json!(author_name)),
        ("is_author", json!(b.author_id == me)),
        ("version", json!(v.version)),
        ("version_id", json!(v.id)),
        ("head_version", json!(b.head_version)),
        ("question_count", json!(b.question_count)),
        ("has_images", json!(b.has_images)),
        ("committed_at", json!(v.committed_at)),
        ("committed_by", json!(v.by)),
        ("message", json!(if hidden { String::new() } else { text(v.message.as_deref()) })),
        ("hide_key", json!(b.hide_key)),
    ] {
        bank.insert(k.into(), val);
    }
    if hidden {
        bank.insert("keys_hidden".into(), true.into());
    }
    if let Some(r) = v.rollback_of {
        bank.insert("rollback_of".into(), r.into());
    }
    if let Some(r) = v.merged_from {
        bank.insert("merged_from".into(), r.into());
    }
    if b.held {
        bank.insert("held".into(), true.into());
    }
    let head = json!({ "circle": { "id": m.id, "name": m.name, "role": m.role }, "bank": Value::Object(bank), "origins": origins_for_read(x, v.origins.as_deref())? }).to_string();
    // A viewer's ETag also covers the decisions the version is stripped with.
    let basis = if hidden { format!("{head}\n{}", v.strip_opts.as_deref().unwrap_or("")) } else { head.clone() };
    let etag = format!("\"{}\"", &hex(&Sha256::digest(basis.as_bytes()))[..32]);
    Ok(BankRead { head, etag, full_key: v.full_key, hidden, strip_opts: v.strip_opts })
}

#[allow(dead_code)]
fn _unused(_: Fail) {}

#[cfg(test)]
#[path = "circlebanks_tests.rs"]
mod tests;
