-- The ad / link source a new account signed up from (`?src=<tag>` on the link, docs: growth report). Set once, on the
-- sign-up that created the account, and never changed. NULL when the link had no tag (or an invalid one). Only the
-- admin's growth report reads it, as counts per tag: no address, id or visit is tied to it anywhere else.
ALTER TABLE users ADD COLUMN signup_source TEXT;
