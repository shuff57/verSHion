//! The chains readers see, and the reader-visible licence check (worker/src/pool/licensecheck.ts), in one place so the
//! read path and the moderation release use the same view. Everything here is computed from the displayed chain data only
//! (public), never from evidence rows or hidden content.
use super::chain::{ChainLink, void_info};
use super::license::{LICENSES, is_license, republish_options};
use super::sqlfrag::is_id;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

/// The displayed chain of each stored chain: void (infringing) links dropped, a void link the admin tied to the rightful
/// item shown as that item, one link per item, and names of deleted accounts shown as "deleted teacher" (resolved now,
/// after the stand-ins).
pub fn display_chains(conn: &Connection, chains: &[Vec<ChainLink>]) -> rusqlite::Result<Vec<Vec<ChainLink>>> {
    let mut linked: Vec<String> = Vec::new();
    for l in chains.iter().flatten() {
        if is_id(&l.item_id) && !linked.contains(&l.item_id) {
            linked.push(l.item_id.clone());
        }
    }
    let (mut voided, mut replaced_by) = (HashSet::new(), HashMap::new());
    if !linked.is_empty() {
        let mut st = conn.prepare("SELECT item_id FROM void_items WHERE item_id IN (SELECT value FROM json_each(?1))")?;
        let rows: Vec<String> = st.query_map(params![serde_json::to_string(&linked).unwrap()], |r| r.get(0))?.collect::<Result<_, _>>()?;
        voided.extend(rows);
        let v: Vec<String> = voided.iter().cloned().collect();
        replaced_by = void_info(conn, &v)?.replaced;
    }
    let shown: Vec<Vec<ChainLink>> = chains
        .iter()
        .map(|ch| {
            let mapped: Vec<ChainLink> = ch
                .iter()
                .map(|l| match replaced_by.get(&l.item_id) {
                    Some(r) => ChainLink { modified: l.modified, ..r.clone() },
                    None => l.clone(),
                })
                .collect();
            mapped
                .iter()
                .enumerate()
                .filter(|(j, l)| !voided.contains(&l.item_id) && (l.item_id.is_empty() || mapped.iter().position(|x| x.item_id == l.item_id) == Some(*j)))
                .map(|(_, l)| l.clone())
                .collect()
        })
        .collect();
    let mut ids: Vec<String> = Vec::new();
    for l in shown.iter().flatten() {
        if is_id(&l.item_id) && !ids.contains(&l.item_id) {
            ids.push(l.item_id.clone());
        }
    }
    let mut gone: HashSet<String> = HashSet::new();
    if !ids.is_empty() {
        let mut st = conn.prepare("SELECT i.id FROM items i JOIN users u ON u.id = i.author_id WHERE i.id IN (SELECT value FROM json_each(?1)) AND u.deleted_at IS NOT NULL")?;
        let rows: Vec<String> = st.query_map(params![serde_json::to_string(&ids).unwrap()], |r| r.get(0))?.collect::<Result<_, _>>()?;
        gone.extend(rows);
    }
    Ok(shown.into_iter().map(|ch| ch.into_iter().map(|l| if gone.contains(&l.item_id) { ChainLink { author_name: "deleted teacher".into(), ..l } } else { l }).collect()).collect())
}

fn cut(out: &mut BTreeSet<&'static str>, opts: &[&str]) {
    out.retain(|l| opts.contains(l));
}

/// What one displayed chain allows: a verbatim link its own licence (CC0: any), a modified one the table's options, a
/// collapsed run its links' options.
pub fn chain_allows(chain: &[ChainLink]) -> Vec<&'static str> {
    let mut out: BTreeSet<&'static str> = LICENSES.iter().copied().collect();
    for l in chain {
        if is_license(&l.license) {
            if l.modified || l.license == "CC0-1.0" {
                cut(&mut out, republish_options(&l.license));
            } else {
                cut(&mut out, &[l.license.as_str()]);
            }
        }
        for x in l.collapsed_licenses.iter().flatten() {
            if is_license(x) {
                cut(&mut out, republish_options(x));
            }
        }
    }
    LICENSES.iter().copied().filter(|l| out.contains(l)).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Position {
    pub pos: i64,
    pub allowed: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LicenseCheck {
    /// Worst first: mismatch, unverified, claimed_independent, unverified_earlier_match, ok.
    pub status: &'static str,
    /// What it is checked against: the sources the licence rule recorded at publish.
    pub basis: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positions: Option<Vec<Position>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// What the licence rule allowed for a question at publish, and the items that decided it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct StoredAllowed {
    pub allowed: Vec<String>,
    pub binders: Vec<String>,
    #[serde(default)]
    pub soft: bool,
    #[serde(default)]
    pub claimed: bool,
    #[serde(default)]
    pub earlier: bool,
}

/// THE reader-visible marker: "consistent with recorded sources". The licences the licence rule allowed when each
/// question was published (item_questions.allowed_json), with one change since: a deciding source found infringing and
/// tied to the rightful item narrows that question to what the rightful item's licence allows (its stand-in link, verbatim
/// or modified as the chain records it). A question without a record falls back to its displayed chain.
pub fn license_check(license: &str, chains: &[Vec<ChainLink>], stored: &[Option<StoredAllowed>], stand_ins: &HashMap<String, ChainLink>) -> LicenseCheck {
    let mut bad: Vec<Position> = Vec::new();
    for (pos, ch) in chains.iter().enumerate() {
        let allowed: Vec<&'static str> = match stored.get(pos).and_then(|s| s.as_ref()) {
            Some(st) => {
                let mut set: BTreeSet<&'static str> = LICENSES.iter().copied().filter(|l| st.allowed.iter().any(|a| a == l)).collect();
                for b in &st.binders {
                    let Some(rep) = stand_ins.get(b) else { continue };
                    if !is_license(&rep.license) {
                        continue;
                    }
                    let link = ch.iter().find(|l| l.item_id == rep.item_id).or_else(|| ch.iter().find(|l| l.item_id.is_empty())).unwrap_or(rep);
                    if link.modified || rep.license == "CC0-1.0" {
                        cut(&mut set, republish_options(&rep.license));
                    } else {
                        cut(&mut set, &[rep.license.as_str()]);
                    }
                }
                LICENSES.iter().copied().filter(|l| set.contains(l)).collect()
            }
            None => chain_allows(ch),
        };
        if !allowed.contains(&license) {
            bad.push(Position { pos: pos as i64, allowed });
        }
    }
    if bad.is_empty() {
        type Pick = fn(&StoredAllowed) -> bool;
        let kinds: [(Pick, &'static str, &'static str); 3] = [
            (|s| s.soft, "unverified", "A source the publisher was served restricts these questions; the item's licence was not checked against it."),
            (|s| s.claimed, "claimed_independent", "The publisher says these questions were written independently of a source they had seen."),
            (|s| s.earlier, "unverified_earlier_match", "Another author published these questions earlier; there is no record of this publisher being served them."),
        ];
        for (pick, status, reason) in kinds {
            let ps: Vec<i64> = stored.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(pick)).map(|(p, _)| p as i64).collect();
            if !ps.is_empty() {
                return LicenseCheck { status, basis: "recorded_sources", positions: Some(ps.into_iter().take(20).map(|pos| Position { pos, allowed: vec![] }).collect()), reason: Some(reason) };
            }
        }
        return LicenseCheck { status: "ok", basis: "recorded_sources", positions: None, reason: None };
    }
    bad.truncate(20);
    LicenseCheck { status: "mismatch", basis: "recorded_sources", positions: Some(bad), reason: Some("The licence of this item is not one its recorded sources allow for these questions.") }
}

/// Parses item_questions.allowed_json.
pub fn stored_allowed(json: Option<&str>) -> Option<StoredAllowed> {
    serde_json::from_str::<StoredAllowed>(json.filter(|s| !s.is_empty())?).ok()
}

/// The stand-ins of void items among the binders of these records (their rightful item, licence at void time).
pub fn stand_ins_of(conn: &Connection, stored: &[Option<StoredAllowed>]) -> rusqlite::Result<HashMap<String, ChainLink>> {
    let mut ids: Vec<String> = Vec::new();
    for b in stored.iter().flatten().flat_map(|s| s.binders.iter()) {
        if is_id(b) && !ids.contains(b) {
            ids.push(b.clone());
        }
    }
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(void_info(conn, &ids)?.replaced)
}

/// The check for stored items (their current publish), by id.
pub fn license_check_of(conn: &Connection, item_ids: &[String]) -> rusqlite::Result<HashMap<String, LicenseCheck>> {
    let mut out = HashMap::new();
    if item_ids.is_empty() {
        return Ok(out);
    }
    let mut st = conn.prepare(
        "SELECT i.id, i.license, json_group_array(json_array(iq.pos, iq.chain_json, iq.allowed_json)) AS qs FROM items i JOIN item_questions iq ON iq.publish_id = i.current_publish_id
        WHERE i.id IN (SELECT value FROM json_each(?1)) GROUP BY i.id",
    )?;
    let rows: Vec<(String, String, String)> = st.query_map(params![serde_json::to_string(item_ids).unwrap()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    for (id, license, qs) in rows {
        let mut qs: Vec<(i64, String, Option<String>)> = serde_json::from_str(&qs).unwrap_or_default();
        qs.sort_by_key(|q| q.0);
        let chains: Vec<Vec<ChainLink>> = qs.iter().map(|q| serde_json::from_str(&q.1).unwrap_or_default()).collect();
        let chains = display_chains(conn, &chains)?;
        let stored: Vec<Option<StoredAllowed>> = qs.iter().map(|q| stored_allowed(q.2.as_deref())).collect();
        let stand = stand_ins_of(conn, &stored)?;
        out.insert(id, license_check(&license, &chains, &stored, &stand));
    }
    Ok(out)
}

#[cfg(test)]
#[path = "licensecheck_tests.rs"]
mod tests;
