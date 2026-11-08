# Interaction task — `dna-protein-synthesis` batch 2 (new figure)

Reference: https://www.biologyfigures.com/dna-protein-synthesis. A new figure, so there is no `ours-before/`:
before this batch the figure did not exist and neither task could be started.

## The tasks

1. *"Type the template strand TACGGATTCAAGCCGATT, then blank the mRNA and the amino acids for my students."*
2. *"Put one wrong base in the mRNA for a multiple-choice option, show the codon chart beside the strands, and
   print the answer key."*

Steps are teacher actions (a click, a typed box, a menu choice). Task 2 starts where task 1 left off, with
the rows set back to Shown.

## Reference side (`playwright-cli -s=parity-dna`, headless, no profile)

| step | action | shot |
|---|---|---|
| 0 | open the page: five rows, Met–Pro–Lys–Phe–Gly | `ref/01-open.png` |
| 1.1 | type `TACGGATTCAAGCCGATT` in the DNA template strand box | `ref/02-type.png` |
| 1.2 | Rows › mRNA › Blank | |
| 1.3 | Rows › Amino acids › Blank | `ref/03-blank.png` |
| 2.1 | open the Mistakes card, How many › 1 (row mRNA is the default) | `ref/04-mistake.png` |
| 2.2 | open the Codon chart card, Show › Both (chart on the right is the default) | `ref/05-chart.png` |
| 2.3 | open the Chart title card, tick Answer key | `ref/06-key.png` |

Seen on the way: the Rows card is open at the start, Amino acids, Strands, Mistakes, Codon chart and Chart title
are folded (one click each to open). Switching Template/Coding/mRNA keeps the text and re-reads it. A stop codon
gets "Stop" as text with no bead and no tRNA. tRNA anticodons are written 3′→5′ under each codon, with no
backbone between them. The mistake is listed under the controls ("mRNA base 2 …") and in the key; it does not
change the amino acids. No base, row or bead on the figure reacts to the pointer (no on-figure actions).

## Ours (after), under Xvfb at 1280×960 points

Before each run the one-time "verSHion is still being built" notice is dismissed (not counted). Scrolling the
side panel with the mouse wheel is not counted, the same as on the reference; the runs use a new
`wheel:x,y,dy` QA step for it (src/app.rs). Task 2 is seeded with `{"seq":"TACGGATTCAAGCCGATT"}`, the state
task 1 left, with the rows back on Shown.

| step | action | shot |
|---|---|---|
| 0 | open: five rows, Met–Glu–Pro–Ala–Phe and Stop | `ours-after/01-open.png` |
| 1.1 | click the DNA template strand box, select all, type `TACGGATTCAAGCCGATT`, Enter (one typed box) | `ours-after/02-type.png` |
| 1.2 | open the Rows section | |
| 1.3 | Rows › mRNA › Blank | |
| 1.4 | Rows › Amino acids › Blank | `ours-after/03-blank.png` |
| 2.1 | open the Mistakes section, How many › 1 (row mRNA is the default; the mistake is listed under it) | `ours-after/04-mistake.png` |
| 2.2 | open the Codon chart section, Show › Both (on the right is the default) | `ours-after/05-chart.png` |
| 2.3 | open the Chart title section, tick Answer key (blanks filled, the mistake ringed, rows printed under the figure) | `ours-after/06-key.png` |

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| task 1: actions to finish | 3 | — (no figure) | 4 |
| task 1: finished? | yes | no | yes |
| task 2: actions to finish | 6 | — (no figure) | 6 |
| task 2: finished? | yes | no | yes |
| dead ends | 0 | — | 0 |

How it differs:
- Input: the same, a typed box plus a Template / Coding / mRNA switch that re-reads the same text; our box's
  label follows the switch ("DNA template strand (3′ to 5′)") and the protein is read out under Reading frame.
- Task 1 costs one more action: the reference opens with its Rows card unfolded; our Sequence section is open
  and Rows is one click away.
- Task 2: both sides open a folded card for each of the three settings. Our Show switch was a three-way dropdown
  at first (two clicks); the options were shortened to Strands / Both / Chart only so it draws as a segmented
  switch (one click).
- Student copy: the reference's Answer key only prints the key line. Ours is the `show_answers` switch, so the
  same tick also fills each blank in the key style and rings the deliberate mistakes, and Student + key exports
  the blank and the filled copy together.
- Shuffle the blanks and Shuffle the mistakes sit in the top card beside Classic setups, not under the slider
  and the mistakes list as on the reference.
- No on-figure actions on either side.

Verdict: task 1 `finished, more actions: Rows starts folded`; task 2 `fewer-or-equal actions and finished`.
