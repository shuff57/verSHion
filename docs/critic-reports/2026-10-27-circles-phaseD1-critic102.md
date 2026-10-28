# Circles phase D1 — critic 102 (product/UX), 2026-10-27

Run: 2026-10-27 evening, read-only on `a02c12d` (diff base `0d17a83`), per
`docs/critic-reports/2026-10-28-round101-102-brief.md`. Scope: everything under `src/share/` in that diff.

**VERDICT: important problem(s)** — one important state defect and two minors.

## I1 (important) — a save failure is drawn as "you can keep working here", with no Try again

`src/share/bank_links.rs:426-437` (the `under_review` early return), with `:321-334` (Send removed for any
`learning`, `SEND_WAITS` appended) and `:491-494` (the real `Learning::Held` warning). State produced at
`src/share/circle_edit.rs:2460-2462` → `hold_inherit` (`:2498-2509`) → `LinkStatus.held` (`:4974`).

A copy made by an earlier build must read the circle's difficulties into the bank. If the banks file cannot be
written then, the pass holds: `inherit_held`, `Learning::Held`, `held = banks_not_saved(...)`. If the circle's newest
version is at the same time held back for review, `strip_view_inner` returns at the `under_review` branch before it
ever reaches the `Learning::Held` match. `strip_view_known` then removes `Send` and appends
"Sending is turned off until the circle can be checked." The result is a calm, **buttonless** strip saying
"You can keep working here" while nothing she types is being written. The real `Held` line — "your own edits aren't
saved either … press Try again; if it keeps failing, free some disk space" in the warning colour with Try again —
is never reached. The Banks-list badge, meanwhile, takes the `held` path (`bank_links.rs:76`, `:95`) and shows
"couldn't save" in the warning colour (`:160-167`), so the two screens contradict each other. If she quits, those
edits are gone: the exact shape of critic 100's I2, which round 19 fixed for `banks_unsaved` (that warning does
survive, at `strip_view_known:339-344`) but not for `inherit_held`.

The same early-return masking applies to `SignIn::Out` (:383), `Check::Offline` (:402), `sent_unread` (:418) and a
restore/trim in flight (:414). Under review is the worst because it leaves no button at all; `stale`, viewer,
keys-kept and viewers'-form cannot combine. No test has an `under_review` heads answer with a failed save.

Suggested direction (not applied): apply the `Held`/`unsaved` warning after the state is chosen — as
`strip_view_known` already does for `unsaved` — or handle `learning` before the `under_review` return.

## m1 (minor) — "Dismiss hides this list" but Dismiss deletes it for good

`src/share/circles_text.rs:103`, `src/share/circle_edit.rs:2592-2594`.

`NEWS_DISMISS = "Dismiss hides this list. Your bank stays as it is."`: `dismiss_news` does
`difficulty_news.take()`, so the rows, the "Put mine back" offer and the only in-app record of her pre-circle
difficulties are gone. "Hides" suggests it can be shown again. Residual of critic 98 minor 6.

## m2 (minor) — the badge promises "Get latest" where that page cannot offer it

`src/share/bank_links.rs:98-100` with `:1032-1033` and the `under_review` strip at `:426-437`.

When the circle's newest version is held for review, the badge still says "newer version" and its hover says
"Get latest on the bank's page brings it in", but that page offers no Get latest and the Get latest screen refuses a
held head. She opens the page expecting a button that is not there.

## Coverage

- §4.2 rows: the code path for every row was read and the produced sentence/buttons checked against the plan — in
  step, unsent, behind, behind+unsent, changes not countable (both missing-base cases), viewer, viewer with local
  edits, viewers' form, checking, not signed in, offline, stale, newest held back, removed, circle closed/left,
  link file unreadable, OtherAccount, keys-kept viewer, promoted viewer, sent-unread. All are drawn; wording differs
  where the plan's was a proposal ("Your copy is up to date", "Use my bank as it is…", "Sign in…").
- §4.1 badge: code read; `probe-banks.png` shows the quiet one-line badge. The 420 ellipsis is code-only (no
  Banks-list-at-420 frame).
- §4.3/§4.4: code-read only — there is **no frame** of the circle page, the join offer, the Send page, the Get latest
  page or any signed-in strip. "Open in My Banks" does land on the right bank (`shared_ui.rs:111-115` →
  `App::open_bank`), and the Shared page has no question list, Send or merge.
- Frames read (23 of the 36 PNGs): `news20-{light,dark}-{1280,420}`, `putall20-light-1280`,
  `news-{light,dark}-{1280,760,420}`, `warn-{light,dark}-{1280,760,420}`, `warnb-light-1280`, `probe-banks`,
  `probe-bank`. Light 1280 round-20 geometry is correct (button column just past the widest row); light 420 is
  correct (28-char stem, three-line rows). **`news20-dark-420.png` is stale**: it shows the 60-char stem and
  five-line rows, unlike `news20-light-420.png`, so the dark 420 geometry is not verified by a frame. The
  banks.json warning has a frame only at 1280 light; at 420 only the links-file path is framed.
- Words: no user-facing "ratings"/"pool" in circle text; "safety copy" used throughout; no "REPLACES" /
  "copy that file over banks.json" / "redeemed". `WORKING_COPY_LINE`, `HAS_WORKING_COPY`, `WORKING_COPY_DONE`,
  `COPY_NO_KEYS` still contain "working copy"/"plain copy" but have no callers.
- Not checked: the Check-now success frame, signed-in strip states, the circle page and join offer at 420.

## Lead's note

I verified I1, m1 and m2 against the live tree before recording them: `held` is set only for `Learning::Held`
(`circle_edit.rs:4974`), `unsaved` is a separate field, the `under_review` return precedes the `Held` match, and
`dismiss_news` takes the list permanently. Per the brief, an important finding stops the round: no fix started, no
new round opened; the owner is told.
