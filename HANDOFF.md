# HANDOFF 2026-10-27 (evening): circles phase D1, rounds 19 and 20 complete, waiting on critics 101/102

Repo `verSHion`, branch `shared-pool-phase1`, last commit is the round 19–20 checkpoint. This file was rewritten after the round-19 takeover session (`vershion-fd`, `240c6322`) lost its builder to the weekly API limit. The limit resets **2026-10-28 16:00 PT**.

D1 rounds 19 and 20 are **committed locally** (the round 19–20 checkpoint, 45 files: 35 under `src/share/` plus this file and the nine critic reports 92-100). **Nothing is pushed.** The rest of the dirty tree belongs to the other sessions (see "Do not touch").

## Where D1 stands

Round 18 (committed in `0d17a83` as WIP) was failed by its critic pair; round 19 was scoped to the two important findings plus cheap minors, with the brief "this is the last scoped round". **All 7 round-19 items are now in the tree, with tests, and pass.** The previous version of this file said item 7 was only partly done; four more pin tests were added after it was written.

| # | Item | State |
|---|------|-------|
| 1 | c100 I2: a save failure that loses her edits warns, survives Dismiss, gives advice | done |
| 2 | c100 I1: Check now says "Checking…" then "Checked just now: still the same" | done |
| 3 | c99 m1/m2: held retry saves the links file first and holds on failure | done |
| 4 | c99 m4/m7/m5/m3: persist held Undo, hold Get latest, one safety copy per model, move sent times | done |
| 5 | c99 m6/m8/m9 | done |
| 6 | c100 minors 1, 2, 5, 7, 9, 10, 12 | done |
| 7 | Pin the mutation gaps | **done**: `s19_m12`, `s19_m17`, `s19_m21`, `s19_m24`, `s19_m61`, `s19_m80` in `bank_links_fix19_tests.rs` |

## Round 20 (2026-10-27): the leftover minors

The owner asked for the leftovers from critic 100 to be worked: **minors 3, 4, 6, 8 and 11 are now done**; minor 12 (walls of text) was not in that set and stays.

| Minor | Change | Test |
|-------|--------|------|
| 3 | A row with no "Put mine back" says why and what she can do: `Kept` rows say the copy moved on and "set it by hand if you want yours"; `Changed` rows add the same tip | `c100_m3_…` |
| 4 | A put-back row names the circle's value ("…the circle's is Hard"); "Put all of mine back" **asks first** ("Put yours back on N questions?" / "Yes, put all back" / "Keep them") and changes nothing until the yes | `c100_m4_…` ×2 |
| 6 | The button column sits just past the widest line that offers one (was at the far edge, ~700 px away at 1280); a row with no button uses the whole width; at a narrow width the stem is cut at 28 chars, so a row is 3 lines, not 4, and "Easy -> Hard" stays whole | `c100_m6_…` ×3 |
| 8 | "Use my bank as it is" and "Get latest" name the same bank: the circle's title, never her bank's own name on one page and the circle's on the next (`merge_title`) | `c100_m8_…` |
| 11 | The news part of the badge has a short form ("difficulty"); "removed from the circle" is said as removed even when `gone()` is also true; `WAITING_NOTE` is "circle not checked yet", not "waiting for the circle check"; the "..." hint says "circle page", the menu's own words | `c100_m11_…` ×3 |

New file: `src/share/bank_links_fix20_tests.rs` (10 tests). Five existing tests changed **deliberately**, each because round 20 reverses what it pinned: `fix18` `c98_m11` (as-is now names the circle's title), `c98_i2` (the news part is now shortened), `c98_m6` (Put all now asks), `fix15`/`fix17`/`fix18` waiting-note assertions (now the constant), `fix18` hint wording.

**Round 20's six behavioural tests are mutation-checked, 6 of 6 killed** (scratch tree `/tmp/d1mut`, own target `/tmp/d1mut-target`, script `/tmp/d1mut-scripts/mut20.py`, logs `/tmp/d1mut20-*.log`):

| Mutant | What it breaks | Test that failed |
|--------|----------------|------------------|
| ma | "Put all" acts on the first press without asking | `c100_m4_put_all_…` |
| mb | a row with no button is held to the button column | `c100_m6_a_row_with_no_button_…` |
| mc | the stem is always cut at 60, whatever the width | `c100_m6_a_narrow_…` |
| md | `gone()` is asked before `seen.removed` (removed said as "left") | `c100_m11_removed_…` |
| me | a put-back row drops the circle's value | `c100_m4_a_put_back_…` |
| mf | a row whose button is gone gives no reason or tip | `c100_m3_…` |

## Verification (2026-10-27 evening, live tree)

- `cargo test --lib`: **1998 passed, 5 failed, 3 ignored** (2006 total, 83.3 s). Log: `/tmp/d1-round20-final2.log`.
- `cargo build --bins`: EXIT 0.
- `cargo check --lib --target wasm32-unknown-unknown`: EXIT 0 (23 warnings, pre-existing).

The 5 failures are **not round 19's**: `bank_links_fix10_tests` ×3, `fix11` ×1, `fix12` ×1. They call `share_core::wasm::publish_prepare_json` and get `{"errors":[{"code":"expected_number","path":"bank.questions[0].points"}]}` on `"points": 1.0`. Cause: the **figures session** added `vishion-core` / `vishion-net` path deps to `Cargo.toml`; their `serde_json` features (`arbitrary_precision`, `preserve_order`) change how share-core's `Node::Num` sees `1.0`, so `as_f64()` returns `None`. Proven not D1's: in a HEAD-only scratch tree (`git archive HEAD` + live `src/share/`) those tests pass. `server/share-core` is untouched and its own suite passes. **The owner has ruled this belongs to the other session; do not edit share-core or the Cargo files as part of D1.**

## Mutation checks on the six new pin tests (2026-10-27 evening)

Method: `git archive HEAD | tar -x -C /tmp/d1mut`, `cp -a src/share/. /tmp/d1mut/src/share/`, `CARGO_TARGET_DIR=/tmp/d1mut-target` (own target, so the shared build lock is untouched), one mutant at a time with the pristine files restored after each.

| Mutant | Where | Result |
|--------|-------|--------|
| m12 | `circle_edit.rs` success path: drop `add_news(l, version, &already);` | **killed** |
| m17 | `(have.len() > known).then_some(have)` → `{ let _ = known; Some(have) }` | survives — **equivalent**, as documented in the test's doc comment (the caller only waits for questions the bank still has) |
| m21 | drop `!r.by.is_empty() &&` from the send-record guard | **killed** |
| m61 | `offered >= 2` → `offered >= 1` for "Put all of mine back" | **killed** |
| m80 | drop `&& !l.as_is` from the partial-count line | **killed** |
| m24 | `settle_all_news`: do not call `self.save()` | **killed** (re-confirmed) |

Scratch tree and logs: `/tmp/d1mut`, `/tmp/d1mut-<mutant>.log`, summary `/tmp/d1mut-summary.log`.

## Screenshots (2026-10-27 evening)

Twelve real-app frames, taken with the QA hooks: `VERSHION_STORE_DIR` (a fixture store), `VERSHION_SCREEN=bank`, `VERSHION_WINSIZE`, `VERSHION_SCRIPT` (`tap:x,y` in egui points), `VERSHION_SHOT`, `VERSHION_SHOT_FRAME=200`, under `xvfb-run`. Window scale is 1.7, so a 1280-point window shoots 2176 px wide; 420 points does work despite `min_inner_size([760,520])`.

In `/tmp/d1shots/` (11 MB, survives until the box reboots):

- `news-{light,dark}-{1280,760,420}.png` — the difficulty news list: three rows, per-row "Put mine back", "Put all of mine back", Dismiss, signed-out strip.
- `news20-{light,dark}-{1280,420}.png` — the same after round 20: the button column beside the rows, and the 420 view with the stem cut at 28 chars.
- `putall20-light-1280.png` — the "Put all of mine back" question (round 20): "Put yours back on 3 questions?…", "Yes, put all back" / "Keep them".
- `warn-{light,dark}-{1280,760,420}.png` — the round-19 I2 warning strip: red panel, "Your difficulty couldn't be put back, and your own edits aren't saved either…", Try again / Sign in… / "…", the news list kept beneath it.

The save failure was produced without code changes by making the banks write fail: `mkdir <store>/banks.json.tmp` (the backend writes `<name>.tmp` then renames, so this fails only `banks.json`). `share-circles.json.tmp` as a directory fails only the links file and gives the other path (m2), also captured once.

**Not captured: the Check-now success feedback** ("Checking…" then "Checked just now: still the same."). It needs a live signed-in circle read; the app is signed out in a fixture store and cannot reach that state offline. The wording and geometry are pinned by `c100_i1_check_now_*` in `bank_links_fix19_tests.rs`. If the owner wants a picture of it, that is a staging run and needs their yes.

Fixtures were made by a temporary `#[ignore]` test appended to `bank_links_fix19_tests.rs` (it ran `many_news(3)` from `bank_links_fix18_tests` and wrote `banks.json`, `share-circles.json`, `settings.json`). It has been removed; re-add it if the shots need regenerating.

## What is left (in order)

1. **Fresh critic pair (101 data safety, 102 product/UX) on round 19.** Blocked until the weekly API limit resets **2026-10-28 16:00 PT**. Rule from the brief: if the pair finds only minors, commit D1 locally and write up the leftovers; if it finds new important problems, tell the owner rather than keep looping.
2. If it passes: **commit D1 locally** and **ask the owner about the Tidy change**. Local commits are fine; never push. See memory `circles-loop-authorisation`.

## Decisions taken by the owner (2026-10-27)

- **Keep "Check now" as the big button** when the circle's version can't be read. Round 19's visible feedback stands.
- **The `vishion-core`/`vishion-net` dep and the 5 share-core test failures belong to the other session** (the figures one). D1 does not touch them; a short bug note with the evidence is enough.
- Two other sessions have the tree dirty: one on figures (it added the vishion deps), one on the db/sharing work.

## Do not touch (other sessions' uncommitted work)

`Cargo.toml`, `Cargo.lock`, `src/ai.rs`, `src/app.rs`, `src/screens.rs`, `src/store.rs`, `src/lib.rs`, `src/model.rs`, `src/figures/`, `src/gradeui.rs`, `src/figuresui.rs`, `src/tidy.rs`, `src/bankui.rs`, `server/`. Final D1 edits go only to live `src/share/`. Naming rules from the brief: "difficulty", never "ratings" or "pool"; "safety copy" is one word.

## Pitfalls

- **Shared cargo target dir.** Other sessions hold the build lock; a `cargo test` can sit at "Blocking waiting for file lock" for minutes. This is not a hang. Use an own `CARGO_TARGET_DIR` for scratch work, as above.
- A full `cargo test --lib` takes about 84 s warm (150 s cold) once it has the lock.
- **Backgrounded `cargo ... | tail` prints nothing until exit.** Redirect to a file and read that.
- Task `.output` files under `/tmp/claude-1000/.../tasks/` vanish when a session restarts, and the per-session scratchpad dirs went with them. Use `/tmp/<name>.log` for long output.
- `xvfb-run` screenshots need the app built first (`target/debug/vershion`); the window scale is 1.7, so divide screenshot pixels by 1.7 to get the egui points a `VERSHION_SCRIPT` click expects.
- `docs/critic-reports/` has many untracked files from this loop (92 to 100, plus the `grade-p*` files from the other session). Do not delete them.

## Related

- `docs/handoff-shared-pool.md` is the standing project handoff (what is being built, standing rules).
- Memory: `circles-phaseD-decisions`, `circles-loop-authorisation`, `shared-pool-accounts`.
- Round 18 condensed reports: `docs/critic-reports/2026-10-27-circles-phaseD1-critic99.md` and `-critic100.md`.

## First commands

```
cd ~/Documents/GitHub/verSHion
git status --short src/share            # nothing: D1 is committed
cargo test --lib                        # expect 1998 pass, 5 fail (the share-core ones), 3 ignored
```
