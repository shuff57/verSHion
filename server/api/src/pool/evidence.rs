//! The evidence for every position of a publish (worker/src/pool/provenance.ts `evidence`): which source items this account
//! was served share a question with each position, aggregated in SQL. Every rule that does not compare content with the
//! subject is applied in SQL BEFORE anything is ranked or limited; the content comparisons are `same_sql` / `near_sql`.
//! The SQL below is the Worker's, statement for statement: the rules live in it, and the cutover copies the Worker's rows.
use super::chain::ChainLink;
use super::hit::ImportHit;
use super::visible::{SubjectParam, near_sql, same_sql};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Source items examined per question: a FIXED allowance, whatever the size of the bank.
pub const EVIDENCE_PER_POSITION: i64 = 32;
/// Credits kept per question: distinct contributors, whatever the size of the bank.
pub const CREDITS_MAX: i64 = 64;
/// ... of which the first CREDITS_HEAD (the originators) are always kept, then the most recent.
pub const CREDITS_HEAD: i64 = 8;
/// The worst-case result one evidence query may hold, reserved before it runs; the positions per query follow from it.
pub const EVIDENCE_RESERVE: usize = 8 * 1024 * 1024;
pub const EVIDENCE_ROW_BYTES: usize = 8 * 1024;

/// Near sources left out of a question: their licences, the first such item and how many.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuzzyCut {
    pub lics: Vec<String>,
    pub item: String,
    pub n: i64,
    /// For each licence, the first item left out that carries it (named by a refusal).
    pub by: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CRoot {
    pub r: String,
    pub s: i64,
    pub n: String,
}

/// What the evidence query needs to know about one position, besides its subject (all key-free).
#[derive(Clone, Debug, Serialize)]
pub struct PositionFacts {
    #[serde(flatten)]
    pub subject: SubjectParam,
    /// The publisher's first publish of this question, or None.
    pub first: Option<i64>,
    /// The subject's full content is the publisher's own earlier SHOWN content.
    pub ownshown: bool,
    /// A short statement (treated as common for every source).
    pub common: bool,
    /// The key is itself a work (a Short's model answer): key-free equality never makes the author first.
    pub keywork: i64,
    /// The served question is 40 words or fewer (independence may then cover a read).
    pub short40: i64,
    /// Independent roots with their first publish: common for a source only if K of them, not its own, came first.
    pub croots: Vec<CRoot>,
    pub ck: i64,
    /// The publisher published this question (key-free) before, hidden or purged included.
    pub ownkf: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Credits {
    pub links: Vec<ChainLink>,
    pub total: i64,
    pub more: Vec<String>,
    pub missing: i64,
}

#[derive(Debug, Default)]
pub struct Evidence {
    pub by_pos: HashMap<usize, Vec<ImportHit>>,
    /// Positions with more admissible source items than were examined, with how many there were.
    pub overflow: HashMap<usize, i64>,
    /// Per question, the hard sources left out past the allowance (their licences).
    pub cut: HashMap<usize, FuzzyCut>,
    /// Per position, the merged credits of the sources a chain follows, oldest first.
    pub credits: HashMap<usize, Credits>,
}

pub struct EvidenceOpts<'a> {
    pub mine: &'a [String],
    pub independent: &'a [String],
}

pub fn evidence(conn: &Connection, me: &str, subjects: &[PositionFacts], o: &EvidenceOpts) -> rusqlite::Result<Evidence> {
    let mut out = Evidence::default();
    // A big bank is read a chunk of positions at a time, each chunk's worst case within the reserve.
    let chunk = (EVIDENCE_RESERVE / (EVIDENCE_ROW_BYTES * (EVIDENCE_PER_POSITION as usize + 1))).max(1);
    let mut off = 0;
    while off < subjects.len() {
        let part = &subjects[off..(off + chunk).min(subjects.len())];
        let e = evidence_chunk(conn, me, part, o)?;
        for (k, v) in e.by_pos {
            out.by_pos.insert(k + off, v.into_iter().map(|h| ImportHit { pos: h.pos + off as i64, ..h }).collect());
        }
        out.overflow.extend(e.overflow.into_iter().map(|(k, v)| (k + off, v)));
        out.cut.extend(e.cut.into_iter().map(|(k, v)| (k + off, v)));
        out.credits.extend(e.credits.into_iter().map(|(k, v)| (k + off, v)));
        off += chunk;
    }
    Ok(out)
}

fn hit_branch(v: &str, last: bool) -> String {
    format!(
        "
      SELECT s.pos, 0 AS by_ref, {same} AS same, {near} AS near, (iq.keyfree_hash = s.kf) AS kf_same, s.first AS sfirst, s.full AS sfull, s.ownshown AS sown, s.common AS sshort, s.croots AS scroots, s.ck AS sck, s.ownkf AS sownkf, s.noexpl AS snoexpl, s.keywork AS skeywork, s.short40 AS sshort40, iq.*
        FROM subj s CROSS JOIN {v} iq ON iq.user_id = ?1 AND iq.match_hash = s.mh
      UNION ALL
      SELECT s.pos, 1, {same}, {near}, (iq.keyfree_hash = s.kf), s.first, s.full, s.ownshown, s.common, s.croots, s.ck, s.ownkf, s.noexpl, s.keywork, s.short40, iq.*
        FROM subj s CROSS JOIN json_each(s.refs) r CROSS JOIN served sv ON sv.reader_id = ?1 AND sv.item_id = r.value ->> '$.item'
             CROSS JOIN {v} iq ON iq.src_publish_id = sv.publish_id AND iq.user_id = ?1 AND iq.qid = r.value ->> '$.qid'{tail}",
        same = same_sql("s", "iq"),
        near = near_sql("s", "iq"),
        tail = if last { "" } else { "\n      UNION ALL" }
    )
}

fn evidence_sql() -> String {
    let hits = format!("{}{}", hit_branch("import_questions_live", false), hit_branch("import_questions_ghost", true));
    format!(
        "WITH subj AS MATERIALIZED (SELECT CAST(j.key AS INTEGER) AS pos, j.value ->> '$.mh' AS mh, j.value ->> '$.full' AS full, j.value ->> '$.kf' AS kf, j.value -> '$.refs' AS refs,
                                          j.value ->> '$.nfull' AS nfull, j.value ->> '$.nkf' AS nkf, j.value ->> '$.nbare' AS nbare, COALESCE(j.value ->> '$.noexpl', 0) AS noexpl,
                                          j.value ->> '$.first' AS first, COALESCE(j.value ->> '$.ownshown', 0) AS ownshown, COALESCE(j.value ->> '$.common', 0) AS common,
                                          COALESCE(j.value ->> '$.ownkf', 0) AS ownkf, COALESCE(j.value -> '$.croots', '[]') AS croots, COALESCE(j.value ->> '$.ck', 3) AS ck,
                                          COALESCE(j.value ->> '$.keywork', 0) AS keywork, COALESCE(j.value ->> '$.short40', 0) AS short40 FROM json_each(?2) j),
    mine AS MATERIALIZED (SELECT value AS id FROM json_each(?4)),
    indep AS MATERIALIZED (SELECT value AS id FROM json_each(?5)),
    bare AS MATERIALIZED (SELECT item_id AS id FROM void_items WHERE replaced_by IS NULL),
    -- (The served publishes' own rows, live or ghost, each driven from its hash index.)
    hit AS ({hits}),
    -- \"Common\": K independent roots other than the source's own that served the SAME content as this serve (licence
    -- sense: choices, shown keys, explanation, sample) before it, never the bare stem. \"First\": the publisher's own first
    -- publish of THIS content (shown, or key-free unless the key is itself a work: a Short's model answer), never of a stem.
    hitc AS (
      SELECT h.*, (h.sshort = 1 OR (SELECT COUNT(DISTINCT cr.value ->> '$.r') FROM json_each(h.scroots) cr
                                     WHERE CAST(cr.value ->> '$.s' AS INTEGER) < h.source_first_seq AND cr.value ->> '$.n' = h.near_hash
                                       AND cr.value ->> '$.r' <> COALESCE((SELECT ri.author_id FROM items ri WHERE ri.id = NULLIF(CASE WHEN COALESCE(h.chain_json ->> '$[0].earlier_match', 0) THEN '' ELSE h.chain_json ->> '$[0].item_id' END, '')), h.author_id)) >= h.sck) AS scommon,
                  (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1
                      AND ((ac.kind = 'nshown' AND ac.hash = h.near_hash) OR (h.skeywork = 0 AND ac.kind = 'kf' AND ac.hash = h.near_kf))) AS owncf,
                  -- The publisher served THIS content (licence sense) first: the source added nothing of its own.
                  EXISTS (SELECT 1 FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'nshown' AND ac.hash = h.near_hash AND ac.first_seq <= h.source_first_seq) AS ownnear,
                  -- The publisher's first publish of this question key-free (its served form, hidden key or not): a copy
                  -- served to her later never binds her.
                  (SELECT MIN(ac.first_seq) FROM author_contents ac WHERE ac.author_id = ?1 AND ac.kind = 'kf' AND ac.hash = h.near_kf) AS ownkfseq,
                  -- An exact copy of something only READ, of a short or generic question (40 words or fewer as served, or
                  -- 2+ independent roots have it), CAN be attested independent: it then binds nothing and is credited as a
                  -- possible source (badge claimed_independent).
                  (h.item_id IN (SELECT id FROM indep) AND h.kind = 'view' AND h.near = 1
                     AND (h.sshort40 = 1 OR (SELECT COUNT(DISTINCT cr.value ->> '$.r') FROM json_each(h.scroots) cr WHERE cr.value ->> '$.n' = h.near_hash) >= 2)) AS claimed
        FROM hit h),
    rows AS MATERIALIZED (
      SELECT h.*, EXISTS (SELECT 1 FROM json_each(h.chain_json) cl WHERE cl.value ->> '$.item_id' IN (SELECT id FROM mine)) AS derived FROM hitc h
       WHERE h.author_id <> ?1 AND h.void_at IS NULL
         AND h.item_id NOT IN (SELECT id FROM mine)
         AND NOT EXISTS (SELECT 1 FROM void_items v WHERE v.item_id = h.item_id)
         AND (h.same OR NOT EXISTS (SELECT 1 FROM json_each(h.chain_json) cl WHERE cl.value ->> '$.item_id' IN (SELECT id FROM bare)))
         AND (h.kind = 'import' OR h.by_ref = 1 OR h.scommon = 0)
         -- Independence (reads and imports alike): never for an exact copy (in the licence sense) of what the account was
         -- served; only for a common question (or a short statement), a changed copy, or the publisher's own question
         -- first (the original author). (A claimed row passes this: an exact, uncommon copy that is not the publisher's own first.)
         AND NOT (h.item_id IN (SELECT id FROM indep) AND (h.scommon = 1 OR h.near = 0 OR h.sownkf = 1))),
    adm AS MATERIALIZED (
      SELECT * FROM rows
       WHERE NOT ((derived OR (owncf IS NOT NULL AND COALESCE(source_first_seq, 0) >= owncf))
                  -- The exception (the publisher copies a later source's ADDITIONS) never applies when the publisher
                  -- published this question first and the source added no explanation, only the key (key-free, so a hidden
                  -- key decides nothing). A copy in the licence sense (points or difficulty changed) too, not only a
                  -- byte-identical one.
                  AND NOT (sfull IS NOT NULL AND near AND form = 'full' AND sown = 0 AND ownnear = 0 AND NOT (sownkf = 1 AND snoexpl = 1)))
         -- (Whatever it added; copying its explanation or model answer is a lift, publish.ts.)
         AND NOT (skeywork = 0 AND ownkfseq IS NOT NULL AND COALESCE(source_first_seq, 0) >= ownkfseq)),
    g AS (SELECT pos, item_id, MIN(qid) AS qid, MIN(match_hash) AS match_hash, MAX(by_ref) AS by_ref, MIN(by_ref) = 0 AS by_match, MAX(same) AS same, MAX(near) AS near,
                 MAX(same AND form = 'full') AS same_full, MAX(same AND kind = 'import') AS same_imported, MAX(kf_same) AS kf_same,
                 MIN(COALESCE(source_first_seq, 0)) AS first_seq, MAX(kind = 'import') AS imported, MAX(version) AS version,
                 MAX(printf('%016d|%s', at, author_name)) AS an, MAX(printf('%016d|%s', at, license)) AS ll, MAX(stem_hash) AS stem_hash,
                 MAX(printf('%016d|', at) || chain_json) AS chain_latest, MIN(at) AS first_at, json_group_array(DISTINCT content_hash) AS contents, MIN(claimed) AS claimed
            FROM adm GROUP BY pos, item_id),
    -- Copies (licence sense) first, EARLIEST first: past the allowance only later copies or adaptations are left out, which
    -- never change a copy's outcome (its earliest original decides; adaptations beside a copy do not narrow it). Without a
    -- copy, an overflow is refused (publish.ts): a cap never skips.
    ranked AS (SELECT g.*, ROW_NUMBER() OVER (PARTITION BY pos ORDER BY near DESC, CASE WHEN near THEN first_seq END, by_ref DESC, same DESC, imported DESC, first_seq, first_at, item_id) AS rn,
                      COUNT(*) OVER (PARTITION BY pos) AS cnt FROM g),
    keep AS MATERIALIZED (SELECT * FROM ranked WHERE rn <= ?3),
    -- The hard sources left out (a copy, an import or a declared origin): their licences (as shown and in their chains), so
    -- the publish can tell whether one would forbid the licence chosen.
    cutx0 AS MATERIALIZED (SELECT pos, item_id, v FROM (
                 SELECT a.pos, a.item_id, l.value AS v FROM ranked rk JOIN adm a ON a.pos = rk.pos AND a.item_id = rk.item_id, json_each(a.licenses_json) l
                  WHERE rk.rn > ?3 AND (rk.near = 1 OR rk.imported = 1 OR rk.by_ref = 1)
                 UNION ALL
                 SELECT a.pos, a.item_id, cl.value ->> '$.license' FROM ranked rk JOIN adm a ON a.pos = rk.pos AND a.item_id = rk.item_id, json_each(a.chain_json) cl
                  WHERE rk.rn > ?3 AND (rk.near = 1 OR rk.imported = 1 OR rk.by_ref = 1) AND COALESCE(cl.value ->> '$.license', '') <> '')),
    cutx AS MATERIALIZED (SELECT pos, json_group_array(v) AS lics, json_group_object(v, item) AS byl, MIN(item) AS item, (SELECT COUNT(DISTINCT c2.item_id) FROM cutx0 c2 WHERE c2.pos = cl1.pos) AS n
                            FROM (SELECT pos, v, MIN(item_id) AS item FROM cutx0 GROUP BY pos, v) cl1 GROUP BY pos),
    ka AS MATERIALIZED (SELECT a.* FROM adm a JOIN keep k ON k.pos = a.pos AND k.item_id = a.item_id),
    la AS MATERIALIZED (SELECT pos, item_id, json_group_array(DISTINCT l.value) AS lic_all, json_group_array(DISTINCT CASE WHEN same THEN l.value END) AS lic_same,
                               json_group_array(DISTINCT CASE WHEN near THEN l.value END) AS lic_near
                          FROM ka, json_each(ka.licenses_json) l GROUP BY pos, item_id),
    lg AS MATERIALIZED (SELECT pos, item_id, json_group_array(json(g)) AS lic_groups FROM (
                          SELECT pos, item_id, content_hash, json_group_array(DISTINCT l.value) AS g FROM ka, json_each(ka.licenses_json) l GROUP BY pos, item_id, content_hash)
                         GROUP BY pos, item_id),
    vs AS MATERIALIZED (SELECT pos, item_id, json_group_array(DISTINCT v.value) AS versions FROM ka, json_each(ka.versions_json) v GROUP BY pos, item_id),
    -- Chain licences per source (every serve's chain): own links grant nothing; a void link grants as its rightful stand-in
    -- (the licence at void time), or nothing without one.
    cl0 AS MATERIALIZED (SELECT ka.pos, ka.item_id AS src, cl.value ->> '$.item_id' AS lid, cl.value ->> '$.license' AS lic, cl.value -> '$.collapsed_licenses' AS col FROM ka, json_each(ka.chain_json) cl),
    cl1 AS (SELECT c0.pos, c0.src, c0.col,
                   CASE WHEN v.item_id IS NULL THEN c0.lic
                        WHEN v.replaced_by IS NULL OR v.replaced_by IN (SELECT id FROM mine) THEN NULL
                        ELSE COALESCE(v.replaced_license, (SELECT license FROM items WHERE id = v.replaced_by)) END AS lic
              FROM cl0 c0 LEFT JOIN void_items v ON v.item_id = c0.lid
             WHERE c0.lid IS NULL OR c0.lid = '' OR c0.lid NOT IN (SELECT id FROM mine)),
    cg AS MATERIALIZED (SELECT pos, src AS item_id, json_group_array(DISTINCT x) AS chain_lics FROM (
                          SELECT pos, src, lic AS x FROM cl1 WHERE lic IS NOT NULL AND lic <> ''
                          UNION ALL SELECT cl1.pos, cl1.src, cv.value FROM cl1, json_each(COALESCE(cl1.col, '[]')) cv) GROUP BY pos, src),
    -- Credits: the chains of the sources a chain follows, merged per item.
    ps AS (SELECT pos, MAX(same) AS any_same FROM keep GROUP BY pos),
    cr0 AS (
      SELECT k.pos, printf('%020d|%s|%06d', k.first_seq, k.item_id, CAST(l.key AS INTEGER)) AS ord, l.value AS link, l.value ->> '$.item_id' AS lid
        FROM keep k JOIN ps ON ps.pos = k.pos CROSS JOIN json_each(substr(k.chain_latest, 18)) l WHERE k.same = 1 OR ps.any_same = 0
      UNION ALL
      SELECT k.pos, printf('%020d|%s|%06d', k.first_seq, k.item_id, 999999),
             json_object('author_name', substr(k.an, 18), 'item_id', k.item_id, 'qid', k.qid, 'version', k.version, 'license', substr(k.ll, 18), 'modified', json(CASE WHEN k.same THEN 'false' ELSE 'true' END)), k.item_id
        FROM keep k JOIN ps ON ps.pos = k.pos WHERE k.same = 1 OR ps.any_same = 0),
    cr1 AS MATERIALIZED (SELECT pos, ord, link, CASE WHEN COALESCE(lid, '') = '' THEN link ELSE lid END AS gk FROM cr0 WHERE COALESCE(lid, '') = '' OR lid NOT IN (SELECT id FROM mine)),
    cm AS MATERIALIZED (SELECT pos, gk, MIN(ord) AS ord, MAX(COALESCE(link ->> '$.modified', 0)) AS anymod FROM cr1 GROUP BY pos, gk),
    cp AS (SELECT pos, gk, link, ROW_NUMBER() OVER (PARTITION BY pos, gk ORDER BY CAST(COALESCE(link ->> '$.version', 0) AS INTEGER), ord) AS r FROM cr1),
    cred AS (SELECT cm.pos, cm.ord, json_set(cp.link, '$.modified', json(CASE WHEN cm.anymod THEN 'true' ELSE 'false' END)) AS link,
                    ROW_NUMBER() OVER (PARTITION BY cm.pos ORDER BY cm.ord) AS n, COUNT(*) OVER (PARTITION BY cm.pos) AS total
               FROM cm JOIN cp ON cp.pos = cm.pos AND cp.gk = cm.gk AND cp.r = 1),
    -- Past CREDITS_MAX: the first CREDITS_HEAD (the originators) and the most recent rest are kept; what is between is one
    -- placeholder with the licences it carried.
    credj AS (SELECT pos, json_group_array(json(link)) AS links, MAX(total) AS total FROM (SELECT * FROM cred WHERE n <= ?7 OR n > total - (?6 - ?7) ORDER BY pos, ord) GROUP BY pos),
    -- Past the credit allowance, what the left-out links granted is kept (a placeholder's licences). How many contributors
    -- the left-out links stand for (a placeholder among them counts its own number).
    credn AS (SELECT pos, SUM(CASE WHEN COALESCE(link ->> '$.item_id', '') = '' AND link ->> '$.author_name' GLOB 'and [0-9]* contributors between'
                                   THEN CAST(substr(link ->> '$.author_name', 5) AS INTEGER) ELSE 1 END) AS missing
                FROM cred WHERE n > ?7 AND n <= total - (?6 - ?7) GROUP BY pos),
    credm AS (SELECT pos, json_group_array(DISTINCT x) AS more FROM (
                SELECT pos, link ->> '$.license' AS x FROM cred WHERE n > ?7 AND n <= total - (?6 - ?7)
                UNION ALL SELECT cred.pos, cv.value FROM cred, json_each(COALESCE(cred.link -> '$.collapsed_licenses', '[]')) cv WHERE cred.n > ?7 AND cred.n <= cred.total - (?6 - ?7))
               WHERE x IS NOT NULL AND x <> '' GROUP BY pos)
    SELECT k.pos, k.item_id, k.qid, k.match_hash, k.by_ref, k.by_match, k.same, k.near, k.same_full, k.same_imported, k.kf_same, k.first_seq, k.imported, k.version, k.an, k.ll,
           k.stem_hash, json_array_length(substr(k.chain_latest, 18)) AS chain_len, k.contents, k.rn, k.cnt, k.claimed,
           la.lic_all, la.lic_same, la.lic_near, lg.lic_groups, vs.versions, cg.chain_lics,
           CASE WHEN k.rn = 1 THEN (SELECT json_object('lics', json(cx.lics), 'item', cx.item, 'n', cx.n, 'by', json(cx.byl)) FROM cutx cx WHERE cx.pos = k.pos) END AS cutj,
           CASE WHEN k.rn = 1 THEN credj.links END AS credits, CASE WHEN k.rn = 1 THEN credj.total END AS credits_total, CASE WHEN k.rn = 1 THEN credm.more END AS credits_more, CASE WHEN k.rn = 1 THEN credn.missing END AS credits_missing FROM keep k
      LEFT JOIN la ON la.pos = k.pos AND la.item_id = k.item_id
      LEFT JOIN lg ON lg.pos = k.pos AND lg.item_id = k.item_id
      LEFT JOIN vs ON vs.pos = k.pos AND vs.item_id = k.item_id
      LEFT JOIN cg ON cg.pos = k.pos AND cg.item_id = k.item_id
      LEFT JOIN credj ON credj.pos = k.pos
      LEFT JOIN credm ON credm.pos = k.pos
      LEFT JOIN credn ON credn.pos = k.pos
     ORDER BY k.pos, k.rn"
    )
}

/// `JSON.parse(v ?? "[]")` filtered to strings (a null inside a group is dropped).
fn strs(v: Option<String>) -> Vec<String> {
    let parsed: Vec<serde_json::Value> = v.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    parsed.into_iter().filter_map(|x| x.as_str().map(String::from)).collect()
}

fn evidence_chunk(conn: &Connection, me: &str, subjects: &[PositionFacts], o: &EvidenceOpts) -> rusqlite::Result<Evidence> {
    let mut out = Evidence::default();
    if subjects.is_empty() {
        return Ok(out);
    }
    let mut st = conn.prepare(&evidence_sql())?;
    let mut rows = st.query(params![
        me,
        serde_json::to_string(subjects).unwrap(),
        EVIDENCE_PER_POSITION,
        serde_json::to_string(o.mine).unwrap(),
        serde_json::to_string(o.independent).unwrap(),
        CREDITS_MAX,
        CREDITS_HEAD
    ])?;
    while let Some(r) = rows.next()? {
        let pos: i64 = r.get("pos")?;
        let an: String = r.get("an")?;
        let ll: String = r.get("ll")?;
        let lic_groups: Vec<serde_json::Value> = r.get::<_, Option<String>>("lic_groups")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let versions: Vec<i64> = r.get::<_, Option<String>>("versions")?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let flag = |n: &str| -> rusqlite::Result<bool> { Ok(r.get::<_, Option<i64>>(n)? == Some(1)) };
        let h = ImportHit {
            pos,
            match_hash: r.get("match_hash")?,
            qid: r.get("qid")?,
            item_id: r.get("item_id")?,
            chain_len: r.get::<_, Option<i64>>("chain_len")?.unwrap_or(0),
            chain_lics: strs(r.get("chain_lics")?),
            author_name: an.chars().skip(17).collect(),
            license: ll.chars().skip(17).collect(),
            version: r.get::<_, Option<i64>>("version")?.unwrap_or(0),
            versions,
            source_first_seq: r.get("first_seq")?,
            stem_hash: r.get("stem_hash")?,
            kind: if r.get::<_, Option<i64>>("imported")?.unwrap_or(0) != 0 { "import".into() } else { "view".into() },
            same: flag("same")?,
            same_full: flag("same_full")?,
            same_imported: flag("same_imported")?,
            near: flag("near")?,
            kf_same: flag("kf_same")?,
            lic_all: strs(r.get("lic_all")?),
            lic_same: strs(r.get("lic_same")?),
            lic_near: strs(r.get("lic_near")?),
            lic_groups: lic_groups.into_iter().map(|g| g.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()).collect(),
            contents: strs(r.get("contents")?),
            by_ref: flag("by_ref")?,
            by_match: flag("by_match")?,
            fuzzy: false,
            claimed: flag("claimed")?,
        };
        let cnt: i64 = r.get("cnt")?;
        if cnt > EVIDENCE_PER_POSITION {
            out.overflow.insert(pos as usize, cnt);
        }
        if let Some(cj) = r.get::<_, Option<String>>("cutj")? {
            out.cut.insert(pos as usize, serde_json::from_str(&cj).unwrap_or_default());
        }
        if let Some(cr) = r.get::<_, Option<String>>("credits")? {
            out.credits.insert(
                pos as usize,
                Credits {
                    links: serde_json::from_str(&cr).unwrap_or_default(),
                    total: r.get::<_, Option<i64>>("credits_total")?.unwrap_or(0),
                    more: strs(r.get("credits_more")?),
                    missing: r.get::<_, Option<i64>>("credits_missing")?.unwrap_or(0),
                },
            );
        }
        out.by_pos.entry(pos as usize).or_default().push(h);
    }
    Ok(out)
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
mod tests;
