// SMTP transport for the pool's mail, in the shape raSHio already sends with: a Cloudflare socket,
// AUTH LOGIN, one plain-text message. Port 465 is implicit TLS (the owner's Stalwart submission
// port); any other port does STARTTLS first. Cloudflare Workers may not open port 25, which is why
// this is submission with a mailbox login, not a relay.
//
// `cloudflare:sockets` is imported inside the send, not at the top: the worker's test suite runs on
// plain Node, which has no such module, and mail.ts must stay importable there (the message building
// and the header rules are what the tests pin).
import type { MailEnv } from "./mail.ts";

export const SMTP_FROM_NAME_DEFAULT = "verSHion";
const TIMEOUT_MS = 20_000;

/** Every setting the transport needs is present. Nothing else switches the pool onto SMTP. */
export function smtpConfigured(env: MailEnv): boolean {
  return Boolean(env.SMTP_HOSTNAME && env.SMTP_USERNAME && env.SMTP_PASSWORD);
}

/** The envelope and header sender: MAIL_FROM when set, else the mailbox that signs in. */
export function smtpFrom(env: MailEnv): string {
  return env.MAIL_FROM?.trim() || env.SMTP_USERNAME!.trim();
}

export function smtpPort(env: MailEnv): number {
  const n = Number(env.SMTP_PORT ?? "465");
  return Number.isInteger(n) && n > 0 && n < 65536 ? n : 465;
}

export function smtpImplicitTls(env: MailEnv): boolean {
  const v = env.SMTP_SECURE?.trim().toLowerCase();
  if (v === "on" || v === "1" || v === "true" || v === "implicit") return true;
  if (v === "off" || v === "0" || v === "false" || v === "starttls") return false;
  return smtpPort(env) === 465;
}

/** A header value may never carry a bare CR or LF: that ends the header line and injects another. */
export function headerValue(value: string): string {
  return String(value ?? "").replace(/[\r\n]+/g, " ").trim();
}

function base64(s: string): string {
  let bin = "";
  for (const byte of new TextEncoder().encode(s)) bin += String.fromCharCode(byte);
  return btoa(bin);
}

/**
 * One line: CR/LF removed first (encoding the raw value would let a client decode the line break
 * back into the header block), then RFC 2047 only when what remains is not plain ASCII.
 */
function subjectHeader(s: string): string {
  const line = headerValue(s);
  return /^[\x20-\x7e]*$/.test(line) ? line : `=?UTF-8?B?${base64(line)}?=`;
}

/**
 * The DATA payload: headers, a blank line, the body, ending with CRLF. The terminating line of a
 * message is a lone ".", so any body line that starts with "." gets a second one (RFC 5321 4.5.2).
 */
export function buildMessage(o: {
  fromName: string;
  from: string;
  to: string;
  subject: string;
  text: string;
  host: string;
  now?: number;
  id?: string;
}): string {
  const now = new Date(o.now ?? Date.now());
  const id = o.id ?? `${now.getTime().toString(16)}.${Math.random().toString(16).slice(2, 10)}`;
  const body = String(o.text ?? "")
    .replace(/\r\n|\r|\n/g, "\r\n")
    .split("\r\n")
    .map((line) => (line.startsWith(".") ? "." + line : line))
    .join("\r\n");
  const name = headerValue(o.fromName || SMTP_FROM_NAME_DEFAULT).replace(/(["\\])/g, "\\$1");
  return (
    `From: "${name}" <${headerValue(o.from)}>\r\n` +
    `To: <${headerValue(o.to)}>\r\n` +
    `Subject: ${subjectHeader(o.subject)}\r\n` +
    `Date: ${now.toUTCString()}\r\n` +
    `Message-ID: <${id}@${headerValue(o.host)}>\r\n` +
    `MIME-Version: 1.0\r\n` +
    `Content-Type: text/plain; charset=utf-8\r\n` +
    `Content-Transfer-Encoding: 8bit\r\n` +
    `\r\n` +
    body.replace(/\r\n$/, "") +
    "\r\n"
  );
}

type Reader = ReadableStreamDefaultReader<Uint8Array>;
type Writer = WritableStreamDefaultWriter<Uint8Array>;

const write = (w: Writer, line: string) => w.write(new TextEncoder().encode(line + "\r\n"));

/** Reads until a complete reply: the continuation lines ("250-...") end at one shaped "NNN ...". */
async function readReply(reader: Reader): Promise<{ code: number; text: string }> {
  const dec = new TextDecoder();
  let buf = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) throw new Error(`SMTP connection closed before a reply: ${buf.trim().replace(/\s+/g, " ").slice(0, 200)}`);
    buf += dec.decode(value, { stream: true });
    // The last element is the empty tail of a complete reply, or a partial line: drop it either way.
    const lines = buf.split("\r\n").slice(0, -1);
    const finals = lines.filter((l) => /^\d{3} /.test(l));
    const last = finals[finals.length - 1];
    if (last) return { code: Number(last.slice(0, 3)), text: buf.trim() };
  }
}

async function command(w: Writer, r: Reader, line: string, want: number[], what: string): Promise<void> {
  await write(w, line);
  const reply = await readReply(r);
  if (!want.includes(reply.code)) {
    throw new Error(`SMTP ${what} answered ${reply.code}: ${reply.text.replace(/\s+/g, " ").slice(0, 200)}`);
  }
}

/** Sends one plain-text mail through SMTP. Throws with the server's own words when it refuses. */
export async function sendViaSmtp(env: MailEnv, to: string, subject: string, text: string): Promise<void> {
  if (!smtpConfigured(env)) throw new Error("SMTP is not configured (SMTP_HOSTNAME, SMTP_USERNAME, SMTP_PASSWORD)");
  const { connect } = await import("cloudflare:sockets");
  const host = env.SMTP_HOSTNAME!.trim();
  const port = smtpPort(env);
  const implicit = smtpImplicitTls(env);
  const from = smtpFrom(env);

  // allowHalfOpen is false by default in workerd; the type declaration in use requires it spelled out.
  const socket = connect({ hostname: host, port }, { secureTransport: implicit ? "on" : "starttls", allowHalfOpen: false });
  const closeQuietly = () => {
    try {
      socket.close();
    } catch {
      /* already closed */
    }
  };
  let slow: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([
      (async () => {
        let w: Writer = socket.writable.getWriter();
        let r: Reader = socket.readable.getReader();
        let reply = await readReply(r);
        if (reply.code !== 220) throw new Error(`SMTP greeting answered ${reply.code}: ${reply.text.replace(/\s+/g, " ").slice(0, 200)}`);
        await command(w, r, `EHLO ${host}`, [250], "EHLO");
        if (!implicit) {
          await command(w, r, "STARTTLS", [220], "STARTTLS");
          w.releaseLock();
          r.releaseLock();
          const secure = socket.startTls();
          w = secure.writable.getWriter();
          r = secure.readable.getReader();
          await command(w, r, `EHLO ${host}`, [250], "EHLO after TLS");
        }
        // AUTH LOGIN is what Stalwart (and Gmail) offer; PLAIN is the fallback when LOGIN is refused.
        await write(w, "AUTH LOGIN");
        const auth = await readReply(r);
        if (auth.code === 334) {
          await command(w, r, base64(env.SMTP_USERNAME!.trim()), [334], "AUTH username");
          await command(w, r, base64(env.SMTP_PASSWORD!.replace(/\s/g, "")), [235], "AUTH password");
        } else if (auth.code === 235) {
          /* already authenticated */
        } else if (auth.code === 500 || auth.code === 504) {
          await command(w, r, `AUTH PLAIN ${base64(`\0${env.SMTP_USERNAME!.trim()}\0${env.SMTP_PASSWORD!.replace(/\s/g, "")}`)}`, [235], "AUTH PLAIN");
        } else {
          throw new Error(`SMTP AUTH answered ${auth.code}: ${auth.text.replace(/\s+/g, " ").slice(0, 200)}`);
        }
        await command(w, r, `MAIL FROM:<${from}>`, [250], "MAIL FROM");
        await command(w, r, `RCPT TO:<${headerValue(to).replace(/[<>]/g, "")}>`, [250, 251], "RCPT TO");
        await command(w, r, "DATA", [354], "DATA");
        const message = buildMessage({ fromName: env.SMTP_FROM_NAME ?? SMTP_FROM_NAME_DEFAULT, from, to, subject, text, host });
        await write(w, message + ".");
        reply = await readReply(r);
        if (reply.code !== 250) throw new Error(`SMTP message refused, ${reply.code}: ${reply.text.replace(/\s+/g, " ").slice(0, 200)}`);
        try {
          await write(w, "QUIT");
        } catch {
          /* the server may already have closed */
        }
      })(),
      new Promise<never>((_, reject) => {
        slow = setTimeout(() => {
          closeQuietly();
          reject(new Error(`SMTP ${host}:${port} did not answer within ${TIMEOUT_MS / 1000} s`));
        }, TIMEOUT_MS);
      }),
    ]);
  } finally {
    if (slow !== undefined) clearTimeout(slow);
    closeQuietly();
  }
}
