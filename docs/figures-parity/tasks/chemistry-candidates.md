# Chromatography and Electrochemical Cells

Reference captured in T3 preview on 2026-11-23. Neither route had a figure ID, renderer, or route mapping at base commit `260684a3`. The closest local entries were Gel Electrophoresis and Circuit Diagram, which model different apparatus and calculations.

## Chromatography

- **CH-1 · Edit inks and show the setup.** The [opening editor](https://chemistryfigures.com/chromatography) has three named inks and an 8 cm solvent front. Adding inks reaches six lanes. Adding dyes to ink A reaches five spots. Toggling Before and after produces two papers. On the first paper the inks sit on the origin line. On the second they have separated. With apparatus enabled the solvent stays below the origin.
- **CH-2 · Measure spots and verify Rf.** With [before/after, measurements, and the answer key on](https://chemistryfigures.com/chromatography?show=both&measurements=1&answerKey=1), A separates into blue at 5.6 cm and yellow at 2.4 cm and B has red at 4.4 cm. Independently dividing by 8.0 cm gives blue 0.70, yellow 0.30, and red 0.55. C contains all three. These ratios match the reference key.
- **Local replay.** Open **Figures → Chemistry → Chromatography**. Turn on **Include paper before development**, **Show travel distances**, and **Label apparatus and guide lines**. Select **Print** and export **Student + key**. Distances are supplied on both copies. The ratios appear on the key, and the same dye has the same gray hatch in both copies. Change the ink count to six and enter a dye list such as `green 3.2, blue 5.6` in Ink 6 to exercise the added lanes.
- **Editor differences.** verSHion uses an ink count and editable comma-separated dye lists. The reference has add/remove buttons and individual color selectors. verSHion uses leaders for spot distances and a dimension arrow for the solvent front. It supports the reference's nine named dye colors and limits of six inks and five dyes per ink.

## Electrochemical Cells

- **EC-1 · Reverse the electrodes.** In the [opening editor](https://chemistryfigures.com/electrochemical-cells), Zn starts on the left and Cu on the right, with a digital reading of 1.10 V. Turn on anode/cathode labels and electron flow. Interchange the two metals. The [reversed cell](https://chemistryfigures.com/electrochemical-cells?left=Cu&right=Zn&electrodes=1&electronFlow=1&answerKey=1) shows Cathode (+) on the left and Anode (−) on the right. Both arrowheads point left. The standard voltage is still `0.34 − (−0.76) = 1.10 V`.
- **EC-2 · Exercise the alternate apparatus and reactions.** Set Al on the left and Ag on the right. Standard reductions of −1.66 V and +0.80 V give 2.46 V. Three Ag⁺ ions are reduced for every Al atom oxidized. The reference key prints this balanced reaction. The reference editor offers a digital voltmeter, dial voltmeter, or coil of wire, plus a custom meter reading from 0.00 to 9.99 V.
- **Local replay.** Open **Figures → Chemistry → Electrochemical Cells** and use the same electrode pairs. Toggle **Label anode and cathode** and **Show electron direction** to inspect the roles and arrows. Choose **Aluminum–silver cell with a dial**, switch the figure to **Print**, turn off **Give meter reading**, and export **Student + key**. The student dial retains its scale and the key adds the needle, standard voltage, and equations. A custom reading is labeled separately from the standard voltage on the key.
- **Editor differences.** verSHion accepts identical half-cells as a 0.00 V standard case with no net reaction or directional arrows. T3 preview kept the prior valid pair when the same metal was selected on both sides. The reference also explains the standard voltage in its Cell controls. In verSHion the worked subtraction appears in the key.

## Reproduce the checks

```sh
cargo test --lib chromatography
cargo test --lib electrochemical_cells
cargo test --lib all_figures_render_with_defaults
cargo test --lib every_figure_opens_with_overlay
cargo test --lib every_figure_panel_has_a_faq
cargo test --lib share_link_round_trips_every_figure_and_example
cargo test --lib keep_their_opening_state
cargo test --lib spec_roundtrip_and_unknown_keys
```

All 12 targeted tests passed. The figure tests check the numerical cases, rendered spot and arrowhead coordinates, balanced equations, answer visibility, and the pixels of rasterized Print exports. The workspace tests exercise the registered editor entries, spec serialization, and share-link reopening. I also inspected native PNG renders of the chromatography pair, reversed copper–zinc cell, and aluminum–silver dial pair.
