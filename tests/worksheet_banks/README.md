# Worksheet-to-bank corpus evaluation

The downloaded PDFs and page PNGs live outside Git, under
`~/Documents/verSHion-test-scans/web-math-corpus`.

`cases.json` is a source-authored inventory: printed problem ranges and which
problems require source figures, including blank plotting grids. It does not
infer expected counts or figures from an AI reply. Source image hashes pin the
inventory to the collected corpus.

The current acceptance target is 98% (raised from the original 95% for new runs).
Both the inference harness and independent audit reconciler default to 98%; use
an explicit `--threshold .95` only when reproducing a historical 95% result.
Frozen reports and references from earlier rounds are not rewritten.
Import coverage, content review, recreated figures,
and saved-bank integrity are reported separately. Missing pages and timeouts
remain in the denominator. A crop is retained for review but does not pass as
an editable reconstruction. Native figure runs must execute `render_figure`,
produce a saved FigureSpec and image, and receive a matching review. A model's
visual review is evidence, not an independently verified mathematical gold key;
uncertified content is not labelled as 98% mathematical accuracy.

The runner uses the real app image-import hook and an isolated store for every
page. Remote calls require --allow-remote and an explicit model. Existing keys
are passed only in the child's environment, never copied to the run directory.
Replay re-scores saved banks without model requests. Cache reuse requires the
source, binary, provider/model, settings and hint identity to match.

Long local Linux runs should acquire a per-run sleep inhibitor, for example
`systemd-inhibit --what=idle:sleep:handle-lid-switch --mode=block --who=verSHion --why='Worksheet validation' python3 tools/worksheet_banks.py ...`.
The inhibitor ends with the runner and changes no permanent power settings.
Reports record active time, wall time and, where Linux BOOTTIME is available,
suspend time separately. Timeout budgets remain explicitly based on monotonic
active time. Host pauses are environmental disturbances, never grounds to remove
failed items from the denominator or to attribute a timeout to a renderer.

## Import modes

`--mode page` (default, the ablation and the mode of the saved baselines) imports every page alone.
`--mode document` imports each source document (cases grouped by `source_id`, pages in original PDF order,
question-only PNGs) in ONE app process through `VERSHION_IMPORT_DOCUMENT`, so a continuation page is read with
the directions the earlier pages of the same document printed. The context starts empty for every document, a
newly printed task wins, and the trace records where each applied direction came from. A document budget is
`--timeout` x pages; a page that fails stays in the denominator and does not discard its siblings.

Scoring maps source numbers through the importer's own per-item provenance (`items` in the trace: every raw
item, saved or dropped), never by zipping raw labels with surviving questions. A figure-only item whose task is
not visible is saved in an explicit review state and never counts as a complete source item.

Source-scope protocol `per_figure_scope_v2` uses application-owned keys and explicit per-figure decisions.
The review can propose only bounds; it cannot change source text, identity or ownership. Changed crops are
rendered and shown again before they can be confirmed. Original, proposed and applied bounds, actual shown
keys, missing previews and both review stages are traced. A failed sibling does not block an unchanged,
confirmed crop. Unconfirmed owners retain available source evidence and an explicit review tag, including
prose items whose crop failed. The scorer requires confirmed raw item indices for this protocol, separately
from saved question indices; legacy traces retain their original page-level gate. These decisions establish
model-reviewed crop scope only, never independent source fidelity or editable recreation on their own.

The figure agent can record `executions` with every maker attempt and its selected-attempt identity.
Logical conversation deltas retain tool schema text, commands, turn budgets, errors and image hashes;
transport headers and credentials are excluded. Source-only observations happen before any candidate,
are explicitly unverified model evidence, and guide only the maker. The reviewer sees the original
request and actual candidate/source images. Small source images are enlarged by integer pixel repetition,
never cropped or reconstructed; original stored media is unchanged. These observations do not supply
an independent source-fidelity pass. `figure_execution_evidence` reports all retained attempt counts;
legacy selected-attempt summaries remain labelled incomplete, and logical-call counts do not claim
provider-side HTTP retry or billing counts. No acceptance threshold changes with these diagnostics.
