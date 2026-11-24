//! The Share menus' "Open from link" paste box: clicking into it must keep the menu open so a
//! link can be pasted (egui's default menu behaviour closes a popup on any click inside it).
use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;

type H<'a> = egui_kittest::Harness<'a, Workspace>;

fn workspace() -> Workspace {
    let (i, _) = find_def("demo-shapes").expect("demo registered under cfg(test)");
    let mut ws = Workspace::new(false);
    ws.select(i);
    ws
}

fn menu_open(h: &H) -> bool {
    h.query_by_label_contains("Open from link").is_some()
}

/// The toolbar's Share icon: a right-click opens the menu with the paste box.
fn toolbar<'a>() -> H<'a> {
    ui_test::harness(
        |ui, ws: &mut Workspace| {
            ws.chem_toolbar(ui);
        },
        workspace(),
    )
}

/// The panel's "Share link" menu button (a click opens it).
fn panel<'a>() -> H<'a> {
    ui_test::harness(
        |ui, ws: &mut Workspace| {
            ui.horizontal(|ui| ws.history_and_presets_ui(ui, false));
        },
        workspace(),
    )
}

fn open_toolbar_menu(h: &mut H) {
    h.run();
    assert!(!menu_open(h), "menu starts closed");
    h.get_by_label("Share link").click_secondary();
    h.run();
    assert!(menu_open(h), "right-click opens the Share menu");
}

fn open_panel_menu(h: &mut H) {
    h.run();
    h.get_by_label_contains("Share link").click();
    h.run();
    assert!(menu_open(h), "clicking Share link opens its menu");
}

/// Click into the paste box, type a link, press Enter: the figure reopens and the menu closes.
fn paste_flow(h: &mut H) {
    h.get_by_role(Role::TextInput).click();
    h.run();
    assert!(menu_open(h), "clicking the paste field must not close the menu");
    assert!(h.get_by_role(Role::TextInput).is_focused(), "the paste field takes focus");

    let link = h.state().share_link().unwrap();
    h.get_by_role(Role::TextInput).type_text(&link);
    h.run();
    assert!(menu_open(h), "typing keeps the menu open");
    assert_eq!(h.state().link_text, link, "the pasted link reaches the field");

    h.key_press(egui::Key::Enter);
    h.run();
    h.run();
    assert!(h.state().link_text.is_empty(), "Enter opens the link and clears the box");
    assert!(h.state().link_err.is_none());
    assert!(!menu_open(h), "opening the link closes the menu, as before");
}

#[test]
fn toolbar_share_menu_paste_field_stays_open_for_pasting() {
    let mut h = toolbar();
    open_toolbar_menu(&mut h);
    paste_flow(&mut h);
}

#[test]
fn toolbar_share_menu_still_closes_on_outside_click_and_escape() {
    let mut h = toolbar();
    open_toolbar_menu(&mut h);
    h.get_by_role(Role::TextInput).click();
    h.run();
    assert!(menu_open(&h));
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!menu_open(&h), "Escape dismisses the menu");

    open_toolbar_menu(&mut h);
    // a click on empty space well below the menu (it opens at the pointer, by the Share icon)
    let away = egui::pos2(20.0, 500.0);
    h.hover_at(away);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: away, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    h.run();
    assert!(!menu_open(&h), "a click outside dismisses the menu");
}

#[test]
fn panel_share_menu_paste_field_stays_open_for_pasting() {
    let mut h = panel();
    open_panel_menu(&mut h);
    paste_flow(&mut h);
}

/// The Presets menu has the same two text fields (preset name, pasted spec) and the same bug.
#[test]
fn panel_presets_menu_text_fields_stay_open_for_typing() {
    let mut h = panel();
    h.run();
    h.get_by_label_contains("Presets").click();
    h.run();
    let open = |h: &H| h.query_by_label_contains("Saved presets stay in this browser").is_some();
    assert!(open(&h), "clicking Presets opens its menu");
    let fields: Vec<_> = h.get_all_by_role(Role::TextInput).collect();
    assert_eq!(fields.len(), 2, "preset name and pasted spec");
    fields[0].click();
    h.run();
    assert!(open(&h), "clicking the preset name field must not close the menu");
    h.get_all_by_role(Role::TextInput).next().unwrap().type_text("mine");
    h.run();
    assert!(open(&h));
    assert_eq!(h.state().preset_name, "mine");
}
