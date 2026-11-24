//! Menus that hold a text field must stay open for a click inside them (egui's default menu
//! behaviour closes a popup on any click inside it, so the field never took focus). Same bug and
//! same fix as the Share menus in `share_menu_tests`.
//!
//! Audit (every `menu_button`, `context_menu` and `Popup::menu` in src/ whose body holds a text
//! field, drag value, slider or colour picker): the toolbar Share menu, the panel Share link and
//! Presets menus (fixed with `share_menu_tests`), the data-entry generator and the node-label
//! menu (already `CloseOnClickOutside`), and the composite-shape "Settings" menu (this file).
use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

type H<'a> = egui_kittest::Harness<'a, Workspace>;

fn composite<'a>() -> H<'a> {
    let (i, _) = find_def("composite-shape").expect("composite shape registered");
    let mut ws = Workspace::new(false);
    ws.select(i);
    ui_test::harness(
        |ui, ws: &mut Workspace| {
            ws.composite_canvas(ui);
        },
        ws,
    )
}

fn menu_open(h: &H) -> bool {
    h.query_by_label_contains("Round lengths to").is_some()
}

fn open_settings(h: &mut H) {
    h.run();
    assert!(!menu_open(h), "menu starts closed");
    h.get_by_label("Settings").click();
    h.run();
    assert!(menu_open(h), "clicking Settings opens its menu");
}

fn click_at(h: &mut H, pos: egui::Pos2) {
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    h.run();
}

#[test]
fn composite_settings_unit_field_stays_open_for_typing() {
    let mut h = composite();
    open_settings(&mut h);
    h.get_by_role(Role::TextInput).click();
    h.run();
    assert!(menu_open(&h), "clicking the Unit field must not close the menu");
    assert!(h.get_by_role(Role::TextInput).is_focused(), "the Unit field takes focus");

    h.get_by_role(Role::TextInput).type_text("ft");
    h.run();
    assert!(menu_open(&h), "typing keeps the menu open");
    assert_eq!(h.state().params.s("unit"), "ft", "the typed unit reaches the figure's parameters");
}

/// Items that closed the menu by default (the rounding and size buttons, the shade checkbox) still do.
#[test]
fn composite_settings_choices_still_close_the_menu() {
    for (label, key, want) in [("Tenths", "dec", 1), ("Hundredths", "angle_dec", 2)] {
        let mut h = composite();
        open_settings(&mut h);
        // "Tenths" and "Hundredths" appear once per rounding row; the first is lengths, the second angles
        let buttons: Vec<_> = h.get_all_by_label(label).collect();
        assert_eq!(buttons.len(), 2, "{label} is offered for lengths and for angles");
        buttons[if key == "dec" { 0 } else { 1 }].click();
        h.run();
        h.run();
        assert_eq!(h.state().params.i(key), want, "{label} sets {key}");
        assert!(!menu_open(&h), "choosing {label} closes the menu, as before");
    }

    let mut h = composite();
    open_settings(&mut h);
    let shade = h.state().params.b("shade");
    h.get_by_label("Shade the shape").click();
    h.run();
    h.run();
    assert_ne!(h.state().params.b("shade"), shade, "the checkbox toggles shading");
    assert!(!menu_open(&h), "toggling shade closes the menu, as before");

    let mut h = composite();
    open_settings(&mut h);
    // the three label-size buttons are each an "A" in its own size: Large, Medium, Small
    let sizes: Vec<_> = h.get_all_by_label("A").collect();
    assert_eq!(sizes.len(), 3, "Large, Medium and Small label buttons");
    sizes[2].click();
    h.run();
    h.run();
    assert_eq!(h.state().params.s("label_size"), "Small");
    assert!(!menu_open(&h), "choosing a label size closes the menu, as before");
}

#[test]
fn composite_settings_still_closes_on_escape_and_outside_click() {
    let mut h = composite();
    open_settings(&mut h);
    h.get_by_role(Role::TextInput).click();
    h.run();
    assert!(menu_open(&h));
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    assert!(!menu_open(&h), "Escape dismisses the menu");

    open_settings(&mut h);
    let screen = h.ctx.content_rect();
    click_at(&mut h, egui::pos2(screen.right() - 4.0, screen.bottom() - 4.0));
    assert!(!menu_open(&h), "a click outside dismisses the menu");
}
