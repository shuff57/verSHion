# HANDOFF 2026-10-28: review findings for the Cloudflare AI worker harness

Repo `verSHion`, branch `shared-pool-phase1`, HEAD `b6e0609` at the time of writing.

This file is a **review handoff, not a build.** Nothing was changed: no source edit, no commit, no push.
Everything below is either read from the tree, run against it, or explicitly marked unverified. The
circles/D1 session owns `HANDOFF.md`; this was written as a separate file on purpose so that file was
not clobbered mid-round.

The brief was: review the app, list unmerged branches, test every file type it claims to import, and
confirm the two existing AI spikes as the starting point for a Cloudflare "AI worker harness" that turns
an uploaded doc/pdf/image into a test bank and makes figures from the app's registry.

---

## Bottom line

1. **The two spikes are the right starting point, and were written to be lifted.** `figure_agent`'s own
   module doc says so: "The loop only needs a `Chat` and the registry, so it is a plain function and can
   later run somewhere other than the desktop app (a Worker relaying tool calls back to a client)."
   The `Chat` trait is commented "A trait so tests (and a future remote host) can stand in for Ollama."
2. **No such harness exists yet.** No "AI worker harness", no `/ws` route, nowhere: not in any branch
   tree, any commit message, the reflog, dangling commits, stashes, the four worktrees,
   `server/worker` (HTTP only), `tools/`, or `origin`. It is new code, not a rediscovered thing.
3. **The model is real.** `@cf/google/gemma-4-26b-a4b-it` on Workers AI: MoE 26B total / 4B active,
   vision yes, tool calling, 256k context. Vision matters — it is load-bearing in both spikes.
4. **But this repo's Worker has no AI binding.** `server/worker/wrangler.toml` declares D1 (`DB`),
   R2 (`POOL`) and cron, and no `[ai]` stanza. Whatever "our Worker AI setup" refers to lives elsewhere.
   Needs the worker name / account before wiring starts.

---

## Branch state (counts move — another session is committing)

`git rev-list --left-right --count main...<branch>`, first read 2026-10-28, **re-read 2026-10-29**:

| Branch | Ahead of `main` | Behind `main` | State |
|---|---|---|---|
| `shared-pool-phase1` (current) | 93 | 60 | Live work. HEAD moved `b6e0609` → `13ac7b9` (+49 commits, circles D2 client r6–r16). **Not pushed** (no remote). |
| `figures-gapfill` | 103 | 19 | Unmerged, stale. Grew 48 → 103 while this doc waited. |
| `question-editor-polish` | 26 | 60 | Unmerged; `origin/question-editor-polish` exists. Its content was re-applied on `main` as `fd3b8d0`, so the local tip is superseded. |
| `figures-circuits`, `figures-computing`, `figures-shell`, `question-editor-on-main`, `worktree-agent-a4f2…`, `worktree-agent-a96d…` | 0 | — | Merged into `main`. |

`origin` has two branches only: `main` and `question-editor-polish`.

---

## The two spikes: what to reuse, exactly

| Seam | Location | Why it carries over unchanged |
|---|---|---|
| `Chat` trait — `fn chat(&mut self, &[Turn]) -> Result<String, String>` | `src/figure_agent.rs:35` | The only model dependency in the loop. A Worker impl replaces `ModelChat` and nothing else changes. |
| Tool dispatch — `{"tool": "list_figures" / "get_figure" / "render_figure", "args": {…}}` | `src/figure_agent.rs:390` | Already a remote-call shape. A worker-as-relay needs **zero** refactor: the client runs the tools, the worker only does model turns. |
| `render(id, params) -> (FigureSpec, PNG, notes)` | `src/figure_agent.rs:196` | Pure. Gives the "emit spec only" vs "render too" choice for free. |
| `figure_eval` + `tests/figure_agent_cases.json` | `src/figure_eval.rs` | A ready acceptance suite for the worker. Run the same cases against the new `Chat`. |
| `battery.py` + gold file | `tests/import_battery/battery.py` | Not a harness — a **scorer**. PDF → page PNGs → live model → bank + cropped figures, graded against gold. Reuse as the CI gate on the import half. |
| `ditto::import_text` | `src/ditto.rs` | The wire format for "doc → test bank". Return this; do not invent a schema. |
| `formats::detect_and_parse` | `src/formats.rs` | Pure, 174 tests, module docs are the format contract. |
| `store::Backend` trait | `src/store.rs` | The fs/IndexedDB seam that must become R2 (or no-store) in a Worker. |
| `server/share-core` (`build-wasm.sh`, `pkg/`) | `server/share-core/` | Existing precedent for compiling Rust to wasm for a Worker, with the exports checked at build time. |

---

## Cloudflare wiring: the shape

- **No key** → new `AiCfg` provider `"workers"`; `WorkersChat` calls `env.AI.run("@cf/google/gemma-4-26b-a4b-it", …)`.
  `request_key()` returns `None` and never touches key storage. Users never see a key field.
- **Key supplied** → the existing providers (`ollama` / `openrouter` / `custom` / `cloud`), untouched.
  `AiCfg::from` already normalizes four; this is a fifth. Per-request dispatch, not per-build.
- Images already go as **base64 data URIs** (`src/ai.rs:868` and `src/ai.rs:1006`), not remote URLs, so the
  documented "Gemma 4 rejects remote image URLs" failure mode does not apply.

### Two things that cannot work the way they are usually imagined

- **You cannot BYOK Workers AI.** Inference is billed to the account owning the binding; there is no
  per-request key. "Still accept keys" only ever means *someone else's* provider. The UI must say so, or
  people will paste a Cloudflare token and be confused when it is ignored.
- **Accepted keys must never touch disk or logs.** `ai.rs` has a hard "never logged, echoed, or in errors"
  rule and the desktop app saves keys to `ai-keys.json` at 0600. A Worker has no disk: accept per request,
  strip from error strings, keep them out of D1 rows and `console.log`.

### "Let anyone use" is a quota problem, not a key problem

An open endpoint plus your binding lets anyone drain your neural allocation, and one figure run is up to
`MAX_STEPS = 8` model round trips plus renders. Plan for the Rate Limiting binding, Turnstile, and a
per-session cap. `workers_dev = false` and `preview_urls = false` already keep it off `*.workers.dev`.

### Long requests

Eight sequential model round trips will outlive a single `fetch`. The pool worker already has D1 + cron,
so the pattern is submit-job → poll (or a Durable Object), not one synchronous call.

---

## What blocks it

1. **Wasm size.** `dist/web` is 119 MB; the app wasm is **19.9 MiB after `wasm-opt -Oz`** (the figure is
   recorded in `Cargo.toml` next to the profile). Workers paid is 10 MB, free is 3 MB. Full worker-side
   parse + render + agent does not fit. A lean crate mirroring `server/share-core` is required, and
   `assets/fonts` alone is 4.4 MB — those bytes must come from R2 or a static asset, not the binary.
2. **`Chat` is sync and native-gated.** `ModelChat` → `ai::chat_turns` uses blocking `ureq`, and `ai.rs`
   carries `cfg(not(target_arch = "wasm32"))` / `cfg(target_arch = "wasm32")` splits. A Worker needs an
   async `Chat` plus a `fetch`-backed impl. `ai.rs` already has `*_async` siblings to copy shapes from.
3. **`to_png` needs fonts it cannot have.** `fontdb()` calls `load_system_fonts()` **and** loads the
   4.4 MB under `assets/fonts`. A Worker has no system fonts.
4. **`Cargo.toml` is not shippable.** `path = "../viSHion/crates/core"` and `"../viSHion/crates/net"` break
   a fresh clone, so a new Worker crate cannot be built from one until those are vendored.
5. **File claims.** The harness touches `src/ai.rs`, `src/figures/`, `src/store.rs`, `server/` and
   `Cargo.toml` — every one of them on the D1 session's "Do not touch" list (see below). No wiring can
   start until those release.

---

## Decisions (owner, 2026-10-29)

1. **Account** — Shuff57@gmail.com's Account, id `bd9289625d5d3d62591c36e5ebe49055`. Worker names come from
   the tree: `vershion-pool` (`wrangler.toml`) and `vershion-pool-staging` (`wrangler.staging.toml`); neither
   file carries an `account_id` and **neither has an `[ai]` stanza** (re-read 2026-10-29). One must be added
   to whichever worker hosts the relay.
2. **Storage — D1 + R2, chosen.** Persistence is in scope from the first version, so `store::Backend` needs
   an R2 implementation and the worker needs D1 tables. **This keeps blocker 5 live:** `src/store.rs` is on
   the D1 session's claim list, and no storage work can start until it releases.
3. **Figures — spec only, chosen.** The worker returns `FigureSpec`; the client renders. That takes blockers
   1 and 3 (wasm size, fonts) out of scope for v1, along with the lean-crate work they force.

The probe path is also decided: the owner re-authenticates wrangler with the Workers AI scope and this
session makes the single live `env.AI.run` call. Until it happens, the three Cloudflare unknowns below stay
open — the storage choice does not depend on them, but the harness's request shape does.

---

## Test evidence

### Import matrix — 30 generated fixtures through `VERSHION_PARSE`, all pass

`cargo test --test import_formats`: **6/6 pass**. Generator and runner kept at `/tmp/vs-import-check/gen.py`
and `/tmp/vs-import-check/run.py`.

Covered: Blackboard text and tab upload, Aiken, Respondus, text2qti, CSV grammar, term lists (`.tsv`),
xlsx (questions / kahoot / vocab), docx, Moodle GIFT and XML, QTI 1.2 / 2.1 / 3.0 / `.qti`, Canvas `.zip`
and `.imscc`, `.parrot.json` (new and legacy), Ditto JSON, embedded PNG/JPG/GIF/WebP/BMP, PDF page
rasterization (2 pages). Correct refusals for: `.doc`, `.xls`, junk `.txt`, junk `.zip`.

Two real defects found in the same pass:

- `VERSHION_CONVERT=<file.pdf>` is broken: it sends the raw PDF bytes as an `image_url` and the provider
  answers `400 Failed to load image or audio file`. The Import screen rasterizes first, so only the
  headless hook is affected. A worker harness would inherit that path.
- The file picker lists `.tsv`, but the CSV grammar is comma-only; only term lists and Blackboard upload
  rows read TSV.

### Health

- `cargo check --lib --target wasm32-unknown-unknown`: exit 0 (23 warnings).
- `cargo test --lib` at review time: 2090 passed, **5 failed**, 3 ignored. I diagnosed the five as
  `vishion-core` / `vishion-net` enabling `serde_json`'s `arbitrary_precision` through feature unification,
  which breaks `share-core`'s `Node::Num::as_f64()` on `1.0`. The D1 session fixed the same thing
  independently in `b6e0609` (`server/share-core/src/json.rs` reads back `$serde_json::private::Number`).
  `HANDOFF.md` then recorded **2099 passed, 0 failed, 4 ignored**; I did not re-run. It has since moved on
  (**2108 passed, 0 failed, 5 ignored** in its later entries, and **2248 passed** at r10 per
  `docs/handoff-2026-10-29-vershion-8a.md`). The five are fixed; the exact count is still unmeasured here.
- `Cargo.toml` still carries the `../viSHion` path deps that cause the feature unification.

### Not run

- The 66 Vitest files under `server/worker/test/`.
- Any wasm/browser runtime check.
- Any visual or GUI review.
- No CI exists anywhere in the repo.

---

## Verified vs unverified

**Verified by reading source:** the trait set, tool dispatch, `render()`, the `cfg` gates, the base64 image
encoding, `wrangler.toml` bindings, branch counts, the absence of any AI harness, the D1 fix in `b6e0609`.

**Verified by running:** the 30-fixture import matrix, `import_formats` 6/6, the wasm32 check, the failure
diagnosis, the branch/commit counts.

### Live probe — 2026-10-29, account `bd92…9055`, model `@cf/google/gemma-4-26b-a4b-it`

Run in a throwaway worker (`/tmp/vs-ai-probe`, `[ai] binding = "AI"`, `wrangler dev --remote`, scratch name
`vershion-ai-probe`, stopped afterwards). Nothing touched `vershion-pool`.

| Call | Input | Warm ms | completion tokens | neurons | Result |
|---|---|---|---|---|---|
| text | demand one JSON object | 1995–2494 | 95–112 | 3.0 | exact object, no fence |
| vision | 2.8 KB PNG via data URI | 1693 (**36 074 cold**) | 112–136 | 5.8 | read "TRIANGLE 42" correctly |
| page | 146 KB quiz page | 11 893–15 116 | 726–766 | 22.7–23.8 | 5/5 questions + options, clean JSON |

The three unknowns:

1. **The call shape works, the response shape does not carry over.** `env.AI.run(MODEL,
   { messages, response_format: { type: "json_object" }, max_tokens })` is accepted, but the answer is
   **OpenAI-shaped**: text at `choices[0].message.content`, plus a `reasoning_content` field and
   `usage.neurons`. `chat_turns`' current parse does not match it; `WorkersChat` has to parse this itself.
2. **`response_format: json_object` is honoured** — all three calls returned pure JSON with no code fence.
   The figure loop's one-JSON-object-per-turn protocol holds.
3. **Latency** — warm: ~2 s text, ~1.7 s small image, ~12–15 s per quiz page (~750 output tokens).
   **Cold start is real: the first vision call took 36 s.** A synchronous `fetch` path needs that headroom;
   a 10-page document is ~2 minutes of model time alone, which settles the submit-job/poll design.

Hidden reasoning inflates `completion_tokens` (95 tokens for a ~10-token answer), so spend tracks reasoning
effort, not output size.

**Still not verified:** the 19.9 MiB is the app-wide wasm, **not** a measurement of any carved-out crate.

---

## Do not touch (the D1 session's list, from `HANDOFF.md`)

`Cargo.toml`, `Cargo.lock`, `src/ai.rs`, `src/app.rs`, `src/screens.rs`, `src/store.rs`, `src/lib.rs`,
`src/model.rs`, `src/figures/`, `src/gradeui.rs`, `src/figuresui.rs`, `src/tidy.rs`, `src/bankui.rs`,
`server/`. D1's own edits go only to `src/share/`.

The working tree is a blend of three features (circles D3, grades, figures) and **cannot** be committed to
`shared-pool-phase1` as-is. `src/gradeui.rs` and `prompts/handwriting.md` exist only as untracked files.
Grading is parked at the owner's request — do not advance it.

---

## First commands

```
cd ~/Documents/GitHub/verSHion
git log --oneline -3                    # expect b6e0609 or a D3 successor
git rev-list --left-right --count main...shared-pool-phase1
cargo test --test import_formats        # 6/6, fast, no lock contention worth worrying about
```

Then, before writing any Worker code: get the worker name / account, and make one live `env.AI.run` call
with a base64 image and a JSON-object prompt. That single call settles the three open questions above.
