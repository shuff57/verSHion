# CRITIC P4-03 — PIECE A: W.marked inline rendering

## Bar reference
`js/render.js:42` (the `R.unit` function): `html = html.replace(/\[illegible\]/gi, '<span class="illegible">[illegible]</span>')` 
→ renders `[illegible]` inline in transcript text flow as `<span class="illegible">[illegible]</span>`, styled by:
- `css/components.css:159`: `.tx-text .illegible { font: inherit; }`
- `css/components.css:179`: `.w.illegible { background: var(--bad-bg); color: var(--bad); text-decoration: underline dashed var(--bad); text-decoration-thickness: 2px; text-underline-offset: 5px; }`
- `css/components.css:182`: `.w .illegible { background: none; padding: 0; color: inherit; }` (inline within word button).

## Our implementation
`src/gradeui.rs:draw_render_text` (line ~231): splits each transcript line at `[illegible]`, renders
non-illegible segments as `ui.label(seg)` and `[illegible]` segments as `ui.label(egui::RichText::new(seg).color(red).size(14.0))`.

## Gap analysis: SAME INPUT, DIFFERENT OUTPUT

**Input:** a transcript line like `"The [illegible] word is hard"` with one `[illegible]` marker.

**Bar output:** The line renders as a SINGLE HTML div with inline span:
`<div class="ln">...The <span class="illegible">[illegible]</span> word is hard...</div>`
- `[illegible]` appears INLINE within the sentence's text flow.
- CSS makes it red (`color: var(--bad)`) with background highlight (`background: var(--bad-bg)`) + dashed underline.

**Our output:** The line renders as SEPARATE egui labels concatenated:
- `ui.label("The ")` → label 1 with normal text
- `ui.label(RichText::new("[illegible]").color(red))` → label 2 with red text on a new line below label 1
- Result: two stacked labels, NOT inline within one line.

**Why this is the biggest gap:**
The bar's `[illegible]` is **inline in the text flow**. Ours is **out-of-flow** — each segment becomes its own label
widget, which egui stacks vertically. The transcript line that should show one line of text with `[illegible]`
embedded now shows two stacked labels. The "inline" requirement is not met: the marker does not appear inline
within the line's text; it appears on a new line below.

**Additional gaps (secondary):**
- No background highlight: bar uses `background: var(--bad-bg)` + `color: var(--bad)`; ours uses `color(0xcc,0x44,0x44)` without background.
- No underline: bar applies `text-decoration: underline dashed var(--bad)` + `text-decoration-thickness: 2px`; ours has no underline.
- No word-key wrap: bar's `.w .illegible` gives clickable key for tray selection; ours has no `.w` wrapper, no data-key → tray cannot focus on illegible markers.

## Verdict: FAIL

The single biggest remaining gap: **our markers are rendered as separate stacked labels, NOT inline within the transcript line's text flow.** The bar embeds the marker span inside the line's single HTML/text node; ours places it as a separate egui widget. Without inline rendering, the user sees fragmented lines instead of a single transcript line with inline illegible markers + highlighter background.

Result: FAIL — needs another cycle to fix inline layout.