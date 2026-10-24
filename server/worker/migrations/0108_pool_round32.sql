-- Round 32: circles, phase A (docs/circles-plan.md sections 5, 13.1; decisions D1-D19).
--
-- A circle is a small private group of approved teachers. Everything here lives in the pool database,
-- keyed by the auth package's user ids; nothing goes into `users`. Circle banks are stored apart from
-- public items: no `served` rows, no authorship records, no licence history, never in browse, search,
-- credits or /items/versions. New tables and indexes only; no existing table or row changes, so the
-- migration is safe on a populated database. (admin_flags keeps its CHECK list: circle cases are the
-- open `circle_reports` rows, cap crossings reuse the existing `cap` kind and circle independence
-- statements the existing `independent_claim` / `independent_repeat` kinds.)

-- ---- circles ----------------------------------------------------------------------------------------
-- `created_by` is history only: the owner is the circle_members row with role 'owner'. A close is
-- either the owner's (14-day undo: `undo_until`), the last member leaving or being erased, or an
-- admin's; the daily purge then removes banks, versions, codes and members, and the row stays as a
-- tombstone with its name and description nulled. `qkeys`: rows this circle holds in circle_qkeys, kept
-- by the statements that write them (the per-circle cap without a COUNT on every send).
CREATE TABLE circles (
  id           TEXT PRIMARY KEY,
  name         TEXT,
  description  TEXT,
  created_by   TEXT NOT NULL,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  closed_at    INTEGER,
  close_reason TEXT CHECK (close_reason IN ('owner_closed', 'last_member_gone', 'admin')),
  undo_until   INTEGER,
  purge_after  INTEGER,
  purged_at    INTEGER,
  qkeys        INTEGER NOT NULL DEFAULT 0 CHECK (qkeys >= 0),
  CHECK ((closed_at IS NULL) = (close_reason IS NULL)),
  CHECK (purged_at IS NOT NULL OR name IS NOT NULL)
);
CREATE INDEX circles_purge ON circles (purge_after) WHERE purge_after IS NOT NULL AND purged_at IS NULL;
-- (the daily resweep's safety net: open circles, each checked for an active owner, once a day)
CREATE INDEX circles_open ON circles (created_at) WHERE closed_at IS NULL AND purged_at IS NULL;

-- One row per (circle, account). Active means left_at IS NULL. Rows of members who left or were removed
-- stay until the circle is purged (the public re-share check needs former membership); re-joining
-- reuses the row with a new joined_at, join_seq and role. Account erasure deletes the account's rows.
-- `join_seq` comes from pool_seq, so "longest-standing" (D18: from joining) never ties.
CREATE TABLE circle_members (
  circle_id   TEXT NOT NULL,
  user_id     TEXT NOT NULL,
  role        TEXT NOT NULL CHECK (role IN ('owner', 'editor', 'viewer')),
  joined_at   INTEGER NOT NULL,
  join_seq    INTEGER NOT NULL,
  left_at     INTEGER,
  left_reason TEXT CHECK (left_reason IN ('left', 'removed', 'circle_closed', 'account_deleted')),
  invite_id   TEXT,
  PRIMARY KEY (circle_id, user_id),
  CHECK ((left_at IS NULL) = (left_reason IS NULL))
);
-- At most one ACTIVE owner per circle, enforced by the database: two racing transfers or successions
-- can never both win.
CREATE UNIQUE INDEX circle_members_one_owner ON circle_members (circle_id) WHERE role = 'owner' AND left_at IS NULL;
CREATE INDEX circle_members_user ON circle_members (user_id, left_at);
-- The succession pick (smallest join_seq per role) and member counts.
CREATE INDEX circle_members_active ON circle_members (circle_id, role, join_seq) WHERE left_at IS NULL;

-- Invite codes (mirrors pool_invites, round 28): only the SHA-256 of the normalised code is stored; the
-- code is shown once and never stored or audited. Single use by default, at most 50 uses, an expiry
-- (7 days by default, 30 at most), revocable (a revocation stops further joins, never undoes one).
CREATE TABLE circle_invites (
  id         TEXT PRIMARY KEY,
  circle_id  TEXT NOT NULL,
  code_hash  TEXT NOT NULL UNIQUE,
  role       TEXT NOT NULL CHECK (role IN ('editor', 'viewer')),
  created_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  max_uses   INTEGER NOT NULL DEFAULT 1 CHECK (max_uses BETWEEN 1 AND 50),
  uses       INTEGER NOT NULL DEFAULT 0 CHECK (uses >= 0 AND uses <= max_uses),
  expires_at INTEGER NOT NULL,
  revoked_at INTEGER,
  revoked_by TEXT,
  note       TEXT
);
CREATE INDEX circle_invites_circle ON circle_invites (circle_id, created_at);
CREATE INDEX circle_invites_creator ON circle_invites (created_by);

-- ---- banks and versions -------------------------------------------------------------------------------
-- `author_id`: the member who first shared the bank ("the item's author" for decision 4 and D17).
-- `license`: 'circle-only' (members may not re-share) or one of the five CC licences (members may
-- re-share under it; phase C), D19. head_version, head_version_id and the listing columns are written by
-- the same compare-and-set statement that accepts a version.
CREATE TABLE circle_banks (
  id              TEXT PRIMARY KEY,
  circle_id       TEXT NOT NULL,
  author_id       TEXT NOT NULL,
  title           TEXT NOT NULL DEFAULT '',
  subject         TEXT NOT NULL DEFAULT '',
  course          TEXT NOT NULL DEFAULT '',
  grade           TEXT NOT NULL DEFAULT '',
  license         TEXT NOT NULL CHECK (license IN ('CC-BY-4.0', 'CC0-1.0', 'CC-BY-SA-4.0', 'CC-BY-NC-4.0', 'CC-BY-NC-SA-4.0', 'circle-only')),
  head_version    INTEGER NOT NULL DEFAULT 0,
  head_version_id TEXT,
  question_count  INTEGER NOT NULL DEFAULT 0,
  has_images      INTEGER NOT NULL DEFAULT 0,
  -- The author was erased and the bank kept (D15): shown as by "deleted teacher".
  anonymized      INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  removed_at      INTEGER,
  removed_reason  TEXT CHECK (removed_reason IN ('author_removed', 'owner_removed', 'circle_closed', 'account_deleted', 'admin', 'privacy', 'takedown')),
  purge_after     INTEGER,
  purged_at       INTEGER,
  CHECK ((removed_at IS NULL) = (removed_reason IS NULL))
);
CREATE INDEX circle_banks_circle ON circle_banks (circle_id, purged_at, removed_at);
CREATE INDEX circle_banks_author ON circle_banks (author_id);
CREATE INDEX circle_banks_purge ON circle_banks (purge_after) WHERE purge_after IS NOT NULL AND purged_at IS NULL;

-- One row per version sent. Linear, immutable once committed (status moves current -> previous ->
-- pruned). `version` is NULL while staging (the body is put to R2 first; the commit assigns it).
-- `commit_id` is the client's idempotency key (a retry with the same id and body replays the first
-- answer). `seq` (pool_seq) orders "who sent this text first" across banks and circles. `bytes`: body
-- + index + the size of each image the version references (charged per version, D3). Phase B uses
-- base_version / merged_from / rollback_of; they are here so it needs no migration for them.
CREATE TABLE circle_versions (
  id             TEXT PRIMARY KEY,
  bank_id        TEXT NOT NULL,
  circle_id      TEXT NOT NULL,
  version        INTEGER,
  commit_id      TEXT NOT NULL UNIQUE,
  request_sha256 TEXT NOT NULL,
  result_json    TEXT,
  committer_id   TEXT NOT NULL,
  base_version   INTEGER,
  merged_from    INTEGER,
  rollback_of    INTEGER,
  seq            INTEGER,
  -- Up to 500 characters; nulled when the sender's account is erased.
  message        TEXT,
  -- Per version: a CC grant is made per version and is irrevocable for it.
  license        TEXT NOT NULL,
  status         TEXT NOT NULL CHECK (status IN ('staging', 'current', 'previous', 'pruned')),
  -- R2 keys under circles/<circle>/<bank>/ ('' once a pruned version's objects are deleted).
  full_key       TEXT NOT NULL,
  index_key      TEXT NOT NULL,
  bytes          INTEGER NOT NULL,
  question_count INTEGER NOT NULL,
  added          INTEGER NOT NULL DEFAULT 0,
  changed        INTEGER NOT NULL DEFAULT 0,
  removed        INTEGER NOT NULL DEFAULT 0,
  -- Declared origins of public material (positions, item ids, versions) the send was checked against.
  origins        TEXT,
  created_at     INTEGER NOT NULL,
  committed_at   INTEGER,
  pruned_at      INTEGER,
  UNIQUE (bank_id, version)
);
-- The circle's storage quota is one indexed SUM over its kept versions.
CREATE INDEX circle_versions_circle ON circle_versions (circle_id, status);
CREATE INDEX circle_versions_committer ON circle_versions (committer_id);
CREATE INDEX circle_versions_bank ON circle_versions (bank_id, status);
CREATE INDEX circle_versions_staging ON circle_versions (created_at) WHERE status = 'staging';
-- (the daily prune stage: pruned versions whose R2 objects are not deleted yet)
CREATE INDEX circle_versions_prune ON circle_versions (pruned_at) WHERE status = 'pruned' AND full_key <> '';

-- The images a kept version uses (the images themselves stay in `blobs` / R2 `blobs/`). The hourly blob
-- GC never deletes an image listed here.
CREATE TABLE circle_version_blobs (
  version_id TEXT NOT NULL,
  sha256     TEXT NOT NULL,
  PRIMARY KEY (version_id, sha256)
) WITHOUT ROWID;
CREATE INDEX circle_version_blobs_sha ON circle_version_blobs (sha256);

-- The key-free match_hash of every question any version of a bank has held, with the version, sequence
-- number and sender that FIRST held it (INSERT OR IGNORE: never overwritten). Used only by the public
-- publish check (D13). Kept until the bank is purged.
CREATE TABLE circle_qkeys (
  circle_id     TEXT NOT NULL,
  match_hash    TEXT NOT NULL,
  bank_id       TEXT NOT NULL,
  first_version INTEGER NOT NULL,
  first_seq     INTEGER NOT NULL,
  first_by      TEXT NOT NULL,
  PRIMARY KEY (circle_id, match_hash, bank_id)
) WITHOUT ROWID;
-- The check's probe: the earliest sender of a hash in a circle is one seek (first_seq ordered).
CREATE INDEX circle_qkeys_first ON circle_qkeys (circle_id, match_hash, first_seq, bank_id, first_by);
CREATE INDEX circle_qkeys_bank ON circle_qkeys (bank_id);

-- ---- moderation -----------------------------------------------------------------------------------------
-- Kept apart from `reports`, whose readers assume a public item. Open rows are the admin's queue.
CREATE TABLE circle_reports (
  id          TEXT PRIMARY KEY,
  circle_id   TEXT NOT NULL,
  bank_id     TEXT NOT NULL,
  version     INTEGER,
  -- NULL once the reporter's account is erased (kept, anonymised, as moderation evidence).
  reporter_id TEXT,
  reason      TEXT NOT NULL CHECK (reason IN ('student_data', 'copyright', 'inappropriate', 'other')),
  note        TEXT,
  at          INTEGER NOT NULL,
  resolved_at INTEGER,
  resolved_by TEXT,
  action      TEXT
);
CREATE UNIQUE INDEX circle_reports_open ON circle_reports (bank_id, reporter_id, reason) WHERE resolved_at IS NULL;
CREATE INDEX circle_reports_reporter ON circle_reports (reporter_id, at);
CREATE INDEX circle_reports_queue ON circle_reports (resolved_at, at);
CREATE INDEX circle_reports_circle ON circle_reports (circle_id, resolved_at);
-- A hold hides a bank from every member but its author (mirrors item_holds). `source`: '' (a report),
-- 'manual' (an admin).
CREATE TABLE circle_holds (
  bank_id TEXT NOT NULL,
  reason  TEXT NOT NULL CHECK (reason IN ('reports', 'student_data', 'admin')),
  source  TEXT NOT NULL DEFAULT '',
  at      INTEGER NOT NULL,
  by      TEXT,
  PRIMARY KEY (bank_id, reason, source)
) WITHOUT ROWID;
CREATE INDEX circle_holds_at ON circle_holds (reason, source, at);
