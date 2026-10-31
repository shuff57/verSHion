// Credits for the built-in AI. The server alone prices a service, caps its pages and gives it a budget
// of model calls: a client names the service and (for page-priced ones) the page count, and nothing else.
import { DAY_MS } from "./store.ts";

export type ServiceId = "import" | "figure" | "handwriting" | "alternatives" | "answer_key" | "grade";

export interface ServiceDef {
  /** Credits charged for a session of `pages` pages (pages is 0 for services that do not take any). */
  credits(pages: number): number;
  takesPages: boolean;
  maxPages: number;
  /** Model calls the session may make. Generous: a page read can retry, redo its picture box and hedge, and a figure fit is several calls. */
  calls(pages: number): number;
  ttlMs: number;
}

const flat = (n: number) => () => n;

export const SERVICES: Record<ServiceId, ServiceDef> = {
  // 2 credits for a whole test, up to 20 pages (to be revisited with real usage).
  import: { credits: flat(2), takesPages: true, maxPages: 20, calls: (p) => p * 40, ttlMs: 2 * 3_600_000 },
  // The figure agent: up to 8 model steps, hedged and with image looks.
  figure: { credits: flat(2), takesPages: false, maxPages: 0, calls: () => 40, ttlMs: 30 * 60_000 },
  // 1 credit per page; each side of a sheet is a page.
  handwriting: { credits: (p) => p, takesPages: true, maxPages: 20, calls: (p) => p * 4, ttlMs: 3_600_000 },
  // Reserved: not in the app yet.
  alternatives: { credits: flat(1), takesPages: false, maxPages: 0, calls: () => 12, ttlMs: 15 * 60_000 },
  answer_key: { credits: flat(1), takesPages: false, maxPages: 0, calls: () => 12, ttlMs: 15 * 60_000 },
  grade: { credits: flat(3), takesPages: true, maxPages: 40, calls: (p) => p * 6 + 12, ttlMs: 3_600_000 },
};

export const WEEK_MS = 7 * DAY_MS;

/** Start of the UTC week (Monday 00:00) containing `now`: credits come back here. */
export function utcWeekStart(now: number): number {
  const day = Math.floor(now / DAY_MS);
  // 1970-01-01 was a Thursday, so (day + 3) % 7 is 0 on a Monday.
  return (day - ((day + 3) % 7)) * DAY_MS;
}

export function isServiceId(v: unknown): v is ServiceId {
  return typeof v === "string" && Object.prototype.hasOwnProperty.call(SERVICES, v);
}

/** The price list the app may show. */
export function describeServices(): Record<string, { credits: number; per_page: boolean; max_pages: number }> {
  return Object.fromEntries(
    Object.entries(SERVICES).map(([id, s]) => [id, { credits: s.credits(s.takesPages ? 1 : 0), per_page: s.takesPages && s.credits(2) > s.credits(1), max_pages: s.maxPages }]),
  );
}
