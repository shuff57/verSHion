//! Batch 9 in the workspace: population growth's Classic setups dropdown loads each of the ten setups, whose values
//! then stay editable; the new choices (axis ends, label modes, a blank lane) are in the panel where they act.
use super::*;

/// The side panel's widgets whose accessible label or value is `text` (combo boxes show their choice as the value).
fn named_or_valued(ctx: &egui::Context, ws: &mut Workspace, text: &str) -> Vec<egui::accesskit::Rect> {
    let out = tall_frame(ctx, ws, vec![]);
    let tree = out.platform_output.accesskit_update.expect("accesskit tree");
    let mut v: Vec<_> = tree.nodes.iter().filter(|(_, nd)| nd.role() != egui::accesskit::Role::Label && (nd.label().is_some_and(|l| l == text) || nd.value().is_some_and(|l| l == text))).filter_map(|(_, nd)| nd.bounds()).filter(|b| b.x0 < 700.0).collect();
    v.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    v
}

fn click_at(ctx: &egui::Context, ws: &mut Workspace, b: egui::accesskit::Rect) {
    let p = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
    tall_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
    let mut ev = btn(p, true);
    ev.extend(btn(p, false));
    tall_frame(ctx, ws, ev);
    tall_frame(ctx, ws, vec![]);
}

/// Population growth: the Classic setups dropdown (opening on "Choose one…") loads each of the reference's ten
/// setups with one pick, as one undo step; the figure draws it, the dropdown names it, and its values stay editable
/// (an edit keeps the rest of the setup and turns the dropdown back to "Choose one…").
#[test]
fn growth_classic_setups_load_from_the_dropdown_and_stay_editable() {
    let setups = figures::classic_setups("population-growth");
    assert_eq!(setups.len(), 10);
    let names: Vec<&str> = setups.iter().map(|e| e.name).collect();
    for want in ["Yeast in a flask", "Deer on an island", "Bacteria doubling every hour", "Exponential and logistic growth", "Per capita growth rate against N",
        "Growth rate and per capita rate", "Population size and growth rate over time", "Overshoot and oscillation around K", "Boom and crash", "Blank graph for students to draw on"] {
        assert!(names.contains(&want), "{want}");
    }
    for (i, e) in setups.iter().enumerate() {
        let (ctx, mut ws) = open_tall("population-growth");
        let def = &reg()[ws.sel.unwrap()];
        let opening = (def.render)(&ws.params);
        let start = ws.params.clone();
        let combo = named_or_valued(&ctx, &mut ws, "Choose one\u{2026}");
        assert_eq!(combo.len(), 1, "the dropdown opens on Choose one…");
        // the setup's entry in the opened list (the panel's own Show choice can carry the same words)
        let before = named_or_valued(&ctx, &mut ws, e.name);
        click_at(&ctx, &mut ws, combo[0]);
        let item: Vec<_> = named_or_valued(&ctx, &mut ws, e.name).into_iter().filter(|b| !before.contains(b)).collect();
        assert_eq!(item.len(), 1, "{} listed once in the dropdown", e.name);
        // a long list scrolls inside its popup: wheel down over the first entry until this one is in view
        let first = named_or_valued(&ctx, &mut ws, setups[0].name).into_iter().find(|b| !before.contains(b) || i == 0).unwrap();
        let over = egui::pos2(((first.x0 + first.x1) / 2.0) as f32, ((first.y0 + first.y1) / 2.0) as f32);
        let mut at = item[0];
        for _ in 0..i {
            tall_frame(&ctx, &mut ws, vec![egui::Event::PointerMoved(over)]);
            tall_frame(&ctx, &mut ws, vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, -1.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }]);
            for _ in 0..3 { tall_frame(&ctx, &mut ws, vec![]); }
            at = named_or_valued(&ctx, &mut ws, e.name).into_iter().filter(|b| !before.contains(b)).next().unwrap();
        }
        click_at(&ctx, &mut ws, at);
        // every value the setup gives is loaded, and the figure draws it
        let want = example_params(def, e);
        for (k, _) in &e.params { assert_eq!(ws.params.0.get(*k), want.0.get(*k), "{}: {k}", e.name); }
        assert_ne!((def.render)(&ws.params), opening, "{}: the figure changes", e.name);
        assert!(named_or_valued(&ctx, &mut ws, "Choose one\u{2026}").is_empty() && named_or_valued(&ctx, &mut ws, e.name).len() > before.len(), "{}: the dropdown names the setup", e.name);
        // editable: a changed growth rate (K for the blank graph, which has no curve) keeps the rest of the setup
        let key = if e.params.iter().any(|(k, v)| *k == "draw_curve" && *v == Value::B(false)) { "k" } else { "r" };
        let loaded = ws.params.clone();
        let mut p = ws.params.clone();
        p.set_f(key, loaded.f(key) * 1.5);
        ws.set_params_with_history(p);
        tall_frame(&ctx, &mut ws, vec![]);
        for (k, v) in &loaded.0 { if k != key { assert_eq!(ws.params.0.get(k), Some(v), "{}: {k} kept", e.name); } }
        assert_eq!(named_or_valued(&ctx, &mut ws, "Choose one\u{2026}").len(), 1, "{}: edited away from the setup", e.name);
        assert_ne!((def.render)(&ws.params), (def.render)(&loaded), "{}: the edit draws", e.name);
        // undo steps back to the setup, then to the opening state
        ws.step_history(true);
        assert_eq!(ws.params.f(key), loaded.f(key));
        ws.step_history(true);
        assert_eq!(ws.params.0, start.0, "{} ({i}): one undo step", e.name);
    }
}

/// Gel electrophoresis: each sample lane's card offers "Leave blank for students" (a ladder's does not); ticking
/// lane 3's keeps its bands on the key (outlined) and empties it on the student copy; it follows the lane when moved.
#[test]
fn gel_lane_cards_leave_a_sample_lane_blank_for_students() {
    let (ctx, mut ws) = open_tall("gel-electrophoresis");
    let def = &reg()[ws.sel.unwrap()];
    let samples = figures::biology::gel::lane_cards(&ws.params).iter().filter(|l| matches!(l.kind, figures::biology::gel::LaneKind::Sample(_))).count();
    assert_eq!(samples, 4);
    assert_eq!(count_named(&ctx, &mut ws, "Leave blank for students"), samples, "one per sample lane, none for the ladder");
    let key_before = (def.render)(&ws.params);
    click_nth(&ctx, &mut ws, "Leave blank for students", 1); // the second sample lane: lane 3, Suspect 1
    assert_eq!(ws.params.s("blank_lanes"), "3");
    assert!(figures::biology::gel::lane_cards(&ws.params)[2].blank);
    let key = (def.render)(&ws.params);
    assert_ne!(key, key_before, "the key marks the lane");
    let mut student = ws.params.clone();
    student.set_b(figures::SHOW_ANSWERS, false);
    assert_ne!((def.render)(&student), key, "the student copy empties it");
    // the Show answers switch is in the panel (the student copy), as on the figures that hide answers
    assert!(figures::hides_answers(&(def.params)()));
    // moved left with its card's arrow, the flag goes with it
    click_nth(&ctx, &mut ws, egui_phosphor::regular::ARROW_LEFT, 2); // lane 3's (lane 1's is there, greyed out)
    assert_eq!(ws.params.s("blank_lanes"), "2");
    assert_eq!(figures::biology::gel::lane_cards(&ws.params)[1].label, "Suspect 1");
    // and unticked
    click_nth(&ctx, &mut ws, "Leave blank for students", 0);
    assert_eq!(ws.params.s("blank_lanes"), "");
}

/// Every batch 9 control is in its figure's panel, in the section the reference puts it, and the label modes and
/// axis ends only where they draw something.
#[test]
fn batch9_controls_are_in_their_sections() {
    let seen = |ctx: &egui::Context, ws: &mut Workspace, t: &str| count_named(ctx, ws, t) + count_text(ctx, ws, t);
    // box plot: Left end and Right end under Number line
    let (ctx, mut ws) = open_tall("box-plot");
    open_section(&ctx, &mut ws, "Number line");
    assert!(seen(&ctx, &mut ws, "Left end") >= 1 && seen(&ctx, &mut ws, "Right end") >= 1, "box plot ends");
    // waves: the x-axis's and y-axis's ends, gone with the axes
    let (ctx, mut ws) = open_tall("waves");
    open_section(&ctx, &mut ws, "x-axis");
    open_section(&ctx, &mut ws, "y-axis");
    for t in ["Left end", "Right end", "Bottom end", "Top end"] { assert!(seen(&ctx, &mut ws, t) >= 1, "waves {t}"); }
    let mut p = ws.params.clone();
    p.set_s("kind", "Longitudinal");
    ws.set_params_with_history(p);
    assert!(seen(&ctx, &mut ws, "Right end") >= 1 && seen(&ctx, &mut ws, "Top end") == 0, "a longitudinal wave has no y-axis ends");
    // projectile: each mark's label choice under Marks, once the mark is on
    let (ctx, mut ws) = open_tall("projectile-motion");
    open_section(&ctx, &mut ws, "Marks");
    assert_eq!(seen(&ctx, &mut ws, "Height label shows"), 0, "not before the height is marked");
    let mut p = ws.params.clone();
    p.set_b("mark_h", true);
    p.set_b("mark_r", true);
    ws.set_params_with_history(p);
    assert!(seen(&ctx, &mut ws, "Height label shows") >= 1 && seen(&ctx, &mut ws, "Range label shows") >= 1);
    assert_eq!(seen(&ctx, &mut ws, "Cliff height label shows"), 0, "no cliff on level ground");
    // population growth: the inflection point's and phases' label choices once they are marked
    let (ctx, mut ws) = open_tall("population-growth");
    assert_eq!(seen(&ctx, &mut ws, "Inflection point label"), 0);
    let mut p = ws.params.clone();
    p.set_b("mark_infl", true);
    p.set_b("mark_phases", true);
    ws.set_params_with_history(p);
    open_section(&ctx, &mut ws, "Marked on the graph");
    assert!(seen(&ctx, &mut ws, "Inflection point label") >= 1 && seen(&ctx, &mut ws, "Growth phase labels") >= 1);
    // predator-prey: Neither among the populations drawn
    let (ctx, mut ws) = open_tall("predator-prey");
    open_section(&ctx, &mut ws, "Graph");
    let defs = (reg()[ws.sel.unwrap()].params)();
    let show = defs.iter().find(|d| d.key == "show").unwrap();
    assert!(matches!(&show.kind, figures::Kind::Choice(o) if o.contains(&figures::biology::popdyn::NEITHER)));
    assert!(seen(&ctx, &mut ws, "Show") >= 1);
    // gel: Show answers under Chart title and answer key
    let (ctx, mut ws) = open_tall("gel-electrophoresis");
    open_section(&ctx, &mut ws, "Chart title and answer key");
    assert!(seen(&ctx, &mut ws, "Show answers") >= 1, "gel Show answers");
}
