use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use figures::chemistry::bond_energy::ID;

#[test]
fn length_box_edits_in_the_selected_unit() {
    let (_, def) = find_def(ID).unwrap();
    let pd = (def.params)()
        .into_iter()
        .find(|pd| pd.key == "length1")
        .unwrap();
    let mut p = figures::start_params(def);
    p.set_s("length_unit", "Å");
    let mut h = ui_test::harness(move |ui, p: &mut Params| param_control(ui, ID, &pd, p), p);
    h.run();
    assert!(h.query_by_label("Bond length").is_some());
    assert!(h.query_by_label("(Å)").is_some());
    assert_eq!(
        h.get_by_role(Role::TextInput).value().as_deref(),
        Some("1.5")
    );
    h.get_by_role(Role::TextInput).focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.get_by_role(Role::TextInput).type_text("1.2");
    h.run();
    assert_eq!(h.state().f("length1"), 120.0);
    assert_eq!(h.state().f("energy1"), 300.0);
}

#[test]
fn editor_share_and_print_pair_keep_the_two_wells() {
    let (i, def) = find_def(ID).unwrap();
    let mut ws = Workspace::new(false);
    ws.select(i);
    assert!(ws.use_example(0));
    ws.params.set_s("length_unit", "Å");
    ws.params.set_b("hide_names", true);
    ws.params.set_s("style", "Print");
    assert!(ws.has_answers());

    let mut reopened = Workspace::new(false);
    reopened.apply_link(&ws.share_link().unwrap()).unwrap();
    assert_eq!(reopened.sel, Some(i));
    assert_eq!(reopened.params.0, ws.params.0);
    let output = reopened.output().unwrap();
    let svg = output.svg.unwrap();
    assert_eq!(svg, def.svg(&ws.params));
    assert!(svg.contains("−400 kJ/mol") && svg.contains("−200 kJ/mol"));
    assert!(svg.contains("Bond A") && svg.contains("Bond B"));
    assert!(!svg.contains("Bond X") && !svg.contains("Bond Y"));
    assert!(output.png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let (student, key) = figures::answer_pair(def, &reopened.params).unwrap();
    assert!(student.contains("Bond A") && student.contains("Bond B"));
    assert!(!student.contains("Bond X") && !student.contains("Bond Y"));
    assert!(!student.contains("1.2 Å") && !student.contains("−400 kJ/mol"));
    for answer in [
        "Bond A = Bond X",
        "Bond B = Bond Y",
        "1.2 Å · 400 kJ/mol",
        "1.8 Å · 200 kJ/mol",
    ] {
        assert!(key.contains(answer), "missing {answer}");
    }
    for image in [&student, &key] {
        assert!(image.contains(r#"stroke-dasharray="6 3""#));
        figures::validate_svg(image).unwrap();
        assert!(
            figures::to_png(image, 2.0)
                .unwrap()
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
    }
}

#[test]
#[ignore = "uses the persistent preset card; run alone with a fresh VERSHION_STORE_DIR"]
fn named_preset_reopens_after_changing_units() {
    let scratch = std::path::PathBuf::from(
        crate::env_var("VERSHION_STORE_DIR").expect("set a scratch store"),
    );
    assert!(scratch.is_dir());
    assert!(
        !scratch.join("figure-presets.json").exists(),
        "start with a fresh store"
    );

    let (_, def) = find_def(ID).unwrap();
    let mut p = example_params(def, &figures::examples_for(ID)[0]);
    p.set_s("length_unit", "Å");
    let mut h = ui_test::harness(
        move |ui, p: &mut Params| {
            if let Some(restored) = preset_card(ui, def, p) {
                *p = restored;
            }
        },
        p.clone(),
    );
    h.run();
    h.get_by_label_contains("Save preset").click();
    h.run();
    h.get_by_role(Role::TextInput).focus();
    h.run();
    h.get_by_role(Role::TextInput).type_text("Two wells");
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert!(scratch.join("figure-presets.json").is_file());

    h.state_mut().set_s("length_unit", "nm");
    h.run();
    h.get_by_label("Two wells").click();
    h.run();
    assert_eq!(h.state().0, p.0);
}
