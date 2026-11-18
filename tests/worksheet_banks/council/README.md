# Worksheet bank council

The starting gate is 95%; changing the scorer or deleting difficult cases does
not count as improving the app. Every source page/item/required figure stays in
the denominator. Source references and candidate audits cover all 353 items;
uncertain source facts remain pending. Process completion and a model's review
of its own render are not source-fidelity passes.

Roles in round 01:

- Sonnet through the existing Claude proxy implements import/prompt/JSON/tool
  fixes and runs actual app inference into isolated test stores.
- Two independent Codex source reviewers divide the 38 uncovered pages by
  complete source documents. They capture source facts before inspecting bank
  outputs and write separate reference proposals, without changing the scorer.
- The primary Codex coordinator reviews changes, reconciles references, checks
  persisted items against originals, and owns acceptance decisions.

Known fixes to investigate: same-document continuation instructions; valid
figure-only questions dropped before their source task can be applied; and
crop/context contamination that can turn question text into a figure title.
Figure reconstruction must preserve given labels/scales/arrows/curves and blank
axes without adding solved answers. Do not blanket-strip legitimate source
figure titles just to pass examples that have none.

Iterate by preserving each baseline, making a bounded change, running meaningful
regressions and fresh real inference, comparing with the original source, and
recording failures. A result without independent evidence remains pending.
Use page/document/time budgets and report exhaustion explicitly.

`tools/worksheet_candidate_audit.py` combines subsequent independent candidate reviews. It pins
each review to the source-reference, source PNG, run-report, saved-bank and trace hashes, requires
every assigned item and evidence, and rechecks actual figure tool/spec/media traces. Missing reviews
and uncertain source facts remain in the denominator. Faithful bitmap crops can pass visual fidelity
but cannot pass editable recreation. Stored content, complete bank items and worksheet export renders
are counted separately; a successful model self-review never supplies an independent visual pass.

Pass the saved full-run `--run`, original `--corpus`, additional source `--reference` files,
per-reviewer `--audit` files and an `--out` path outside the source corpus inputs. This is a no-inference
evaluation step. Candidate reviews are immutable snapshots, so a new bank/binary run needs new reviews.

The existing corpus has already been run, so a split of it is not an untouched
holdout. Before claiming generalization at 95%, collect fresh worksheets and
freeze their references before final validation. Group splits by document so
continuation pages cannot leak between development and validation.

Source images, reports, round assignments and proposed references live outside
Git in ~/Documents/verSHion-test-scans/web-math-corpus/bank-tests/council-round-01.
Keep original user banks/settings and source PDFs/PNGs unchanged. Credentials
remain in process memory and never enter the reports.
