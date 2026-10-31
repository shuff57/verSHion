// Small pieces the relay and the import jobs share.

export type Part = { type: "text"; text: string } | { type: "image_url"; image_url: { url: string } };
export type Message = { role: string; content: string | Part[] };

export const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });

export const err = (status: number, code: string, message: string, headers?: Record<string, string>): Response =>
  json(status, { error: { code, message } }, headers);

/** Decoded byte count of a base64 image data URI, or null when the URI is not one we accept. */
export function dataUriBytes(url: string): number | null {
  const m = /^data:image\/(?:png|jpeg|webp);base64,([A-Za-z0-9+/=]+)$/.exec(url);
  return m ? Math.floor((m[1]!.length * 3) / 4) : null;
}

/** What one model turn gave back: the text, and what it cost. Never the provider's own body. */
export type ModelOut =
  | { ok: true; reply: Record<string, unknown>; text: string; inTokens: number; outTokens: number; costUsd: number }
  | { ok: false; code: "failed" | "empty" };
