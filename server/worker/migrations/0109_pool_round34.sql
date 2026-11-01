-- Round 34 (circles, after critics 41a/41b F3): a transfer of ownership is an OFFER the new owner accepts.
-- At most one pending offer per circle (a new offer replaces it). It is void once it expires (7 days), once
-- the offerer is no longer the circle's active owner, or once the target is no longer an active member in
-- good standing; the accept checks all of that, and the target's circles_owned cap, in one statement. Rows
-- go when accepted, declined, cancelled, replaced, when the circle is purged, when either account is erased,
-- and daily once expired. A new table only; nothing else changes.
CREATE TABLE circle_transfers (
  circle_id  TEXT PRIMARY KEY,
  from_user  TEXT NOT NULL,
  to_user    TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);
CREATE INDEX circle_transfers_to ON circle_transfers (to_user);
CREATE INDEX circle_transfers_from ON circle_transfers (from_user);
CREATE INDEX circle_transfers_expiry ON circle_transfers (expires_at);
