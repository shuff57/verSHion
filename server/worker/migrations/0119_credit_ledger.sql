-- Purchased credits for the built-in AI (docs: billing). Append-only: the balance is the sum of a
-- principal's deltas. (kind, ref) is unique, so a replayed Stripe event or a second refund sweep
-- cannot credit or debit twice. Rows are kept until the teacher deletes the account.
--   purchase  +n  ref = Stripe checkout session id
--   spend     -n  ref = ai_sessions.id (the part of a session's price the weekly allowance did not cover)
--   refund    +n  ref = ai_sessions.id (the session expired without an answered call)
--   reversal  -n  ref = Stripe refund / dispute id (may take the balance below zero, which blocks spending)
--   grant     +n  ref = admin note id
CREATE TABLE IF NOT EXISTS credit_ledger (
  id        TEXT PRIMARY KEY,
  principal TEXT NOT NULL,
  delta     INTEGER NOT NULL,
  kind      TEXT NOT NULL CHECK (kind IN ('purchase','spend','refund','reversal','grant')),
  ref       TEXT NOT NULL,
  at        INTEGER NOT NULL,
  -- What a purchase cost in US cents (purchase rows only): refunds and disputes take back credits pro rata.
  cents     INTEGER,
  UNIQUE (kind, ref)
);
CREATE INDEX IF NOT EXISTS credit_ledger_principal ON credit_ledger (principal);

-- The part of a session's price drawn from the purchased balance; the rest came from the weekly allowance.
ALTER TABLE ai_sessions ADD COLUMN paid_credits INTEGER NOT NULL DEFAULT 0;
