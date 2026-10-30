-- AI import jobs (docs/ai-harness-spec.md §3-4): one row per uploaded document the Workers AI relay
-- converts in the background. `result` is the JSON of the pages converted so far (rewritten after each
-- page); a result above 256 KB lives in R2 (ai/jobs/<id>/result.json) and the column is NULL. `error` is a
-- fixed, sanitised sentence, never a provider message, a prompt or a key. Deleted after AI_JOB_TTL_HOURS by
-- the hourly cron.
CREATE TABLE IF NOT EXISTS ai_jobs (
  id          TEXT PRIMARY KEY,
  principal   TEXT NOT NULL,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  status      TEXT NOT NULL CHECK (status IN ('queued','running','done','failed')),
  pages_total INTEGER NOT NULL,
  pages_done  INTEGER NOT NULL DEFAULT 0,
  hint        TEXT,
  error       TEXT,
  result      TEXT
);
CREATE INDEX IF NOT EXISTS ai_jobs_principal_created ON ai_jobs (principal, created_at);
CREATE INDEX IF NOT EXISTS ai_jobs_created ON ai_jobs (created_at);
