// A JSON body is read with a hard limit (round 8 I4): a body without (or with a false) Content-Length
// is never buffered beyond MAX_BODY_BYTES before it is refused.
import { describe, expect, it } from "vitest";
import { HttpError, readJson } from "../src/http.js";

function chunked(total: number, pulled: { n: number }): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    pull(c) {
      if (pulled.n >= total) return c.close();
      const k = Math.min(1024, total - pulled.n);
      c.enqueue(new Uint8Array(k).fill(32));
      pulled.n += k;
    },
  });
}

describe("readJson body limit", () => {
  it("refuses a 1 MB chunked body after reading at most ~16 KB", async () => {
    const pulled = { n: 0 };
    const req = new Request("https://api.example.org/auth/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: chunked(1 << 20, pulled), duplex: "half" } as RequestInit);
    const e = await readJson(req).catch((x: unknown) => x);
    expect(e).toBeInstanceOf(HttpError);
    expect((e as HttpError).status).toBe(413);
    expect(pulled.n).toBeLessThanOrEqual(16 * 1024);
  });

  it("still reads a small body without a Content-Length", async () => {
    const req = new Request("https://api.example.org/auth/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: new ReadableStream({ start: (c) => (c.enqueue(new TextEncoder().encode('{"a":1}')), c.close()) }), duplex: "half" } as RequestInit);
    expect(await readJson(req)).toEqual({ a: 1 });
  });
});

describe("readJson duplicate keys", () => {
  const req = (body: string) => new Request("https://api.example.org/x", { method: "POST", headers: { "Content-Type": "application/json" }, body });
  it("refuses an object with the same key twice, at any depth and however escaped", async () => {
    for (const b of ['{"reason":"wrong_answer","reason":"student_data"}', '{"a":{"x":1,"x":2}}', '{"a":[{"k":1,"\\u006b":2}]}']) {
      const e = await readJson(req(b)).catch((x) => x);
      expect(e).toBeInstanceOf(HttpError);
      expect((e as HttpError).code).toBe("invalid_json");
    }
  });
  it("accepts the same key in sibling objects and keys that equal string values", async () => {
    expect(await readJson(req('{"a":{"k":1},"b":{"k":2},"c":"a","d":["a","a"],"e":"x\\"y"}'))).toEqual({ a: { k: 1 }, b: { k: 2 }, c: "a", d: ["a", "a"], e: 'x"y' });
  });
});
