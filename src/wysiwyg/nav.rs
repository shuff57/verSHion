//! Navigation over visible spots (question editor phase 4).
//!
//! Pure functions over one unit's [`Ana`]: no egui, no state. A *spot* is a place the caret can
//! be seen at (`Ana::spot`): an atom (`$..$`, `___`) is one spot, collapsed markers are none.
//! `TextEdit` moves its caret over raw chars, so on its own a Shift+Arrow beside a formula selects
//! the raw `$`, and a mouse drag can end inside an atom. These functions say where the caret and the
//! selection ends *should* be, and the editor writes the answer back with `set_cursor`. The markup
//! string is never touched here.

use super::Ana;

/// A caret position for the visible `spot` (the editor's canonical one among the equal-looking
/// raw positions), or None when the unit has no such spot.
fn position_of(ana: &Ana, spot: usize) -> Option<usize> {
    let n = ana.role.len();
    (0..=n).find(|&q| ana.inside_atom(q).is_none() && ana.spot(q) == spot).map(|q| ana.canonical(q, None, false))
}

/// The caret one visible spot to the right (`dir > 0`) or left (`dir < 0`) of `p`; `p` itself
/// at the end of the text. An atom is crossed whole, collapsed markers are skipped.
pub fn step(ana: &Ana, p: usize, dir: i32) -> usize {
    let n = ana.role.len();
    let p = p.min(n);
    // inside an atom (should not happen): its edge in the direction of travel
    if let Some(a) = ana.inside_atom(p) {
        return if dir > 0 { a.end } else { a.start };
    }
    let here = ana.spot(p);
    let target = if dir > 0 {
        if here >= ana.spot(n) {
            return p;
        }
        here + 1
    } else {
        if here == 0 {
            return p;
        }
        here - 1
    };
    position_of(ana, target).unwrap_or(p)
}

/// Shift+Left / Shift+Right: the anchor stays (moved off the inside of an atom), the caret moves
/// one visible spot. Returns `(anchor, caret)`.
pub fn extend(ana: &Ana, anchor: usize, caret: usize, dir: i32) -> (usize, usize) {
    let (a, c) = snap_range(ana, anchor, caret);
    (a, step(ana, c, dir))
}

/// Move the ends of a selection so that none is inside an atom (an end that is the later one
/// goes to the atom's end, the earlier one to its start) and a range that holds no visible spot
/// collapses to one caret. `(anchor, caret)` in, `(anchor, caret)` out.
pub fn snap_range(ana: &Ana, anchor: usize, caret: usize) -> (usize, usize) {
    let n = ana.role.len();
    let (anchor, caret) = (anchor.min(n), caret.min(n));
    if anchor == caret {
        // a collapsed caret inside an atom goes to its nearer edge
        return match ana.inside_atom(caret) {
            Some(a) if caret - a.start <= a.end - caret => (a.start, a.start),
            Some(a) => (a.end, a.end),
            None => (caret, caret),
        };
    }
    let out = |e: usize, later: bool| match ana.inside_atom(e) {
        Some(a) if later => a.end,
        Some(a) => a.start,
        None => e,
    };
    let (a, c) = (out(anchor, anchor > caret), out(caret, caret > anchor));
    if ana.spot(a) == ana.spot(c) {
        let q = ana.canonical(c, None, false);
        return (q, q);
    }
    (a, c)
}

/// What a key does while the whole field is selected.
#[derive(Clone, Debug, PartialEq)]
pub enum AllOp {
    /// Backspace or Delete.
    Clear,
    /// Typed or pasted text.
    Type(String),
    /// Enter.
    Newline,
}

/// The text that replaces the whole field after `op`, given what earlier keys of the same frame
/// already made of it (`None`: nothing yet). Backspace after typed text in the same frame takes
/// that text back first.
pub fn all_apply(cur: Option<String>, op: AllOp) -> Option<String> {
    match (cur, op) {
        (None, AllOp::Clear) => Some(String::new()),
        (Some(mut t), AllOp::Clear) => {
            t.pop();
            Some(t)
        }
        (c, AllOp::Type(s)) => Some(c.unwrap_or_default() + &s),
        (c, AllOp::Newline) => Some(c.unwrap_or_default() + "\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wysiwyg::analyze;

    fn ana(s: &str) -> Ana {
        analyze(s, &|c: &str| !c.trim().is_empty() && !c.starts_with(char::is_whitespace) && !c.ends_with(char::is_whitespace))
    }

    #[test]
    fn a_step_crosses_a_formula_whole_and_skips_markers() {
        let a = ana("ab $x^2$ c");
        // from before the formula, one step lands after it
        assert_eq!(step(&a, 3, 1), 8);
        assert_eq!(step(&a, 8, -1), 3);
        // bold markers cost no step: `**b**` is one visible char
        let b = ana("**b**c");
        assert_eq!(b.spot(step(&b, 0, 1)), 1);
        assert_eq!(b.spot(step(&b, step(&b, 0, 1), 1)), 2);
    }

    #[test]
    fn whole_field_keys_replace_everything() {
        assert_eq!(all_apply(None, AllOp::Clear), Some(String::new()));
        assert_eq!(all_apply(None, AllOp::Type("ab".into())), Some("ab".into()));
        let t = all_apply(None, AllOp::Type("ab".into()));
        assert_eq!(all_apply(t, AllOp::Clear), Some("a".into()));
        assert_eq!(all_apply(Some("x".into()), AllOp::Newline), Some("x\n".into()));
    }

    #[test]
    fn steps_stop_at_the_ends() {
        let a = ana("$x$");
        assert_eq!(step(&a, 0, -1), 0);
        assert_eq!(step(&a, 3, 1), 3);
        assert_eq!(step(&ana(""), 0, 1), 0);
    }

    #[test]
    fn extending_over_a_formula_selects_all_of_it_or_nothing_of_it() {
        let a = ana("ab $x^2$ c");
        let (an, ca) = extend(&a, 3, 3, 1);
        assert_eq!((an, ca), (3, 8));
        // one more press moves past the next char, never into the atom
        let (an, ca) = extend(&a, 8, 8, -1);
        assert_eq!((an, ca), (8, 3));
    }

    #[test]
    fn a_selection_ending_inside_an_atom_snaps_outward() {
        let a = ana("ab $x^2$ c");
        assert_eq!(snap_range(&a, 0, 5), (0, 8));
        assert_eq!(snap_range(&a, 7, 1), (8, 1));
        assert_eq!(snap_range(&a, 4, 6), (3, 8));
    }

    #[test]
    fn a_range_of_only_hidden_markers_collapses() {
        let a = ana("a**b**");
        // chars 1..3 are the opening `**`: nothing visible is selected
        let (s, e) = snap_range(&a, 1, 3);
        assert_eq!(s, e);
    }

    /// For random texts: stepping never lands in an atom, moves by exactly one spot, and a
    /// snapped selection never has an end inside an atom.
    #[test]
    fn property_steps_and_snaps_stay_outside_atoms() {
        const TOK: &[&str] = &["a", "bc", " ", "**", "$x$", "$a+b$", "___", "_{2}", "^{3}", "\n", "é", "~~", "`c`", "[t](https://x.y)"];
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..4000 {
            let k = (next() % 9) as usize;
            let s: String = (0..k).map(|_| TOK[(next() % TOK.len() as u64) as usize]).collect();
            let a = ana(&s);
            let n = a.role.len();
            for p in 0..=n {
                for dir in [-1, 1] {
                    let q = step(&a, p, dir);
                    assert!(a.inside_atom(q).is_none(), "{s:?} step {p} {dir} -> {q}");
                    if q != p {
                        let want = (a.spot(p) as i64 + dir as i64) as usize;
                        assert!(a.spot(q) == want || a.inside_atom(p).is_some(), "{s:?} {p} -> {q} spots {} {}", a.spot(p), a.spot(q));
                    }
                }
                for c in 0..=n {
                    let (x, y) = snap_range(&a, p, c);
                    assert!(a.inside_atom(x).is_none() && a.inside_atom(y).is_none(), "{s:?} snap {p} {c} -> {x} {y}");
                }
            }
        }
    }
}

/// Whole-field selection, Shift+Arrow over atoms and undo across fields, through real egui frames
/// (no window), two editors side by side like a stem and an answer.
#[cfg(test)]
mod frames {
    use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, TextEdit};

    use crate::wysiwyg::{Opts, UiId};

    struct H {
        ctx: egui::Context,
        stems: Vec<String>,
        ids: Vec<UiId>,
        t: f64,
        /// The blank answers mirror of editor 0, when it edits a fill-in-the-blank stem.
        rows: Option<Vec<String>>,
    }

    impl H {
        fn new(stems: &[&str]) -> H {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            let ids = (0..stems.len()).map(|i| UiId::new(("nav_frames", i))).collect();
            let mut h = H { ctx, stems: stems.iter().map(|s| s.to_string()).collect(), ids, t: 0.0, rows: None };
            h.run(3);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            let mut input = RawInput::default();
            input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0)));
            input.time = Some(self.t);
            input.events = events;
            self.t += 0.05;
            let ids = self.ids.clone();
            let stems = &mut self.stems;
            let rows = &mut self.rows;
            let o = self.ctx.run_ui(input, |ui| {
                for (i, s) in stems.iter_mut().enumerate() {
                    let blanks = if i == 0 { rows.as_mut() } else { None };
                    crate::wysiwyg::editor(ui, ids[i], s, Opts { size: 14.5, width: 600.0, structured: i == 0, newlines: true, hint: "", blanks, toolbar: false, lock_tab: false });
                    ui.add_space(10.0);
                }
            });
            o.drop_without_applying_deltas();
        }

        fn run(&mut self, n: usize) {
            for _ in 0..n {
                self.frame(vec![]);
            }
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) {
            self.frame(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }]);
            self.run(3);
        }

        fn text(&mut self, s: &str) {
            self.frame(vec![Event::Text(s.to_string())]);
            self.run(3);
            // typing runs coalesce by time: let the run end
            self.t += 2.0;
        }

        fn focus(&mut self, i: usize) {
            let n = self.stems[i].chars().count();
            crate::wysiwyg::focus_end(&self.ctx, self.ids[i], n);
            self.run(4);
        }

        /// (anchor, caret) of the first unit of editor `i`.
        fn sel(&self, i: usize) -> (usize, usize) {
            let uid = self.ids[i].with(("wys_unit", 0usize, 0usize));
            let r = TextEdit::load_state(&self.ctx, uid).and_then(|s| s.cursor.char_range()).expect("a cursor");
            (usize::from(r.secondary.index), usize::from(r.primary.index))
        }
    }

    /// Frame cost of typing into a stem (`cargo test --release --lib bench_ -- --ignored --nocapture`).
    fn bench(name: &str, stem: &str) {
        let mut h = H::new(&[stem]);
        h.focus(0);
        h.run(5);
        let n = 30;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            h.frame(vec![Event::Text("x".into())]);
        }
        let per = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        eprintln!("BENCH {name}: {per:.2} ms per typed character (one frame, native)");
    }

    #[test]
    #[ignore]
    fn bench_typing_frame_cost() {
        let line = "The quick brown fox jumps over the lazy dog and keeps running far away";
        bench("1 line", line);
        bench("5 lines", &vec![line; 5].join("\n"));
        bench("30 lines", &vec![line; 30].join("\n"));
        let f = "A formula $x^2+1$ and $\\frac{a}{b}$ and $\\sqrt{y}$ inside a line of text";
        bench("30 lines + 3 formulas", &(0..30).map(|i| if i == 5 || i == 15 || i == 25 { f } else { line }).collect::<Vec<_>>().join("\n"));
        let mixed: Vec<String> = (0..30).map(|i| if i % 3 == 1 { format!("- item {i} {line}") } else { format!("{line} {i}") }).collect();
        bench("30 lines mixed list/paragraph", &mixed.join("\n"));
    }

    const LIST: &str = "intro $x$\n- a\n- b\ntail";

    #[test]
    fn opening_and_drawing_a_stem_never_rewrites_it() {
        // every construct and every fixture string: a few frames with a focus change and no key
        // leave the markup byte for byte as it was (what is stored is what was loaded)
        for s in crate::wysiwyg::tests::full_corpus() {
            let mut h = H::new(&[s.as_str()]);
            h.focus(0);
            h.run(2);
            assert_eq!(h.stems[0], s, "drawing {s:?}");
        }
    }

    fn kev(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }
    }
    fn tev(s: &str) -> Event {
        Event::Text(s.to_string())
    }

    #[test]
    fn keys_of_one_frame_apply_in_order_on_an_empty_stem() {
        // char, Enter, char in ONE frame (a fast typist, a key-repeat burst, a paste of key events)
        let mut h = H::new(&[""]);
        h.focus(0);
        h.frame(vec![tev("a"), kev(Key::Enter, Modifiers::NONE), tev("b")]);
        h.run(4);
        assert_eq!(h.stems[0], "a\nb", "Enter between two letters is never dropped");
    }

    #[test]
    fn keys_in_back_to_back_frames_without_idle_frames_all_land() {
        for gap in [0usize, 1, 2] {
            let mut h = H::new(&[""]);
            h.focus(0);
            for e in [tev("a"), kev(Key::Enter, Modifiers::NONE), tev("b")] {
                h.frame(vec![e]);
                h.run(gap);
            }
            h.run(4);
            assert_eq!(h.stems[0], "a\nb", "gap {gap}");
        }
    }

    #[test]
    fn a_whole_burst_in_one_frame_matches_the_same_keys_one_frame_each() {
        // every kind of key in one frame gives the same text as the slow path
        let seqs: Vec<Vec<Event>> = vec![
            vec![tev("ab"), kev(Key::Backspace, Modifiers::NONE), tev("c")],
            vec![tev("one"), kev(Key::Enter, Modifiers::NONE), tev("two"), kev(Key::ArrowUp, Modifiers::NONE), tev("X")],
            vec![tev("ab"), kev(Key::ArrowLeft, Modifiers::NONE), tev("Z"), kev(Key::ArrowRight, Modifiers::NONE), tev("Y")],
            vec![tev("hi"), kev(Key::Enter, Modifiers::NONE), kev(Key::Enter, Modifiers::NONE), tev("x")],
            vec![tev("- a"), kev(Key::Enter, Modifiers::NONE), tev("b"), kev(Key::Enter, Modifiers::NONE), kev(Key::Enter, Modifiers::NONE), tev("c")],
            vec![tev("a"), kev(Key::Enter, Modifiers::NONE), kev(Key::Backspace, Modifiers::NONE), tev("b")],
            vec![tev("a"), kev(Key::Home, Modifiers::NONE), tev("b"), kev(Key::End, Modifiers::NONE), tev("c")],
        ];
        for (i, seq) in seqs.iter().enumerate() {
            let mut slow = H::new(&[""]);
            slow.focus(0);
            for e in seq.clone() {
                slow.frame(vec![e]);
                slow.run(3);
            }
            let mut fast = H::new(&[""]);
            fast.focus(0);
            fast.frame(seq.clone());
            fast.run(12);
            assert_eq!(fast.stems[0], slow.stems[0], "sequence {i}");
        }
    }

    #[test]
    fn select_all_covers_every_block_and_delete_clears_the_stem() {
        let mut h = H::new(&[LIST]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.stems[0], "", "the list went with the paragraphs");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.stems[0], LIST, "one undo brings it all back");
    }

    #[test]
    fn typing_over_a_whole_field_selection_replaces_all_of_it() {
        let mut h = H::new(&[LIST]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        h.text("Z");
        assert_eq!(h.stems[0], "Z");
        h.text("y");
        assert_eq!(h.stems[0], "Zy", "the selection ended with the first key");
    }

    #[test]
    fn an_arrow_ends_the_whole_field_selection_at_the_start_or_end() {
        let mut h = H::new(&[LIST]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        h.key(Key::ArrowRight, Modifiers::NONE);
        h.text("!");
        assert_eq!(h.stems[0], format!("{LIST}!"));
        h.key(Key::A, Modifiers::COMMAND);
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.text("^");
        assert!(h.stems[0].starts_with('^'), "{:?}", h.stems[0]);
    }

    #[test]
    fn ctrl_home_and_ctrl_end_cross_blocks() {
        let mut h = H::new(&[LIST]);
        h.focus(0);
        h.key(Key::Home, Modifiers::COMMAND);
        h.text("^");
        assert!(h.stems[0].starts_with("^intro"), "{:?}", h.stems[0]);
        h.key(Key::End, Modifiers::COMMAND);
        h.text("$");
        assert!(h.stems[0].ends_with("tail$"), "{:?}", h.stems[0]);
    }

    #[test]
    fn select_all_in_a_one_unit_field_stays_native() {
        let mut h = H::new(&["one $x$ two"]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        assert_eq!(h.sel(0), (0, 11), "egui selected the unit itself");
        h.text("Q");
        assert_eq!(h.stems[0], "Q");
    }

    #[test]
    fn shift_arrow_beside_a_formula_selects_the_whole_formula() {
        let mut h = H::new(&["a $x^2$ b"]);
        crate::wysiwyg::focus_end(&h.ctx, h.ids[0], 2);
        h.run(4);
        assert_eq!(h.sel(0), (2, 2));
        h.key(Key::ArrowRight, Modifiers::SHIFT);
        assert_eq!(h.sel(0), (2, 7), "all of $x^2$, never a part of it");
        h.key(Key::ArrowRight, Modifiers::SHIFT);
        assert_eq!(h.sel(0), (2, 8), "then the next visible char");
        h.key(Key::ArrowLeft, Modifiers::SHIFT);
        h.key(Key::ArrowLeft, Modifiers::SHIFT);
        assert_eq!(h.sel(0), (2, 2), "back to a caret beside the formula");
    }

    #[test]
    fn shift_arrow_skips_collapsed_markers() {
        let mut h = H::new(&["**ab**c"]);
        crate::wysiwyg::focus_end(&h.ctx, h.ids[0], 2);
        h.run(4);
        h.key(Key::ArrowRight, Modifiers::SHIFT);
        let (a, c) = h.sel(0);
        assert_eq!((a, c), (2, 3), "one visible letter");
    }

    #[test]
    fn a_key_typed_right_after_a_focus_move_goes_to_the_field_that_took_the_focus() {
        // (Tab in an answer row: the form calls focus_end and the next key arrives in the very next frame)
        let mut h = H::new(&["stem", "ans"]);
        h.focus(0);
        crate::wysiwyg::focus_end(&h.ctx, h.ids[1], 3);
        h.frame(vec![Event::Text("x".into())]);
        h.run(3);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem", "ansx"));
    }

    #[test]
    fn a_divider_typed_in_stays_above_the_text_typed_after_it() {
        let mut h = H::new(&[""]);
        h.focus(0);
        h.text("top");
        h.key(Key::Enter, Modifiers::NONE);
        h.key(Key::Enter, Modifiers::NONE);
        h.text("---");
        h.key(Key::Enter, Modifiers::NONE);
        h.text("after");
        let s = &h.stems[0];
        assert!(s.find("---").unwrap() < s.find("after").unwrap(), "{s:?}");
        assert!(s.starts_with("top"));
    }

    #[test]
    fn copy_keeps_a_whole_field_selection_in_every_shape_of_stem() {
        for stem in ["top\n\nbot", "top\nbot", "- a\n- b", "1. a\n2. b", "intro\n- a\n- b", "- a\n- b\n\ntail", "**b** and $x$\n\nsecond"] {
            for variant in 0..3 {
                let mut h = H::new(&[stem]);
                h.focus(0);
                h.key(Key::A, Modifiers::COMMAND);
                let copy = match variant {
                    0 => vec![kev(Key::C, Modifiers::COMMAND), Event::Copy],
                    1 => vec![Event::Copy],
                    _ => vec![kev(Key::C, Modifiers::COMMAND)],
                };
                h.frame(copy);
                h.run(3);
                h.frame(vec![tev("Q")]);
                h.run(4);
                assert_eq!(h.stems[0], "Q", "stem {stem:?} variant {variant}");
            }
        }
    }

    #[test]
    fn copy_keeps_the_selection_of_a_stem_typed_in_as_a_list() {
        for typed_with_gaps in [true, false] {
            let mut h = H::new(&[""]);
            h.focus(0);
            for e in [tev("- a"), kev(Key::Enter, Modifiers::NONE), tev("b")] {
                h.frame(vec![e]);
                h.run(if typed_with_gaps { 3 } else { 0 });
            }
            h.run(4);
            assert_eq!(h.stems[0], "- a\n- b");
            h.key(Key::A, Modifiers::COMMAND);
            h.frame(vec![kev(Key::C, Modifiers::COMMAND), Event::Copy]);
            h.run(3);
            h.frame(vec![tev("Q")]);
            h.run(4);
            assert_eq!(h.stems[0], "Q", "gaps {typed_with_gaps}");
        }
    }

    #[test]
    fn a_modifier_key_going_down_does_not_end_a_whole_field_selection() {
        // the browser sends Ctrl itself, then C, in one frame
        let mut h = H::new(&["- a\n- b"]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        h.frame(vec![kev(Key::ControlLeft, Modifiers::COMMAND), kev(Key::C, Modifiers::COMMAND), Event::Copy]);
        h.run(3);
        h.frame(vec![kev(Key::ShiftLeft, Modifiers::SHIFT), tev("Q")]);
        h.run(4);
        assert_eq!(h.stems[0], "Q");
        // and Shift + a letter in a plain stem is not split apart
        let mut h = H::new(&[""]);
        h.focus(0);
        h.frame(vec![kev(Key::ShiftLeft, Modifiers::SHIFT), kev(Key::A, Modifiers::SHIFT), tev("A"), tev("b")]);
        h.run(4);
        assert_eq!(h.stems[0], "Ab");
    }

    #[test]
    fn undoing_a_make_blank_takes_its_answer_row_with_it() {
        let mut h = H::new(&["the quick fox"]);
        h.rows = Some(Vec::new());
        h.focus(0);
        // select "fox" and Make Blank (Ctrl+Shift+K)
        h.frame(vec![kev(Key::End, Modifiers::NONE)]);
        h.run(2);
        for _ in 0..3 {
            h.frame(vec![kev(Key::ArrowLeft, Modifiers::SHIFT)]);
            h.run(2);
        }
        h.key(Key::K, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(h.stems[0], "the quick ___", "made a blank");
        assert_eq!(h.rows.as_deref(), Some(&["fox".to_string()][..]), "its answer is the word");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.stems[0], "the quick fox", "undo puts the word back");
        assert_eq!(h.rows.as_deref(), Some(&[][..] as &[String]), "and the blank's answer row goes");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(h.stems[0], "the quick ___", "redo");
        assert_eq!(h.rows.as_deref(), Some(&["fox".to_string()][..]), "redo brings its answer back");
    }

    #[test]
    fn undo_does_not_step_back_through_a_change_made_from_outside() {
        // a type switch rewrites the stem under the editor: its older history no longer applies
        let mut h = H::new(&[""]);
        h.focus(0);
        h.text("the quick ___");
        h.stems[0] = "the quick fox".to_string();
        h.run(3);
        h.key(Key::Z, Modifiers::COMMAND);
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.stems[0], "the quick fox", "nothing to undo across the outside change");
        // what is typed after it undoes back to it, in one step
        h.focus(0);
        h.text("!");
        assert_eq!(h.stems[0], "the quick fox!");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.stems[0], "the quick fox");
    }

    #[test]
    fn typed_characters_with_their_key_events_stay_in_one_frame() {
        // the browser sends a key event and a text event per character: a burst of them is one
        // run of text, applied in one frame (not two frames per character)
        let mut h = H::new(&[""]);
        h.focus(0);
        let mut ev = Vec::new();
        for c in "hello".chars() {
            let k = Key::from_name(&c.to_string().to_uppercase()).unwrap();
            ev.push(kev(k, Modifiers::NONE));
            ev.push(tev(&c.to_string()));
        }
        h.frame(ev);
        assert_eq!(h.stems[0], "hello", "all five characters in the one frame");
    }

    #[test]
    fn control_characters_are_not_typed_or_pasted() {
        let mut h = H::new(&["ab"]);
        h.focus(0);
        h.text("\u{7}");
        h.text("x\u{1}y");
        h.frame(vec![Event::Paste("p\u{2}q\nr".into())]);
        h.run(3);
        assert_eq!(h.stems[0], "abxypq\nr");
        assert!(crate::wysiwyg::check(&h.stems[0]).is_ok(), "the field never falls back to the raw view");
    }

    #[test]
    fn copy_keeps_the_whole_field_selected() {
        let mut h = H::new(&[LIST]);
        h.focus(0);
        h.key(Key::A, Modifiers::COMMAND);
        h.frame(vec![Event::Key { key: Key::C, physical_key: Some(Key::C), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }, Event::Copy]);
        h.run(3);
        h.text("Z");
        assert_eq!(h.stems[0], "Z", "typing replaced the selection after a copy");
    }

    #[test]
    fn a_new_session_drops_every_undo_step_of_the_old_one() {
        let mut h = H::new(&["stem", "ans"]);
        h.focus(0);
        h.text("1");
        h.focus(1);
        h.text("2");
        crate::wysiwyg::new_session(&h.ctx);
        h.stems = vec!["fresh".into(), "new".into()];
        h.run(3);
        h.key(Key::Z, Modifiers::COMMAND);
        h.key(Key::Z, Modifiers::COMMAND);
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("fresh", "new"), "nothing of the old session came back");
    }

    #[test]
    fn undo_walks_back_through_two_fields_in_typing_order() {
        let mut h = H::new(&["stem", "ans"]);
        h.focus(0);
        h.text("1");
        h.focus(1);
        h.text("2");
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem1", "ans2"));
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem1", "ans"), "the answer first");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem", "ans"), "then the stem, though the answer had the focus");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem1", "ans"), "redo goes forward in the same order");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!((h.stems[0].as_str(), h.stems[1].as_str()), ("stem1", "ans2"));
    }
}
