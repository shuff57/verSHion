// Run: npm test  (no dependencies)
import assert from "node:assert/strict";
import { test } from "node:test";
import { onDeleteContent } from "../src/cleanup.ts";
import { assertMailModeSafe, makeMailer } from "../src/mail.ts";
import { buildMessage, headerValue, smtpConfigured, smtpFrom, smtpImplicitTls, smtpPort } from "../src/mail_smtp.ts";

test("MAIL_MODE=log refuses a non-local APP_URL", () => {
  for (const APP_URL of ["https://app.example.org", "https://localhost.evil.org", "http://127.0.0.1.nip.io", "", undefined]) {
    assert.throws(() => makeMailer({ MAIL_MODE: "log", APP_URL }), /only allowed/, String(APP_URL));
  }
});

test("MAIL_MODE=log runs on localhost", async () => {
  for (const APP_URL of ["http://localhost:5173", "http://127.0.0.1:8080", "http://[::1]:3000"]) assertMailModeSafe({ MAIL_MODE: "log", APP_URL });
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    await makeMailer({ MAIL_MODE: "log", APP_URL: "http://localhost:5173" }).send({ kind: "verify", to: "a@b.org", token: "t" });
  } finally {
    console.log = orig;
  }
  assert.match(logs[0]!, /localhost:5173\/account\/verify#token=t/);
});

test("other modes do not need a local APP_URL", () => {
  assert.doesNotThrow(() => makeMailer({ APP_URL: "https://app.example.org" }));
});

test("onDelete returns the pool cleanup statements (no longer a logging stub)", () => {
  const prepared: string[] = [];
  const db = { prepare: (sql: string) => ({ bind: () => (prepared.push(sql), {}) }) } as unknown as D1Database;
  const errs: string[] = [];
  const orig = console.error;
  console.error = (m: string) => void errs.push(m);
  try {
    assert.ok(onDeleteContent(db, "u1", 0).length >= 5);
  } finally {
    console.error = orig;
  }
  assert.deepEqual(errs, []);
  assert.ok(prepared.some((s) => /removed_reason = 'account_deleted'/.test(s)));
  assert.ok(prepared.some((s) => /UPDATE reports SET reporter_id = NULL/.test(s)));
});

test("every mail kind renders, including device enrolment", async () => {
  const logs: string[] = [];
  const orig = console.log;
  console.log = (m: string) => void logs.push(m);
  try {
    const mailer = makeMailer({ MAIL_MODE: "log", APP_URL: "http://localhost:5173" });
    for (const kind of ["verify", "reset", "signup_existing", "password_changed", "device_enrol"] as const)
      await mailer.send({ kind, to: "a@b.org", token: "tok", accountVerified: true });
  } finally {
    console.log = orig;
  }
  assert.equal(logs.length, 5);
  assert.match(logs[4]!, /account\/enrol#token=tok/);
});

// ---- SMTP transport (mail_smtp.ts): the parts a Node test can pin are the ones the server sees ----

const SMTP = { SMTP_HOSTNAME: "smtp.example.org", SMTP_USERNAME: "pool@example.org", SMTP_PASSWORD: "p w" };

test("SMTP is used only when its three settings are all present", () => {
  assert.equal(smtpConfigured(SMTP), true);
  for (const missing of ["SMTP_HOSTNAME", "SMTP_USERNAME", "SMTP_PASSWORD"] as const) {
    const e: Record<string, string> = { ...SMTP };
    delete e[missing];
    assert.equal(smtpConfigured(e as never), false, missing);
  }
  assert.equal(smtpConfigured({}), false);
});

test("the sender is MAIL_FROM when set, else the mailbox that signs in", () => {
  assert.equal(smtpFrom(SMTP), "pool@example.org");
  assert.equal(smtpFrom({ ...SMTP, MAIL_FROM: " verSHion <notes@example.org> " }), "verSHion <notes@example.org>");
});

test("port defaults to 465 and 465 means implicit TLS; SMTP_SECURE overrides both", () => {
  assert.equal(smtpPort({}), 465);
  assert.equal(smtpPort({ SMTP_PORT: "587" }), 587);
  assert.equal(smtpPort({ SMTP_PORT: "not a port" }), 465);
  assert.equal(smtpPort({ SMTP_PORT: "99999" }), 465);
  assert.equal(smtpImplicitTls({}), true);
  assert.equal(smtpImplicitTls({ SMTP_PORT: "587" }), false);
  assert.equal(smtpImplicitTls({ SMTP_PORT: "587", SMTP_SECURE: "on" }), true);
  assert.equal(smtpImplicitTls({ SMTP_SECURE: "starttls" }), false);
});

test("header values cannot end their line early (injection), and a non-ASCII subject is encoded", () => {
  assert.equal(headerValue("a\r\nBcc: evil@example.org"), "a Bcc: evil@example.org");
  assert.equal(headerValue("a\nb\rc"), "a b c");
  const m = buildMessage({
    fromName: "verSHion",
    from: "pool@example.org",
    to: "teacher@example.org\r\nBcc: evil@example.org",
    subject: "Confirm your address\r\nBcc: evil@example.org",
    text: "hello",
    host: "smtp.example.org",
    now: 1_700_000_000_000,
    id: "abc",
  });
  const head = m.split("\r\n\r\n")[0];
  assert.equal(head.split("\r\n").filter((l) => l.startsWith("Bcc:"))?.length ?? 0, 0, "no injected Bcc line");
  assert.match(head, /^From: "verSHion" <pool@example\.org>$/m);
  assert.match(head, /^To: <teacher@example\.org Bcc: evil@example\.org>$/m);
  assert.match(head, /^Subject: Confirm your address Bcc: evil@example\.org$/m);
  // A non-ASCII subject travels as an RFC 2047 word, not raw bytes in the header.
  const encoded = buildMessage({ fromName: "verSHion", from: "a@b.org", to: "c@d.org", subject: "Confirmez votre adresse — verSHion", text: "x", host: "h" });
  assert.match(encoded, /^Subject: =\?UTF-8\?B\?[A-Za-z0-9+/=]+\?=$/m);
});

test("the DATA payload ends with CRLF, normalises line endings and dot-stuffs a leading dot", () => {
  const m = buildMessage({ fromName: "verSHion", from: "a@b.org", to: "c@d.org", subject: "s", text: "one\ntwo\r\n.\n.leading and . hidden", host: "h" });
  assert.ok(m.endsWith("\r\n"), "ends with CRLF so the terminating dot sits on its own line");
  assert.ok(!/[^\r]\n/.test(m), "every line break is CRLF");
  const body = m.split("\r\n\r\n")[1];
  assert.equal(body, "one\r\ntwo\r\n..\r\n..leading and . hidden\r\n");
  assert.ok(m.split("\r\n").includes(".") === false, "no bare dot line in the payload");
});
