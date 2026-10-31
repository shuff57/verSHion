-- OpenRouter is the only upstream: a call's cost is what OpenRouter billed (usage.cost, US dollars), and the
-- account's daily cap is a dollar cap. `neurons` (the retired Workers AI unit) stays, always 0.
-- `session` links a chat turn to its credit session, so what each service costs can be measured and credits priced.
ALTER TABLE ai_usage ADD COLUMN cost_usd REAL NOT NULL DEFAULT 0;
ALTER TABLE ai_usage ADD COLUMN session TEXT;
CREATE INDEX IF NOT EXISTS ai_usage_session ON ai_usage (session);
