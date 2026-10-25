-- Round 44 (circles phase C, docs/circles-plan.md 13.3): copyright re-upload block checks on circle sends. A send or
-- rollback whose questions or images a takedown's re-upload block holds (blocked_hashes, the public path's rules) is
-- withheld like a student-data match, and the bank is held while that version is the head: a `circle_holds` row with
-- the new reason 'copyright' (source 'blocklist'). circle_holds keeps its reasons in a CHECK list, so the table is
-- rebuilt (it is small: holds only), every row copied as it is, and its index made again. Nothing else changes.
CREATE TABLE circle_holds_r44 (
  bank_id TEXT NOT NULL,
  reason  TEXT NOT NULL CHECK (reason IN ('reports', 'student_data', 'admin', 'copyright')),
  source  TEXT NOT NULL DEFAULT '',
  at      INTEGER NOT NULL,
  by      TEXT,
  PRIMARY KEY (bank_id, reason, source)
) WITHOUT ROWID;
INSERT INTO circle_holds_r44 (bank_id, reason, source, at, by) SELECT bank_id, reason, source, at, by FROM circle_holds;
DROP TABLE circle_holds;
ALTER TABLE circle_holds_r44 RENAME TO circle_holds;
CREATE INDEX IF NOT EXISTS circle_holds_at ON circle_holds (reason, source, at);
-- The next is 0114.
