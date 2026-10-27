# Circles phase D1, product/UX critic 72 (as a non-technical teacher), round 4

Scope: the uncommitted phase D1 tree on `shared-pool-phase1` (HEAD 142e6d6) after round 4, which fixed critic 69's I1
and I2 and critic 70's I1. Checked here:

- the "Restoring version N in the circle..." strip and the restore and trim result strips;
- the "up to date" claims;
- per-bank notices;
- the Banks-list sign-in and failed rows;
- "pool" in every string a teacher can see on a circle screen;
- whether "verSHion" and "verSHion's server" read naturally;
- whether one server error reads the same on every screen.

The known exceptions are left out: `EMAIL_NEEDS_ACCOUNT` and the "shared pool" invite-code errors are waiting for the
owner to sign off their wording. A fresh critic that built none of this wrote this report.

## Evidence

- **Fresh copy and rig.**
  - `scratchpad/c72src` is a copy of the current tree. `diff -rq` against the repo's `src/share` and `src/bankui.rs`
    shows two differences only:
    - `circle_edit.rs` has QA hooks added at its end: `qa_act`, `qa_rolled`, `qa_trimmed` and `qa_review`. They call
      the real `on_rolled`, `on_trimmed`, `act_for` and Act job, so every strip line and note in the frames is the
      code's own text, not a string I typed in. (Round 4's own frames typed their notices in through `QA_NOTICE`.)
    - `bank_links_fix4_tests.rs` has probe `c72_p1` added.
  - The rig is `scratchpad/c72rig` (binary `scratchpad/c72bin/c72rig`). It is round 4's rig plus `x-*` modes: restoring,
    trimming, restore done / held / 500 / offline, trim done / 500, notice plus wait note, Unlink while sending, the Get
    latest review with circle-removed rows, Get latest 500, History restore 500 and done, Banks rows for 500 / 403 /
    offline, and the circle page 500.
- **Frames.** On my own Xvfb `:72` (now stopped): 27 modes at 1280, 760 and 420 px, light and dark, 162 frames in total.
  All exit 0; one timeout was re-run. They are in
  `/tmp/claude-1000/-home-shuff57-Documents-GitHub-verSHion/51523046-d785-4ff5-8e5f-a55400288e50/scratchpad/c72shots/`.
  Sheets are in the scratchpad: `c72_strips1280.png`, `c72_strips1280b.png`, `c72_merge.png`, `c72_500a.png`,
  `c72_banksrows.png`, `c72_420a.png`, `c72_420b.png` and `c72_760.png`.
- **Suite.** `cargo test --lib share::` on the copy: 771 passed (the repo's 770 plus the probe).
- **Rules kept.** The only repo file changed is this report. Nothing was committed. Port 8799 was not used. The rig on
  18791 (workerd pid 274528) was left running.

## Blockers

None. Nothing loses unsent work, and no screen claims a bank is up to date while a restore or trim is changing the
circle.

## Important

### I1. Round 4's new `circle_words` names verSHion's server as the one who changed questions on the Get latest review

`circle_words` used to map "the pool" to "the circle". Round 4 changed it to "verSHion's server" / "verSHion". But
`circle_edit_ui.rs:680` also runs it over the merge row notes from `merge::build_plan` (`merge.rs:537-581`). There "the
pool" means the circle's other members, who changed the questions.

**On screen (`x-merge-notes-*`, every width and theme).** A row tagged "Removed from the circle" reads:

> verSHion's server dropped this question. It is kept unless you remove it. Kept questions are no longer linked to verSHion.

and

> verSHion's server dropped this question. You changed it, so it is kept unless you remove it.

**By code, the same thing happens to these notes:**

- "You and verSHion made the same change."
- "verSHion's server changed this question and you entered its answer key. verSHion's wording is taken and your key is
  kept." This is the follow-copy / missing-key case.
- "If you use verSHion's, your answer key is kept."
- "verSHion changed the choices, so verSHion's version can not take your key."

**What a teacher concludes.** The server deleted a question by itself. "No longer linked to verSHion" sounds as if the
question will not be saved, so the safe-looking choice is "Remove it from my bank".

**This is a regression.** In round 3 the same notes said "The circle dropped this question ... no longer linked to the
circle", which was correct.

**"pool" also still reaches this page.** `merge::apply_merge` errors go through `finish_merge`'s `back()` without any
mapping (`circle_edit.rs:2381`). A teacher can see:

- "The pool's version changed while you were reviewing. Nothing was applied; start the merge again."
- "The pool's version is missing a question that was reviewed."

**Why the tests missed it.** `w3` scans 13 circle files but not `merge.rs`. `w2` covers server refusals only.

**Fix.**
- Give the merge text its own mapping before it is drawn: "The pool's" becomes "The circle's", "the pool's" becomes
  "the circle's", "The pool" becomes "The circle", "the pool" becomes "the circle".
- Use it for the row notes and for the Review's `problem`. Keep `circle_words` for server errors.
- Pin it with a test: a rendered Review that has a removed-in-circle row asserts "The circle dropped" and no
  "verSHion's server" (the rig's `x-merge-notes` builds one).

### I2. A bank's notice outlives its truth, on its own strip and above its own screens

Probe `c72_p1` (scripted session):

1. A restores v1 and goes Back while the restore is on its way. The strip then says "Version 4 is the latest. You have
   version 3." and "Version 4 was restored from version 1: it is now the circle's newest." That is correct.
2. 30 frames later it is unchanged.
3. Someone sends v5 and the person presses Check now. The strip now reads:

   > Shared with Algebra team. Version 5 is the latest. You have version 3.
   >
   > Version 4 was restored from version 1: it is now the circle's newest.

**Why it stays.** `strip_notice` clears a bank's notice only when another linked bank's strip draws after it was seen.
`forget_account` also clears it. Nothing else does:

- a send, a Get latest or a check does not replace or clear it;
- there is no Dismiss.

With one linked bank, it stays until sign-out or restart. `page_notice` also draws it above that bank's Send, Get latest
and History pages (`circle_edit_ui.rs:32`).

**The same happens to other notices.**
- "The version was sent, but the server's answer could not be read. Press Get latest to catch up." stays after that Get
  latest.
- "Your bank is now used as it is ... the next send sends it as it is" stays after that send.

**Fix.**
- Drop a bank's notice once it has been seen and the bank's link moves on: `after_send`, a merge applied, a check that
  changes `head_seen` or `base_version`.
- Or drop it once it has been seen and that bank's screen is opened.
- Or give the note a small Dismiss.

**Test:** the p1 sequence: no "it is now the circle's newest" next to "Version 5 is the latest".

### I3 (cheap). The share-into-a-circle page promises something phase D1 made false

`circle_send::NEW_BANK_NOTE` (`circle_send.rs:120`, drawn at `publish_ui.rs:1101`, just above "Your confirmation")
says:

> This adds a new bank to the circle. Sending a changed version of the same bank comes in a later update of verSHion.

The very next screen says: "\"<bank>\" in My Banks is now linked to it: its page shows where it stands, and you send
your changes from there." A teacher deciding whether to share is told they can never update it.

**Fix:** "This adds a new bank to the circle and links this bank to it: you send later changes from its page in My
Banks."

This was judged from the code: the share window needs a signed-in Shared page.

## Brief checklist

| Item | Verdict |
|---|---|
| "Restoring version N in the circle..." strip | **Holds.** The real handler gives "Shared with Algebra team. Restoring version 6 in the circle..." with a spinner (trim: "Trimming the bank's old versions in the circle..."). There are no buttons. "History of the bank" in "..." is refused with the wait note. The strip is legible at 1280, 760 and 420 px in both themes. |
| Restore result strip | **Mostly holds.** Done, held, 500, offline, trim done and trim 500 all show on the strip. After a restore, the line moves to "Version 8 is the latest. You have version 7." with Get latest as the main button. Two problems: how long the note stays is I2, and its wording differs from the History page for the same result. The History page says "Done. Version 10 is now the circle's newest, with the content of version 6."; the strip says "Version 8 was restored from version 6", which can be read the wrong way round. Using the History page's sentence would fix both. |
| "Up to date" claims | **Holds** during a restore or trim, after one (the head moves), and after a failed one. The remaining false claim is I2's contradiction. While the account is still being looked up at start, the strip states the last check's version without "when last checked". That lasts only a moment, so it is not reported. |
| Per-bank notices | **Never shown on another bank** (`x-notes-both`: A's own note and the wait note sit together under the strip). How long a notice stays is I2. |
| Banks-list sign-in / failed rows | **Hold** at every width and in both themes. Sign in... has no Dismiss. Other-account rows are one line with no titles. 500, 403 and offline rows are plain. |
| No "pool" | **Not met on Get latest** (I1: merge-apply errors). Every other circle screen I drew or grepped is free of it. Not reported: the Shared page's account guide ("Could not reach the pool", "A person at the pool checks every account", "The pool is invite-only") belongs with the invite-code wording waiting for the owner's sign-off. |
| "verSHion" / "verSHion's server" reads naturally | **Yes, except I1.** "verSHion's server had a problem of its own", "as many times as verSHion allows today", "verSHion first checks the viewers' copy" and "verSHion's reviewers" all read well. |
| One server error, one wording | **Holds.** A 500 reads "verSHion's server had a problem of its own. Nothing was changed; try again in a minute." in all seven places: the Banks row, Get latest, Send, a restore on History, the strip's restore and trim notes, Bank settings and the circle page (frames `x-banks-errors`, `x-merge-failed500`, `d-send-failed500`, `x-history-failed500`, `x-restore-500`, `x-trim-500`, `d-settings-hidefail`, `x-circle-act500`). |

## What works

- The restore and trim that critic 69 I1 lost are now kept and reported. The strip never says "up to date" while one
  runs or after one lands.
- Notices are kept per bank, and the wait note is no longer hidden by another bank's notice.
- Server faults no longer blame "the circle", and Bank settings' answer-key text is free of "pool".
- The Banks-list rows are honest and every button works.
