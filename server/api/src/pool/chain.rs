//! A displayed derivation chain link and the void-item lookups (the `ChainLink` of provenance.ts, `voidInfo`).
use super::sqlfrag::{is_id, visible_to_others};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChainLink {
    pub author_name: String,
    pub item_id: String,
    pub qid: String,
    pub version: i64,
    pub license: String,
    pub modified: bool,
    /// A placeholder for links left out (no item): the licences they carried still grant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_licenses: Option<Vec<String>>,
    /// A source the publisher attested independence of (a short or generic read).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possible_source: Option<bool>,
    /// An earlier identical publication by another author, with no evidence of a serve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub earlier_match: Option<bool>,
    /// What of the source this question uses when it is not the question itself: "image", "text" or "part".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

pub struct VoidInfo {
    pub voided: HashSet<String>,
    pub replaced: HashMap<String, ChainLink>,
}

/// The void items among `ids`, and for those the admin tied to the rightful author's item, the link that stands in for
/// them: that item, its author and its licence at void time. An item not visible to others is never named.
pub fn void_info(conn: &Connection, ids: &[String]) -> rusqlite::Result<VoidInfo> {
    let mut out = VoidInfo { voided: HashSet::new(), replaced: HashMap::new() };
    let mut want: Vec<&String> = Vec::new();
    for x in ids {
        if is_id(x) && !want.contains(&x) {
            want.push(x);
        }
    }
    if want.is_empty() {
        return Ok(out);
    }
    let sql = format!(
        "SELECT v.item_id, v.replaced_by, COALESCE(v.replaced_license, i.license) AS license, i.public_version,
              COALESCE((SELECT ua.display_name FROM users ua WHERE ua.id = COALESCE(v.replaced_author_id, i.author_id) AND ua.deleted_at IS NULL), 'deleted teacher') AS author_name,
              CASE WHEN i.id IS NOT NULL AND {} THEN 1 ELSE 0 END AS shown
         FROM void_items v LEFT JOIN items i ON i.id = v.replaced_by AND i.purged_at IS NULL LEFT JOIN users u ON u.id = i.author_id
        WHERE v.item_id IN (SELECT value FROM json_each(?1))",
        visible_to_others()
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(params![serde_json::to_string(&want).unwrap()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<i64>>(3)?, r.get::<_, String>(4)?, r.get::<_, i64>(5)?))
    })?;
    for row in rows {
        let (item_id, replaced_by, license, public_version, author_name, shown) = row?;
        out.voided.insert(item_id.clone());
        // The stand-in carries the licence and name of the rightful item AT VOID TIME; an item not visible to others
        // (unlisted, held, removed) is never named: a neutral label, same licence.
        if let (Some(by), Some(license)) = (replaced_by, license) {
            if by.is_empty() || license.is_empty() {
                continue;
            }
            let link = if shown == 1 {
                ChainLink { author_name, item_id: by, qid: String::new(), version: public_version.unwrap_or(1), license, modified: true, ..Default::default() }
            } else {
                ChainLink { author_name: "the rightful author".into(), item_id: String::new(), qid: String::new(), version: 0, license, modified: true, ..Default::default() }
            };
            out.replaced.insert(item_id, link);
        }
    }
    Ok(out)
}

/// The void items among `ids`.
pub fn voided_items(conn: &Connection, ids: &[String]) -> rusqlite::Result<HashSet<String>> {
    Ok(void_info(conn, ids)?.voided)
}
