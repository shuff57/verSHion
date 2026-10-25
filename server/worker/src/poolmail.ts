// Mail for pool notices. Ids only: no item text goes into a mail.
import { sendPlain, sendTo, type MailEnv } from "./mail.ts";
import { isDeliverable } from "./pool/circlemail.ts";
import type { PoolNotice } from "./pool/shared.ts";

const EMAIL = /^[^\s@"(),:;<>[\]\\]{1,64}@[a-z0-9.-]{1,253}\.[a-z]{2,63}$/i;

/** Round 42: the circle invitation's fixed subject (no person's text ever goes into a header). */
export const CIRCLE_INVITE_SUBJECT = "verSHion: you are invited to a circle";
const CIRCLE_CODE = /^C(-[0-9A-Z]{4}){6}$/;

/**
 * Round 42 (D5): the circle invitation, a FIXED text. Round 46 (critics 53a variant 7, 53b V1): it carries no text any
 * person chose, not even the circle's name (the app shows the circle and its owner before anyone joins), and it says
 * what joining needs (an approved account that redeemed the pool's invite code) and when the code expires, to the
 * minute. The code, and a link holding it in the fragment (never sent to a server). Owner-approved wording: README
 * "Round 46".
 */
export function circleInviteText(appUrl: string | undefined, n: Extract<PoolNotice, { kind: "circle_email_invite" }>): string {
  const as = n.role === "editor" ? "an editor" : "a viewer";
  const when = new Date(n.expiresAt).toISOString().slice(0, 16).replace("T", " ");
  const link = appUrl ? `\nOr open this link:\n${appUrl.replace(/\/+$/, "")}/circles/join#code=${n.code}\n` : "";
  return (
    `You are invited to join a circle on verSHion as ${as}. A circle is a small private group of teachers who share question banks. ` +
    `The app shows you the circle's name and its owner before you join.\n\n` +
    `To join, you need a verSHion account that an administrator has approved and that has redeemed an invite code for the shared pool. ` +
    `If you do not have one, this code cannot be used yet.\n\n` +
    `In verSHion, open Shared, then Circles, then "Join a circle with a code", and enter this code:\n\n` +
    `${n.code}\n${link}\n` +
    `The code works once and expires on ${when} UTC. If you did not expect this invitation, ignore this mail: nothing happens unless the code is used.`
  );
}

export function poolNotifier(env: MailEnv & { DB: D1Database }) {
  return async (n: PoolNotice): Promise<void> => {
    switch (n.kind) {
      case "student_data_report":
        // Immediately and outside every budget: the item is already hidden.
        return sendTo(env, "admins", "verSHion: student data reported (item hidden)", `Item ${n.itemId} was reported as containing student data and is hidden until you review it (report ${n.reportId}). Confirm and purge, or release the hold, from the pool reports.`);
      case "takedown_author": {
        const u = await env.DB.prepare(`SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(n.authorId).first<{ email: string | null }>();
        if (!u?.email) return;
        return sendPlain(
          env,
          u.email,
          "verSHion: an item was taken down after a copyright notice",
          `One of your shared items (${n.itemId}) was taken down after a copyright notice (action ${n.actionId}). If you believe it was removed by mistake or misidentification, you can send a counter-notice within 30 days from "My shared items" in the app (an item you deleted is listed there as "taken down, awaiting your counter-notice"). Each takedown that is not reversed counts as a strike under the repeat-infringer policy in the Terms.`,
        );
      }
      case "counter_notice": {
        const when = new Date(n.restoreNotBefore).toISOString().slice(0, 10);
        const text = `A counter-notice was received for DMCA action ${n.actionId}. The material will be restored no sooner than ${when} (10 business days) unless you notify us that you have filed a court action.`;
        if (EMAIL.test(n.complainantContact.trim())) return sendPlain(env, n.complainantContact.trim(), "Counter-notice received", text);
        return sendTo(env, "admins", "verSHion: forward this counter-notice by hand", `${text}\n\nThe complainant's contact is not an email address; forward it to them yourself.`);
      }
      case "cap":
        return sendTo(env, "admins", `verSHion: an account reached its daily ${n.metric} limit`, `Account ${n.userId} reached its daily ${n.metric} limit. See the pool flags.`);
      case "privacy_text_match":
        // Round 37: a circle send or rollback (ids only; circle content is opened through the audited admin route).
        if (n.circleId)
          return sendTo(
            env,
            "admins",
            "verSHion: a circle bank matches confirmed student-data text",
            // (round 40, critic 47a V2: the version is withheld until released BY VERSION; a bank-wide release does not)
            // (round 41, critic 48: a notice queued before round 40 has no version: the flag's detail names it)
            n.version !== undefined
              ? `Version ${n.version} of bank ${n.itemId} in circle ${n.circleId} has question text that matches (or rewords) a confirmed student-data block. That version is withheld from every member; while it is the newest version the bank is hidden from the circle but for its author and owner, who can roll it back or replace it. Until you decide, the bank cannot be purged. See the pool flags (privacy_text_match); open it with GET /admin/pool/circle-bank?id=${n.itemId}&version=${n.version}&basis=privacy (audited). If it is a false positive, release THAT version: POST /admin/pool/circle-release {"bank_id": "${n.itemId}", "version": ${n.version}, "note": "..."}. A bank-wide release (without "version") lifts the bank's holds but leaves the version withheld.`
              : `A version of bank ${n.itemId} in circle ${n.circleId} has question text that matches (or rewords) a confirmed student-data block; that version is withheld from every member and the bank cannot be purged until you decide. The privacy_text_match flag in the pool flags names the version: open it with GET /admin/pool/circle-bank (id, that version, basis=privacy; audited), and if it is a false positive release that version with POST /admin/pool/circle-release {bank_id, version, note}. A bank-wide release (without a version) leaves it withheld.`,
          );
        // Round 12: the text says what happened. An exact or normalised match is held at publish (its
        // publisher sees only "held for review"); a near-rewording is only flagged and stays live.
        return sendTo(
          env,
          "admins",
          "verSHion: a new publish matches confirmed student-data text",
          n.held
            ? `Item ${n.itemId} was just published with question text that matches a confirmed student-data block. It is held for review; its author sees only "held for review". See the pool flags (privacy_text_match), and release it with /admin/pool/release-hold if it is a false positive.`
            : `Item ${n.itemId} was just published with question text close to (a rewording of) a confirmed student-data block. It is live: review it now in the pool flags (privacy_text_match, "near"), and hold or purge it if it contains the student's data.`,
        );
      case "cron_failure":
        return sendTo(env, "admins", `verSHion: pool maintenance step failed (${n.stage})`, `The scheduled pool step "${n.stage}" failed; the other steps still ran. It is retried on the next run. See the worker logs and the pool flags (cron_failure).`);
      case "wasm_trap":
        return sendTo(env, "admins", "verSHion: share-core trapped repeatedly", `share-core trapped ${n.traps} times in an hour; each trap failed one request and its instance was re-created. See the worker logs and the pool flags (wasm_trap).`);
      case "key_mismatch":
        return sendTo(env, "admins", "verSHion: the pool refuses to run under a different hash key", `POOL_HASH_KEY in this deployment (key id ${n.keyId}) is not the key the pool's data was made with (key id ${n.storedKeyId}). Every pool write and cron run is refused (503 pool_key_mismatch) until the original key is restored. The key cannot be rotated: restore it from your backup.`);
      case "d1_size":
        return sendTo(env, "admins", `verSHion: pool database at ${n.percent}% of its size limit`, `The pool database is ${n.bytes} bytes, ${n.percent}% of D1's size limit. Nothing has failed yet; plan room now (purge, archive, or move to a larger database). See the pool flags (d1_size).`);
      case "copy_held": {
        const u = await env.DB.prepare(`SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(n.authorId).first<{ email: string | null }>();
        if (!u?.email) return;
        const why = n.cause === "student_data" ? "an admin confirmed that a question it shares with another item contains student data" : "a copyright takedown covers questions it shares with the item taken down";
        return sendPlain(env, u.email, "verSHion: one of your shared items is on hold", `Your shared item ${n.itemId} is on hold: ${why}. See "My shared items" in the app to remove those questions and republish, or to appeal.`);
      }
      case "key_leak_report": {
        // Round 17: an importer says the served text gives the hidden key away (who is never said).
        const u = await env.DB.prepare(`SELECT email FROM users WHERE id = ?1 AND deleted_at IS NULL`).bind(n.authorId).first<{ email: string | null }>();
        if (!u?.email) return;
        return sendPlain(env, u.email, "verSHion: a teacher reported a visible answer hint", `A teacher who uses your shared item ${n.itemId} reported that what students see gives the hidden answer key away. Please check the question text and choices, fix them and republish. If two teachers report it, the item is held for review until you do.`);
      }
      case "circle_report":
        // Round 32: ids only (circle content is opened by an admin only through the audited admin route).
        return sendTo(
          env,
          "admins",
          n.reason === "student_data" ? `verSHion: student data reported in a circle${n.held ? " (bank hidden)" : ""}` : "verSHion: a circle bank was reported",
          `Bank ${n.bankId} in circle ${n.circleId} was reported (${n.reason}, report ${n.reportId}).${n.held ? " It is hidden from the circle until you review it." : ""} See GET /admin/pool/circle-reports; opening its content is audited.`,
        );
      case "circle_copyright_match":
        // Round 44: ids only (circle content is opened through the audited admin route, basis=copyright).
        return sendTo(
          env,
          "admins",
          "verSHion: a circle bank matches a copyright re-upload block",
          `Version ${n.version} of bank ${n.itemId} in circle ${n.circleId} holds material a takedown's re-upload block covers. That version is withheld from every member; while it is the newest version the bank is hidden from the circle but for its author and owner, who can roll it back or replace it. Until you decide, the bank cannot be purged. See the pool flags (blocklist); open it with GET /admin/pool/circle-bank?id=${n.itemId}&version=${n.version}&basis=copyright (audited). If the sender may share it, release THAT version: POST /admin/pool/circle-release {"bank_id": "${n.itemId}", "version": ${n.version}, "note": "..."}. A bank-wide release (without "version") lifts the bank's holds but leaves the version withheld.`,
        );
      case "circle_email_invite": {
        // (round 42: one recipient, as the pool checked it, and a code of the circle-code shape; anything else is
        // never mailed)
        const to = n.to;
        if (!isDeliverable(to) || !CIRCLE_CODE.test(n.code)) return;
        return sendPlain(env, to, CIRCLE_INVITE_SUBJECT, circleInviteText(env.APP_URL, n));
      }
      case "repeat_infringer":
        return sendTo(env, "everyone", "verSHion: repeat-infringer threshold reached", `Account ${n.userId} has ${n.strikes} upheld copyright strikes in 12 months. Under the repeat-infringer policy, suspend it with /admin/suspend.`);
    }
  };
}
