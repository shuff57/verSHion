-- Round 42 (circles phase C, docs/circles-plan.md D5): email invites. An email invite IS a circle code (the same
-- single-use hashed code a normal invite stores; the code itself is only ever in the mail), so a join goes through
-- the usual code join and every gate in front of it. `kind` tells the owner's list which codes were mailed. The
-- address is never stored: the per-address daily cap keeps an HMAC of the mailbox (its own HKDF purpose), one row per
-- mailbox and day, deleted by the daily cron after two days.
-- Round 46 (critic 53a variant 2; edited in place, not applied anywhere yet): `email_label`, a MASKED form of the
-- address made at invite time (its first letter and the domain, "j***@gmail.com"), so the owner can tell invites
-- apart and revoke the right one; nulled with its inviter's erasure and by the daily cron once the invite is long dead.
ALTER TABLE circle_invites ADD COLUMN kind TEXT NOT NULL DEFAULT 'code' CHECK (kind IN ('code', 'email'));
ALTER TABLE circle_invites ADD COLUMN email_label TEXT;
CREATE INDEX IF NOT EXISTS circle_invites_label ON circle_invites (expires_at) WHERE email_label IS NOT NULL;
CREATE TABLE IF NOT EXISTS circle_email_sends (
  addr TEXT NOT NULL,
  day  INTEGER NOT NULL,
  n    INTEGER NOT NULL CHECK (n >= 0),
  PRIMARY KEY (addr, day)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS circle_email_sends_day ON circle_email_sends (day);
-- Columns with defaults, an index and a new table; no existing row changes. (Round 48: IF NOT EXISTS where SQLite
-- allows it; an ADD COLUMN cannot have it.) The next is 0112.
