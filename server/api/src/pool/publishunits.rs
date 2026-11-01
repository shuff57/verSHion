//! Material a question uses besides itself is a licence unit of its own (worker/src/pool/publish.ts `publishOnce`, rounds 16-22): an explanation or Short model
//! answer of `LIFT_WORDS`+ words ("text"), its image (exact bytes or the same decoded pixels) and one of a source's questions as a part of this one or this one as
//! one of its parts ("part"). A unit binds when the account was SERVED the publish that holds it, unless the account had it first, it is stock, or the account
//! uploaded the image first. HTTP-free; every lookup is bounded per key, so nothing anyone else publishes multiplies the work.
use super::authctx::AuthorCtx;
use super::ctx::{PoolCtx, PoolError, Res};
use super::offer::offered_sql;
use super::publishfacts::Facts;
use super::served::served_form;
use rusqlite::{Row, params};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Distinct unit keys one publish may check.
pub const UNIT_KEYS_MAX: usize = 60_000;
/// How often a key is counted anywhere, at most.
pub const OCC_SCAN: i64 = 64;
/// The size of the account's own served rows that a hot key may cost.
pub const HOT_SCAN: i64 = 4096;
/// ... and the budget for all hot keys together.
pub const HOT_BUDGET: i64 = 262_144;
/// The earliest sources of one unit that are kept.
pub const UNIT_SOURCES_PER_KEY: i64 = 8;
/// Of a key's occurrences, the earliest this many (by when they were stored) are looked at.
pub const UNIT_OCC_WINDOW: i64 = 8;
/// Distinct authors whose items have a unit before it is stock.
pub const GENERIC_UNIT_AUTHORS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Text,
    Image,
    Part,
}
impl UnitKind {
    fn parse(s: &str) -> UnitKind {
        match s {
            "image" => UnitKind::Image,
            "part" => UnitKind::Part,
            _ => UnitKind::Text,
        }
    }
    fn tag(self) -> &'static str {
        match self {
            UnitKind::Text => "text",
            UnitKind::Image => "image",
            UnitKind::Part => "part",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub key: String,
    pub kind: UnitKind,
    pub part: Option<i64>,
    pub hidden: bool,
    pub stem_only: bool,
}

#[derive(Debug, Clone)]
pub struct Lift {
    pub item_id: String,
    pub qid: String,
    pub version: i64,
    pub license: String,
    pub author_name: String,
    pub seq: i64,
    pub chain: String,
    pub kind: UnitKind,
    pub key: String,
    pub served: bool,
    /// "import" or "view" when the account was served it.
    pub ev_kind: Option<&'static str>,
    pub received: Vec<String>,
    pub stock: bool,
    pub own_first: bool,
    pub visible: bool,
    pub uploaded_first: bool,
    pub hidden: bool,
    pub stem_only: bool,
}

#[derive(Debug, Default)]
pub struct Units {
    pub units_at: Vec<Vec<Unit>>,
    pub lifts: HashMap<usize, Vec<Lift>>,
}

fn outer_sql() -> String {
    format!(
        "         SELECT o.pos, o.key, o.kind, o.hidden, o.stem_only, i.id AS item_id, MIN(o.qid) AS qid,
                CASE WHEN MAX(s.kind = 'import') = 1 THEN 'import' ELSE 'view' END AS ev_kind,
                json_group_array(json(s.licenses_json)) AS lics_nested, json_group_array(json(COALESCE(o.now_lic, '[]'))) AS now_lics,
                (SELECT json_group_array(DISTINCT cl.license) FROM content_licences cl WHERE cl.item_id = i.id AND cl.ckey = 'u:' || o.key
                    AND (cl.until_at IS NULL OR cl.until_at >= (SELECT MIN(s4.first_at) FROM served s4 WHERE s4.reader_id = ?1 AND s4.item_id = i.id AND s4.void_at IS NULL))) AS hist_lics,
                (SELECT json_group_array(DISTINCT v) FROM (SELECT i.license AS v WHERE (EXISTS (SELECT 1 FROM item_questions cq WHERE cq.publish_id = i.public_publish_id AND (cq.bk_expl = o.key OR cq.bk_ans = o.key))
                                                                 OR EXISTS (SELECT 1 FROM item_part_keys cpk WHERE cpk.publish_id = i.public_publish_id AND cpk.key = o.key)
                                                                 OR EXISTS (SELECT 1 FROM item_blobs cb LEFT JOIN blobs cbx ON cbx.sha256 = cb.sha256 WHERE cb.publish_id = i.public_publish_id AND (cb.sha256 = o.key OR cbx.pixel_hash = o.key)))
                                                           AND {})) AS licenses_json,
                MIN(o.seq) AS seq, MAX(o.chain_json) AS chain, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = o.key) AS own_seq,
                (i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = i.id)) AS visible,
                (SELECT MIN(sp.committed_at) FROM item_publishes sp WHERE sp.item_id = i.id) AS src_at,
                (SELECT MIN(bu.first_at) FROM blob_uploads bu WHERE bu.uploader_id = ?1 AND (bu.sha256 = o.key OR bu.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = o.key))) AS my_upload_at",
        offered_sql("i")
    )
}

const OCC_SQL: &str = "SELECT k.value AS key,
                (SELECT COUNT(*) FROM (SELECT 1 FROM item_questions x WHERE x.bk_expl = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_questions x WHERE x.bk_ans = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_part_keys x WHERE x.key = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM item_blobs x WHERE x.sha256 = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM blobs bx JOIN item_blobs x ON x.sha256 = bx.sha256 WHERE bx.pixel_hash = k.value LIMIT ?2))
              + (SELECT COUNT(*) FROM (SELECT 1 FROM ghost_units x WHERE x.key = k.value LIMIT ?2)) AS n
           FROM json_each(?1) k";

const W_CTE: &str = "w AS MATERIALIZED (SELECT CAST(j.value ->> '$.pos' AS INTEGER) AS pos, j.value ->> '$.key' AS key, j.value ->> '$.kind' AS kind, CAST(j.value ->> '$.part' AS INTEGER) AS part,
                                           CAST(j.value ->> '$.hidden' AS INTEGER) AS hidden, CAST(j.value ->> '$.stemOnly' AS INTEGER) AS stem_only,
                                           CAST(j.value ->> '$.hot' AS INTEGER) AS hot FROM json_each(?2) j)";

fn cold_occ() -> String {
    format!(
        "
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$.\"' || w.key || '\"'), iq.seq) AS seq, iq.qid, iq.chain_json, NULL AS now_lic FROM w CROSS JOIN item_questions iq ON w.hot = 0 AND w.kind = 'text' AND (iq.bk_expl = w.key OR iq.bk_ans = w.key)
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, pk.publish_id, uq.seq, uq.qid, uq.chain_json, NULL FROM w CROSS JOIN item_part_keys pk
                  ON w.hot = 0 AND pk.key = w.key AND (w.kind = 'text' OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json, NULL FROM w CROSS JOIN item_blobs ib
                  ON w.hot = 0 AND w.kind = 'image' AND (ib.sha256 = w.key OR (w.key LIKE 'px:%' AND ib.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = w.key)))
                  CROSS JOIN item_publishes ip ON ip.publish_id = ib.publish_id AND {}
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, gu.publish_id, COALESCE(gu.seq, gq.seq, 0), COALESCE(gq.qid, ''), COALESCE(gq.chain_json, '[]'), gu.now_licenses FROM w CROSS JOIN ghost_units gu
                  ON w.hot = 0 AND gu.key = w.key AND gu.kind = w.kind AND (w.kind <> 'part' OR gu.part >= 0 OR w.part >= 0)
                  LEFT JOIN ghost_questions gq ON gq.publish_id = gu.publish_id AND gq.pos = gu.pos",
        served_form("ib", "ip")
    )
}

fn hot_occ() -> String {
    format!(
        "
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$.\"' || hk.value || '\"'), iq.seq) AS seq, iq.qid, iq.chain_json, NULL AS now_lic
                  FROM served sh CROSS JOIN item_questions iq ON iq.publish_id = sh.publish_id CROSS JOIN json_each(json_array(iq.bk_expl, iq.bk_ans)) hk
                  CROSS JOIN w ON w.hot = 1 AND w.kind = 'text' AND w.key = hk.value WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, pk.publish_id, uq.seq, uq.qid, uq.chain_json, NULL
                  FROM served sh CROSS JOIN item_part_keys pk ON pk.publish_id = sh.publish_id
                  CROSS JOIN w ON w.hot = 1 AND w.key = pk.key AND (w.kind = 'text' OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json, NULL
                  FROM served sh CROSS JOIN item_blobs ib ON ib.publish_id = sh.publish_id CROSS JOIN item_publishes ip ON ip.publish_id = ib.publish_id AND {}
                  LEFT JOIN blobs bx ON bx.sha256 = ib.sha256
                  CROSS JOIN w ON w.hot = 1 AND w.kind = 'image' AND (w.key = ib.sha256 OR w.key = bx.pixel_hash) WHERE sh.reader_id = ?1 AND sh.void_at IS NULL
                UNION ALL
                SELECT w.pos, w.key, w.kind, w.hidden, w.stem_only, gu.publish_id, COALESCE(gu.seq, gq.seq, 0), COALESCE(gq.qid, ''), COALESCE(gq.chain_json, '[]'), gu.now_licenses
                  FROM served sh CROSS JOIN ghost_units gu ON gu.publish_id = sh.publish_id
                  CROSS JOIN w ON w.hot = 1 AND gu.key = w.key AND gu.kind = w.kind AND (w.kind <> 'part' OR gu.part >= 0 OR w.part >= 0)
                  LEFT JOIN ghost_questions gq ON gq.publish_id = gu.publish_id AND gq.pos = gu.pos WHERE sh.reader_id = ?1 AND sh.void_at IS NULL",
        served_form("ib", "ip")
    )
}

fn served_outer(occ: &str) -> String {
    format!(
        "WITH {W_CTE}, occ AS MATERIALIZED ({occ}),
         g AS (
{}
           FROM occ o CROSS JOIN served s ON s.reader_id = ?1 AND s.publish_id = o.publish_id AND s.void_at IS NULL
           JOIN items i ON i.id = s.item_id JOIN users u ON u.id = i.author_id
          WHERE i.author_id <> ?1 AND i.id NOT IN (SELECT value FROM json_each(?3)) AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = i.id)
          GROUP BY o.pos, o.key, i.id)
       SELECT * FROM (SELECT g.*, ROW_NUMBER() OVER (PARTITION BY g.pos, g.key ORDER BY g.seq, g.item_id) AS rk FROM g) WHERE rk <= {UNIT_SOURCES_PER_KEY}",
        outer_sql()
    )
}

fn retained_sql() -> String {
    format!(
        "WITH {W_CTE},
              hit AS MATERIALIZED (
                SELECT w.pos, w.key, w.kind, iq.publish_id, COALESCE(json_extract(iq.unit_seqs, '$.\"' || w.key || '\"'), iq.seq) AS seq, iq.qid, iq.chain_json FROM w CROSS JOIN item_questions iq
                  ON w.kind = 'text' AND w.hidden = 0
                 AND (iq.rowid IN (SELECT x.rowid FROM item_questions x WHERE x.bk_expl = w.key ORDER BY x.rowid LIMIT ?4)
                      OR iq.rowid IN (SELECT x.rowid FROM item_questions x WHERE x.bk_ans = w.key ORDER BY x.rowid LIMIT ?4))
                UNION ALL
                SELECT w.pos, w.key, w.kind, pk.publish_id, uq.seq, uq.qid, uq.chain_json FROM w CROSS JOIN item_part_keys pk
                  ON ((w.kind = 'text' AND w.hidden = 0) OR (w.kind = 'part' AND (pk.part >= 0 OR w.part >= 0)))
                 AND pk.rowid IN (SELECT x.rowid FROM item_part_keys x WHERE x.key = w.key ORDER BY x.rowid LIMIT ?4)
                  JOIN item_questions uq ON uq.publish_id = pk.publish_id AND uq.pos = pk.pos
                UNION ALL
                SELECT w.pos, w.key, w.kind, ib.publish_id, ib.seq, '' AS qid, '[]' AS chain_json FROM w CROSS JOIN item_blobs ib
                  ON w.kind = 'image'
                 AND (ib.rowid IN (SELECT x.rowid FROM item_blobs x WHERE x.sha256 = w.key ORDER BY x.rowid LIMIT ?4)
                      OR ib.rowid IN (SELECT x.rowid FROM item_blobs x WHERE x.sha256 IN (SELECT bx.sha256 FROM blobs bx WHERE bx.pixel_hash = w.key) ORDER BY x.rowid LIMIT ?4)))
         SELECT h.pos, h.key, h.kind, i.id AS item_id, MIN(h.qid) AS qid, i.public_version AS version, i.license, COALESCE(u.display_name, 'deleted teacher') AS author_name,
                MIN(h.seq) AS seq, MAX(h.chain_json) AS chain,
                MAX((SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = w.key)) AS own_seq,
                MAX(i.visibility = 'public' AND i.current_publish_id IS NOT NULL AND i.removed_at IS NULL AND NOT EXISTS (SELECT 1 FROM item_holds hh WHERE hh.item_id = i.id)) AS visible
           FROM hit h CROSS JOIN item_publishes p ON p.publish_id = h.publish_id AND p.status IN ('current', 'previous') JOIN w ON w.pos = h.pos AND w.key = h.key
           JOIN items i ON i.id = p.item_id JOIN users u ON u.id = i.author_id
          WHERE i.author_id <> ?1 AND i.purged_at IS NULL AND i.id NOT IN (SELECT value FROM json_each(?3))
            AND (h.kind <> 'image' OR EXISTS (SELECT 1 FROM item_blobs sb WHERE sb.publish_id = p.publish_id AND {}))
            AND ((i.visibility = 'public' AND i.removed_at IS NULL) OR EXISTS (SELECT 1 FROM served sv WHERE sv.item_id = i.id AND sv.reader_id = ?1))
            AND NOT EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'unit' AND ac.hash = h.key AND ac.first_seq <= h.seq)
          GROUP BY h.pos, h.key, i.id",
        served_form("sb", "p")
    )
}

/// One occurrence of a unit key in an item.
struct Occ {
    pos: usize,
    key: String,
    kind: UnitKind,
    item_id: String,
    qid: String,
    version: i64,
    license: String,
    author_name: String,
    seq: i64,
    chain: Option<String>,
    own_seq: Option<i64>,
    visible: bool,
    // served rows only
    ev_kind: Option<&'static str>,
    received: Vec<String>,
    hidden: bool,
    stem_only: bool,
    src_at: Option<i64>,
    my_upload_at: Option<i64>,
}

fn json_strings(s: Option<String>) -> Vec<String> {
    s.and_then(|s| serde_json::from_str::<Vec<Option<String>>>(&s).ok()).map(|v| v.into_iter().flatten().collect()).unwrap_or_default()
}
fn json_nested(s: Option<String>) -> Vec<String> {
    s.and_then(|s| serde_json::from_str::<Vec<Vec<Option<String>>>>(&s).ok()).map(|v| v.into_iter().flatten().flatten().collect()).unwrap_or_default()
}
fn base_occ(r: &Row) -> rusqlite::Result<Occ> {
    Ok(Occ {
        pos: r.get::<_, i64>("pos")? as usize,
        key: r.get("key")?,
        kind: UnitKind::parse(&r.get::<_, String>("kind")?),
        item_id: r.get("item_id")?,
        qid: r.get::<_, Option<String>>("qid")?.unwrap_or_default(),
        version: r.get::<_, Option<i64>>("version")?.unwrap_or(0),
        license: r.get("license")?,
        author_name: r.get("author_name")?,
        seq: r.get::<_, Option<i64>>("seq")?.unwrap_or(0),
        chain: r.get("chain")?,
        own_seq: r.get("own_seq")?,
        visible: r.get::<_, Option<i64>>("visible")?.unwrap_or(0) == 1,
        ev_kind: None,
        received: vec![],
        hidden: false,
        stem_only: false,
        src_at: None,
        my_upload_at: None,
    })
}

fn units_of(f: &Facts, pixel_of: &HashMap<String, String>) -> Vec<Vec<Unit>> {
    f.img_at
        .iter()
        .enumerate()
        .map(|(i, img)| {
            let mut out: Vec<Unit> = f.served_text[i].iter().map(|k| Unit { key: k.clone(), kind: UnitKind::Text, part: None, hidden: false, stem_only: false }).collect();
            out.extend(f.hidden_text[i].iter().map(|k| Unit { key: k.clone(), kind: UnitKind::Text, part: None, hidden: true, stem_only: false }));
            if let Some(sha) = img {
                out.push(Unit { key: sha.clone(), kind: UnitKind::Image, part: None, hidden: false, stem_only: false });
                if let Some(px) = pixel_of.get(sha) {
                    out.push(Unit { key: px.clone(), kind: UnitKind::Image, part: None, hidden: false, stem_only: false });
                }
            }
            out.extend(f.unit_keys[i].iter().map(|u| Unit { key: u.key.clone(), kind: UnitKind::Part, part: Some(u.part), hidden: false, stem_only: u.stem_only }));
            out
        })
        .collect()
}

/// Looks up every unit of the publish. A preview's lookups count against its own daily allowance, weighted by the units looked up.
#[allow(clippy::too_many_arguments)]
pub fn lookup(x: &PoolCtx, me: &str, dry: bool, actx: &AuthorCtx, f: &Facts, unit_keys_max: usize) -> Res<Units> {
    let mut pixel_of: HashMap<String, String> = HashMap::new();
    let mut shas: Vec<&String> = vec![];
    for s in f.img_at.iter().flatten() {
        if !shas.contains(&s) {
            shas.push(s);
        }
    }
    if !shas.is_empty() {
        let mut st = x.conn.prepare("SELECT sha256, pixel_hash FROM blobs WHERE sha256 IN (SELECT value FROM json_each(?1)) AND pixel_hash IS NOT NULL")?;
        for r in st.query_map([json!(shas).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, b) = r?;
            pixel_of.insert(a, b);
        }
    }
    let units_at = units_of(f, &pixel_of);
    let want: Vec<(usize, &Unit)> = units_at.iter().enumerate().flat_map(|(pos, us)| us.iter().map(move |u| (pos, u))).collect();
    let mut want_keys: Vec<&String> = vec![];
    let mut seen: HashSet<&String> = HashSet::new();
    for (_, u) in &want {
        if seen.insert(&u.key) {
            want_keys.push(&u.key);
        }
    }
    if want_keys.len() > unit_keys_max {
        return Err(PoolError::new(422, "too_many_units")
            .with("limit", json!(unit_keys_max))
            .with("message", json!("This bank has more distinct texts, images and parts than one publish can check. Publish it as several smaller banks."))
            .into());
    }
    let mut out = Units { units_at: units_at.clone(), lifts: HashMap::new() };
    if want.is_empty() {
        return Ok(out);
    }
    if dry {
        x.require_daily(me, "preview_units", want.len() as i64)?;
    }
    // How often each key occurs anywhere, counted only up to OCC_SCAN. A key past that is matched from this account's OWN served content instead, whose
    // size no one else controls (the choice changes the plan, never the answer).
    let mut occ_n: Vec<(String, i64)> = vec![];
    {
        let mut st = x.conn.prepare(OCC_SQL)?;
        for r in st.query_map(params![json!(want_keys).to_string(), OCC_SCAN + 1], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            occ_n.push(r?);
        }
    }
    let over: Vec<&String> = occ_n.iter().filter(|(_, n)| *n > OCC_SCAN).map(|(k, _)| k).collect();
    let mut by_key_after_all: HashSet<String> = HashSet::new();
    if !over.is_empty() {
        let sn: i64 = x.conn.query_row(&format!("SELECT COUNT(*) FROM (SELECT 1 FROM served WHERE reader_id = ?1 LIMIT {})", HOT_SCAN + 1), [me], |r| r.get(0))?;
        let b = sn.min(HOT_SCAN);
        let recount: Vec<&String> = if b > OCC_SCAN { over.iter().take((HOT_BUDGET / b) as usize).copied().collect() } else { vec![] };
        if !recount.is_empty() {
            let mut again: HashMap<String, i64> = HashMap::new();
            let mut st = x.conn.prepare(OCC_SQL)?;
            for r in st.query_map(params![json!(recount).to_string(), b + 1], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
                let (k, n) = r?;
                again.insert(k, n);
            }
            for k in recount {
                if again.get(k).copied().unwrap_or(0) <= b {
                    by_key_after_all.insert(k.clone());
                }
            }
        }
    }
    let hot_keys: HashSet<String> = occ_n.iter().filter(|(k, n)| *n > OCC_SCAN && !by_key_after_all.contains(k)).map(|(k, _)| k.clone()).collect();
    let wj = Value::Array(
        want.iter()
            .map(|(pos, u)| json!({ "pos": pos, "key": u.key, "kind": u.kind.tag(), "part": u.part.unwrap_or(-1), "hidden": u.hidden as i64, "stemOnly": u.stem_only as i64, "hot": hot_keys.contains(&u.key) as i64 }))
            .collect(),
    )
    .to_string();
    let mine = json!(actx.mine.iter().collect::<Vec<_>>()).to_string();

    // (1) What this account was SERVED: the units of every publish it was served (read from that publish's own key set, or its ghost once it is gone).
    let served_q = |occ: &str| -> rusqlite::Result<Vec<Occ>> {
        let mut st = x.conn.prepare(&served_outer(occ))?;
        let rows = st.query_map(params![me, wj, mine], |r| {
            let mut o = base_occ(r)?;
            o.ev_kind = Some(if r.get::<_, String>("ev_kind")? == "import" { "import" } else { "view" });
            let mut rec = json_nested(r.get("lics_nested")?);
            rec.extend(json_nested(r.get("now_lics")?));
            rec.extend(json_strings(r.get("hist_lics")?));
            rec.extend(json_strings(r.get("licenses_json")?));
            let mut seen = HashSet::new();
            o.received = rec.into_iter().filter(|l| seen.insert(l.clone())).collect();
            o.hidden = r.get::<_, i64>("hidden")? == 1;
            o.stem_only = r.get::<_, i64>("stem_only")? == 1;
            o.src_at = r.get("src_at")?;
            o.my_upload_at = r.get("my_upload_at")?;
            Ok(o)
        })?;
        rows.collect()
    };
    let mut served_rows = served_q(&cold_occ())?;
    if !hot_keys.is_empty() {
        served_rows.extend(served_q(&hot_occ())?);
    }
    // (2) Where else it occurs (retained publishes, as served to non-authors): stock, and relays of material this account was never served. Never for
    // hidden-only texts; per key only the earliest UNIT_OCC_WINDOW occurrences are looked at.
    let rows: Vec<Occ> = {
        let mut st = x.conn.prepare(&retained_sql())?;
        let it = st.query_map(params![me, wj, mine, UNIT_OCC_WINDOW], base_occ)?;
        it.collect::<rusqlite::Result<_>>()?
    };
    // Stock: GENERIC_UNIT_AUTHORS or more authors who published the unit INDEPENDENTLY (in a listed item, never served an earlier holder of it, and not
    // descending from one by lineage).
    let occ_all: Vec<&Occ> = rows.iter().chain(served_rows.iter()).collect();
    let mut by_key: HashMap<&str, Vec<&Occ>> = HashMap::new();
    for o in &occ_all {
        by_key.entry(o.key.as_str()).or_default().push(o);
    }
    let all_items: Vec<&String> = {
        let mut v: Vec<&String> = vec![];
        for o in &occ_all {
            if !v.contains(&&o.item_id) {
                v.push(&o.item_id);
            }
        }
        v
    };
    let mut item_author: HashMap<String, String> = HashMap::new();
    let mut served_pairs: HashSet<String> = HashSet::new();
    if !all_items.is_empty() {
        let aj = json!(all_items).to_string();
        let mut st = x.conn.prepare("SELECT id, author_id FROM items WHERE id IN (SELECT value FROM json_each(?1))")?;
        for r in st.query_map([&aj], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, b) = r?;
            item_author.insert(a, b);
        }
        let authors: HashSet<&String> = item_author.values().collect();
        let mut st = x.conn.prepare("SELECT s.reader_id, s.item_id FROM json_each(?1) a CROSS JOIN served s ON s.reader_id = a.value WHERE s.item_id IN (SELECT value FROM json_each(?2))")?;
        for r in st.query_map(params![json!(authors).to_string(), aj], |r| Ok(format!("{}/{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            served_pairs.insert(r?);
        }
    }
    let mut stock_memo: HashMap<String, bool> = HashMap::new();
    let mut stock = |key: &str| -> bool {
        if let Some(m) = stock_memo.get(key) {
            return *m;
        }
        let mut by_item: Vec<&Occ> = vec![];
        for o in by_key.get(key).map(Vec::as_slice).unwrap_or(&[]) {
            match by_item.iter().position(|e| e.item_id == o.item_id) {
                Some(p) => by_item[p] = o,
                None => by_item.push(o),
            }
        }
        by_item.sort_by_key(|o| o.seq);
        let mut indep: HashMap<&String, i64> = HashMap::new();
        // At least GENERIC_UNIT_AUTHORS - 1 of those published it BEFORE the earliest source this account was served (or, served none, before its first holder).
        let served_seqs: Vec<i64> = served_rows.iter().filter(|o| o.key == key).map(|o| o.seq).collect();
        let first_source = served_seqs.iter().copied().min().unwrap_or_else(|| by_item.first().map_or(0, |o| o.seq));
        for (k, o) in by_item.iter().enumerate() {
            let Some(author) = item_author.get(&o.item_id) else { continue };
            if !o.visible {
                continue;
            }
            let lineage: Vec<String> = serde_json::from_str::<Vec<Value>>(o.chain.as_deref().unwrap_or("[]"))
                .map(|ls| ls.iter().filter(|l| !l.get("earlier_match").is_some_and(|e| e.as_bool() == Some(true))).filter_map(|l| l.get("item_id").and_then(Value::as_str).map(String::from)).collect())
                .unwrap_or_default();
            let earlier = by_item[..k].iter().filter(|e| e.seq < o.seq && item_author.get(&e.item_id) != Some(author));
            let mut blocked = false;
            for e in earlier {
                if served_pairs.contains(&format!("{author}/{}", e.item_id)) || lineage.contains(&e.item_id) {
                    blocked = true;
                    break;
                }
            }
            if blocked {
                continue;
            }
            let cur = indep.entry(author).or_insert(i64::MAX);
            *cur = (*cur).min(o.seq);
        }
        let out = indep.len() >= GENERIC_UNIT_AUTHORS && indep.values().filter(|s| **s < first_source).count() >= GENERIC_UNIT_AUTHORS - 1;
        stock_memo.insert(key.to_string(), out);
        out
    };
    let mut seen: HashSet<String> = HashSet::new();
    for o in &served_rows {
        let l = Lift {
            item_id: o.item_id.clone(),
            qid: o.qid.clone(),
            version: o.version,
            license: o.license.clone(),
            author_name: o.author_name.clone(),
            seq: o.seq,
            chain: o.chain.clone().unwrap_or_else(|| "[]".into()),
            kind: o.kind,
            key: o.key.clone(),
            served: true,
            ev_kind: o.ev_kind,
            received: o.received.clone(),
            stock: stock(&o.key),
            own_first: o.own_seq.is_some_and(|s| s <= o.seq),
            visible: o.visible,
            uploaded_first: o.kind == UnitKind::Image && matches!((o.my_upload_at, o.src_at), (Some(a), Some(b)) if a < b),
            hidden: o.hidden,
            stem_only: o.stem_only,
        };
        seen.insert(format!("{}/{}/{}", o.pos, o.item_id, o.key));
        // (A source deriving from this account's item may still add its OWN material: only the account's own earlier use of the very unit exempts it.)
        if l.own_first || l.stock {
            continue;
        }
        out.lifts.entry(o.pos).or_default().push(l);
    }
    // A retained occurrence in an item this account has a serve record of whose publish's units are not known (a serve of a publish gone since, whose ghost
    // kept question hashes only) binds as SERVED, with the licences that record holds. An item never served is the soft "earlier publication" path.
    let mut rec: HashMap<String, (&'static str, Vec<String>)> = HashMap::new();
    let mut relay: Vec<&String> = vec![];
    for o in &rows {
        if !relay.contains(&&o.item_id) {
            relay.push(&o.item_id);
        }
    }
    if !relay.is_empty() {
        let mut st = x.conn.prepare(
            "SELECT item_id, MAX(kind = 'import') AS imp, json_group_array(DISTINCT l) AS lics FROM (
               SELECT s.item_id, s.kind, l.value AS l FROM json_each(?2) it CROSS JOIN served s ON s.item_id = it.value AND s.reader_id = ?1, json_each(s.licenses_json) l
                WHERE s.void_at IS NULL
                  AND NOT EXISTS (SELECT 1 FROM item_publishes kp WHERE kp.publish_id = s.publish_id)
                  AND NOT EXISTS (SELECT 1 FROM ghost_publishes kg WHERE kg.publish_id = s.publish_id AND kg.units_known = 1)
               UNION ALL
               SELECT im.item_id, im.kind, l.value FROM imports im, json_each(COALESCE(im.licenses_json, json_array(im.license))) l WHERE im.user_id = ?1 AND im.item_id IN (SELECT value FROM json_each(?2))
                  AND NOT EXISTS (SELECT 1 FROM served v WHERE v.reader_id = ?1 AND v.item_id = im.item_id))
             GROUP BY item_id",
        )?;
        for r in st.query_map(params![me, json!(relay).to_string()], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, String>(2)?)))? {
            let (item, imp, lics) = r?;
            let lics: Vec<String> = serde_json::from_str(&lics).unwrap_or_default();
            rec.insert(item, (if imp == Some(1) { "import" } else { "view" }, lics));
        }
    }
    for o in &rows {
        if seen.contains(&format!("{}/{}/{}", o.pos, o.item_id, o.key)) {
            continue;
        }
        let had = rec.get(&o.item_id);
        let l = Lift {
            item_id: o.item_id.clone(),
            qid: o.qid.clone(),
            version: o.version,
            license: o.license.clone(),
            author_name: o.author_name.clone(),
            seq: o.seq,
            chain: o.chain.clone().unwrap_or_else(|| "[]".into()),
            kind: o.kind,
            key: o.key.clone(),
            served: had.is_some(),
            ev_kind: had.map(|h| h.0),
            received: had.map(|h| h.1.clone()).unwrap_or_default(),
            stock: stock(&o.key),
            own_first: o.own_seq.is_some_and(|s| s <= o.seq),
            visible: o.visible,
            uploaded_first: false,
            hidden: false,
            stem_only: false,
        };
        if l.own_first || l.stock {
            continue;
        }
        out.lifts.entry(o.pos).or_default().push(l);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "publishunits_tests.rs"]
mod tests;
