//! Batch 11 (track "last"): the free-body-diagram's add buttons in its Forces card, the pedigree's click on a person
//! then a change in the person panel, and the Classic setups of the pedigree and the punnett-square, all driven
//! through the workspace as a teacher uses them (the side panel and the preview, by pointer events).
use super::*;
use crate::figures::biology::pedigree as ped;

/// Side-panel and popup widgets as a screen reader finds them: (role, label, rect, disabled), top to bottom.
fn panel_widgets(ctx: &egui::Context, ws: &mut Workspace) -> Vec<(egui::accesskit::Role, String, egui::Rect, bool)> {
    let out = tall_frame(ctx, ws, vec![]);
    let tree = out.platform_output.accesskit_update.expect("accesskit tree");
    let mut v: Vec<_> = tree
        .nodes
        .iter()
        .filter_map(|(_, nd)| {
            let b = nd.bounds()?;
            let r = egui::Rect::from_min_max(egui::pos2(b.x0 as f32, b.y0 as f32), egui::pos2(b.x1 as f32, b.y1 as f32));
            Some((nd.role(), nd.label().map(|s| s.to_string()).or_else(|| nd.value().map(|s| s.to_string())).unwrap_or_default(), r, nd.is_disabled()))
        })
        .filter(|w| w.2.min.x < 700.0)
        .collect();
    v.sort_by(|a, b| a.2.min.y.total_cmp(&b.2.min.y));
    v
}

fn press_at(ctx: &egui::Context, ws: &mut Workspace, p: egui::Pos2) {
    tall_frame(ctx, ws, vec![egui::Event::PointerMoved(p)]);
    let mut ev = btn(p, true);
    ev.extend(btn(p, false));
    tall_frame(ctx, ws, ev);
    tall_frame(ctx, ws, vec![]);
}

/// Clicks the enabled panel button whose words (an icon glyph and a "+ " before them aside) are `words`.
fn press_button(ctx: &egui::Context, ws: &mut Workspace, words: &str) {
    let ws_list = panel_widgets(ctx, ws);
    let hit = ws_list
        .iter()
        .filter(|w| !w.3 && w.0 != egui::accesskit::Role::Label && w.0 != egui::accesskit::Role::TextRun)
        .find(|w| w.1.trim_start_matches(|c: char| !c.is_alphanumeric()).trim() == words)
        .unwrap_or_else(|| panic!("no button {words:?} in {:?}", ws_list.iter().map(|w| &w.1).collect::<Vec<_>>()));
    press_at(ctx, ws, hit.2.center());
}

/// Picks `setup` from the Classic setups list: opens the list, scrolls it so the setup is at its top (the list is
/// shorter than its six setups), then clicks it.
fn pick_classic(ctx: &egui::Context, ws: &mut Workspace, setup: &str) {
    let combo = panel_widgets(ctx, ws).into_iter().find(|w| w.0 == egui::accesskit::Role::ComboBox).expect("the Classic setups list").2;
    press_at(ctx, ws, combo.center());
    let items = |ctx: &egui::Context, ws: &mut Workspace| panel_widgets(ctx, ws).into_iter().filter(|w| w.0 == egui::accesskit::Role::Button && w.2.min.y > combo.max.y - 1.0 && w.2.min.x > combo.min.x && w.2.max.x < combo.max.x).collect::<Vec<_>>();
    let list = items(ctx, ws);
    let first = list.first().expect("the open list").2;
    let item = list.iter().find(|w| w.1 == setup).unwrap_or_else(|| panic!("{setup} in the open list")).2;
    let dy = item.center().y - first.center().y;
    if dy > 0.0 {
        tall_frame(ctx, ws, vec![egui::Event::PointerMoved(first.center())]);
        tall_frame(ctx, ws, vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -dy), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }]);
        for _ in 0..12 {
            tall_frame(ctx, ws, vec![]);
        }
    }
    let item = items(ctx, ws).into_iter().find(|w| w.1 == setup).unwrap().2;
    press_at(ctx, ws, item.center());
    assert!(ws.undo.len() > 0, "{setup} picked");
}

/// cap-02e: the Forces card's buttons each add a force pointing the usual way, labelled, as one undo step; friction
/// and air resistance take their way from the velocity drawn.
#[test]
fn free_body_add_buttons_add_a_labelled_force_pointing_the_usual_way() {
    let (ctx, mut ws) = open_tall("free-body-diagram");
    open_section(&ctx, &mut ws, "Forces");
    let start = ws.params.clone();
    let open_svg = ws.output().unwrap().svg.unwrap();
    let n0 = start.i("n");
    assert_eq!(n0, 2, "the opening state: gravity and the normal force");
    let want = [
        ("Gravity", 270.0, "F_g"),
        ("Normal", 90.0, "F_N"),
        ("Friction", 180.0, "F_f"),
        ("Tension", 90.0, "F_T"),
        ("Applied", 0.0, "F_app"),
        ("Spring", 0.0, "F_s"),
        ("Air resistance", 180.0, "F_air"),
    ];
    for (name, ang, lab) in want {
        let undo0 = ws.undo.len();
        press_button(&ctx, &mut ws, name);
        let k = n0 + 1;
        assert_eq!(ws.params.i("n"), k, "{name}: one more force");
        assert_eq!(ws.params.f(&format!("f{k}_ang")), ang, "{name}: its direction");
        assert_eq!(ws.params.s(&format!("f{k}_lab")), lab, "{name}: its label");
        assert_eq!(ws.params.s(&format!("f{k}_lab_mode")), "Text", "{name}: written");
        assert_eq!(ws.undo.len(), undo0 + 1, "{name}: one undo step");
        let svg = ws.output().unwrap().svg.unwrap();
        assert_ne!(svg, open_svg, "{name}: drawn");
        ws.step_history(true);
        assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", start.0), "{name}: undone");
    }
    // moving left: friction to the right, air resistance against the velocity
    let mut p = start.clone();
    p.set_s("motion", "Velocity");
    p.set_f("mdir", 180.0);
    ws.set_params_with_history(p);
    press_button(&ctx, &mut ws, "Friction");
    assert_eq!(ws.params.f("f3_ang"), 0.0, "friction against a body moving left");
    press_button(&ctx, &mut ws, "Air resistance");
    assert_eq!(ws.params.f("f4_ang"), 0.0, "air resistance against a body moving left");
    // falling: air resistance up
    let mut p = start.clone();
    p.set_s("motion", "Velocity");
    p.set_f("mdir", 270.0);
    ws.set_params_with_history(p);
    press_button(&ctx, &mut ws, "Air resistance");
    assert_eq!(ws.params.f("f3_ang"), 90.0, "air resistance on a falling body points up");
}

/// cap-04h: a click on a person in the pedigree (on the preview, where the workspace draws its own click targets)
/// selects them and opens their panel, and the panel changes them, as one undo step drawn in the figure; the click
/// edits nothing by itself. The workspace keeps the selection, not the figure's params, so this is not a figure handle.
#[test]
fn pedigree_click_a_person_then_change_them() {
    let (ctx, mut ws) = open_tall("pedigree");
    for _ in 0..2 {
        tall_frame(&ctx, &mut ws, vec![]);
    }
    let before = ws.output().unwrap().svg.unwrap();
    let (_, _, whos) = ped::people_of(&ws.params);
    // someone unaffected in the second generation
    let target = whos.iter().position(|w| w.gi == 1 && w.status == 0).expect("an unaffected person in generation II");
    let (o, k) = ws.preview_map.expect("the preview is drawn");
    let at = egui::pos2(o.x + (whos[target].x * k) as f32, o.y + (whos[target].y * k) as f32);
    let start = ws.params.clone();
    press_at(&ctx, &mut ws, at);
    assert_eq!(ws.ped_sel, Some(target), "the click selects the person");
    assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", start.0), "selecting edits nothing");
    press_button(&ctx, &mut ws, "Affected");
    let fam = ped::family_of(&ws.params);
    assert_eq!(fam.person(target).unwrap().status, 2, "the person is changed to affected");
    assert_eq!(ws.ped_sel, Some(target), "and stays selected");
    let after = ws.output().unwrap().svg.unwrap();
    assert_ne!(after, before, "the change is drawn");
    assert_eq!(ped::people_of(&ws.params).2[target].status, 2, "drawn affected");
    ws.step_history(true);
    assert_eq!(format!("{:?}", ws.params.0), format!("{:?}", start.0), "one undo step");
}

/// cap-03: a classic family loads from the Classic setups list, then a click on a person in it changes them like any
/// family (the loaded family stays editable).
#[test]
fn pedigree_classic_family_loads_and_stays_editable() {
    let (ctx, mut ws) = open_tall("pedigree");
    pick_classic(&ctx, &mut ws, "Hemophilia, royal family");
    assert_eq!(ws.params.s("inherit"), "X-linked recessive");
    let classic = ped::classics().into_iter().next().unwrap();
    let fam_text = classic.params.iter().find(|(k, _)| *k == "family").map(|(_, v)| match v { figures::Value::S(t) => t.clone(), _ => String::new() }).unwrap();
    assert_eq!(ws.params.s("family"), fam_text, "the royal family is loaded");
    assert!(ped::hand_changed(&ws.params), "as a family a teacher changes by hand");
    let svg = ws.output().unwrap().svg.unwrap();
    assert!(svg.contains("Hemophilia in Queen Victoria"), "its title");
    // click Victoria's son Edward (generation II, first) and make him affected
    let (_, _, whos) = ped::people_of(&ws.params);
    let edward = whos.iter().position(|w| w.gi == 1 && w.num == 1).unwrap();
    let (o, k) = ws.preview_map.expect("preview");
    press_at(&ctx, &mut ws, egui::pos2(o.x + (whos[edward].x * k) as f32, o.y + (whos[edward].y * k) as f32));
    assert_eq!(ws.ped_sel, Some(edward));
    press_button(&ctx, &mut ws, "Child");
    assert_eq!(ped::family_of(&ws.params).len(), ped::Family::parse(&fam_text).unwrap().len() + 2, "a partner and a child added to the loaded family");
}

/// cap-02b: a Punnett classic cross loads from the same list and its cross stays editable in the panel.
#[test]
fn punnett_classic_cross_loads_from_the_list() {
    let (ctx, mut ws) = open_tall("punnett-square");
    pick_classic(&ctx, &mut ws, "ABO blood type");
    assert_eq!(ws.params.s("parents"), "I^A i x I^B i");
    assert_eq!(ws.params.s("dominance"), "Codominance");
    let svg = ws.output().unwrap().svg.unwrap();
    assert!(svg.contains("type AB"), "the blood types drawn");
    press_button(&ctx, &mut ws, "Complete");
    assert_eq!(ws.params.s("dominance"), "Complete");
    assert!(ws.output().unwrap().svg.unwrap().contains("type AB"), "I^A I^B stays type AB under complete dominance");
}
