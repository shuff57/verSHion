// D1 side of the Workers AI relay: usage rows for the quota, and import job rows.
// Schema: migrations/0116_ai_jobs.sql and 0117_ai_usage.sql. Nothing here reads a request body.

export const HOUR_MS = 3_600_000;
export const DAY_MS = 86_400_000;
export const USAGE_KEEP_MS = 90 * DAY_MS;
/** A result above this lives in R2 instead of the D1 row. */
export const RESULT_D1_MAX = 256 * 1024;

export type JobStatus = "queued" | "running" | "done" | "failed";

export interface JobRow {
  id: string;
  principal: string;
  created_at: number;
  updated_at: number;
  status: JobStatus;
  pages_total: number;
  pages_done: number;
  hint: string | null;
  error: string | null;
  result: string | null;
}

export interface Usage {
  kind: "chat" | "import_page";
  inTokens: number;
  outTokens: number;
  neurons: number;
}

/** Start of the UTC day containing `now`: the daily neuron cap resets here. */
export const utcDayStart = (now: number): number => now - (now % DAY_MS);

export function newJobId(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

export class AiStore {
  private readonly db: D1Database;
  constructor(db: D1Database) {
    this.db = db;
  }

  async record(principal: string, u: Usage, now: number): Promise<void> {
    await this.db
      .prepare(`INSERT INTO ai_usage (id, principal, at, kind, in_tokens, out_tokens, neurons) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)`)
      .bind(newJobId(), principal, now, u.kind, Math.max(0, Math.floor(u.inTokens)), Math.max(0, Math.floor(u.outTokens)), Math.max(0, u.neurons))
      .run();
  }

  /** Chat turns one principal made since `since`. */
  async chatTurnsSince(principal: string, since: number): Promise<number> {
    const r = await this.db
      .prepare(`SELECT COUNT(*) AS n FROM ai_usage WHERE principal = ?1 AND at >= ?2 AND kind = 'chat'`)
      .bind(principal, since)
      .first<{ n: number }>();
    return Number(r?.n ?? 0);
  }

  /** Neurons the whole account spent since `since` (the binding is billed to one account). */
  async neuronsSince(since: number): Promise<number> {
    const r = await this.db.prepare(`SELECT COALESCE(SUM(neurons), 0) AS n FROM ai_usage WHERE at >= ?1`).bind(since).first<{ n: number }>();
    return Number(r?.n ?? 0);
  }

  async jobsSince(principal: string, since: number): Promise<number> {
    const r = await this.db.prepare(`SELECT COUNT(*) AS n FROM ai_jobs WHERE principal = ?1 AND created_at >= ?2`).bind(principal, since).first<{ n: number }>();
    return Number(r?.n ?? 0);
  }

  async createJob(id: string, principal: string, pagesTotal: number, hint: string, now: number): Promise<void> {
    await this.db
      .prepare(`INSERT INTO ai_jobs (id, principal, created_at, updated_at, status, pages_total, pages_done, hint) VALUES (?1, ?2, ?3, ?3, 'queued', ?4, 0, ?5)`)
      .bind(id, principal, now, pagesTotal, hint || null)
      .run();
  }

  /** A job, only for the principal that made it: anyone else gets the same null as for a missing id. */
  async job(id: string, principal: string): Promise<JobRow | null> {
    return this.db.prepare(`SELECT * FROM ai_jobs WHERE id = ?1 AND principal = ?2`).bind(id, principal).first<JobRow>();
  }

  async progress(id: string, status: JobStatus, pagesDone: number, result: string | null, error: string | null, now: number): Promise<void> {
    await this.db
      .prepare(`UPDATE ai_jobs SET status = ?2, pages_done = ?3, result = ?4, error = ?5, updated_at = ?6 WHERE id = ?1`)
      .bind(id, status, pagesDone, result, error, now)
      .run();
  }

  /** Ids of expired jobs (for their R2 results), then the rows, and usage past its retention. */
  async prune(now: number, jobTtlMs: number): Promise<string[]> {
    const old = await this.db.prepare(`SELECT id FROM ai_jobs WHERE created_at < ?1`).bind(now - jobTtlMs).all<{ id: string }>();
    await this.db.prepare(`DELETE FROM ai_jobs WHERE created_at < ?1`).bind(now - jobTtlMs).run();
    await this.db.prepare(`DELETE FROM ai_usage WHERE at < ?1`).bind(now - USAGE_KEEP_MS).run();
    return (old.results ?? []).map((r) => r.id);
  }
}
