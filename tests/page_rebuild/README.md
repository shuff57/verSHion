# Local page rebuild harness

Run the local builders before spending model calls. This harness builds
documentaSHion's existing editable worksheet layouts, draws verSHion figures
locally, and validates DOCX/PDF output through WordCraft and Poppler.

```sh
python3 tools/page_rebuild.py \
  --manifest "$HOME/Documents/verSHion-test-scans/documentashion/manifest.json" \
  --layouts "$HOME/.local/share/documentashion/ws-corpus/out" \
  --out /tmp/vershion-page-rebuild
```

Requirements: Python with Pillow and pypdf; wordcraft-cli, pdftoppm, pdftotext,
Tesseract; the documentaSHion release binary and its local vershion-figure helper.
Both binary paths can be overridden. No account settings, keys or model calls
are needed. The runner refuses extraction/review commands that invoke remote AI.

Every manifest entry is accounted for, including repeat scans and blank backs.
Bindings identify a candidate printed layout and rotation. Independent source
OCR checks identity and placement; a binding alone is not a fidelity pass.
Original images are checksum checked and never modified.

Outputs include editable per-page DOCX files, PDFs, rendered PNGs, source
previews, stage logs, a combined PDF, a side-by-side HTML index and an atomic JSON
report. Identical layouts share a build keyed by layout and toolchain hashes.
Cached artifacts are checksum verified and revalidated by the current checks.
OCR has its own source/rotation/PDF/tool/validator cache. Timeouts kill the whole
builder process group, and subprocess output is drained to avoid pipe deadlocks.

Checks cover ordered text retention, every native equation's glyphs and
fraction/power/root structure, table/figure counts, missing plotted curves,
unexpected pagination, off-page text, table rules crossing text and visible
tab-leader answer lines. Recognized doubled LaTeX commands and grid row
separators are normalized with an audit trail. Reviewed source-specific
corrections are pinned in layout_fixes.json and refuse unexpected inputs.

These layouts are rebuild inputs, not an independent transcription oracle.
Source OCR cannot certify exact math or read worked solutions. Successful
serialization never approves source fidelity automatically. Printed worksheet
rebuilds, visual layout review, and handwriting/key transcription are reported
separately. Blank backs remain explicit output pages.

```sh
python3 -m unittest discover -s tests/page_rebuild -p 'test_*.py'
cargo test --lib import_eval::tests -- --include-ignored
```

The native evaluator preserves math operators, counts omitted answers as lost,
keeps the positions of unkeyed blanks, and keys cached replies by page content,
prompt, provider/model and output schema. Old ordinal page-N.txt files are
ignored. Fully cached/replay runs make no service/model call; partial runs
charge only uncached pages.

The older import_battery/battery.py now defaults to an isolated local Ollama
profile and rejects cloud aliases. OpenRouter requires --provider openrouter,
--allow-remote and an explicit --model. It no longer copies saved keys into
its output folder. Failed blank/photo conversions fail their control checks.
