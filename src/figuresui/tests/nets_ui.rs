//! The nets figure's cube-net picker, driven through the workspace as a teacher uses it: each of the 11 nets is a
//! picture to click, and the click draws that net.
use super::last_b11::{panel_widgets, press_at};
use super::*;
use crate::figures::nets;

/// The Measures card has a row per measure, each with its own label button (the reference's "Label at ..." popover).
#[test]
fn the_measures_card_names_each_row_and_its_label() {
    let (ctx, mut ws) = open_tall(nets::ID);
    let w = panel_widgets(&ctx, &mut ws);
    let names: Vec<&str> = w.iter().map(|x| x.1.as_str()).collect();
    for want in ["Length", "Width", "Height", "5 cm", "3 cm", "4 cm", "3D shape"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // a worked-out measure shows its value in its row, faintly, whatever its label
    let mut q = ws.params.clone();
    q.set_s("shape", "Cylinder");
    ws.set_params_with_history(q);
    let w = panel_widgets(&ctx, &mut ws);
    assert!(w.iter().any(|x| x.1 == "18.8 cm"), "{:?}", w.iter().map(|x| &x.1).collect::<Vec<_>>());
}

/// cap-02: with Cube chosen, the Shape section shows the 11 nets as pictures named Net 1 to Net 11; clicking one
/// draws it, as one undo step.
#[test]
fn the_net_card_picks_any_of_the_eleven_cube_nets() {
    let (ctx, mut ws) = open_tall(nets::ID);
    assert_eq!(nets::ID, "solid-net");
    let mut q = ws.params.clone();
    q.set_s("shape", "Cube");
    ws.set_params_with_history(q);
    // the Shape section opens on its own, as the reference's Shape card is always shown
    let w = panel_widgets(&ctx, &mut ws);
    let nets: Vec<_> = w.iter().filter(|x| x.1.starts_with("Net ") && x.1[4..].parse::<u32>().is_ok()).collect();
    assert_eq!(nets.len(), 11, "{:?}", w.iter().map(|x| &x.1).collect::<Vec<_>>());
    let mut seen = std::collections::BTreeSet::new();
    for k in [6, 11, 2, 1] {
        let w = panel_widgets(&ctx, &mut ws);
        let before = ws.output().unwrap().svg.unwrap();
        let undo0 = ws.undo.len();
        let b = w.iter().find(|x| x.1 == format!("Net {k}")).unwrap_or_else(|| panic!("Net {k}"));
        press_at(&ctx, &mut ws, b.2.center());
        assert_eq!(ws.params.i("cube_net"), k);
        assert_eq!(ws.undo.len(), undo0 + 1, "one undo step");
        let after = ws.output().unwrap().svg.unwrap();
        assert_ne!(after, before, "net {k} is drawn");
        seen.insert(after);
    }
    assert_eq!(seen.len(), 4);
    // a prism has no net picker
    let mut q = ws.params.clone();
    q.set_s("shape", "Prism");
    ws.set_params_with_history(q);
    let w = panel_widgets(&ctx, &mut ws);
    assert!(!w.iter().any(|x| x.1 == "Net 1"));
}
