# Mechanics worksheet figures

Captured in T3 Code Preview on 2026-11-23. The baseline checkout was `260684a3`. Its Physics registry could draw inclined planes, free-body diagrams, energy bar charts, circular motion and spring scale readings. Each of the five tasks below needed a dedicated figure. The existing generators keep their own IDs, parameters and renderers.

| Task | Reference replay and observed result | verSHion replay and expected result |
| --- | --- | --- |
| LEVER-01 | Open the Lever page, open **Title and answer key**, and enable the key. The opening effort is 100 N and MA is 3. Choose Class 3. The arms swap to 0.5 m (effort) and 1.5 m (load) and the output gives 900 N. [Replay](https://physicsfigures.com/lever?class=3&effortArm=0.5&loadArm=1.5&answerKey=1) | In **Figures → Physics → Lever**, use the default 300 N and 1.5 m / 0.5 m. Tick **Show answers** in Worksheet or export **Student + key**. Verify MA 3 and 100 N on the key and a blank effort on the student copy. Choose Class 3 and check 900 N with 0.5 m and 1.5 m. |
| PENDULUM-01 | Select **One bob**, open Forces, and choose components. The release view shows weight, tension, and both components. The opening Swing view has two equal-height extreme positions and a lower central bob. [Replay](https://physicsfigures.com/pendulum?picture=one&arc=0&heightMark=0&forces=components) | In **Pendulum**, set Swing, 60°, and mark velocity. The outer bobs are at half the string length below the pivot, and the rise above the middle bob is half the length. The velocity points away from the release side. Switch to One bob and Components for the release diagram. Use **Labels and marks** to clear height or path when needed. |
| COASTER-01 | Set the loop to point 2. Its default radius is 8 m. In **Object**, place the cart upside down at the top of the loop. The track still starts at 40 m and passes through 10, 25, and 5 m points. [Replay](https://physicsfigures.com/roller-coaster?loopAt=2&objectOnLoop=1) | In **Roller Coaster**, set loop point 2 and radius 8 m. Put the cart at the loop top. Mark the first stretch rough and label it 25 m. The 10 m point is the loop bottom, the centre is at 18 m and the top is at 26 m. The first span is hatched and has its own horizontal distance label. |
| SPRING-01 | Switch to **Three positions**, then to Horizontal. The 40% amplitude view gives two inward acceleration arrows and one velocity arrow at equilibrium. [Replay](https://physicsfigures.com/spring?orientation=horizontal&picture=positions&equilibriumLine=1) | In **Spring**, use Horizontal, 2 in parallel and −30% with Before and after. Each spring compresses to 70% of the original length. Then choose Hanging, Three positions, and 40% amplitude. The displacements are equal on either side of the loaded equilibrium and the two acceleration arrows point inward. |
| TORQUE-01 | Change Support to Wall and open the Torque results. For 6 N at 0.3 m, 3 N at 0.9 m, and a cable at 1 m and 30°, the reported tension is 9 N and the wall reactions are about 7.79 N horizontal and 4.5 N up. [Replay](https://physicsfigures.com/torque?support=wall) | Use the default beam and loads. A pivot at 0.5 m gives 1.2 N·m in each direction. Select Two supports at 0.1 and 0.9 m for reactions of 4.5 N each. For Wall and cable, use 1 m and 30° and check the key against the reference. Set Pivot, **Solve for load** to 1, and load 2 to 6 N. Export a student/key pair and check that the solved 12 N appears only on the key. |

## Verification

```sh
VERSHION_MECHANICS_EVIDENCE_DIR=/tmp/vershion-mechanics-evidence \
  cargo test --lib mechanics_figures
cargo test --lib physics_sweeps_params
cargo test --lib physics_examples_render
cargo test --lib every_figure_panel_has_a_faq
cargo test --lib share_link_round_trips_every_figure_and_example
```

The focused tests compare SVG geometry and labels against independent arithmetic. Lever includes mass-to-weight conversion, and torque includes centimetres, mass, signed moments, and the fixed-wall case. The pendulum release test checks the drawn bobs and force labels. The spring and coaster tests measure geometry because the reference figures leave numeric worksheet answers to students. PNG tests check that all five Print outputs contain exclusively grey pixels, that Colour output contains chromatic ink, and that examples rasterize. The workspace test opens the actual editor selection, runs frames, pastes an edited draft, exports and reloads its stored spec, and checks the regenerated SVG. Student/key content is tested through the shared pair renderer before the native file dialog. The desktop print dialog was outside this check.

The optional evidence directory receives SVG and PNG output from the focused tests. The live capture in this worktree also wrote `reference/routes.json` with opening control inventories and post-edit text, and `reference/*-reference-*.png` with opening and changed states. The linked replay URLs reproduce the edited reference states without the scratch files.

## Remaining gaps

- Lever starts with the effort blank on a student copy. The edited key draws the forces and prints the working. The reference shows the numeric effort by default and lets the author blank it separately. The local effort label can still be entered explicitly.
- Roller Coaster supports up to six points in the numbered editor. It has no one-click reordering or removal of a middle point, and neighboring rough stretches keep separate distance marks. Large loops leave room by reducing the scale of the entire drawing, preserving the height-to-radius ratio.
- Torque supports up to six loads with a count control. Its key gives directional sums and support results. The reference shows separate load calculations in a live working panel and offers direct per-load removal. A teacher can enter equivalent loads using the numbered controls.
- Pendulum, Spring and Roller Coaster draw the geometry and the selected symbolic marks. Their reference routes also leave the speed, energy, period, or spring-constant calculations to students.
