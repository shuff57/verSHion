// D1 side of the Workers AI relay: usage rows for the quota, and import job rows.
// Schema: migrations/0116_ai_jobs.sql and 0117_ai_usage.sql. Nothing here reads a request body.

export const HOUR_MS = 3_600_000;
export const DAY_MS = 86_400_000;
export const USAGE_KEEP_MS = 90 * DAY_MS;
/** A result above this lives in R2 instead of the D1 row. */
export const RESULT_D1_MAX = 256 * 1024;

export interface SessionRow {
  id: string;
  principal: string;
  service: string;
  credits: number;
  calls_allowed: number;
  calls_used: number;
  calls_ok: number;
  pages: number;
  created_at: number;
  expires_at: number;
}

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

  /** Weekly-allowance credits used since `weekStart`: the part of each session the purchased balance did not pay. An expired session with no answered call is not counted. */
  async creditsSpent(principal: string, weekStart: number, now: number): Promise<number> {
    const r = await this.db
      .prepare(`SELECT COALESCE(SUM(credits - paid_credits), 0) AS n FROM ai_sessions WHERE principal = ?1 AND created_at >= ?2 AND NOT (calls_ok = 0 AND expires_at < ?3)`)
      .bind(principal, weekStart, now)
      .first<{ n: number }>();
    return Number(r?.n ?? 0);
  }

  /** Purchased credits: the sum of the principal's ledger. Negative after a refund or dispute took back credits already spent. */
  async purchasedBalance(principal: string): Promise<number> {
    const r = await this.db.prepare(`SELECT COALESCE(SUM(delta), 0) AS n FROM credit_ledger WHERE principal = ?1`).bind(principal).first<{ n: number }>();
    return Number(r?.n ?? 0);
  }

  /** Adds a ledger row once; false when (kind, ref) is already there (a replayed Stripe event). */
  async ledger(principal: string, delta: number, kind: "purchase" | "reversal" | "grant", ref: string, now: number): Promise<boolean> {
    const r = await this.db
      .prepare(`INSERT OR IGNORE INTO credit_ledger (id, principal, delta, kind, ref, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)`)
      .bind(crypto.randomUUID().replace(/-/g, ""), principal, delta, kind, ref, now)
      .run();
    return (r.meta?.changes ?? 0) === 1;
  }

  /** Gives back the purchased part of sessions that expired without an answered call. Idempotent per session. */
  async refundExpired(principal: string, now: number): Promise<void> {
    await this.db
      .prepare(
        `INSERT OR IGNORE INTO credit_ledger (id, principal, delta, kind, ref, at)
         SELECT lower(hex(randomblob(16))), principal, paid_credits, 'refund', id, ?2 FROM ai_sessions
         WHERE principal = ?1 AND paid_credits > 0 AND calls_ok = 0 AND expires_at < ?2`,
      )
      .bind(principal, now)
      .run();
  }

  /**
   * Charges and opens a session atomically. The weekly allowance pays first; `paid` is what the purchased
   * balance covers. The session row goes in only if both limits still hold, and the ledger spend is written
   * only if that row exists, in one batch, so two requests racing at either limit cannot both pass.
   */
  async startSession(s: { id: string; principal: string; service: string; credits: number; paid: number; callsAllowed: number; pages: number; now: number; expiresAt: number; weekStart: number; allowance: number }): Promise<boolean> {
    const [ins] = await this.db.batch([
      this.db
        .prepare(
          `INSERT INTO ai_sessions (id, principal, service, credits, paid_credits, calls_allowed, calls_used, calls_ok, pages, created_at, expires_at)
           SELECT ?1, ?2, ?3, ?4, ?11, ?5, 0, 0, ?6, ?7, ?8
           WHERE (SELECT COALESCE(SUM(credits - paid_credits), 0) FROM ai_sessions WHERE principal = ?2 AND created_at >= ?9 AND NOT (calls_ok = 0 AND expires_at < ?7)) + ?4 - ?11 <= ?10
             AND (?11 = 0 OR (SELECT COALESCE(SUM(delta), 0) FROM credit_ledger WHERE principal = ?2) >= ?11)`,
        )
        .bind(s.id, s.principal, s.service, s.credits, s.callsAllowed, s.pages, s.now, s.expiresAt, s.weekStart, s.allowance, s.paid),
      this.db
        .prepare(`INSERT INTO credit_ledger (id, principal, delta, kind, ref, at) SELECT lower(hex(randomblob(16))), ?2, -?3, 'spend', ?1, ?4 WHERE ?3 > 0 AND EXISTS (SELECT 1 FROM ai_sessions WHERE id = ?1)`)
        .bind(s.id, s.principal, s.paid, s.now),
    ]);
    return (ins.meta?.changes ?? 0) === 1;
  }

  /** Takes one model call from the session's budget; false when it is not this principal's, expired or used up. */
  async takeCall(id: string, principal: string, now: number): Promise<boolean> {
    const r = await this.db
      .prepare(`UPDATE ai_sessions SET calls_used = calls_used + 1 WHERE id = ?1 AND principal = ?2 AND expires_at > ?3 AND calls_used < calls_allowed`)
      .bind(id, principal, now)
      .run();
    return (r.meta?.changes ?? 0) === 1;
  }

  async callAnswered(id: string): Promise<void> {
    await this.db.prepare(`UPDATE ai_sessions SET calls_ok = calls_ok + 1 WHERE id = ?1`).bind(id).run();
  }

  /** Ids of expired jobs (for their R2 results), then the rows, and usage past its retention. */
  async prune(now: number, jobTtlMs: number): Promise<string[]> {
    const old = await this.db.prepare(`SELECT id FROM ai_jobs WHERE created_at < ?1`).bind(now - jobTtlMs).all<{ id: string }>();
    await this.db.prepare(`DELETE FROM ai_jobs WHERE created_at < ?1`).bind(now - jobTtlMs).run();
    await this.db.prepare(`DELETE FROM ai_usage WHERE at < ?1`).bind(now - USAGE_KEEP_MS).run();
    await this.db.prepare(`DELETE FROM ai_sessions WHERE created_at < ?1`).bind(now - USAGE_KEEP_MS).run();
    return (old.results ?? []).map((r) => r.id);
  }
}
