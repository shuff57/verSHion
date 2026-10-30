-- Workers AI relay usage (docs/ai-harness-spec.md §5): one row per model turn that returned text, so the
-- per-principal hourly turn cap and the account's daily neuron cap can be counted. No prompt, image or
-- reply is stored, only sizes. Pruned after 90 days by the hourly cron.
CREATE TABLE IF NOT EXISTS ai_usage (
  id         TEXT PRIMARY KEY,
  principal  TEXT NOT NULL,
  at         INTEGER NOT NULL,
  kind       TEXT NOT NULL CHECK (kind IN ('chat','import_page')),
  in_tokens  INTEGER NOT NULL,
  out_tokens INTEGER NOT NULL,
  neurons    REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS ai_usage_principal_at ON ai_usage (principal, at);
CREATE INDEX IF NOT EXISTS ai_usage_at ON ai_usage (at);
