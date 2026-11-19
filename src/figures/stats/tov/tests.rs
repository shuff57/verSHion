use super::*;
use crate::figures::{Params, SHOW_ANSWERS};

fn base() -> Params { Params::defaults(&params()) }
fn with(kv: &[(&str, &str)]) -> Params {
    let mut p = base();
    for (k, v) in kv {
        p.set_s(k, v);
    }
    p
}
fn ys(p: &Params) -> Vec<String> { model(p).cols.iter().map(|c| c.y.inline()).collect() }
fn xs(p: &Params) -> Vec<String> { model(p).cols.iter().map(|c| c.x.inline()).collect() }

#[test]
fn the_opening_rule_is_worked_out() {
    let p = base();
    assert_eq!(xs(&p), ["\u{2212}2", "\u{2212}1", "0", "1", "2"]);
    assert_eq!(ys(&p), ["\u{2212}3", "\u{2212}1", "1", "3", "5"]);
    let svg = render(&p);
    assert!(svg.contains("\u{2212}3") && svg.contains(">5<"), "{svg}");
}

#[test]
fn fractions_stay_exact_and_in_lowest_terms() {
    let p = with(&[("rule", "y = x/2 + 1"), ("from", "0"), ("to", "2"), ("step", "1/2")]);
    assert_eq!(xs(&p), ["0", "1/2", "1", "3/2", "2"]);
    assert_eq!(ys(&p), ["1", "5/4", "3/2", "7/4", "2"]);
    let p = with(&[("rule", "y = 3(2)^x"), ("from", "-2"), ("to", "1")]);
    assert_eq!(ys(&p), ["3/4", "3/2", "3", "6"]);
    let p = with(&[("rule", "y = x^2 - 3"), ("xs", "A list"), ("list", "-1/3, 1 1/2")]);
    assert_eq!(xs(&p), ["\u{2212}1/3", "1 1/2"]);
    assert_eq!(ys(&p), ["\u{2212}26/9", "\u{2212}3/4"]);
    // decimals typed in the x-values or the rule keep the values in decimals
    let p = with(&[("rule", "y = x^2 - 3"), ("xs", "A list"), ("list", "-1/3, 0.5")]);
    assert_eq!(ys(&p), ["\u{2212}2.89", "\u{2212}2.75"]);
    let p = with(&[("rule", "y = 0.1x")]);
    assert_eq!(ys(&p), ["\u{2212}0.2", "\u{2212}0.1", "0", "0.1", "0.2"]);
}

#[test]
fn decimals_and_rounding() {
    let p = with(&[("rule", "y = 1/x"), ("from", "1"), ("to", "3"), ("as", "Decimals")]);
    assert_eq!(ys(&p), ["1", "0.5", "0.33"]);
    let p = with(&[("rule", "y = 1/x"), ("from", "1"), ("to", "3"), ("as", "Decimals"), ("round", "4 decimal places")]);
    assert_eq!(ys(&p)[2], "0.3333");
    // not a fraction: rounded even with Fractions chosen; whole numbers
    let p = with(&[("rule", "y = sqrt x"), ("xs", "A list"), ("list", "2, 4, 9/4"), ("round", "Whole numbers")]);
    assert_eq!(ys(&p), ["1", "2", "3/2"]);
    let p = with(&[("rule", "y = sqrt(x)"), ("xs", "A list"), ("list", "2"), ("round", "1 decimal place")]);
    assert_eq!(ys(&p), ["1.4"]);
    // a decimal step writes decimal x-values
    let p = with(&[("from", "0"), ("to", "1"), ("step", "0.5")]);
    assert_eq!(xs(&p), ["0", "0.5", "1"]);
}

#[test]
fn undefined_where_the_rule_has_no_value() {
    let p = with(&[("rule", "y = 1/x")]);
    assert_eq!(ys(&p), ["\u{2212}1/2", "\u{2212}1", "undefined", "1", "1/2"]);
    let p = with(&[("rule", "y = sqrt(x)"), ("from", "-1"), ("to", "1")]);
    assert_eq!(ys(&p)[0], "undefined");
    let p = with(&[("rule", "y = tan x"), ("from", "0°"), ("to", "90°"), ("step", "45°")]);
    assert_eq!(ys(&p), ["0", "1", "undefined"]);
}

#[test]
fn degrees_and_pi_are_written_as_typed_and_trig_is_exact_where_it_can_be() {
    let p = with(&[("rule", "y = sin x"), ("from", "0°"), ("to", "90°"), ("step", "30°")]);
    assert_eq!(xs(&p), ["0°", "30°", "60°", "90°"]);
    assert_eq!(ys(&p), ["0", "1/2", "0.87", "1"]);
    let p = with(&[("rule", "y = cos x"), ("from", "0"), ("to", "2pi"), ("step", "pi/2")]);
    assert_eq!(xs(&p), ["0", "\u{3c0}/2", "\u{3c0}", "3\u{3c0}/2", "2\u{3c0}"]);
    assert_eq!(ys(&p), ["1", "0", "\u{2212}1", "0", "1"]);
    let p = with(&[("rule", "y = x/2"), ("from", "0"), ("to", "\u{3c0}"), ("step", "\u{3c0}/2")]);
    assert_eq!(ys(&p), ["0", "\u{3c0}/4", "\u{3c0}/2"]);
}

#[test]
fn typed_pairs_take_words_and_empty_cells_are_blanks() {
    let p = with(&[("source", "Typed"), ("typed", "Mon, sunny\nTue\t3\n4,\n, 8")]);
    let m = model(&p);
    assert_eq!(m.cols.len(), 4);
    assert_eq!(m.cols[0].y.inline(), "sunny");
    assert_eq!(m.cols[1].y.inline(), "3");
    assert!(m.cols[2].y_blank && m.cols[2].y_empty);
    assert!(m.cols[3].x_blank && m.cols[3].x_empty);
}

#[test]
fn the_key_lists_each_blank_with_the_headings() {
    let p = with(&[("heads", "Input / Output"), ("blanks", "y4, x2")]);
    let a = answers(&model(&p));
    assert!(a.contains(&"Output when Input = 1: 3".to_string()), "{a:?}");
    assert!(a.contains(&"Input when Output = \u{2212}1: \u{2212}1".to_string()), "{a:?}");
    assert!(a.iter().any(|l| l == "Linear: the rate of change is 2."), "{a:?}");
}

#[test]
fn linear_or_not() {
    let a = answers(&model(&with(&[("rule", "y = x^2")])));
    assert!(a.iter().any(|l| l.starts_with("Not linear")), "{a:?}");
    let a = answers(&model(&with(&[("rule", "y = 2^x")])));
    assert!(a.iter().any(|l| l == "Not linear: each y is 2 times the one before."), "{a:?}");
    let a = answers(&model(&with(&[("rule", "y = x/2"), ("from", "0"), ("to", "1"), ("step", "1/4")])));
    assert!(a.iter().any(|l| l == "Linear: the rate of change is 1/2."), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "1, 5\n2, 8\n3, 11\n4,")])));
    assert!(a.iter().any(|l| l == "Linear: y = 3x + 2, a rate of change of 3."), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "0, 4\n1, 2\n2, 0")])));
    assert!(a.iter().any(|l| l == "Linear: y = \u{2212}2x + 4, a rate of change of \u{2212}2."), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "Mon, sunny\nTue, rain")])));
    assert!(a.iter().all(|l| !l.contains("inear")), "{a:?}");
}

#[test]
fn the_student_copy_hides_every_answer() {
    let mut p = with(&[("blanks", "y4, y5"), ("show_rule", "Blank line")]);
    let key = render(&p);
    assert!(key.contains(KEY_INK) && key.contains(">The rule: y = 2x + 1</text>"), "{key}");
    p.set_b(SHOW_ANSWERS, false);
    let student = render(&p);
    assert!(!student.contains(KEY_INK), "{student}");
    assert!(!student.contains("2x"), "the blanked rule leaks: {student}");
    assert!(!student.contains("inear"), "{student}");
    // the y's left blank are not drawn: 3 and 5 appear nowhere as cell text or in the description
    assert!(!student.contains(">3<") && !student.contains(">5<"), "{student}");
    assert!(!student.contains("y 3") && !student.contains("y 5"), "{student}");
    assert!(student.contains("y blank"), "{student}");
}

#[test]
fn every_y_blank_at_once() {
    let mut p = with(&[]);
    p.set_b("blank_y", true);
    p.set_b(SHOW_ANSWERS, false);
    let svg = render(&p);
    for v in ["\u{2212}3<", ">3<", ">5<"] {
        assert!(!svg.contains(v), "{v} {svg}");
    }
    assert!(svg.contains(">\u{2212}2<"), "the x-values stay");
}

#[test]
fn clicking_a_value_blanks_it_and_clicking_again_fills_it_in() {
    assert_eq!(ID, "table-of-values");
    let mut p = base();
    toggle(&mut p, 2, true, 5);
    assert_eq!(p.s("blanks"), "y3");
    toggle(&mut p, 0, false, 5);
    assert_eq!(p.s("blanks"), "x1, y3");
    toggle(&mut p, 2, true, 5);
    assert_eq!(p.s("blanks"), "x1");
    // with every y blank, clicking one y fills that one in and keeps the others blank
    let mut p = base();
    p.set_b("blank_y", true);
    toggle(&mut p, 1, true, 5);
    assert!(!p.b("blank_y"));
    assert_eq!(p.s("blanks"), "y1, y3, y4, y5");
    // a handle per value cell, with a tip saying what the click does
    let hs = handles(&base());
    assert_eq!(hs.len(), 10);
    assert!(hs.iter().any(|h| h.tip == "y for x = \u{2212}2: click to make a blank"), "{:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>());
    let mut q = base();
    let h = hs.iter().find(|h| h.tip.starts_with("y for x = 1")).unwrap();
    (h.click.as_ref().unwrap())(&mut q);
    assert_eq!(q.s("blanks"), "y4");
}

#[test]
fn runs_down_and_plain_headings_change_the_drawing() {
    let a = render(&base());
    let b = render(&with(&[("runs", "Down")]));
    let c = render(&with(&[("head_look", "Plain")]));
    assert_ne!(a, b);
    assert_ne!(a, c);
    let l = layout(&with(&[("runs", "Down")]), &model(&with(&[("runs", "Down")])));
    assert!(!l.across);
    assert!(l.cells[2].y > l.cells[0].y, "down: the next x is below");
}

#[test]
fn headings_and_titles() {
    let svg = render(&with(&[("heads", "n / aₙ")]));
    assert!(svg.contains(r#"baseline-shift="sub""#), "{svg}");
    let svg = render(&with(&[("heads", "Your own"), ("head1", "Figure"), ("head2", "")]));
    assert!(svg.contains(">Figure<"), "{svg}");
    let svg = render(&with(&[("title_kind", "Text"), ("title", "Savings")]));
    assert!(svg.contains(">Savings<"));
    let svg = render(&with(&[("show_rule", "Written"), ("rule", "y = x^2 - 3")]));
    // a power is raised by a share of the size (7.56 at 18): numeric shifts stack when nested
    // the power is its own element at 70% of the size (12.6 at 18)
    assert!(svg.contains(r#"font-size="12.60""#), "{svg}");
}

#[test]
fn odd_input_never_panics() {
    for (k, v) in [
        ("rule", "((((("),
        ("rule", "y = x^64"),
        ("rule", "y = 10^x^x^x"),
        ("rule", "y = sin"),
        ("rule", "="),
        ("rule", "y = zz"),
        ("from", "abc"),
        ("step", "0"),
        ("step", "-1"),
        ("step", "0.0000001"),
        ("list", ",,,"),
        ("blanks", "y0, q7, y999, x"),
    ] {
        for xs in XS {
            let p = with(&[(k, v), ("xs", xs)]);
            let _ = render(&p);
            let _ = warnings(&p);
            let _ = handles(&p);
        }
    }
    let deep = format!("y = {}x{}", "(".repeat(500), ")".repeat(500));
    let _ = render(&with(&[("rule", &deep)]));
    let p = with(&[("rule", "y = zz")]);
    assert!(warnings(&p).iter().any(|w| w.contains("Can't read the rule")), "{:?}", warnings(&p));
    let p = with(&[("step", "0.0000001")]);
    assert_eq!(model(&p).cols.len(), MAX_COLS);
}

#[test]
fn powers_read_the_way_they_are_written() {
    // a number and x straight after ^ are the whole exponent, as the rule is written above the table
    let p = with(&[("rule", "y = 2^2x"), ("from", "0"), ("to", "3")]);
    assert_eq!(ys(&p), ["1", "4", "16", "64"]);
    let p = with(&[("rule", "y = e^2x"), ("from", "0"), ("to", "2"), ("as", "Decimals")]);
    assert_eq!(ys(&p), ["1", "7.39", "54.60"]);
    assert!(markup("y = e^2x").contains("<sup1>2x</sup1>"), "{}", markup("y = e^2x"));
    assert!(!answers(&model(&p)).iter().any(|l| l.starts_with("Linear")));
    // a space ends the power
    let p = with(&[("rule", "y = x^2 x"), ("from", "2"), ("to", "2")]);
    assert_eq!(ys(&p), ["8"]);
    // a power after a bracketed argument applies to the function's value
    let p = with(&[("rule", "y = sin(x)^2"), ("from", "0"), ("to", "pi"), ("step", "pi/2")]);
    assert_eq!(ys(&p), ["0", "1", "0"]);
    let p = with(&[("rule", "y = sqrt(x)^2")]);
    assert_eq!(ys(&p), ["undefined", "undefined", "0", "1", "2"]);
    // sin x^2 is still the sine of x squared
    let p = with(&[("rule", "y = sin x^2"), ("xs", "A list"), ("list", "pi")]);
    assert_eq!(ys(&p), [round_str((std::f64::consts::PI * std::f64::consts::PI).sin(), 2)]);
    // the superscript is 2 in x^2sin(x), not 2s
    assert!(markup("y = x^2sin(x)").contains("x<sup1>2</sup1> sin(x)"), "{}", markup("y = x^2sin(x)"));
}

#[test]
fn a_blank_is_the_same_size_on_both_copies_whatever_its_answer() {
    let copies = |kv: &[(&str, &str)]| {
        let mut key = with(kv);
        key.set_b(SHOW_ANSWERS, true);
        let mut student = key.clone();
        student.set_b(SHOW_ANSWERS, false);
        (layout(&key, &model(&key)), layout(&student, &model(&student)))
    };
    // 1/x with the undefined y blank: that column is no wider than the others
    // the key copy may be wider (its answer lines), which only moves the centred table
    let sizes = |l: &Layout| l.cells.iter().map(|c| (c.w, c.h)).collect::<Vec<_>>();
    let (lk, ls) = copies(&[("rule", "y = 1/x"), ("blanks", "y3")]);
    assert_eq!(sizes(&lk), sizes(&ls));
    let widest_other = ls.cells.iter().filter(|c| c.col != 2).map(|c| c.w).fold(0.0, f64::max);
    assert!(ls.cells.iter().filter(|c| c.col == 2).all(|c| c.w <= widest_other + 1e-9), "{:?}", ls.cells);
    // the only fraction is the blank one: rows stay short on both copies
    let kv = [("rule", "y = x/2"), ("xs", "A list"), ("list", "2, 4, 1"), ("blanks", "y3")];
    let (lk, ls) = copies(&kv);
    assert_eq!(sizes(&lk), sizes(&ls));
    assert!(!ls.stacked && !lk.stacked);
    // and the key still writes the answer, inline in its box
    let svg = render(&with(&kv));
    assert!(svg.contains(">1/2<"), "{svg}");
}

#[test]
fn the_blank_rule_line_does_not_hint_at_the_rules_length() {
    let mut a = with(&[("show_rule", "Blank line")]);
    a.set_b(SHOW_ANSWERS, false);
    let mut b = a.clone();
    b.set_s("rule", "y = 3x^2 - 12x + 17 + sqrt(x + 4)");
    // the first line drawn is the rule's blank line: the same length for a short and a long rule
    let len = |svg: &str| {
        let el = svg.split("<line").nth(1).unwrap().split("/>").next().unwrap().to_string();
        let at = |k: &str| el.split(&format!("{k}=\"")).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
        at("x2") - at("x1")
    };
    assert!((len(&render(&a)) - len(&render(&b))).abs() < 1e-6);
}

#[test]
fn notes_for_a_step_that_misses_to_and_a_long_heading() {
    let p = with(&[("from", "0"), ("to", "5"), ("step", "2")]);
    assert!(warnings(&p).iter().any(|w| w == "The step does not land on To, so the last x-value is 4."), "{:?}", warnings(&p));
    assert!(warnings(&base()).is_empty(), "{:?}", warnings(&base()));
    let p = with(&[("heads", "Your own"), ("head1", "Number of weeks since the start of the school year in September")]);
    assert_eq!(model(&p).heads[0], "Number of weeks since the start of the school year in\u{2026}");
    assert!(warnings(&p).iter().any(|w| w.contains("heading is longer")));
}

#[test]
fn a_radical_is_its_own_tspan() {
    let svg = render(&with(&[("rule", "y = sqrt(x + 3)"), ("show_rule", "Written")]));
    // the radical is a text element of its own in a serif face; the rest of the rule stays sans
    assert!(svg.contains(&format!(r#"font-family="{RADICAL_FONT}" fill="{INK}">√</text>"#)), "{svg}");
    assert!(svg.contains(">(x + 3)</text>"), "{svg}");
}

/// The written rule's runs as one string: a run raised n powers deep is <supn>…</supn>, a √ is √.
fn markup(rule: &str) -> String {
    let m = model(&with(&[("rule", rule)]));
    let level = |scale: f64| ((scale.ln() / 0.7f64.ln()).round()) as i32;
    rhs_runs(&m)
        .iter()
        .map(|r| {
            let (t, sc) = match r {
                Run::Text { text, scale, .. } => (text.clone(), *scale),
                Run::Radical { scale, .. } => ("√".to_string(), *scale),
            };
            match level(sc) {
                0 => t,
                n => format!("<sup{n}>{t}</sup{n}>"),
            }
        })
        .collect()
}

#[test]
fn the_written_rule_is_the_rule_worked_out() {
    // 2^2^x is 2 to the (2 to the x): written with the power inside the power, worked out the same
    let p = with(&[("rule", "y = 2^2^x"), ("from", "0"), ("to", "2")]);
    assert_eq!(ys(&p), ["2", "4", "16"]);
    assert_eq!(markup("y = 2^2^x"), "2<sup1>2</sup1><sup2>x</sup2>");
    // the inner power rises a further 0.294 of the size on top of the outer 0.42
    let m = model(&with(&[("rule", "y = 2^2^x")]));
    let rises: Vec<f64> = rhs_runs(&m).iter().map(|r| match r { Run::Text { rise, .. } | Run::Radical { rise, .. } => *rise }).collect();
    assert!((rises[1] - 0.42).abs() < 1e-9 && (rises[2] - 0.714).abs() < 1e-9, "{rises:?}");
    // a bracketed exponent is grouped by the superscript, with its own powers inside it
    assert_eq!(markup("y = e^(-x^2)"), "e<sup1>\u{2212}x</sup1><sup2>2</sup2>");
    let p = with(&[("rule", "y = e^(-x^2)"), ("xs", "A list"), ("list", "0, 1")]);
    assert_eq!(ys(&p), ["1", "0.37"]);
    // a space after ^ is still the power
    assert_eq!(ys(&with(&[("rule", "y = 2^ x"), ("from", "0"), ("to", "2")])), ["1", "2", "4"]);
    assert_eq!(markup("y = 2^ x"), "2<sup1>x</sup1>");
    // e^sin x: the whole function is the power
    assert_eq!(markup("y = e^sin x"), "e<sup1>sin x</sup1>");
    // a radical inside a power is its own run, raised and shrunk with the power, and the power goes on after it
    let m = model(&with(&[("rule", "y = e^(sqrt x)")]));
    let runs = rhs_runs(&m);
    assert!(runs.iter().any(|r| matches!(r, Run::Radical { scale, rise } if (*scale - 0.7).abs() < 1e-9 && (*rise - 0.42).abs() < 1e-9)), "{runs:?}");
    assert_eq!(markup("y = e^(sqrt x)"), "e<sup1>√</sup1><sup1>x</sup1>");
}

#[test]
fn exact_where_it_is_exact_even_through_trig() {
    let p = with(&[("rule", "y = sin(x)^2"), ("xs", "A list"), ("list", "0, pi/6, pi/4, pi/3")]);
    assert_eq!(ys(&p), ["0", "1/4", "1/2", "3/4"]);
    let p = with(&[("rule", "y = cos x * cos x"), ("xs", "A list"), ("list", "30°, 45°, 60°")]);
    assert_eq!(ys(&p), ["3/4", "1/2", "1/4"]);
    // √2 stays rounded
    assert_eq!(ys(&with(&[("rule", "y = sqrt x"), ("xs", "A list"), ("list", "2")])), ["1.41"]);
    // big whole numbers are exact
    let p = with(&[("rule", "y = 3^x"), ("xs", "A list"), ("list", "38, 40")]);
    assert_eq!(ys(&p), ["1350851717672992089", "12157665459056928801"]);
    // a huge approximate value is written in powers of ten, not with made-up last digits
    assert_eq!(round_str(1.2157665459056929e19, 2), "1.2158 \u{d7} 10\u{b9}\u{2079}");
    // a rounded value keeps its places; one that needs fewer is written short
    assert_eq!(round_str(0.39999, 2), "0.40");
    assert_eq!(round_str(0.5, 2), "0.5");
}

#[test]
fn common_spellings_are_read() {
    for (rule, at, want) in [("y = sinx", "pi/2", "1"), ("y = lnx", "1", "0"), ("y = sqrtx", "9", "3"), ("y = sin^2 x", "pi/6", "1/4"), ("y = 2sin 2x + 1", "pi/12", "2")] {
        let p = with(&[("rule", rule), ("xs", "A list"), ("list", at)]);
        assert_eq!(ys(&p), [want], "{rule}");
    }
    let p = with(&[("rule", "y = sin x"), ("xs", "A list"), ("list", "0\u{ba}, 30\u{ba}")]);
    assert_eq!(xs(&p), ["0°", "30°"]);
    // two numbers with the comma missing are not read as their product
    let p = with(&[("xs", "A list"), ("list", "1, 5 6, 3")]);
    assert_eq!(xs(&p), ["1", "3"]);
    assert!(warnings(&p).iter().any(|w| w.contains("5 6")), "{:?}", warnings(&p));
    // a typed x that is not exact is written as typed
    let p = with(&[("xs", "A list"), ("list", "e, 2")]);
    assert_eq!(xs(&p), ["e", "2"]);
}

#[test]
fn the_answers_use_the_tables_number_form_and_need_three_points_for_linear() {
    let a = answers(&model(&with(&[("rule", "y = x^(3/2)"), ("xs", "A list"), ("list", "4, 9/4")])));
    assert!(a.iter().all(|l| !l.contains("inear")), "{a:?}");
    let a = answers(&model(&with(&[("rule", "y = x/3"), ("as", "Decimals")])));
    // exact even in Decimals: 1/3 does not end, so it stays a fraction rather than a rounded 0.33
    assert!(a.contains(&"Linear: the rate of change is 1/3.".to_string()), "{a:?}");
    let a = answers(&model(&with(&[("rule", "y = 0.1x + 2.5")])));
    assert!(a.contains(&"Linear: the rate of change is 0.1.".to_string()), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "0.5, 1.25\n1, 2.5\n1.5, 3.75")])));
    assert!(a.contains(&"Linear: y = 2.5x, a rate of change of 2.5.".to_string()), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "2, 5\n4, 10\n6, 15")])));
    assert!(a.contains(&"Linear: y = (5/2)x, a rate of change of 5/2.".to_string()), "{a:?}");
}

#[test]
fn down_tips_name_the_row() {
    let hs = handles(&with(&[("runs", "Down")]));
    assert!(hs.iter().any(|h| h.tip == "x in row 2: click to make a blank"), "{:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>());
}

#[test]
fn large_irrationals_are_never_made_into_fractions() {
    let p = with(&[("rule", "y = e^x"), ("from", "0"), ("to", "10")]);
    let y = ys(&p);
    assert!(y.iter().skip(1).all(|v| !v.contains('/')), "{y:?}");
    assert_eq!(y[10], "22026.47");
    let p = with(&[("rule", "y = 1000 sqrt(x)"), ("xs", "A list"), ("list", "62, 89")]);
    assert_eq!(ys(&p), ["7874.01", "9433.98"]);
    // while a rounding-error fraction still snaps
    assert_eq!(snap(N::F(0.5000000000000001)), N::Q(Rat { n: 1, d: 2 }));
}

#[test]
fn a_power_on_a_function_name_leaves_its_argument_alone() {
    for (rule, want) in [("y = sin^2x", "1/4"), ("y = cos^2x", "3/4"), ("y = sin\u{b2}x", "1/4"), ("y = cos^3x", "0.65")] {
        let p = with(&[("rule", rule), ("xs", "A list"), ("list", "pi/6")]);
        assert_eq!(ys(&p), [want], "{rule}");
    }
}

#[test]
fn rates_and_lines_are_exact_not_rounded() {
    let a = answers(&model(&with(&[("rule", "y = x/3"), ("as", "Decimals"), ("round", "Whole numbers")])));
    assert!(a.contains(&"Linear: the rate of change is 1/3.".to_string()), "{a:?}");
    let a = answers(&model(&with(&[("rule", "y = 0.075x"), ("from", "100"), ("to", "500"), ("step", "100")])));
    assert!(a.contains(&"Linear: the rate of change is 0.075.".to_string()), "{a:?}");
    let a = answers(&model(&with(&[("source", "Typed"), ("typed", "0, 0.125\n1, 0.25\n2, 0.375")])));
    assert!(a.contains(&"Linear: y = 0.125x + 0.125, a rate of change of 0.125.".to_string()), "{a:?}");
    // an exact decimal is rounded exactly: 1.005 to 2 places is 1.01
    assert_eq!(ys(&with(&[("rule", "y = 1.005x"), ("xs", "A list"), ("list", "1, 2")])), ["1.01", "2.01"]);
    assert_eq!(rat_round(Rat { n: 1, d: 3 }, 2), "0.33");
    assert_eq!(rat_round(Rat { n: 2, d: 5 }, 2), "0.4");
    assert_eq!(rat_round(Rat { n: -1, d: 1000 }, 2), "0.00");
    // an exact big number in Decimals is written in full
    assert_eq!(ys(&with(&[("rule", "y = 10^x"), ("xs", "A list"), ("list", "16"), ("as", "Decimals")])), ["10000000000000000"]);
}

#[test]
fn spellings_kept_as_typed() {
    let p = with(&[("xs", "A list"), ("list", "1 1/2, sqrt 2")]);
    assert_eq!(xs(&p), ["1 1/2", "\u{221a}2"]);
    let a = answers(&model(&with(&[("blanks", "x2, y2"), ("runs", "Down")])));
    assert!(a.iter().any(|l| l.starts_with("Row 2: ")), "{a:?}");
    assert!(markup("y = x2").contains("x \u{b7} 2"), "{}", markup("y = x2"));
}

#[test]
fn the_text_before_a_radical_ends_flush_against_it() {
    let m = model(&with(&[("rule", "y = x - sqrt(x)")]));
    let svg = draw_runs(100.0, 50.0, 18.0, "middle", &rule_runs(&m), INK, false);
    // "y = x − " is fitted to its width, which ends exactly where the √ starts
    let el = svg.split("<text ").nth(1).unwrap();
    let at = |k: &str| el.split(&format!(r#"{k}=""#)).nth(1).unwrap().split('"').next().unwrap().parse::<f64>().unwrap();
    let end = at("x") + at("textLength");
    let rad = svg.split("<text ").nth(2).unwrap();
    let rx: f64 = rad.split(r#"x=""#).nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(rad.contains(RADICAL_FONT) && (rx - end).abs() < 0.02, "{svg}");
}

#[test]
fn big_approximations_never_print_unknown_digits() {
    let p = with(&[("rule", "y = e^x"), ("xs", "A list"), ("list", "25, 33")]);
    let y = ys(&p);
    assert_eq!(y[0], "72004899337.39");
    assert_eq!(y[1], "2.1464 \u{d7} 10\u{b9}\u{2074}");
    // exact big numbers stay in full
    assert_eq!(ys(&with(&[("rule", "y = 2^x"), ("xs", "A list"), ("list", "60")])), ["1152921504606846976"]);
}

#[test]
fn tiny_values_stay_tiny_and_are_not_called_linear() {
    let p = with(&[("rule", "y = e^(-x)"), ("from", "28"), ("to", "32"), ("as", "Decimals"), ("round", "4 decimal places")]);
    assert!(ys(&p).iter().all(|v| v == "0.0000"), "{:?}", ys(&p));
    let a = answers(&model(&p));
    assert!(!a.iter().any(|l| l.starts_with("Linear")), "{a:?}");
    assert!(a.iter().any(|l| l.contains("times the one before")), "{a:?}");
    // e^-20 to 2 places is 0.00, like its neighbours, not a bare 0
    let p = with(&[("rule", "y = e^(-x)"), ("from", "0"), ("to", "40"), ("step", "10")]);
    assert_eq!(ys(&p)[2], "0.00");
    assert_eq!(snap(N::F(9.4e-14)), N::F(9.4e-14));
    assert_eq!(snap(N::F(1.2e-16)), N::q(0));
    // a rate over π x-values that is exactly 1 says so, not "about 1"
    let a = answers(&model(&with(&[("rule", "y = x + 1"), ("from", "0"), ("to", "pi"), ("step", "pi/4")])));
    assert!(a.contains(&"Linear: the rate of change is 1.".to_string()), "{a:?}");
}

#[test]
fn text_after_a_radical_starts_flush_against_it() {
    let m = model(&with(&[("rule", "y = 2sqrt(x+3) - sqrt(x + 2)")]));
    let svg = draw_runs(200.0, 50.0, 18.0, "middle", &rule_runs(&m), INK, false);
    // every element starts where the one before it ends: text by its fitted textLength, √ by 0.62 of the size
    let els: Vec<(f64, f64)> = svg
        .split("<text ")
        .skip(1)
        .map(|el| {
            let get = |k: &str| el.split(&format!(r#" {k}=""#)).nth(1).or_else(|| el.split(&format!(r#"{k}=""#)).nth(1)).map(|v| v.split('"').next().unwrap().parse::<f64>().unwrap());
            let x = get("x").unwrap();
            let w = if el.contains(RADICAL_FONT) { 0.62 * 18.0 } else { get("textLength").unwrap() };
            (x, w)
        })
        .collect();
    assert_eq!(els.len(), 5, "{svg}");
    for w in els.windows(2) {
        assert!((w[1].0 - (w[0].0 + w[0].1)).abs() < 0.02, "{els:?}");
    }
}

#[test]
fn a_long_word_answer_is_cut_to_its_box_on_the_key() {
    let p = with(&[("source", "Typed"), ("typed", "Mon, sunny\nTue, rain\nWednesday, partly cloudy with a chance of rain"), ("blanks", "y3")]);
    let svg = render(&p);
    assert!(svg.contains('\u{2026}'), "{svg}");
    assert!(answers(&model(&p)).iter().any(|l| l.ends_with("partly cloudy with a chance of rain")));
}

#[test]
fn decimal_x_values_and_wording_as_typed() {
    let p = with(&[("rule", "y = 2x"), ("xs", "A list"), ("list", "0.0000001, 0.1234567")]);
    assert_eq!(xs(&p), ["0.0000001", "0.1234567"]);
    let a = answers(&model(&with(&[("rule", "y = 1/x"), ("blanks", "x3")])));
    assert!(a.contains(&"x in column 3: 0".to_string()), "{a:?}");
    let hs = handles(&with(&[("source", "Typed"), ("typed", ", 8\n2, 9")]));
    assert!(hs.iter().any(|h| h.tip == "y in column 1: click to make a blank"), "{:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>());
    let p = with(&[("rule", "y = sin x"), ("xs", "A list"), ("list", "pi/2, 30°")]);
    assert!(warnings(&p).iter().any(|w| w.contains("mixes degrees")));
}

#[test]
fn powers_and_radicals_together_are_written_in_order() {
    // each run is a flat element of its own; drawn in order, flush, the rule reads as typed
    assert_eq!(markup("y = 3x^2 - sqrt(2x)"), "3x<sup1>2</sup1> \u{2212} √(2x)");
    assert_eq!(markup("y = x^2 + sqrt(x)"), "x<sup1>2</sup1> + √(x)");
    assert_eq!(markup("y = sqrt(x^2 + 1)"), "√(x<sup1>2</sup1> + 1)");
    let m = model(&with(&[("rule", "y = 3x^2 - sqrt(2x)")]));
    let svg = draw_runs(200.0, 50.0, 18.0, "middle", &rule_runs(&m), INK, false);
    assert!(!svg.contains("<tspan"), "{svg}");
    let els: Vec<(f64, f64)> = svg
        .split("<text ")
        .skip(1)
        .map(|el| {
            let get = |k: &str| el.split(&format!(r#" {k}=""#)).nth(1).or_else(|| el.split(&format!(r#"{k}=""#)).nth(1)).map(|v| v.split('"').next().unwrap().parse::<f64>().unwrap());
            let size = get("font-size").unwrap();
            let w = if el.contains(RADICAL_FONT) { 0.62 * size } else { get("textLength").unwrap() };
            (get("x").unwrap(), w)
        })
        .collect();
    for w in els.windows(2) {
        assert!((w[1].0 - (w[0].0 + w[0].1)).abs() < 0.02, "{els:?}");
    }
}

#[test]
fn small_negatives_round_to_an_unsigned_zero_and_powers_of_ten_read() {
    assert_eq!(round_str(-0.001, 2), "0.00");
    assert_eq!(ys(&with(&[("rule", "y = 1.41 - sqrt(x)"), ("xs", "A list"), ("list", "2")])), ["0.00"]);
    let p = with(&[("rule", "y = sqrt x"), ("xs", "A list"), ("list", "1e4, 2.5e-3")]);
    assert_eq!(ys(&p)[0], "100");
}

#[test]
fn notes_for_typed_pairs_with_spaces_and_a_heading_row() {
    let p = with(&[("source", "Typed"), ("typed", "1 5\n2 8\n3 11")]);
    assert!(warnings(&p).iter().any(|w| w.contains("comma or a tab")), "{:?}", warnings(&p));
    let p = with(&[("source", "Typed"), ("typed", "x, y\n1, 5\n2, 8")]);
    assert!(warnings(&p).iter().any(|w| w.contains("looks like headings")), "{:?}", warnings(&p));
    // words that are data, not headings, get no note
    let p = with(&[("source", "Typed"), ("typed", "Mon, sunny\nTue, rain")]);
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    assert!(warnings(&with(&[("source", "Typed")])).is_empty());
}
