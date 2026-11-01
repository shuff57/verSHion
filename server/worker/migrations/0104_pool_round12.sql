-- Round 12 (critics 16a / 16b). Never edit 0102 or 0103 once applied; this file adds to them.

-- Pre-flight (round 20, critic25b F5): this migration runs once. D1 records it in d1_migrations; applied
-- by hand a second time, it stops HERE (the table exists) before changing anything. To undo it, restore
-- the database (D1 Time Travel, or the export taken before applying: see README "Migration 0104").
CREATE TABLE pool_migration_0104_already_applied (at INTEGER);

-- M3: when an account was FIRST served an item (imports.at moves with every re-read).
ALTER TABLE imports ADD COLUMN first_at INTEGER;
UPDATE imports SET first_at = at WHERE first_at IS NULL;

-- M1: everything a question adds to its stem, as served (choices, shown keys, explanation, sample):
-- the qid lineage and the author's own rewordings need the same body. NULL for rows published before
-- this migration: such rows never continue a lineage (conservative), they still count as content.
ALTER TABLE item_questions ADD COLUMN body_hash TEXT;

-- M1: author_contents gains 'nshown' (the licence-sense content the author served), the only
-- content that makes an author "first"; 'stem' (round 11, plantable) is no longer written or read.
CREATE TABLE author_contents_new (
  author_id  TEXT NOT NULL,
  match_hash TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('shown', 'kf', 'stem', 'nshown', 'unit')),
  hash       TEXT NOT NULL,
  first_seq  INTEGER NOT NULL,
  PRIMARY KEY (author_id, kind, hash, match_hash)
);
INSERT INTO author_contents_new SELECT author_id, match_hash, kind, hash, first_seq FROM author_contents WHERE kind <> 'stem';
-- Backfill 'nshown' from every retained publish (the durable record starts here).
INSERT OR IGNORE INTO author_contents_new (author_id, match_hash, kind, hash, first_seq)
  SELECT p.author_id, iq.match_hash, 'nshown', CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END, MIN(iq.seq)
    FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id
   WHERE p.status IN ('current', 'previous') AND CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END IS NOT NULL
   GROUP BY p.author_id, iq.match_hash, CASE WHEN p.hide_key = 1 THEN iq.near_stripped ELSE iq.near_full END;
DROP TABLE author_contents;
ALTER TABLE author_contents_new RENAME TO author_contents;
CREATE INDEX author_contents_match ON author_contents (author_id, match_hash);

-- M4 / M6: a question's block keys besides its exact match_hash (shared.ts QKEY_COLS): the
-- normalised skeleton, full content, explanation and model / accepted answers. Rows from before this
-- migration get their normalised keys from the cron backfill (items.fold_rev = 0); until then they
-- match blocks by match_hash only, as before.
ALTER TABLE item_questions ADD COLUMN bk_norm TEXT;
ALTER TABLE item_questions ADD COLUMN bk_full TEXT;
ALTER TABLE item_questions ADD COLUMN bk_expl TEXT;
ALTER TABLE item_questions ADD COLUMN bk_ans TEXT;
-- Round 13: the normalised stem alone (`s:`), so student data in a stem is held whatever the answer.
ALTER TABLE item_questions ADD COLUMN bk_stem TEXT;
CREATE INDEX item_questions_bk_stem ON item_questions (bk_stem);
UPDATE item_questions SET bk_full = 'f:' || near_full WHERE near_full IS NOT NULL;
CREATE INDEX item_questions_bk_norm ON item_questions (bk_norm);
CREATE INDEX item_questions_bk_full ON item_questions (bk_full);
CREATE INDEX item_questions_bk_expl ON item_questions (bk_expl);
CREATE INDEX item_questions_bk_ans ON item_questions (bk_ans);

-- M4: each block row remembers the question (match_hash) it was selected from, so an unblock by the
-- question lifts every key blocked for it.
ALTER TABLE privacy_blocks ADD COLUMN qhash TEXT;
ALTER TABLE blocked_hashes ADD COLUMN qhash TEXT;
UPDATE privacy_blocks SET qhash = hash WHERE qhash IS NULL;
UPDATE blocked_hashes SET qhash = hash WHERE qhash IS NULL;
CREATE INDEX privacy_blocks_qhash ON privacy_blocks (qhash);
CREATE INDEX blocked_hashes_qhash ON blocked_hashes (qhash);

-- M4: an admin's release of one hold of one item, with the block keys it used then (its later
-- publishes are not held again by those keys; new blocked material still holds).
CREATE TABLE hold_releases (
  item_id   TEXT NOT NULL,
  source    TEXT NOT NULL,
  keys_json TEXT NOT NULL,
  at        INTEGER NOT NULL,
  by        TEXT NOT NULL,
  note      TEXT,
  PRIMARY KEY (item_id, source)
);

-- M6: the stem bands of privacy-blocked questions, so a near-rewording of confirmed student-data text
-- raises an admin flag (privacy_text_match, near) at publish.
CREATE TABLE privacy_bands (
  band    TEXT NOT NULL,
  qhash   TEXT NOT NULL,
  tokens  TEXT,
  item_id TEXT NOT NULL,
  at      INTEGER NOT NULL,
  PRIMARY KEY (band, qhash)
);
CREATE INDEX privacy_bands_qhash ON privacy_bands (qhash);

-- Browse search (round 12): the walk starts from the word index; served_words also holds the words of
-- the title, subject and tags (rebuilt for existing items by the cron backfill).
CREATE INDEX served_words_word ON served_words (word, publish_id);
CREATE INDEX items_public_publish ON items (public_publish_id);

-- Backfill (round 12): items published before rounds 11-12 get their folds, search words, block keys
-- and body hashes recomputed by the cron from their stored bodies (fold_rev 0 -> 1); publishes write 1.
ALTER TABLE items ADD COLUMN fold_rev INTEGER NOT NULL DEFAULT 0;

-- Round 14 (B3): earlier identical publications are looked up by served content.
CREATE INDEX item_questions_near_full ON item_questions (near_full);
CREATE INDEX item_questions_near_stripped ON item_questions (near_stripped);

-- Round 14 (B6): the stem tokens a qid's lineage started from (each step must stay near them).
ALTER TABLE item_questions ADD COLUMN lineage_root TEXT;

-- Round 16 (critic21c B2): earlier publications are found in every served form.
CREATE INDEX item_questions_near_kf ON item_questions (near_kf);
CREATE INDEX item_questions_near_bare_full ON item_questions (near_bare_full);
CREATE INDEX item_questions_near_bare_stripped ON item_questions (near_bare_stripped);
-- ... and by a near-rewording of their served stems: bands of every publish under user 'pub'
-- (existing publishes: by the backfill).

-- Round 16 (critic21c B4): the keys of a held item deleted by its author (hash-only), so the same
-- content published again stays held until the reports are resolved or the keys expire.
CREATE TABLE carried_keys (
  author_id TEXT NOT NULL,
  item_id   TEXT NOT NULL,
  reason    TEXT NOT NULL,
  key       TEXT NOT NULL,
  until     INTEGER NOT NULL,
  PRIMARY KEY (item_id, key)
);
CREATE INDEX carried_keys_author ON carried_keys (author_id, key);

-- Round 16 (critic21c B5): block keys of each Multipart part (stem, explanation, answers; served form).
CREATE TABLE item_part_keys (
  publish_id TEXT NOT NULL,
  pos        INTEGER NOT NULL,
  part       INTEGER NOT NULL,
  key        TEXT NOT NULL,
  tokens     TEXT, -- a part stem key's words and MinHash bands (near-rewordings of blocked student data)
  bands      TEXT,
  PRIMARY KEY (publish_id, pos, part, key)
);
CREATE INDEX item_part_keys_key ON item_part_keys (key);

-- Round 17 (critic22c M3): the words a question adds to its stem as served (with its kind), on the
-- question and on each serve: a near stem with another body is another question. Old rows: backfill.
ALTER TABLE item_questions ADD COLUMN body_tokens TEXT;
-- Round 21 (critic27a V3): a keyed sketch of the stem and body tokens (hashes, no words): what a ghost
-- keeps so a near-rewording of a publish someone was served still binds once it is gone.
ALTER TABLE item_questions ADD COLUMN tok_sk TEXT;
ALTER TABLE item_questions ADD COLUMN body_sk TEXT;
-- (and the student-data bands keep no words: their plaintext tokens go; a band without a sketch is
-- matched on the band alone, conservatively: critic27b E1)
UPDATE privacy_bands SET tokens = NULL;
ALTER TABLE import_questions ADD COLUMN body_tokens TEXT;

-- Round 17 (critic22c M5a): a PNG's decoded-pixel hash (canonical RGBA), so a re-encoded image is
-- the same image for licence evidence. NULL for other formats and for blobs uploaded before.
ALTER TABLE blobs ADD COLUMN pixel_hash TEXT;
CREATE INDEX blobs_pixel_hash ON blobs (pixel_hash);

-- Round 17 (critic22e S8): when each account first uploaded an image's bytes (ordering evidence).
CREATE TABLE blob_uploads (
  sha256      TEXT NOT NULL,
  uploader_id TEXT NOT NULL,
  first_at    INTEGER NOT NULL,
  PRIMARY KEY (sha256, uploader_id)
);
CREATE INDEX blob_uploads_user ON blob_uploads (uploader_id);

-- Round 17 (critic22e A2): the "still migrating" gate (items waiting for the backfill).
CREATE INDEX items_fold_rev ON items (fold_rev, purged_at);

-- Round 17: reports gain the reason 'key_leak' (an importer saw the hidden answer key given away).
CREATE TABLE reports_new (
  id          TEXT PRIMARY KEY,
  item_id     TEXT NOT NULL,
  publish_id  TEXT,
  reporter_id TEXT,
  reason      TEXT NOT NULL CHECK (reason IN ('wrong_answer', 'copyright', 'student_data', 'inappropriate', 'other', 'key_leak')),
  note        TEXT,
  at          INTEGER NOT NULL,
  resolved_at INTEGER,
  resolved_by TEXT,
  action      TEXT
);
INSERT INTO reports_new SELECT id, item_id, publish_id, reporter_id, reason, note, at, resolved_at, resolved_by, action FROM reports;
DROP TABLE reports;
ALTER TABLE reports_new RENAME TO reports;
CREATE UNIQUE INDEX reports_open ON reports (item_id, reporter_id, reason) WHERE resolved_at IS NULL;
CREATE INDEX reports_reporter ON reports (reporter_id, at);

-- Readers' own stem-band rows are no longer written or read (the public band index serves the fuzzy
-- lookup): the ones a 0103-era database holds go.
DELETE FROM stem_bands WHERE user_id <> 'pub' AND user_id NOT LIKE 'own:%';

-- Round 18 (critic23d V5): small pool state (when the migration window opened).
CREATE TABLE pool_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);

-- Round 19 (critic24c, coordinator's compact design): what binds a reader is ONE row per (reader,
-- served publish), `served`, matched at publish time against that publish's own key set (its
-- item_questions, part / unit keys and served images), never per-reader copies of keys. A retired,
-- deleted or purged publish that anyone was served leaves a key-only ghost (hashes and credits, no
-- text), kept while a served row references it.
CREATE TABLE served (
  reader_id     TEXT NOT NULL,
  item_id       TEXT NOT NULL,
  publish_id    TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('import', 'view')),
  license       TEXT NOT NULL,
  licenses_json TEXT NOT NULL,
  first_at      INTEGER NOT NULL,
  at            INTEGER NOT NULL,
  void_at       INTEGER,
  PRIMARY KEY (reader_id, publish_id)
);
CREATE INDEX served_item ON served (item_id, reader_id);
CREATE INDEX served_publish ON served (publish_id);
CREATE INDEX served_reader_kind ON served (reader_id, kind);
-- Round 22 (critic29a V-a): every licence an item had at each served content (`q:` and the served
-- content hash of a question, `u:` and a unit key), from when it was published with it until a publish
-- without it (or the item's purge): a serve counts every licence in force since it, however many
-- publishes there were. Hashes and licences only.
-- Round 22 (critic29a B2): the keyed MinHash bands of each question (HMAC-SHA256 under the 'sketch-v1'
-- subkey of POOL_HASH_KEY, 64 bits, sorted) and of each part stem; a ghost is found by these only
-- (ghost_bands), and privacy rows keep these only. Unkeyed bands live only while the item does.
ALTER TABLE item_questions ADD COLUMN kbands TEXT;
-- Round 22 (critic29b B1): each text unit's own seq ({key: seq}); a row without it dates its texts by
-- the question (older publishes), and a republish carries a text's seq only from this map.
ALTER TABLE item_questions ADD COLUMN unit_seqs TEXT;
ALTER TABLE item_part_keys ADD COLUMN kbands TEXT;
CREATE TABLE ghost_bands (
  band    TEXT NOT NULL,
  item_id TEXT NOT NULL,
  qid     TEXT NOT NULL,
  PRIMARY KEY (band, item_id, qid)
);
CREATE INDEX ghost_bands_item ON ghost_bands (item_id, qid);
-- Round 22 (critic29b): accounts whose erasure batch committed; a deleted account without a row is
-- erased again by the cron until it commits. Accounts deleted before this migration count as done.
CREATE TABLE erasure_done (user_id TEXT PRIMARY KEY, at INTEGER NOT NULL);
INSERT OR IGNORE INTO erasure_done (user_id, at) SELECT id, deleted_at FROM users WHERE deleted_at IS NOT NULL;
-- Rounds 22-23 (critic29b, critic30b T1): an erasure also nulls the notes the account WROTE; an
-- admin's note about it or its items is evidence and stays (its personal data is scrubbed when it is
-- written); nothing else of the row changes.
DROP TRIGGER pool_audit_no_update;
CREATE TRIGGER pool_audit_no_update BEFORE UPDATE ON pool_audit
WHEN NOT (
  (NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS OLD.actor_id AND NEW.actor_kind = OLD.actor_kind
   AND NEW.action = OLD.action AND NEW.item_id IS OLD.item_id AND NEW.target_user_id IS OLD.target_user_id
   AND NEW.detail IS OLD.detail AND NEW.note IS NULL
   AND (EXISTS (SELECT 1 FROM users WHERE id = OLD.target_user_id AND deleted_at IS NOT NULL)
        OR EXISTS (SELECT 1 FROM users WHERE id = OLD.actor_id AND deleted_at IS NOT NULL)))
  OR (NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS NULL AND NEW.actor_kind = OLD.actor_kind AND OLD.action = 'item.report'
   AND NEW.action = OLD.action AND NEW.item_id IS OLD.item_id AND NEW.target_user_id IS OLD.target_user_id AND NEW.note IS OLD.note
   AND NEW.detail IS json_remove(OLD.detail, '$.report_id')
   AND EXISTS (SELECT 1 FROM users WHERE id = OLD.actor_id AND deleted_at IS NOT NULL))
)
BEGIN
  SELECT RAISE(ABORT, 'pool_audit is append-only');
END;
-- Round 23 (critic30b A2): a ghost keeps the keyed block keys a takedown plan needs (the qkind, the
-- normalised skeleton, stem and full keys; each part's keys), so a takedown of a purged item blocks
-- exactly the positions selected, by the live rules. Fingerprints only.
CREATE TABLE ghost_block_keys (
  publish_id TEXT NOT NULL,
  pos        INTEGER NOT NULL,
  part       INTEGER NOT NULL,
  key        TEXT NOT NULL,
  PRIMARY KEY (publish_id, pos, part, key)
);
-- Round 23 (critic30b F2/F3): notices that must not be lost, written with the action they report and
-- sent until sent (ids and kinds; the payload names ids only).
CREATE TABLE pool_outbox (id TEXT PRIMARY KEY, kind TEXT NOT NULL, payload TEXT NOT NULL, at INTEGER NOT NULL, sent_at INTEGER);
CREATE INDEX pool_outbox_unsent ON pool_outbox (sent_at, at);
-- Round 23 (critic30b F1): a ghost uphold claimed but not yet finished (the cron resumes it).
ALTER TABLE dmca_actions ADD COLUMN uphold_pending INTEGER NOT NULL DEFAULT 0;
-- Round 23 (critic30a B3): each author's durable keyed record of her stems (keyed bands, keyed token
-- and body sets, first seq): her own earlier versions define "earlier" for her, whatever was retained.
-- Keyed values only; erasure deletes them.
CREATE TABLE author_stems (
  author_id TEXT NOT NULL,
  band      TEXT NOT NULL,
  qhash     TEXT NOT NULL,
  tok_sk    TEXT NOT NULL,
  body_sk   TEXT,
  bk_expl   TEXT NOT NULL DEFAULT '',
  bk_ans    TEXT NOT NULL DEFAULT '',
  first_seq INTEGER NOT NULL,
  PRIMARY KEY (author_id, band, qhash, bk_expl, bk_ans)
) WITHOUT ROWID;
-- Round 23 (owner): the teacher's affirmation of the publishing terms, per publish: when, which terms
-- version, by whom (a pseudonymous id after erasure). Kept while the publish or its ghost exists; shown
-- to admins only. An affirmation, not verified.
CREATE TABLE attestations (publish_id TEXT PRIMARY KEY, item_id TEXT NOT NULL, user_id TEXT NOT NULL, at INTEGER NOT NULL, terms_version TEXT NOT NULL);
CREATE INDEX attestations_item ON attestations (item_id, at);
CREATE TABLE content_licences (
  item_id  TEXT NOT NULL,
  ckey     TEXT NOT NULL,
  license  TEXT NOT NULL,
  from_at  INTEGER NOT NULL,
  until_at INTEGER,
  -- (round 23 W1: one row per item, content and licence, re-opened when offered again)
  PRIMARY KEY (item_id, ckey, license)
);
-- Round 23 (critic30a B4): what each item's current publish offers (its contents and units); the
-- licence history counts only while the item offers them publicly.
CREATE TABLE item_offer (item_id TEXT NOT NULL, ckey TEXT NOT NULL, PRIMARY KEY (item_id, ckey)) WITHOUT ROWID;
CREATE INDEX content_licences_open ON content_licences (item_id, until_at);
-- (seeded from the retained publishes, questions only: units of older publishes are read as before)
INSERT OR IGNORE INTO content_licences (item_id, ckey, license, from_at, until_at)
  SELECT p.item_id, 'q:' || CASE WHEN p.hide_key = 1 THEN iq.stripped_content ELSE iq.full_content END, p.license, MIN(COALESCE(p.committed_at, p.created_at)),
         CASE WHEN MAX(p.status = 'current' AND i.visibility = 'public') = 1 THEN NULL
              ELSE MAX(COALESCE((SELECT MIN(COALESCE(p2.committed_at, p2.created_at)) FROM item_publishes p2 WHERE p2.item_id = p.item_id AND COALESCE(p2.committed_at, p2.created_at) > COALESCE(p.committed_at, p.created_at)), COALESCE(p.committed_at, p.created_at))) END
    FROM item_publishes p JOIN items i ON i.id = p.item_id JOIN item_questions iq ON iq.publish_id = p.publish_id
   WHERE p.status IN ('current', 'previous') AND (CASE WHEN p.hide_key = 1 THEN iq.stripped_content ELSE iq.full_content END) IS NOT NULL
   GROUP BY p.item_id, 2, p.license;
INSERT OR IGNORE INTO item_offer (item_id, ckey)
  SELECT p.item_id, 'q:' || CASE WHEN p.hide_key = 1 THEN iq.stripped_content ELSE iq.full_content END FROM items i JOIN item_publishes p ON p.publish_id = i.current_publish_id
    JOIN item_questions iq ON iq.publish_id = p.publish_id WHERE (CASE WHEN p.hide_key = 1 THEN iq.stripped_content ELSE iq.full_content END) IS NOT NULL;
CREATE TABLE ghost_publishes (
  publish_id     TEXT PRIMARY KEY,
  item_id        TEXT NOT NULL,
  author_id      TEXT NOT NULL,
  hide_key       INTEGER NOT NULL,
  public_version INTEGER NOT NULL,
  license        TEXT NOT NULL,
  at             INTEGER NOT NULL,
  -- 0: made by this migration from pre-0104 evidence, which kept question hashes only (its texts,
  -- images and parts are unknown: a serve of it falls back to the item's retained publishes, D1).
  units_known    INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX ghost_publishes_item ON ghost_publishes (item_id);
CREATE TABLE ghost_questions (
  publish_id         TEXT NOT NULL,
  qid                TEXT NOT NULL,
  pos                INTEGER NOT NULL,
  match_hash         TEXT NOT NULL,
  full_content       TEXT,
  stripped_content   TEXT,
  keyfree_content    TEXT,
  near_full          TEXT,
  near_stripped      TEXT,
  near_kf            TEXT,
  near_bare_full     TEXT,
  near_bare_stripped TEXT,
  chain_json         TEXT NOT NULL,
  seq                INTEGER NOT NULL,
  stem_hash          TEXT,
  bk_expl            TEXT,
  bk_ans             TEXT,
  -- Rounds 20-21 (critic25a seed 29, critic27a V1): every licence the item had, when its public version
  -- served this very question, from when the ghost was made until the item went (JSON array; each later
  -- retirement or purge of the item adds its licence then): CC grants are irrevocable, so they outlive it.
  now_licenses       TEXT,
  tok_sk             TEXT, -- (round 21 V3: the keyed token sketches of item_questions, never words)
  body_sk            TEXT,
  -- (round 23 A2: the keyed block keys a takedown plan needs)
  qkind              TEXT,
  bk_norm            TEXT,
  bk_stem            TEXT,
  bk_full            TEXT,
  PRIMARY KEY (publish_id, qid)
);
CREATE INDEX ghost_questions_match ON ghost_questions (match_hash);
CREATE INDEX ghost_questions_bk_expl ON ghost_questions (bk_expl);
CREATE INDEX ghost_questions_bk_ans ON ghost_questions (bk_ans);
CREATE INDEX ghost_questions_stem ON ghost_questions (stem_hash);
CREATE TABLE ghost_units (
  publish_id TEXT NOT NULL,
  pos        INTEGER NOT NULL,
  part       INTEGER NOT NULL,
  key        TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('text', 'image', 'part')),
  now_licenses TEXT, -- (as ghost_questions.now_licenses, for this unit)
  seq        INTEGER, -- (round 21, critic27a V2: when the unit was first published, as the live rows say)
  PRIMARY KEY (publish_id, key, pos, part)
);
CREATE INDEX ghost_units_key ON ghost_units (key);
CREATE INDEX item_questions_stem_hash ON item_questions (stem_hash);

-- The evidence of a database migrated from 0102/0103: one served row per (reader, served publish),
-- from imports (every licence shown kept, an import winning over a read); the 0102-era evidence rows of
-- publishes no longer retained become ghosts.
INSERT INTO served (reader_id, item_id, publish_id, kind, license, licenses_json, first_at, at)
  SELECT im.user_id, im.item_id, COALESCE(im.real_publish_id, im.publish_id),
         CASE WHEN MAX(im.kind = 'import') = 1 THEN 'import' ELSE 'view' END,
         substr(MAX(printf('%016d|%s', im.at, im.license)), 18),
         (SELECT json_group_array(DISTINCT l.value) FROM imports i2, json_each(COALESCE(i2.licenses_json, json_array(i2.license))) l
           WHERE i2.user_id = im.user_id AND COALESCE(i2.real_publish_id, i2.publish_id) = COALESCE(im.real_publish_id, im.publish_id)),
         MIN(COALESCE(im.first_at, im.at)), MAX(im.at)
    FROM imports im WHERE im.user_id NOT LIKE 'deleted:%' -- (an erased reader's anonymised serves bind nothing: round 20 F3)
   GROUP BY im.user_id, COALESCE(im.real_publish_id, im.publish_id);
-- Round 20 (critic25a E2): evidence voids made before 0104 (written to import_questions.void_at only)
-- carry over. An admin or upheld-takedown void of an ITEM (its last evidence.void / evidence.restore
-- audit is a void) marks the item (void_items) and every serve of it; a void of a READER (the same, by
-- target user) every serve of theirs; and a reader's serves of an item whose old evidence rows were
-- ALL voided stay voided, whatever the audit trail says.
INSERT OR IGNORE INTO void_items (item_id, at, by, reason)
  SELECT a.item_id, a.at, a.actor_id, 'migrated' FROM pool_audit a
   WHERE a.action = 'evidence.void' AND a.item_id IS NOT NULL
     AND NOT EXISTS (SELECT 1 FROM pool_audit b WHERE b.item_id = a.item_id AND b.action IN ('evidence.void', 'evidence.restore') AND (b.at > a.at OR (b.at = a.at AND b.id > a.id)));
-- (Round 21, critic27a M-B4: a void covers the serves that existed when it was made, as HEAD's did.)
UPDATE served SET void_at = (SELECT v.at FROM void_items v WHERE v.item_id = served.item_id)
 WHERE void_at IS NULL AND item_id IN (SELECT item_id FROM void_items)
   AND first_at <= (SELECT v.at FROM void_items v WHERE v.item_id = served.item_id);
UPDATE served SET void_at = (SELECT MAX(a.at) FROM pool_audit a WHERE a.target_user_id = served.reader_id AND a.item_id IS NULL AND a.action = 'evidence.void')
 WHERE void_at IS NULL
   AND first_at <= (SELECT MAX(a.at) FROM pool_audit a WHERE a.target_user_id = served.reader_id AND a.item_id IS NULL AND a.action = 'evidence.void')
   AND (SELECT a.action FROM pool_audit a WHERE a.target_user_id = served.reader_id AND a.item_id IS NULL AND a.action IN ('evidence.void', 'evidence.restore') ORDER BY a.at DESC, a.id DESC LIMIT 1) = 'evidence.void';
UPDATE served SET void_at = (SELECT MAX(iq.void_at) FROM import_questions iq WHERE iq.user_id = served.reader_id AND iq.item_id = served.item_id)
 WHERE void_at IS NULL
   AND EXISTS (SELECT 1 FROM import_questions iq WHERE iq.user_id = served.reader_id AND iq.item_id = served.item_id)
   AND NOT EXISTS (SELECT 1 FROM import_questions iq WHERE iq.user_id = served.reader_id AND iq.item_id = served.item_id AND iq.void_at IS NULL);
INSERT OR IGNORE INTO ghost_publishes (publish_id, item_id, author_id, hide_key, public_version, license, at, units_known)
  SELECT s.publish_id, s.item_id, MAX(im.author_id), MAX(im.form = 'stripped'), MAX(im.version), MAX(im.license), MAX(im.at), 0
    FROM served s JOIN imports im ON im.user_id = s.reader_id AND COALESCE(im.real_publish_id, im.publish_id) = s.publish_id
   WHERE NOT EXISTS (SELECT 1 FROM item_publishes p WHERE p.publish_id = s.publish_id)
   GROUP BY s.publish_id, s.item_id;
-- (The 0103 rows are per served content, aggregated across versions: a ghost takes the contents its
-- readers' rows of that item hold.)
INSERT OR IGNORE INTO ghost_questions (publish_id, qid, pos, match_hash, full_content, stripped_content, keyfree_content, chain_json, seq, stem_hash)
  SELECT gp.publish_id, iq.qid, 0, MIN(iq.match_hash), MIN(CASE WHEN iq.form = 'full' THEN iq.content_hash END), MIN(CASE WHEN iq.form = 'stripped' THEN iq.content_hash END),
         MIN(iq.keyfree_hash), MAX(iq.chain_json), MIN(iq.source_first_seq), MIN(iq.stem_hash)
    FROM ghost_publishes gp JOIN served s ON s.publish_id = gp.publish_id JOIN import_questions iq ON iq.user_id = s.reader_id AND iq.item_id = gp.item_id
   GROUP BY gp.publish_id, iq.qid;
-- 0103 united the licences of one served content across its serves: each served row of that reader
-- and item keeps every licence its content was shown under.
UPDATE served SET licenses_json = (SELECT json_group_array(DISTINCT v) FROM (
    SELECT l.value AS v FROM json_each(served.licenses_json) l
    UNION SELECT l2.value FROM import_questions iq, json_each(iq.licenses_json) l2
     WHERE iq.user_id = served.reader_id AND iq.item_id = served.item_id
       AND EXISTS (SELECT 1 FROM item_questions x WHERE x.publish_id = served.publish_id AND x.qid = iq.qid)))
 WHERE EXISTS (SELECT 1 FROM import_questions iq WHERE iq.user_id = served.reader_id AND iq.item_id = served.item_id);
DROP TABLE import_questions;

-- Every retained publish's questions and every ghost's, with what a reader was served of them.
-- The evidence the licence rule reads (the columns of the former import_questions table): a reader's
-- served publishes' questions, as served, with every licence shown to the reader, and the item's
-- licence NOW when its public version serves this very content (a re-read would show it; CC grants
-- are irrevocable, so the union is what the reader may use; a changed version's licence never covers
-- older content). Round 20: whether or not the item is still listed (the offer was public; a void cuts
-- it), and for a ghost also the licence frozen when it was made (the item may be gone since). Two plain join views (live publishes, ghosts of gone ones) so the planner can drive
-- either from a hash index (CROSS JOIN: the question rows lead, the served row is a key lookup; a
-- consumer reaching them by served item drives `served` itself first); `import_questions` is their
-- union, for queries where it is the only table.
CREATE VIEW import_questions_live AS
  SELECT sv.reader_id AS user_id, sv.item_id, q.qid,
         CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END AS content_hash,
         p.author_id, q.match_hash, q.keyfree_content AS keyfree_hash,
         CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END AS near_hash, q.near_kf,
         CASE WHEN p.hide_key = 1 THEN q.near_bare_stripped ELSE q.near_bare_full END AS near_bare,
         q.stem_tokens, CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END AS form, sv.kind, q.chain_json,
         COALESCE(u.display_name, 'deleted teacher') AS author_name, sv.license,
         (SELECT json_group_array(DISTINCT v) FROM (SELECT l.value AS v FROM json_each(sv.licenses_json) l
                                                   -- (round 22 V-a: every licence the item had at this very content since the serve)
                                                   UNION SELECT cl.license FROM content_licences cl
                                                          WHERE cl.item_id = sv.item_id AND cl.ckey = 'q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END)
                                                            AND (cl.until_at IS NULL OR cl.until_at >= sv.first_at)
                                                   UNION SELECT i.license FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                                                           JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = q.qid
                                                          WHERE i.id = sv.item_id
                                                            AND (CASE WHEN cp.hide_key = 1 THEN cq.stripped_content ELSE cq.full_content END)
                                                                = (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END))) AS licenses_json,
         p.public_version AS version, json_array(p.public_version) AS versions_json, q.seq AS source_first_seq, q.stem_hash, sv.at, sv.void_at,
         q.publish_id AS src_publish_id, 1 AS rev, q.body_tokens, q.tok_sk, q.body_sk
    FROM item_questions q CROSS JOIN item_publishes p ON p.publish_id = q.publish_id CROSS JOIN served sv ON sv.publish_id = q.publish_id LEFT JOIN users u ON u.id = p.author_id;
CREATE VIEW import_questions_ghost AS
  SELECT sv.reader_id AS user_id, sv.item_id, q.qid,
         CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END AS content_hash,
         p.author_id, q.match_hash, q.keyfree_content AS keyfree_hash,
         CASE WHEN p.hide_key = 1 THEN q.near_stripped ELSE q.near_full END AS near_hash, q.near_kf,
         CASE WHEN p.hide_key = 1 THEN q.near_bare_stripped ELSE q.near_bare_full END AS near_bare,
         NULL AS stem_tokens, CASE WHEN p.hide_key = 1 THEN 'stripped' ELSE 'full' END AS form, sv.kind, q.chain_json,
         COALESCE(u.display_name, 'deleted teacher') AS author_name, sv.license,
         (SELECT json_group_array(DISTINCT v) FROM (SELECT l.value AS v FROM json_each(sv.licenses_json) l
                                                   -- (round 22 V-a: every licence the item had at this very content since the serve)
                                                   UNION SELECT cl.license FROM content_licences cl
                                                          WHERE cl.item_id = sv.item_id AND cl.ckey = 'q:' || (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END)
                                                            AND (cl.until_at IS NULL OR cl.until_at >= sv.first_at)
                                                   UNION SELECT i.license FROM items i JOIN item_publishes cp ON cp.publish_id = i.public_publish_id
                                                           JOIN item_questions cq ON cq.publish_id = cp.publish_id AND cq.qid = q.qid
                                                          WHERE i.id = sv.item_id
                                                            AND (CASE WHEN cp.hide_key = 1 THEN cq.stripped_content ELSE cq.full_content END)
                                                                = (CASE WHEN p.hide_key = 1 THEN q.stripped_content ELSE q.full_content END)
                                                   UNION SELECT nl.value FROM json_each(COALESCE(q.now_licenses, '[]')) nl)) AS licenses_json,
         p.public_version AS version, json_array(p.public_version) AS versions_json, q.seq AS source_first_seq, q.stem_hash, sv.at, sv.void_at,
         q.publish_id AS src_publish_id, 1 AS rev, NULL AS body_tokens, q.tok_sk, q.body_sk
    FROM ghost_questions q CROSS JOIN ghost_publishes p ON p.publish_id = q.publish_id CROSS JOIN served sv ON sv.publish_id = q.publish_id LEFT JOIN users u ON u.id = p.author_id;
CREATE VIEW import_questions AS
  SELECT * FROM import_questions_live UNION ALL SELECT * FROM import_questions_ghost;
