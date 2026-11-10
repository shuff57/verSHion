# Interaction task: `dot-and-cross`, batch 6

## The task

"Draw magnesium chloride with every ion shown, then make it a find-the-mistake question: one chloride electron drawn as
an open circle and the magnesium's charge left as a blank box."

Capabilities: cap-01d (type a formula), cap-03a (every ion drawn), cap-06a (click an electron to cycle its mark),
cap-06c (click one ion's charge to show it, blank it or leave it off).

## Reference (chemistryfigures.com/dot-and-cross, headless playwright, 1280x800)

The reference opens on H2O. Its ionic compounds open with every ion drawn.

| # | action | shot |
|---|---|---|
| 0 | (opening state) | ref/00-open.png |
| 1 | type MgCl2 in "Formula or name": [Cl]- [Mg]2+ [Cl]- | ref/01-type-formula.png |
| 2 | click a chloride electron (a dot): it becomes a cross | ref/02-click-electron.png |
| 3 | click it again: an open circle | ref/03-click-electron-again.png |
| 4 | click Mg's "2+": a blank box | ref/04-click-charge.png |

## Ours before (main at e0d5cd4)

Picking Ionic compound and then MgCl2 (2 clicks) drew one Mg ion and one chloride ion with a 2 in front
(ours-before/02-pick-mgcl2.png). There was no formula box, no every-ion layout, and the figure had no handles, so no
electron or single charge could be changed. Ion charges: Blank blanked every charge at once. Not finished.

## Ours after (figtest build under Xvfb, 1280x800)

Nothing seeded. The alpha notice was dismissed first and is not counted. Scrolling the panel is not counted either; on
the reference the controls are already in view.

| # | action | shot |
|---|---|---|
| 1 | type MgCl2 in "Formula or name" (the first box in the panel): Mg2+ and 2[Cl]- | ours-after/01-type-formula.png |
| 2 | open the Drawing section | |
| 3 | Ions: open the list and pick Every ion (2 clicks): [Cl]- [Mg]2+ [Cl]- | ours-after/02-every-ion.png |
| 4 | click a chloride electron (a cross): it becomes an open circle | ours-after/03-click-electron.png |
| 5 | click Mg's "2+": a blank box | ours-after/04-click-charge.png |

The Question section's summary then reads "Electrons drawn, 2 edits", as the reference's does.

## Result

| | reference | ours before | ours after |
|---|---:|---:|---:|
| actions to finish | 4 | - | 6 (1 typed, 1 section, 2 for the list, 2 figure clicks) |
| finished? | yes | no | yes |
| dead ends | 0 | - | 0 |

How it differs:
- Layout default: the reference draws every ion as soon as an ionic compound is typed. Ours keeps "One of each,
  numbered" as the default so that figures saved before this batch draw as they did, so every ion is a setting.
- Marks: on the reference the chloride's own electrons are dots and the gained one a cross; ours draws them the other
  way round (Swap dots and crosses gives the reference's), so reaching an open circle takes one click on ours and two
  on the reference. The cycle is the same: dot, cross, open circle, open square, nothing.
- Clicks: both act on the figure itself. Ours outlines the electron, symbol or charge under the pointer and names what
  a click does ("Electron 5 on Cl: cross. Click for open circle").
- Student copy: both keep the edits (they are the question) and drop the answer key, which still describes the
  correct diagram.

Verdict: `finished, more actions: every ion is a setting on ours (the reference's default) because old saved figures
keep their numbered layout, and our Drawing section starts closed`.
