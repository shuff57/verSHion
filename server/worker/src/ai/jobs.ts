// Import jobs: a document's pages converted to Ditto text in the background (spec §3).
//
// The worker returns each page's raw model text, not a parsed document: parsing and picture cropping
// need the page raster and live in the app's `ditto::import_text`, so there is one parser, not two.
// Pages run one at a time inside `ctx.waitUntil`; a job whose worker is evicted mid-run shows as failed
// once it has not moved for STALE_MS.
import { DITTO_PROMPT } from "./prompt.ts";
import { AiStore, DAY_MS, RESULT_D1_MAX, utcDayStart, newJobId, type JobRow } from "./store.ts";
import { dataUriBytes, err, json, type Message, type ModelOut } from "./util.ts";

export const MAX_HINT_CHARS = 500;
export const STALE_MS = 10 * 60_000;
const MIMES = ["image/png", "image/jpeg", "image/webp"];
const JOB_ID = /^[0-9a-f]{32}$/;

export interface JobLimits {
  maxPages: number;
  maxImageBytes: number;
  importPerDay: number;
  dailyNeurons: number;
}

export interface JobDeps {
  store: AiStore;
  /** `env.POOL`: results above 256 KB go here. */
  bucket?: R2Bucket;
  /** One model turn; the relay's own retry and shaping live behind it. */
  model(messages: Message[]): Promise<ModelOut>;
  now(): number;
  limits: JobLimits;
}

type PageResult = { page: number; text?: string; error?: string };

/** The prompt the app sends for one page, hint included (src/ai.rs `transcription_prompt`). */
export function pagePrompt(hint: string): string {
  const h = hint.trim();
  return h ? `${DITTO_PROMPT}\n\nUser note about this source:\n${h}` : DITTO_PROMPT;
}

type Validated = { hint: string; images: string[] } | { error: string };

function validate(body: Record<string, unknown>, l: JobLimits): Validated {
  const hint = typeof body.hint === "string" ? body.hint.trim() : "";
  if (hint.length > MAX_HINT_CHARS) return { error: `hint is longer than ${MAX_HINT_CHARS} characters` };
  const pages = body.pages;
  if (!Array.isArray(pages) || pages.length === 0) return { error: "pages must be a non-empty array" };
  if (pages.length > l.maxPages) return { error: `more than ${l.maxPages} pages` };
  const images: string[] = [];
  for (const [i, p] of pages.entries()) {
    const { mime, b64 } = (p ?? {}) as { mime?: unknown; b64?: unknown };
    if (typeof mime !== "string" || !MIMES.includes(mime)) return { error: `pages[${i}].mime must be png, jpeg or webp` };
    if (typeof b64 !== "string") return { error: `pages[${i}].b64 must be a base64 string` };
    const url = `data:${mime};base64,${b64}`;
    const bytes = dataUriBytes(url);
    if (bytes === null) return { error: `pages[${i}].b64 is not base64` };
    if (bytes > l.maxImageBytes) return { error: `pages[${i}] image is too large` };
    images.push(url);
  }
  return { hint, images };
}

/** `POST /ai/import`: validates, applies the quotas, records the job, starts it, answers 202. */
export async function startJob(d: JobDeps, body: Record<string, unknown>, principal: string, wait: (p: Promise<unknown>) => void): Promise<Response> {
  const v = validate(body, d.limits);
  if ("error" in v) return err(400, "bad_request", v.error);
  const now = d.now();
  if ((await d.store.jobsSince(principal, now - DAY_MS)) >= d.limits.importPerDay)
    return err(429, "rate_limited", "Too many imports today. Try again tomorrow.", { "Retry-After": String(Math.ceil((utcDayStart(now) + DAY_MS - now) / 1000)) });
  if ((await d.store.neuronsSince(utcDayStart(now))) >= d.limits.dailyNeurons)
    return err(429, "quota_exhausted", "The AI allowance for today is used up.", { "Retry-After": String(Math.ceil((utcDayStart(now) + DAY_MS - now) / 1000)) });
  const id = newJobId();
  await d.store.createJob(id, principal, v.images.length, v.hint, now);
  wait(runJob(d, id, principal, v.hint, v.images));
  return json(202, { job_id: id });
}

async function runJob(d: JobDeps, id: string, principal: string, hint: string, images: string[]): Promise<void> {
  const out: PageResult[] = [];
  let ok = 0;
  try {
    const prompt = pagePrompt(hint);
    await d.store.progress(id, "running", 0, null, null, d.now());
    for (const [i, url] of images.entries()) {
      if ((await d.store.neuronsSince(utcDayStart(d.now()))) >= d.limits.dailyNeurons) {
        out.push({ page: i + 1, error: "quota_exhausted" });
      } else {
        const r = await d.model([{ role: "user", content: [{ type: "text", text: prompt }, { type: "image_url", image_url: { url } }] }]);
        if (r.ok) {
          ok += 1;
          out.push({ page: i + 1, text: r.text });
          await d.store.record(principal, { kind: "import_page", inTokens: r.inTokens, outTokens: r.outTokens, neurons: r.neurons }, d.now());
        } else {
          out.push({ page: i + 1, error: "model_failed" });
        }
      }
      await save(d, id, "running", out, null);
    }
    await save(d, id, ok > 0 ? "done" : "failed", out, ok > 0 ? null : "No page could be read.");
  } catch {
    // A store or bucket failure: say so plainly, never the exception's own text.
    try {
      await d.store.progress(id, "failed", out.length, null, "The job stopped unexpectedly.", d.now());
    } catch {
      /* the database itself is down: the stale check will report it */
    }
  }
}

async function save(d: JobDeps, id: string, status: "running" | "done" | "failed", pages: PageResult[], error: string | null): Promise<void> {
  const text = JSON.stringify({ pages });
  let stored: string | null = text;
  if (text.length > RESULT_D1_MAX) {
    if (!d.bucket) throw new Error("result too large and no bucket");
    await d.bucket.put(`ai/jobs/${id}/result.json`, text);
    stored = null;
  }
  await d.store.progress(id, status, pages.length, stored, error, d.now());
}

/** `GET /ai/import/:id`: the job's state and the pages converted so far. Only its owner may read it. */
export async function readJob(d: JobDeps, id: string, principal: string): Promise<Response> {
  const row = JOB_ID.test(id) ? await d.store.job(id, principal) : null;
  if (!row) return err(404, "not_found", "No such job.");
  let { status, error } = row;
  if ((status === "queued" || status === "running") && d.now() - row.updated_at > STALE_MS) {
    status = "failed";
    error = "The job was interrupted. Start it again.";
    await d.store.progress(id, "failed", row.pages_done, row.result, error, d.now());
  }
  return json(200, { status, pages_done: row.pages_done, pages_total: row.pages_total, error, result: await resultOf(d, row) });
}

async function resultOf(d: JobDeps, row: JobRow): Promise<unknown> {
  if (row.result !== null) return JSON.parse(row.result);
  if (row.pages_done === 0 || !d.bucket) return { pages: [] };
  const o = await d.bucket.get(`ai/jobs/${row.id}/result.json`);
  return o ? JSON.parse(await o.text()) : { pages: [] }; // r2-object: our own stored result, not a request body
}

/** Hourly cron: drop expired jobs (and their R2 results) and old usage rows. */
export async function pruneAi(db: D1Database, bucket: R2Bucket | undefined, now: number, jobTtlMs: number): Promise<number> {
  const ids = await new AiStore(db).prune(now, jobTtlMs);
  if (bucket) for (const id of ids) await bucket.delete(`ai/jobs/${id}/result.json`);
  return ids.length;
}
