//! Coordinate grid, batch 7, in the workspace: inline edits move nothing else, and every batch 7 control lands where the
//! panel shows it (the row style keys in each row's style menu, the axis ends and fraction switches in their axis section).
use super::*;

/// An inline rename of either axis label on a grid using every batch 7 control (rows styled, ends set, fraction ranges)
/// sets that label and nothing else: no count-by, range or style moves (batch 3 once snapped count by 1 to 1.05).
#[test]
fn an_inline_axis_rename_on_a_styled_grid_moves_nothing_else() {
    for key in ["xlabel", "ylabel"] {
        let (ctx, mut ws) = open_fig("coordinate-grid");
        for (k, v) in [("xmin", -3.5), ("xmax", 3.5), ("xstep", 0.5), ("ymin", -2.25), ("ymax", 2.25), ("ystep", 0.25)] { ws.params.set_f(k, v); }
        for (k, v) in [("equations", "y = arcsec(x); y = 2x + 1; x = 1; y = tan(x)"), ("eq1_line", "Dotted"), ("eq2_weight", "Thick"), ("eq3_arrows", "Both ends"), ("eq4_line", "Dashed"), ("x_left_end", "Circle"), ("x_right_end", "Line arrow"), ("y_top_end", "Circle"), ("y_bottom_end", "None")] { ws.params.set_s(k, v); }
        ws.params.set_b("x_fractions", true);
        ws.params.set_b("y_fractions", true);
        comp_frame(&ctx, &mut ws, vec![]);
        let before = ws.params.clone();
        ws.editing = Some((InlineEdit::key(key), "time (s)".to_string(), true));
        comp_frame(&ctx, &mut ws, vec![]);
        comp_frame(&ctx, &mut ws, vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
        comp_frame(&ctx, &mut ws, vec![]);
        assert_eq!(ws.params.s(key), "time (s)");
        for (k, v) in &before.0 {
            if k != key { assert_eq!(ws.params.0.get(k), Some(v), "{key}: {k} moved"); }
        }
    }
}

/// The panel: the axis ends and fraction switches are in their axis's section; the row style keys are in no section
/// (the Equations card's row menu owns them, as it owns the row colours); the menu offers them for rows 1 to 4 only.
#[test]
fn batch7_controls_are_where_the_panel_shows_them() {
    let (_, def) = find_def("coordinate-grid").unwrap();
    let pdefs = (def.params)();
    let secs = figures::layout::sections(def.id, &pdefs, None);
    let sec_of = |k: &str| secs.iter().find(|s| s.items.iter().any(|&i| pdefs[i].key == k)).map(|s| s.title.clone());
    for k in ["x_left_end", "x_right_end", "x_fractions"] { assert_eq!(sec_of(k).as_deref(), Some("x-axis"), "{k}"); }
    for k in ["y_bottom_end", "y_top_end", "y_fractions"] { assert_eq!(sec_of(k).as_deref(), Some("y-axis"), "{k}"); }
    for k in ["eq1_line", "eq2_weight", "eq3_arrows", "eq4_line", "eq1_color"] { assert_eq!(sec_of(k), None, "{k} must live in the row menu"); }
    for (i, c) in ["eq1_color", "eq2_color", "eq3_color", "eq4_color"].iter().enumerate() {
        let keys: Vec<&str> = figures::row_style_keys(c).iter().map(|k| k.0).collect();
        assert_eq!(keys, vec![["eq1_line", "eq2_line", "eq3_line", "eq4_line"][i], ["eq1_weight", "eq2_weight", "eq3_weight", "eq4_weight"][i], ["eq1_arrows", "eq2_arrows", "eq3_arrows", "eq4_arrows"][i]]);
    }
    // the number line's row colours (and every other figure's) get no such menu
    for c in ["ineq_color", "ineq2_color", "pt_color", ""] { assert!(figures::row_style_keys(c).is_empty()); }
    // each menu option draws a figure different from Automatic (a dead choice would draw the same), on a row whose
    // automatic look is none of them (typed dashed, the graph's lines Thin, no arrows)
    let open = figures::start_params(def);
    let mut p = open.clone();
    p.set_s("equations", "y = x^2 - 4 dashed");
    p.set_s("line_width", "Thin");
    let auto = def.svg(&p);
    for (k, _, opts) in figures::row_style_keys("eq1_color") {
        for o in opts.iter().filter(|o| !matches!(**o, "Automatic" | "None" | "Dashed" | "Thin")) {
            let mut q = p.clone();
            q.set_s(k, o);
            assert_ne!(def.svg(&q), auto, "{k} = {o} draws nothing new");
        }
    }
    // the opening grid and the workspace open with every batch 7 control at its old-look default
    let (_, ws) = open_fig("coordinate-grid");
    for k in ["eq1_line", "eq1_weight", "x_left_end", "y_top_end"] { assert_eq!(ws.params.s(k), "Automatic"); }
    assert_eq!(ws.params.s("eq1_arrows"), "None");
    assert!(!ws.params.b("x_fractions") && !ws.params.b("y_fractions"));
}
