-- Round 48 (circles phase C, critic 55a V6): an email invite to an address that already has an open invite in the
-- circle rotates that invite's code instead of making a second one. To find it, the invite keeps an HMAC of the mailbox
-- and the circle (`email_key`, the invite's own HKDF purpose, never the address), nulled with its masked label: with
-- its inviter's erasure and by the daily cron once the invite is long dead. A nullable column and a partial index; no
-- existing row changes.
ALTER TABLE circle_invites ADD COLUMN email_key TEXT;
CREATE INDEX IF NOT EXISTS circle_invites_email_key ON circle_invites (circle_id, email_key) WHERE email_key IS NOT NULL;
-- The next is 0116.
