# Circles phase D: one workspace (planning document)

Written 2026-10-25. **Plan only: no code, no migration, nothing decided beyond section 2.** Builds on
`docs/circles-plan.md` (sections 4, 10, 13; decisions D1-D19), `server/worker/README.md` (rounds 37-53 and the route
table under "Round 32"), and the client in `src/share/` (`circle_links.rs`, `circle_edit*.rs`, `circles*.rs`,
`circle_versions_api.rs`, `merge.rs`) and `src/bankui.rs`. UNCONFIRMED marks anything not checked in code.
Naming: "phase D1/D2/D3" are this plan's phases; "decision D16" etc. are the circles plan's decisions; open questions
for the owner are Q1-Q10 (section 14).

---

## 1. Summary

Circle banks move into My Banks. A bank linked to a circle shows a badge in the list and a status strip on its page;
sending, getting the latest, history and compare all happen there. The Shared page keeps the circle itself (people,
invites, activity, the list of banks) and opens banks in My Banks; it no longer edits anything. Two verbs replace
"working copy" and "plain copy": **Add to my banks** (linked) and **Make my own copy** (unlinked). Each question gets
its own history, compare and "Restore this question to version N" (a local edit, sent like any other). The send
summary gets tick boxes, so a teacher can send some changes and keep others on this computer.

Backend: one new read route in phase D2 (per-question change lists from the indexes the server already keeps). No
migration, no share-core change. Phases D1 and D3 are client only.

## 2. Owner decisions taken (multiple choice, 2026-10-25)

1. **My Banks is the only workspace.** Shared banks appear in My Banks with a circle badge and a status strip
   ("Shared with <circle>. Version 7 is the latest. You have 3 unsent changes" + Send my changes / Get latest). The
   circle page becomes people, invites, activity and a list of banks, each with "Open in My Banks". No editing there.
2. **Two verbs:** "Add to my banks" (linked, syncs; joining a circle offers it) and "Make my own copy" (new unlinked
   bank with a lineage note).
3. **Per-question history:** a list of changes per question (who, when, message) from the kept versions (last 20 in
   full, decision D16); "Restore this question to version N" as a LOCAL edit that is then sent; side-by-side compare
   of any two versions of a question. Withheld versions show no text; pruned versions show only history lines.
4. **Partial send:** tick boxes on the send summary; unticked changes stay local and stay marked unsent; the
   linked-copy stale/fingerprint logic must stay right.

## 3. Where things are today

| Place | File | What it does now |
|---|---|---|
| Banks list | `src/bankui.rs` `banks_body`, `bank_card`, `bank_list_row` | Local banks only. No circle awareness. |
| Bank page | `src/bankui.rs` `bank_edit_body`, `bank_toolbar` | Editor; "Share" menu: Share..., From pool..., Update from pool.... No circle line. |
| Question card | `src/bankui.rs` `draw_card` | Right-click menu: Duplicate, Delete. |
| Shared page | `src/share/shared_ui.rs` | Tabs Browse, Circles, Mine, Imported, Reports, Settings (guided steps first). |
| Circles list / circle page | `src/share/circles_ui.rs` | Circle page cards: Banks ("Add a bank..."), My working copies, Members, Codes and invitations, What happened lately, This circle. |
| Circle bank page | `circles_ui.rs` `bank_page` | Summary, "Get a copy" (working copy for editors, plain copy otherwise), answer keys for viewers (`hide_section`), Versions (`circle_edit_ui::bank_section`: Send my changes, Get latest and merge, Forget, History of versions), More (licence, remove, report), first 100 questions. |
| Send / merge / history / version / restore / trim | `circle_edit.rs` (model), `circle_edit_ui.rs` (draws in place of the circle page) | Whole-bank send with compare-and-set; three-way merge; bank history, look at a version, whole-bank restore (decision D17), trim. |
| Links | `circle_links.rs`, file `share-circles.json` | One `CircleLink` per working copy: circle and bank ids, base version, base snapshot (500 questions / 1.5 MB cap), local id -> qid, head seen, `bank_modified` + `bank_print` (stale check). |
| Plain copies | `browse.rs` `record_circle_copy`, `share-imported.json` | Recorded like public imports, with update notices under "Things I imported". |
| Share to circle | `circle_send.rs`, publish window | Sends version 1. The source bank is NOT linked. |
| Sync of linked banks | `circles.rs` `sync` | `edit.sync(banks)` runs only while the Circles model is open on the Shared page. |

## 4. Target UX (proposals for the owner to confirm; wording is the proposal)

### 4.1 My Banks list: the badge

One quiet line under the bank name on cards and list rows, only for linked banks, read from the local link file (no
network): pill "Circle", the circle name, then at most one state, by priority "newer version", "3 unsent changes",
"not linked any more" (left, removed, closed). Example: `[Circle] Algebra team · 3 unsent changes`.

### 4.2 Bank page: the status strip

A strip under the bank's name and subtitle, above the toolbar, only for linked banks. One sentence line, then
buttons. The strip's "..." menu: History of the bank, Circle page, Unlink....

```
[Circle] Shared with Algebra team. Version 7 is the latest. You have 3 unsent changes.
[Send my changes...]  [History]  [...]
```

| State | Line | Buttons (primary first) |
|---|---|---|
| In step, nothing to send | Shared with Algebra team. Version 7 is the latest. You have it. | History |
| Unsent changes | Shared with Algebra team. Version 7 is the latest. You have 3 unsent changes. | Send my changes..., History |
| Behind | Shared with Algebra team. Version 9 is the latest. You have version 7. | Get latest, History |
| Behind + unsent | ... Version 9 is the latest. You have version 7 and 3 unsent changes. Get the latest first, then send. | Get latest, Send my changes..., History |
| Some held back (phase D3) | ... You have 3 unsent changes (1 kept back). | as above |
| Changes not countable (no base on this computer) | ... verSHion can't count your changes here; Send compares with the circle. | Send my changes..., Get latest |
| Viewer (follow copy) | Shared with Algebra team (you can view). Version 9 is the latest. You have version 7. | Get latest, History |
| Viewer with local edits | ... You changed 2 questions here. They stay on this computer; viewers can't send. | Get latest |
| Viewers' form copy | Answer keys are hidden from viewers in this bank, so your copy has none. | Get latest |
| Checking | Shared with Algebra team. Checking for a newer version... | (spinner) |
| Not signed in | Shared with Algebra team. Version 7 when last checked, 2 days ago. Sign in to send or get newer versions. | Open account... |
| Offline / pool error | Couldn't reach the pool. Your bank works as usual. | Try again |
| Stale (bank put back from a saved copy) | This bank was put back from a saved copy, so verSHion can't tell what you changed since version 7. | Get latest, Treat as in step..., Unlink... |
| Newest version held back | The circle's newest version is held back for review. You can keep working here. | Send my changes... (as today's "send on top") |
| Bank removed from circle | The bank was removed from the circle (it can be brought back until 3 Nov). Your copy stays as it is. | Unlink... |
| Circle closed / I left / removed | You are no longer in Algebra team (or: the circle was closed). Your copy stays; it can't be sent. | Unlink... |
| Link file unreadable | The list of shared banks couldn't be read. Nothing was changed. | Try again |

Send, Get latest (merge), whole-bank History, Look at a version, Restore (decision D17) and Trim open as pages in
place of the question grid, with "< Back to Unit 2 quiz" at the top. They are today's `circle_edit_ui` pages,
unchanged in logic, with "Get latest and merge" renamed "Get latest". "Unlink..." is today's "Forget this link".

### 4.3 The circle page on the Shared page

Cards, in order: header (name, my role, members, banks); **Banks**; **Members**; **Codes and invitations** (owner);
**What happened lately**; **This circle** (rename, leave, close). "My working copies" goes (the badges replace it).
Each bank row:

```
Unit 2 quiz                                    [Open in My Banks]
by A. Teacher · version 9 · 24 questions       In My Banks as "Unit 2 quiz"
                                               More: Bank settings · Report... · Remove...
```

"Open in My Banks" switches to the Banks screen on the linked bank. When the bank is not in My Banks yet, the row
shows **Add to my banks** and **Make my own copy** instead (4.4). "Bank settings" (owner and author only) is a
read-only page holding what is not editing: answer keys for viewers (today's `hide_section`, unchanged), licence
(author), History of versions, remove / bring back / delete for good. No question list, no send, no merge (Q1).

### 4.4 Add vs copy, and joining

```
Add to my banks        Puts this bank in My Banks, linked to the circle. Get newer versions when you
                       choose, and (as an editor) send your changes back. Nothing changes by itself.
Make my own copy       A separate bank that is yours alone. Not linked: the circle's changes don't reach it
                       and yours don't go back. Its details say where it came from.
```

* Editors and owner: Add makes an **edit** link (today's working copy). Viewers: Add makes a **follow** link (Get
  latest only, no Send; Q2). A viewer of a bank that hides its keys gets a follow link to the viewers' form; that link
  can never send, even after a promotion (the strip then offers "Add a full copy").
* Already added: the row says "In My Banks as ...", Add is not offered (one link per circle bank and per local bank,
  as today), Make my own copy still is.
* Own copy: lineage note in the bank's details, "From the circle "Algebra team": "Unit 2 quiz" by A. Teacher,
  version 9, 25 Oct 2026." Not linked, no update notices (Q7).
* Joining: after "Join", the page shows "You joined Algebra team as an editor. Add its banks to My Banks?" with a tick
  per bank (all ticked when there are 10 or fewer, none ticked above that), [Add the ticked banks] [Not now].
* Sharing a bank into a circle links THAT bank at version 1, so the author does not get a second copy (Q6).

### 4.5 Per-question history panel

From the question card's menu, "History in the circle" (linked banks, questions the circle knows). Opens in place of
the question grid:

```
< Unit 2 quiz
Question 5 · Multiple choice · "Solve 2x + 3 = 7"
Your copy: changed, not sent yet.                         [Compare with...]
Version 9 · B. Teacher · 2 days ago · "Fixed a distractor"     Changed   [Compare] [Restore...]
Version 7 · held back for review · no details
Version 6 · You · last week · "Unit 2 rewrite"                 Changed   [Compare] [Restore...]
Version 3 · A. Teacher · 3 weeks ago                           Added     [Compare] [Restore...]
Versions 1-2 are no longer kept in full: who and when only.              [Show every version]
```

Only versions in which this question changed are listed, plus one line per gap ("Changes between version 6 and
version 8: version 7 is held back"). "Show every version" lists all lines (pruned ones as who/when/message only).
Restore asks: "Replace your copy of question 5 with version 6's? Only your copy changes; the circle gets it when you
send. A restore point is saved first." Then the strip shows it as an unsent change. A question the circle has since
removed comes back as a new question when sent (UNCONFIRMED whether the server would keep its old qid; the plan
assumes not and says so). Restore is offered on edit links only; follow links can compare.

### 4.6 Compare view

Two pickers (any kept, readable version, and "Your copy"); defaults: the chosen version and the one before it.
Fields: stem, choices / answers and key, points, topics, explanation, picture. Changed fields carry a left accent bar
and "changed"; unchanged fields are folded ("Points, topics: the same"). Two columns from 760 px; below that, stacked
("Version 6" block above "Version 9" block). Field comparison reuses `merge.rs`'s `FieldDiff`.

### 4.7 Send summary with tick boxes (phase D3)

```
Send my changes to "Unit 2 quiz" (Algebra team). Based on version 7.
Tick the changes to send. Unticked ones stay on this computer, still shown as unsent.
[x] Changed  Q3   "Which of these is prime?"           Compare
[x] Added    Q12  "Solve 2x + 3 = 7"
[ ] Removed  Q8   "Simplify 3(x + 2)"
2 of 3 ticked.   Tick all · Untick all
Note for the circle (optional) [__________]
Licence line · [ ] own-work statement (unchanged)
[Send 2 changes]  [Cancel]
```

Result: "Sent version 8 with 2 of your 3 changes. 1 change stays on this computer, not sent." When the base can not be
had (section 6): "Choosing which changes to send needs the version your copy is based on, and the circle no longer keeps it.
Send everything, or get the latest first." (ticks greyed, all ticked).

### 4.8 At 420 px

Strip: line wraps, buttons wrap to their own row, "..." holds History. Badge: one line, circle name ellipsised before
the state. Circle page rows: title on its own line, buttons below. History panel: each version on two lines (who/when,
then message + tag + buttons). Compare: stacked. Send rows: two lines (tag + number, then the stem cut at one line).
Every new state gets a headless frame at 420 px and 1280 px in the seeded rig (as phases A-C), and the owner's look on
a real screen before a phase is called done.

## 5. Data and model

### 5.1 Linked bank identity

The link stays where it is: `share-circles.json`, one `CircleLink` per local bank, never in `banks.json` (a bank file
shared or exported carries nothing of it). No change to `model.rs`. New fields, all with serde defaults so today's file
loads unchanged:

| Field | Default | Meaning |
|---|---|---|
| `mode` | `edit` | `edit` (owner/editor: send + get latest) or `follow` (viewer: get latest only) |
| `viewers_form` | false | made from the viewers' form of a bank that hides its keys: never sendable |
| `held_back` | [] | local question ids the person unticked at the last send (phase D3) |
| `role_seen`, `role_seen_at` | none | role from the last check, to draw the strip without a call |

### 5.2 How the badge and strip find their link

`CircleEditModel::links()` loads the file once (local read only; a file that can't be read is never written over, as
today). The list builds a map local id -> link per frame (at most a few hundred). Unsent counts use `changes_since`
with the existing `LinkStatus` cache (keyed by `modified` and question count), computed only for banks on screen.
Network happens only when a linked bank's page is open and the person is signed in: one `POST /circles/heads` (one
`item_reads`) and one `GET /circles` (roles, closed state), at most once per 10 minutes per bank, plus "Check now" in
the strip menu. The strip drives `edit.frame()` and `edit.sync()` itself, so it works without the Shared page open.

### 5.3 Where the per-question timeline comes from: decided, one new backend read route

Every version's R2 index already holds, per question, its qid, a keyed content hash `h` and the key-free match hash
`m` (`circlesend.ts` `Index`). Pruning deletes body and index together.

| | A. Client reads every kept body | B. New route over the kept indexes (chosen) |
|---|---|---|
| Calls to open a question's history | up to 20 version reads (20 `item_reads`) | 1 (`item_reads` 1; 304 when nothing moved) |
| Bytes, 990-question 450 KB bank | about 9 MB down | answer usually under 10 KB (worst about 700 KB) |
| Bytes, 3.8 MB bank | about 76 MB down | the same as above |
| Server work | 20 streamed reads | about 20 R2 gets of about 170 KB (UNCONFIRMED size), two held at once; no heavy slot, no share-core |
| Viewer of a bank hiding keys | 20 stripped reads: heavy slot each, 20 of 100 `circle_hidden_reads`, and Matching shuffles look like changes | not served (5.4) |
| Privacy | as today's reads | needs its own rules (5.4) and a critic round |

Text is still read only when needed: compare and restore read the two (or one) version bodies with the existing
`GET .../versions/:n`, held in a small in-memory cache (4 bodies, dropped on sign-out or account switch; nothing
with keys is written to disk beyond the linked bank itself).

### 5.4 Privacy rules for the timeline

* Same reader rule as the history route: a member who may read the bank; non-members and former members 404; a held
  bank only to its author (and owner, round 40).
* Lists are computed only between versions that are kept AND readable. A withheld version (any `review_copy`) gets a
  line and no list, and is never one side of a comparison: the next readable version is compared with the readable one
  before it (`compared_to`), and the app shows the gap. No text of a withheld version is ever served, as today.
* Pruned versions: lines only (the existing history route), as decided.
* **Viewers of a bank that hides its keys get no per-question lists**: `h` hashes the full form and `m` includes
  labels a strip may remove, so a list would tell a viewer that, say, question 5's key changed. Their history panel
  shows bank lines (messages blank, as round 43 rules) and, per Q4, compare of two versions' viewers' forms read on
  demand (two stripped reads, each counted in `circle_hidden_reads`; Matching word banks compared sorted, so a fresh
  shuffle is not a change). That is the stripped form's history and nothing more.
* Messages, names and "deleted teacher" come from the existing history route; the new route carries ids and qids only.

### 5.5 Read budgets (per person per day; existing caps unless marked new)

| Action | Cost |
|---|---|
| Open a linked bank | heads + circles list, at most every 10 minutes per bank |
| Open a question's history | 1 change-list read per bank and head (cached for the session; 304 free) |
| Compare | 2 version reads (viewers of a hiding bank: 2 of 100 `circle_hidden_reads`) |
| Restore one question | 1 version read (0 when compared just before) |
| Send summary with ticks (phase D3) | 1 version read (the base) per send page |
| Join, add n banks | n head reads and their pictures |
| New cap | `circle_change_reads`, proposed 200 a day (UNCONFIRMED number) |

## 6. Partial send: semantics

* **An unsent change** is a question that differs from the link's base, the circle version this copy was last in
  step with: *added* (no circle id), *changed* (has an id, `merge::ours_changed` against the base snapshot), *removed*
  (id in the link, gone here). A move alone is not a change: the order sent follows this bank.
* **The tick list is exactly that set.** Nothing else can be partly sent. Default: all ticked; questions in
  `held_back` start unticked (Q5). Nothing ticked: Send is disabled ("Tick at least one change").
* **Composing the version** (a pure function, tested alone): walk this bank in order; a question with an id is sent as
  it is here if ticked or unchanged, else as the base version has it (the raw question JSON from the base body, with
  its `published.qid`, so the server sees it as unchanged); a question without an id is sent only if ticked; an
  unticked removal is put back, as the base has it, after the question that came before it in the base.
* **The base needed**: the base version's body, read once per send page from the circle (`GET .../versions/:base`).
  The link's snapshot (`merge::Snap`) has no `difficulty` or `figure`, so it can neither list a change to those nor
  rebuild a question; the summary therefore compares every field against the body. Base not kept or withheld:
  whole-bank send only, said plainly (4.7).
* **Pictures**: an unticked question keeps the base's picture. If that file is not on this computer it is fetched from
  the circle (hash checked, as the merge does) before the send; never sent as missing.
* **Checks run on the composed version**, not the bank: keyless questions, the more-than-half removal guard (428),
  the answer-key decisions and picture step (they are tied to the payload's sha256, so a new selection asks again),
  and the commit id (the same selection retried after a lost answer reuses it; a new selection gets a new one).
* **What is stored after success** (a new `after_partial_send`, beside `after_send`): `base_version`,
  `base_version_id` = the new version; base snapshot and local id -> qid pairs = the COMPOSED questions (an unticked
  removal keeps its pair although the question is gone here); `bank_modified` and `bank_print` = this bank as it was
  when the send started (the send does not change the bank); `held_back` = the unticked ids still differing.
  Consequence, tested as an invariant: right after the send, the next summary lists exactly the unticked changes,
  and `changes_since` counts the same ones (a difficulty- or figure-only change aside, 13.9).
* **Stale / fingerprint**: unchanged rule. A bank edited while the send was in flight is newer than `bank_modified`, so
  it is not stale and the new edits show as unsent. Using the composed bank's print here would be wrong (the bank
  would look "put back"): a test pins it.
* **Merge**: unchanged three-way merge. BASE = the composed version, so unticked changes show as "changed only by you"
  (kept by default). `held_back` survives a merge for questions still present.
* **Conflict (409)**: as today, nothing stored; ticks stay in memory; after "Get latest" the list is rebuilt from the
  new base and unticked marks are kept by local id.
* A whole-bank send is the case where everything is ticked and keeps today's path and tests.

## 7. Backend changes

Phase D2 only. **`GET /circles/:cid/banks/:bid/changes`** (a member who may read the bank):
`200 {bank_id, head_version, per_question, reason?, versions: [{version, compared_to, added: [qid], changed: [qid],
removed: [qid]}]}`, newest first, for kept and readable versions only, at most 40 indexes read one after another (two
in memory). `per_question: false, reason: "keys_hidden"` (no `versions`) for a viewer of a bank that hides its keys.
`ETag` over the head version id and the ids of the readable kept versions; `If-None-Match` gives 304 and is not
counted. Counts one `item_reads` and one `circle_change_reads`. A missing index (R2) makes that version a gap, never an
error. No migration (the next stays `0116`), no share-core change, no write. Measured before the round closes:
`EXPLAIN QUERY PLAN`, `meta.rows_read` at 60 and 1,000 versions (expected about the history page's 131 + 40), time and
memory on a 990-question bank with 20 kept versions. README route table updated.

Nothing else: partial send uses `PUT .../banks/:bid` as it is; the strip uses `heads` and `GET /circles` as they are.

## 8. Hooks outside `src/share/` (to be approved by the owner before touching)

The protected files are `src/screens.rs`, `src/theme.rs`, `src/figures*`, `src/figuresui*`, `src/fonts.rs`,
`src/main.rs`; `src/app.rs` needs the owner told first. **None of them needs a change.** The Shared page can switch to
the bank itself (`draw_shared` is already an `impl App` in `shared_ui.rs` and can call the existing `open_bank`), and
the strip drives the circle model's frame, sync and repaint from the bank page, so no `app.rs` tick is needed.

`src/bankui.rs` is not on the protected list, but it is the owner's bank screen and earlier share hooks there were
listed for the owner, so every line is listed here. Each is one call into a new `src/share/bank_links_ui.rs`:

| # | Where | Change | Phase |
|---|---|---|---|
| H1 | `bank_card` and `bank_list_row` | one line each: `bank_links_ui::badge(ui, app, &r.id)` | D1 |
| H2 | `bank_edit_body`, after the header row | `if bank_links_ui::strip(ui, self, bank_id) { return; }` (true when a circle page is open: the grid is not drawn; it saves banks itself when a merge or restore changed them) | D1 |
| H3 | `draw_card`, the question's right-click menu | one item: `bank_links_ui::question_menu(ui, app, bank_id, &q.id)` (draws nothing for unlinked banks) | D2 |

Not proposed: a badge on Home's "Your banks" (`screens.rs`, protected; Q9) and a strip in the pop-over window (owner's
`draw_popover`). Both are honest limits (section 13).

## 9. Privacy and abuse

* **No new reach.** Every text the new screens show is one the person can already read with today's routes; the new
  route serves qids and version numbers only, under the history route's reader rule.
* **Hidden keys.** Viewers of a hiding bank never see full-form change lists (5.4); their compare uses stripped
  reads, capped. A follow link made from the viewers' form can never send (client rule; the server's
  `422 keyless_question` and the viewer's `403` stay the backstop).
* **Withheld versions.** No list, no compare, no text; the gap is named, the reason is not (the round 44 rule; the
  round 51 exception for owner and author-editor is untouched).
* **Probing.** Per-question lists make the documented "changed 0 vs 1" probe (round 41, 48 V3) name the question.
  It still needs committed versions (100 a day) and the whole question; documented, not changed.
* **Local data.** `held_back` and the in-memory body cache hold nothing the linked bank does not; the cache is
  dropped on sign-out and account switch (the critic 60/61 rule).
* **Automation.** Heads checks throttled per bank; change lists cached per head and 304-able; a new daily cap.
  Partial sends are ordinary sends: own-work statement every time, 100 a day (decision D10), the heavy slot as now.
* **Attestation scope.** The own-work statement covers what is sent; unticked changes are not sent and not attested.

## 10. Test plan (test-first, mutation-checked)

Each test is written first and must fail on the code before its change; each round's builder keeps a mutation
driver in the scratchpad, and the orchestrator checks that every listed mutation is killed. Suites: `cargo test --lib
share::`, clippy (native and `wasm32`), the headless frames; worker `npm test` and `npm run typecheck` with local
workerd on 18791/18792 only (never 8799).

**Phase D1** (`src/share/bank_links_tests.rs`, scripted sessions as `circle_edit_tests.rs`):
link file with and without the new fields loads the same; an unreadable file is not written over; badge and strip
state for every row of the 4.2 table; no network on app start, on the Banks list or on an unlinked bank; heads check
throttled and only when signed in; Add makes edit vs follow links by role; a viewers'-form follow link never offers or
performs a send (also after promotion); Make my own copy leaves no link and writes the lineage line; share-to-circle
links the source bank at version 1 with its qids; join offer ticks by count; account switch drops what was read.
Mutations: follow link allowed to send; badge reading the network; heads throttle removed; share-to-circle not
linking.

**Phase D2** (worker `test/pool/round54.test.ts`; client `bank_links_tests.rs`):
lists equal a brute-force diff of the bodies for random send sequences (oracle, as 37-9); withheld versions have no
list and are never `compared_to`; pruned ones absent; viewer of a hiding bank gets `per_question: false` (and a
demoted author, and a viewer while hiding is turned on mid-sequence); non-member and former member 404; held bank only
to author/owner; 304 not counted; cap enforced; a missing index is a gap; rows and time flat from 60 to 1,000
versions. Client: timeline built from lines + lists with gaps; restore writes a restore point first, changes only that
question, then counts as one unsent change; restore of a question removed in the circle comes back as added; compare
of viewers' forms ignores Matching shuffle order. Mutations: include withheld in a diff; serve lists to hiding-bank
viewers; restore without restore point; compare not normalising the word bank.

**Phase D3** (`src/share/partial_send_tests.rs`, plus a staging check):
composition (pure) for every mix of ticked/unticked added/changed/removed; after success the next summary == the
unticked set (property test over random banks); a difficulty-only change is listed; not stale after a partial send,
and not stale when the bank was edited during the send; three-way merge after a partial send keeps unticked as
"yours"; 409 keeps ticks; whole-bank only when the base is gone or withheld; an unticked question's missing
picture is fetched; keyless and large-change checks on the composed payload; decisions re-asked for a new selection;
commit id stable for the same selection. Against a local worker: an all-but-one-unticked send reports `changed` == 1
(the raw base JSON round-trips as unchanged; UNCONFIRMED until run). Mutations: `mark_bank` with the composed bank;
base snapshot from the local bank; unticked removal dropped; `held_back` cleared on merge.

**Critics each round** (fresh, as before): a product/Moodle-parity critic (Moodle's question history and preview of
versions; whether a Moodle user would lose work or be surprised; Moodle behaviour established from its code,
UNCONFIRMED here) and a security/ASVS critic (access to the new route, hidden keys, withheld versions, local cache).
Blocker classes: the circles plan's list, plus "unsent work lost or sent unticked", "a linked bank shown as in step
when a change the snapshot tracks is unsent", "a full-form detail reaching a viewer of a hiding bank".

## 11. Migration of existing links and copies

* **Working copies** (`share-circles.json`): load as `mode: edit`, nothing else changes; they get badges and strips at
  once. Base, qids and stale fields keep their meaning.
* **Plain copies from circles** (in `share-imported.json` with a circle credit): stay as own copies with their update
  notices under "Things I imported" (Q7). Not converted into links: a link needs a base the copy may not have kept.
* **Authors' source banks** shared before phase D are not linked (nothing recorded which local bank was sent). The
  circle page offers Add to my banks; the strip is absent on the old source bank. A one-time line on the circle page
  for the author: "Your original bank is not linked. Add the circle's bank to My Banks to work on it with the circle."
* **"My working copies"** on the circle page goes; "Forget this link" becomes "Unlink...".
* **Downgrade**: an older build ignores the new fields and would treat a follow link as a working copy; the server
  refuses its sends (viewer `403`, keyless `422`). Documented.
* **Deep links** `shared:circle:<cid>[:<bid>]` (in `app.rs`) keep working: they land on the circle page (a bank id
  scrolls to its row). The seeded rig scripts are updated in `src/share/`, not in `app.rs`.

## 12. Phasing into builder / critic rounds

**Phase D1, navigation and verbs** (client; about 1,500 lines plus tests; 3-4 rounds). New `bank_links.rs` (model, no
egui) and `bank_links_ui.rs`; circle page rework in `circles_ui.rs`; join offer; share-to-circle linking; hooks H1, H2
after the owner approves them. Acceptance: every 4.2 state drawn with its wording at 420 and 1280 px; Send, Get
latest, History, Restore, Trim reachable from the bank page and behaving exactly as before (the existing circle
tests pass unchanged); no Send, merge or question list on the Shared page; Open in My Banks lands on the right bank;
Add vs copy by role; no network until a linked bank's page opens; only H1 and H2 outside `src/share/`; one clean
critic pair.

**Phase D2, per-question history and compare** (worker route first, 1-2 backend rounds; then client, 2-3 rounds).
Acceptance: route per section 7 with round54 tests each failing on round 53's code and the measurements recorded in
the README; panel, gaps, compare at both widths, restore as a local edit with a restore point; hiding-bank viewers per
Q4; hook H3 approved; one clean critic pair per side.

**Phase D3, partial send** (client; about 800 lines plus tests; 2-3 rounds). Acceptance: section 6 rules each pinned
by a test with its mutation killed; the staging round-trip check run once with the owner's go-ahead (staging needs
the owner's yes per phase, as before); a whole-bank send unchanged; one clean critic pair. Order: D1, then D2 and D3
(independent of each other; D3 first if the owner wants partial send sooner).

## 13. Risks and honest limits

1. **Composition correctness.** Partial send rests on the raw base JSON round-tripping as "unchanged" and on
   pictures of unticked questions being available. Unproved until the staging check; if it fails, the composer must
   convert through `merge::convert_question`, which may drop fields (risk of silently changing a question).
2. **Re-opening tuned flows.** The answer-key, picture and licence flows took critics 57-62 to settle; this plan
   leaves them where they are (Bank settings on the circle page) to avoid regressions. Moving them is Q1.
3. **Background answers wait.** Without an `app.rs` tick, a send or merge answer that arrives while the person is on
   another screen is applied when the bank page or Shared page is next drawn (the link logic already copes).
4. **Two computers.** Links are per device; another computer does not know this one's unsent or kept-back changes.
5. **History depth.** Only the kept versions (20, decision D16) have question-level detail; older ones are lines only,
   and the first kept version can only say "as it was then". A trim shortens it.
6. **Viewers of hiding banks** see no per-question list; compare costs hidden reads (100 a day).
7. **Not shown**: no badge on Home's "Your banks", no strip in the pop-over window, no field- or word-level merge.
8. **Cost unmeasured**: index sizes, route time and memory are estimates until D2's measurements.
9. **Strip count vs send list.** The strip counts from the snapshot (no network), which misses a change to only a
   question's difficulty or figure; the send summary, which reads the base, lists it. The strip may say "in step" for
   such a bank (today's behaviour too). Adding those fields to `Snap` is possible (serde default) but touches the
   merge's stored base; not proposed in phase D.
10. Every send still uploads the whole composed bank (up to 4 MB), as in phase B.

## 14. Owner decisions still needed (each with a recommendation)

| # | Question | Recommendation |
|---|---|---|
| Q1 | Where do bank settings live (answer keys for viewers, licence, remove, report)? | Keep them on the circle page as "Bank settings" (not editing; least risk to the critic-62 flows). Bank history also reachable there. |
| Q2 | Can viewers "Add to my banks"? | Yes, as a follow link (Get latest only). A viewers'-form link never sends. |
| Q3 | Timeline source | The new read route (section 5.3), not 20 body reads per bank. |
| Q4 | Viewers of a bank that hides keys | Bank lines plus on-demand compare of two viewers' forms; no per-question list. |
| Q5 | Send summary defaults | All ticked; questions unticked last time start unticked ("kept back") until ticked. |
| Q6 | Sharing a bank into a circle | The shared bank becomes the linked bank (no second copy). |
| Q7 | Own copies and old plain copies | New own copies: lineage note, no update notices. Old plain copies keep their notices. |
| Q8 | Checking for newer versions | Automatically when a linked bank's page opens, at most every 10 minutes, plus "Check now". |
| Q9 | Badge on Home's "Your banks" (needs a one-line `screens.rs` hook) | Not in phase D; later if wanted. |
| Q10 | Behind AND with unsent changes | Primary button Get latest; Send stays offered (it would meet the 409 and lead to the merge). |


## 15. Owner answers (2026-10-25)

Q1-Q10: all recommendations accepted. Three one-line hooks in `src/bankui.rs` (badge, status strip, question-menu History item) approved. Build order: D1, then D2, then D3, each with the builder + critics loop and local commits. `screens.rs`, `app.rs`, `main.rs`, `theme.rs`, `fonts.rs`, `figures*` stay untouched.

Later answers (2026-10-25, during D1 critics): a read-only Preview of circle banks before adding; "Add ticked banks" finishes in the background after leaving the Shared page; layout-only changes in `src/bankui.rs` (row badge under the name, centred "..." menu) allowed; a fourth one-line hook in `bankui::banks_body` (`bank_links_ui::adding_rows`) approved; one name, "My Banks", in circle wording.
