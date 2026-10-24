-- Round 28 (owner decisions): the 3-year retention of audit and takedown records, and the invite-code
-- gate in front of pool use.

-- ---- Retention ------------------------------------------------------------------------------------
-- pool_audit stays append-only for its first three years. A row older than 1,096 days (never less
-- than three calendar years) may be deleted, and only by the retention stage of the daily cron
-- (src/pool/cron.ts `retention`), which also keeps every row an open dispute, an appeal, a hold or a
-- kept takedown record still needs. The floor is the DATABASE's own clock, so no application clock or
-- stray DELETE can remove a younger row. 1,096 days = 94,694,400,000 ms (RETENTION_MS in cron.ts).
DROP TRIGGER pool_audit_no_delete;
CREATE TRIGGER pool_audit_no_delete BEFORE DELETE ON pool_audit
WHEN OLD.at >= CAST(strftime('%s', 'now') AS INTEGER) * 1000 - 94694400000
BEGIN
  SELECT RAISE(ABORT, 'pool_audit is append-only');
END;
-- The retention stage walks the oldest rows first and looks takedown records up by notice and source.
CREATE INDEX pool_audit_at ON pool_audit (at);
CREATE INDEX dmca_actions_notice ON dmca_actions (notice_id);
CREATE INDEX item_holds_source ON item_holds (source);

-- ---- Invite codes ---------------------------------------------------------------------------------
-- An admin issues a code (POST /admin/pool/invites); an account redeems it (POST /me/invite) and only
-- then may use the pool (with admin approval as before: both are required). The code itself is never
-- stored: `code_hash` is the SHA-256 (hex) of its normalised form (24 Crockford base32 characters,
-- 120 random bits). Single use by default; `max_uses` up to 1,000; optional expiry; revocable (a
-- revocation stops further redemptions, it never undoes one).
CREATE TABLE pool_invites (
  id         TEXT PRIMARY KEY,
  code_hash  TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  created_by TEXT NOT NULL,
  max_uses   INTEGER NOT NULL CHECK (max_uses BETWEEN 1 AND 1000),
  uses       INTEGER NOT NULL DEFAULT 0 CHECK (uses >= 0 AND uses <= max_uses),
  expires_at INTEGER,
  revoked_at INTEGER,
  revoked_by TEXT,
  note       TEXT
);
CREATE INDEX pool_invites_created ON pool_invites (created_at, id);
-- One row per account that may use the pool past the gate. `invite_id` NULL: an account that was
-- already approved (teacher or admin) when this migration ran, so existing approved accounts keep
-- working. Erasure deletes the account's row (the invite keeps its use count).
CREATE TABLE pool_invite_redemptions (
  user_id   TEXT PRIMARY KEY,
  invite_id TEXT,
  at        INTEGER NOT NULL
);
CREATE INDEX pool_invite_redemptions_invite ON pool_invite_redemptions (invite_id, at);
INSERT OR IGNORE INTO pool_invite_redemptions (user_id, invite_id, at)
  SELECT id, NULL, CAST(strftime('%s', 'now') AS INTEGER) * 1000 FROM users WHERE trust IN ('teacher', 'admin') AND deleted_at IS NULL;
