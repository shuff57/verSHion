-- Forward migration from the committed 0102_pool.sql (commit 1d311ff) to the schema of rounds 6-11.
-- 0102 is never edited again: a database that applied the committed 0102 (staging) migrates with this
-- file. Evidence rows of the committed schema (import_questions keyed by publish) are rebuilt as
-- per-content rows from imports and the served publish's item_questions (round 16, critic21d B8):
-- pre-migration serves keep binding. Their near/content hashes are recomputed by the cron backfill.

ALTER TABLE items ADD COLUMN license_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE items ADD COLUMN title_fold TEXT NOT NULL DEFAULT '';
ALTER TABLE items ADD COLUMN subject_fold TEXT NOT NULL DEFAULT '';
ALTER TABLE items ADD COLUMN purge_failures INTEGER NOT NULL DEFAULT 0;
ALTER TABLE items ADD COLUMN purge_retry_at INTEGER;
ALTER TABLE items ADD COLUMN appeal_decided_at INTEGER;
ALTER TABLE items ADD COLUMN appeal_decision TEXT;
ALTER TABLE item_questions ADD COLUMN near_full TEXT;
ALTER TABLE item_questions ADD COLUMN near_stripped TEXT;
ALTER TABLE item_questions ADD COLUMN near_kf TEXT;
ALTER TABLE item_questions ADD COLUMN near_bare_full TEXT;
ALTER TABLE item_questions ADD COLUMN near_bare_stripped TEXT;
ALTER TABLE item_questions ADD COLUMN short INTEGER NOT NULL DEFAULT 0;
ALTER TABLE item_questions ADD COLUMN stem_tokens TEXT;
ALTER TABLE item_questions ADD COLUMN stem_bands TEXT;
ALTER TABLE item_questions ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE item_tags ADD COLUMN fold TEXT NOT NULL DEFAULT '';
ALTER TABLE blobs ADD COLUMN privacy_purged_at INTEGER;
ALTER TABLE item_blobs ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dmca_actions ADD COLUMN rightful_item_id TEXT;
CREATE INDEX item_tags_fold ON item_tags (fold);

-- Evidence: one durable row per (account, source item, qid, served content) (round 7 H4).
DROP INDEX IF EXISTS import_questions_match;
ALTER TABLE import_questions RENAME TO import_questions_0102;
CREATE TABLE import_questions (
  user_id      TEXT NOT NULL,
  item_id      TEXT NOT NULL,
  qid          TEXT NOT NULL,
  -- What this account was served of the question (the stripped content of a hidden source).
  content_hash TEXT NOT NULL,
  author_id    TEXT NOT NULL,
  match_hash   TEXT NOT NULL,
  keyfree_hash TEXT NOT NULL,
  -- item_questions.near_full / near_stripped of what was served, and near_kf (round 9 J1).
  near_hash    TEXT,
  near_kf      TEXT,
  near_bare    TEXT,
  stem_tokens  TEXT,
  form         TEXT NOT NULL CHECK (form IN ('full', 'stripped')),
  -- 'import' once it was ever imported; 'view' if only read.
  kind         TEXT NOT NULL CHECK (kind IN ('import', 'view')),
  -- The source question's own chain (oldest first) at the latest serve.
  chain_json   TEXT NOT NULL,
  author_name  TEXT NOT NULL,
  license      TEXT NOT NULL,
  licenses_json TEXT NOT NULL,
  version      INTEGER NOT NULL,
  versions_json TEXT NOT NULL,
  -- item_questions.seq of the served content (per content, round 7 H2): the earliest seen.
  source_first_seq INTEGER NOT NULL,
  stem_hash    TEXT,
  at           INTEGER NOT NULL,
  -- Set when the source was found infringing (an upheld takedown) or by an admin: the row no longer
  -- counts as licence or attribution evidence. Audited; reversible (a put-back clears it).
  void_at      INTEGER,
  PRIMARY KEY (user_id, item_id, qid, content_hash)
);
CREATE INDEX import_questions_match ON import_questions (user_id, match_hash);
CREATE INDEX import_questions_item ON import_questions (item_id);
-- Round 16 (critic21d B8): the committed schema's rows, kept. Each maps through its imports row to
-- the publish the account was served (real_publish_id for a read of a hidden publish); the served
-- content is that publish's stripped or full content hash. src_publish_id lets the cron backfill
-- fill in the near / key-free hashes once it has recomputed them (rev = 0 until then).
ALTER TABLE import_questions ADD COLUMN src_publish_id TEXT;
ALTER TABLE import_questions ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
-- (Round 17, critic22e S6: one row per served content, aggregated: an import wins over a read, and
-- every licence the account was shown is kept.)
INSERT OR IGNORE INTO import_questions
  (user_id, item_id, qid, content_hash, author_id, match_hash, keyfree_hash, near_hash, near_kf, near_bare, stem_tokens, form, kind,
   chain_json, author_name, license, licenses_json, version, versions_json, source_first_seq, stem_hash, at, void_at, src_publish_id, rev)
SELECT o.user_id, o.item_id, o.qid, o.content_hash, MAX(COALESCE(im.author_id, '')), MIN(o.match_hash), MIN(o.keyfree_hash), NULL, NULL, NULL, NULL,
       MAX(COALESCE(im.form, 'full')),
       CASE WHEN MAX(COALESCE(im.kind, 'import') = 'import') = 1 THEN 'import' ELSE 'view' END,
       MAX(o.chain_json), MAX(o.author_name),
       MAX(printf('%016d|%s', COALESCE(im.at, 0), COALESCE(im.license, ''))),
       (SELECT json_group_array(DISTINCT l.value) FROM import_questions_0102 o2 LEFT JOIN imports im2 ON im2.user_id = o2.user_id AND im2.publish_id = o2.publish_id,
               json_each(COALESCE(im2.licenses_json, json_array(COALESCE(im2.license, '')))) l
         WHERE o2.user_id = o.user_id AND o2.item_id = o.item_id AND o2.qid = o.qid AND o2.content_hash = o.content_hash AND l.value <> ''),
       MAX(COALESCE(im.version, 1)), json_group_array(DISTINCT COALESCE(im.version, 1)),
       MIN(COALESCE(o.source_first_seq, 0)), MIN(o.stem_hash), MAX(COALESCE(im.at, 0)), MAX(o.void_at),
       MAX(COALESCE(im.real_publish_id, o.publish_id)), 0
  FROM import_questions_0102 o
  LEFT JOIN imports im ON im.user_id = o.user_id AND im.publish_id = o.publish_id
 GROUP BY o.user_id, o.item_id, o.qid, o.content_hash;
-- (license: the latest one shown; strip the ordering prefix)
UPDATE import_questions SET license = substr(license, 18) WHERE rev = 0 AND license LIKE '________________|%';
DROP TABLE import_questions_0102;
CREATE INDEX import_questions_rev ON import_questions (rev, item_id);

-- New flag kinds (rounds 6-11): the CHECK constraint needs a rebuild.
CREATE TABLE admin_flags_new (
  id          TEXT PRIMARY KEY,
  kind        TEXT NOT NULL CHECK (kind IN ('license_soft', 'origin_unverified', 'origin_modified', 'origin_suspect', 'blocklist', 'cap', 'repeat_infringer', 'appeal', 'dmca_copy', 'instant_hold_cap', 'needs_reupload', 'independent_claim', 'privacy_backlog', 'evidence_overflow', 'independent_repeat', 'privacy_text_match', 'cron_failure',
                                       'license_conflict', 'license_grandfathered', 'appeal_overdue',
                                       'appeal_escalated', 'appeal_auto_restored', 'license_mismatch', 'counter_notice', 'hold_cleared', 'key_leak',
                                       'd1_size', 'backfill_skipped', 'migration_stalled', 'wasm_trap')),
  item_id     TEXT,
  publish_id  TEXT,
  user_id     TEXT,
  detail      TEXT,
  -- Dedup key for flags that must be raised at most once (e.g. one cap alert per user, day and metric).
  dedup       TEXT UNIQUE,
  at          INTEGER NOT NULL,
  resolved_at INTEGER,
  resolved_by TEXT
);
INSERT INTO admin_flags_new (id, kind, item_id, publish_id, user_id, detail, dedup, at, resolved_at, resolved_by) SELECT id, kind, item_id, publish_id, user_id, detail, dedup, at, resolved_at, resolved_by FROM admin_flags;
DROP TABLE admin_flags;
ALTER TABLE admin_flags_new RENAME TO admin_flags;

-- Per-channel alert budgets (round 8 I6c); the hourly counts are ephemeral.
DROP TABLE pool_alert_budget;
CREATE TABLE pool_alert_budget (hour INTEGER NOT NULL, channel TEXT NOT NULL DEFAULT 'general', sent INTEGER NOT NULL, PRIMARY KEY (hour, channel));

DROP TRIGGER pool_audit_no_update;
CREATE TRIGGER pool_audit_no_update BEFORE UPDATE ON pool_audit
WHEN NOT (
  (NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS OLD.actor_id AND NEW.actor_kind = OLD.actor_kind
   AND NEW.action = OLD.action AND NEW.item_id IS OLD.item_id AND NEW.target_user_id IS OLD.target_user_id
   AND NEW.detail IS OLD.detail AND NEW.note IS NULL
   AND EXISTS (SELECT 1 FROM users WHERE id = OLD.target_user_id AND deleted_at IS NOT NULL))
  -- Round 10: a deleted account's report audits lose their link to it (actor and report id), nothing else.
  OR (NEW.id = OLD.id AND NEW.at = OLD.at AND NEW.actor_id IS NULL AND NEW.actor_kind = OLD.actor_kind AND OLD.action = 'item.report'
   AND NEW.action = OLD.action AND NEW.item_id IS OLD.item_id AND NEW.target_user_id IS OLD.target_user_id AND NEW.note IS OLD.note
   AND NEW.detail IS json_remove(OLD.detail, '$.report_id')
   AND EXISTS (SELECT 1 FROM users WHERE id = OLD.actor_id AND deleted_at IS NOT NULL))
)
BEGIN
  SELECT RAISE(ABORT, 'pool_audit is append-only');
END;

CREATE TABLE pool_seq (id INTEGER PRIMARY KEY CHECK (id = 1), n INTEGER NOT NULL);

INSERT INTO pool_seq (id, n) VALUES (1, 0);
CREATE TABLE author_firsts (
  author_id  TEXT NOT NULL,
  match_hash TEXT NOT NULL,
  first_seq  INTEGER NOT NULL,
  PRIMARY KEY (author_id, match_hash)
);
CREATE TABLE stem_bands (
  user_id TEXT NOT NULL,
  band    TEXT NOT NULL,
  item_id TEXT NOT NULL,
  qid     TEXT NOT NULL,
  PRIMARY KEY (user_id, band, item_id, qid)
);
CREATE TABLE author_contents (
  author_id  TEXT NOT NULL,
  match_hash TEXT NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('shown', 'kf', 'stem')),
  hash       TEXT NOT NULL,
  first_seq  INTEGER NOT NULL,
  PRIMARY KEY (author_id, kind, hash, match_hash)
);
CREATE INDEX author_contents_match ON author_contents (author_id, match_hash);
CREATE TABLE r2_orphans (key TEXT PRIMARY KEY, at INTEGER NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, next_at INTEGER NOT NULL DEFAULT 0);
CREATE TABLE void_items (
  item_id TEXT PRIMARY KEY,
  at      INTEGER NOT NULL,
  by      TEXT,
  reason  TEXT NOT NULL,
  -- The rightful author's item named on the takedown (round 7 M1): links to the void item are
  -- shown and granted as links to it, so copies stay bound to the rightful licence and credit.
  replaced_by TEXT,
  -- Its licence AT VOID TIME (round 8) and its author (round 9: the account, whose name is resolved
  -- at read, so a deleted account's name is never kept here): the stand-in shows these, never the
  -- item's current licence, and never an item that is not visible to others (then a neutral label).
  replaced_license   TEXT,
  replaced_author_id TEXT
);
CREATE TABLE dmca_exempt (
  action_id TEXT NOT NULL,
  user_id   TEXT NOT NULL,
  at        INTEGER NOT NULL,
  PRIMARY KEY (action_id, user_id)
);
CREATE TABLE preview_tokens_used (
  token_hash TEXT PRIMARY KEY,
  at         INTEGER NOT NULL
);

-- ---- round 11 ----------------------------------------------------------------------------------
-- The licences the licence rule allowed for each question at publish time, and the items that
-- decided it (P5): what `license_check` reports ("consistent with recorded sources").
ALTER TABLE item_questions ADD COLUMN allowed_json TEXT;
-- The publish sequence of the qid's LINEAGE (P1): carried from the qid's previous content while the
-- new content continues it (the same question, a typo or reword of it), so an original author's
-- edit is still "first" against a later copier.
ALTER TABLE item_questions ADD COLUMN lineage_seq INTEGER;
-- A hold's snapshot at the admin action (P3): {publish_id, positions}; notices never re-evaluate it.
ALTER TABLE item_holds ADD COLUMN detail TEXT;
CREATE INDEX admin_flags_publish ON admin_flags (publish_id);
CREATE INDEX admin_flags_user_at ON admin_flags (user_id, at);
CREATE INDEX admin_flags_kind_item ON admin_flags (kind, item_id);
CREATE INDEX pool_audit_actor ON pool_audit (actor_id);
CREATE INDEX items_current ON items (current_publish_id);
CREATE INDEX void_items_replaced ON void_items (replaced_by);
CREATE INDEX item_questions_publish_lineage ON item_questions (publish_id, lineage_seq);
CREATE INDEX stem_bands_item ON stem_bands (item_id, qid);
CREATE INDEX import_questions_key ON import_questions (user_id, item_id, qid);
-- Account deletion's rewrites of others' stored chains (the name becomes "deleted teacher"), done by
-- the cron in bounded rowid batches (round 11), one cursor per table.
CREATE TABLE deletion_rewrites (
  user_id  TEXT NOT NULL,
  tbl      TEXT NOT NULL CHECK (tbl IN ('item_questions', 'import_questions', 'ghost_questions')),
  cursor   INTEGER NOT NULL DEFAULT 0,
  at       INTEGER NOT NULL,
  done_at  INTEGER,
  PRIMARY KEY (user_id, tbl)
);
CREATE INDEX deletion_rewrites_open ON deletion_rewrites (at) WHERE done_at IS NULL;
-- Question-text search (round 11): the words of a publish's SERVED stems (the stripped form when the
-- key is hidden), never the full form. Browse reaches it through items.public_publish_id.
CREATE TABLE served_words (publish_id TEXT NOT NULL, word TEXT NOT NULL, PRIMARY KEY (publish_id, word)) WITHOUT ROWID;
CREATE INDEX blocked_hashes_action ON blocked_hashes (action_id, kind);

-- ---- backfill of rows published before this migration (round 12) ---------------------------------
-- Sequence numbers: each publish's place in commit order, a content keeping the first publish of the
-- same (item, qid, match_hash); images likewise. The counter continues after them.
-- (No TEMP tables: D1 refuses them; each statement derives the commit order itself.)
-- (Round 22, critic29b: the order and each content's first place are computed ONCE and joined; the
-- correlated form re-derived them per row, quadratic: 165 s at 40k publishes, past D1's 30 s.)
WITH pub_ord AS MATERIALIZED (SELECT publish_id, item_id, ROW_NUMBER() OVER (ORDER BY committed_at, created_at, publish_id) AS o FROM item_publishes WHERE committed_at IS NOT NULL),
     firsts AS MATERIALIZED (SELECT po.item_id, q.qid, q.match_hash, MIN(po.o) AS s FROM pub_ord po JOIN item_questions q ON q.publish_id = po.publish_id GROUP BY po.item_id, q.qid, q.match_hash),
     m AS MATERIALIZED (SELECT q.publish_id, q.qid, f.s FROM item_questions q JOIN pub_ord po ON po.publish_id = q.publish_id
                          JOIN firsts f ON f.item_id = po.item_id AND f.qid = q.qid AND f.match_hash = q.match_hash WHERE q.seq = 0)
UPDATE item_questions SET seq = m.s FROM m WHERE item_questions.publish_id = m.publish_id AND item_questions.qid = m.qid AND item_questions.seq = 0;
UPDATE item_questions SET lineage_seq = seq WHERE lineage_seq IS NULL AND seq > 0;
WITH pub_ord AS MATERIALIZED (SELECT publish_id, item_id, ROW_NUMBER() OVER (ORDER BY committed_at, created_at, publish_id) AS o FROM item_publishes WHERE committed_at IS NOT NULL),
     firsts AS MATERIALIZED (SELECT po.item_id, b.sha256, MIN(po.o) AS s FROM pub_ord po JOIN item_blobs b ON b.publish_id = po.publish_id GROUP BY po.item_id, b.sha256),
     m AS MATERIALIZED (SELECT DISTINCT b.publish_id, b.sha256, f.s FROM item_blobs b JOIN pub_ord po ON po.publish_id = b.publish_id
                          JOIN firsts f ON f.item_id = po.item_id AND f.sha256 = b.sha256 WHERE b.seq = 0)
UPDATE item_blobs SET seq = m.s FROM m WHERE item_blobs.publish_id = m.publish_id AND item_blobs.sha256 = m.sha256 AND item_blobs.seq = 0;
UPDATE pool_seq SET n = MAX(n, (SELECT COUNT(*) FROM item_publishes WHERE committed_at IS NOT NULL)) WHERE id = 1;
-- The authors' first publishes and own contents (durable records).
INSERT OR IGNORE INTO author_firsts (author_id, match_hash, first_seq)
  SELECT p.author_id, iq.match_hash, MIN(iq.seq) FROM item_questions iq JOIN item_publishes p ON p.publish_id = iq.publish_id
   WHERE iq.seq > 0 GROUP BY p.author_id, iq.match_hash;
-- Search folds: ASCII lower case now; the cron re-folds every such item exactly (items.fold_rev, 0104).
UPDATE items SET title_fold = ' ' || lower(title), subject_fold = lower(subject) WHERE title_fold = '' AND subject_fold = '';
UPDATE item_tags SET fold = lower(tag) WHERE fold = '';
