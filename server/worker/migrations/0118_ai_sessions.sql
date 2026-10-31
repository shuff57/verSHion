-- Credits for the built-in AI (one session per service use). A signed-in teacher starts a session
-- ("import a test", "figure agent", ...); the server prices it, charges the credits at the start and
-- gives the session a budget of model calls. Credits spent in a week = the sum of the week's
-- sessions. A session that expired without a single answered call is not counted (lazy refund).
-- No prompt, image or reply is stored. Pruned after 90 days by the hourly cron.
CREATE TABLE IF NOT EXISTS ai_sessions (
  id            TEXT PRIMARY KEY,
  principal     TEXT NOT NULL,
  service       TEXT NOT NULL,
  credits       INTEGER NOT NULL,
  calls_allowed INTEGER NOT NULL,
  calls_used    INTEGER NOT NULL DEFAULT 0,
  calls_ok      INTEGER NOT NULL DEFAULT 0,
  pages         INTEGER NOT NULL DEFAULT 0,
  created_at    INTEGER NOT NULL,
  expires_at    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ai_sessions_principal_at ON ai_sessions (principal, created_at);
