use super::*;
use crate::figures::interact;

fn defaults() -> Params {
    Params::defaults(&params())
}

fn with(kv: &[(&str, &str)]) -> Params {
    let mut p = defaults();
    for (k, v) in kv {
        match params().iter().find(|d| d.key == *k).map(|d| d.default.clone()) {
            Some(Value::B(_)) => p.set_b(k, *v == "true"),
            Some(Value::I(_)) => p.set_i(k, v.parse().unwrap()),
            _ => p.set_s(k, v),
        }
    }
    p
}

/// The text of every `<text>` element, tspans flattened, in drawing order.
fn texts(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        let after = &rest[i..];
        let open_end = after.find('>').unwrap();
        let close = after.find("</text>").unwrap();
        let inner = &after[open_end + 1..close];
        let mut s = String::new();
        let mut tag = false;
        for ch in inner.chars() {
            match ch {
                '<' => tag = true,
                '>' => tag = false,
                c if !tag => s.push(c),
                _ => {}
            }
        }
        out.push(s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        rest = &after[close + 7..];
    }
    out
}

/// Texts drawn in bold.
fn bold_texts(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in svg.split("<text").skip(1) {
        let head = &chunk[..chunk.find('>').unwrap()];
        if head.contains(r#"font-weight="700""#) {
            out.extend(texts(&format!("<text{}", &chunk[..chunk.find("</text>").unwrap() + 7])));
        }
    }
    out
}

fn travel() -> Params {
    with(&[
        ("row_labels", "Year 7, Year 8, Year 9"),
        ("col_labels", "Walk, Bus, Car"),
        ("cells", "12, , 8 = 30\n15, 8, = 29\n, 14, 5 = 28\nTotal, 36, 32, 19 = 87"),
        ("blank", CHOOSE),
    ])
}

#[test]
fn labels_are_trimmed_capped_and_padded() {
    assert_eq!(labels(" a , b;c ", "Row", 0), ["a", "b", "c"]);
    assert_eq!(labels("", "Row", 2), ["Row 1", "Row 2"]);
    assert_eq!(labels(&vec!["x"; 20].join(","), "Row", 0).len(), MAX_LINES);
}

#[test]
fn grid_pads_missing_counts_with_zero() {
    assert_eq!(grid("1, 2, 3", 2, 2), vec![vec![1.0, 2.0], vec![3.0, 0.0]]);
    assert_eq!(grid("", 1, 2), vec![vec![0.0, 0.0]]);
}

#[test]
fn blanked_cells_are_distinct_and_spread() {
    for nr in 1..6 {
        for nc in 1..6 {
            for c in 0..=nr * nc + 2 {
                let b = blanked(nr, nc, c);
                assert_eq!(b.len(), c.min(nr * nc));
                let mut s = b.clone();
                s.sort();
                s.dedup();
                assert_eq!(s.len(), b.len(), "{nr}x{nc} c={c}: {b:?}");
                assert!(b.iter().all(|i| *i < nr * nc));
            }
        }
    }
    let three = blanked(3, 3, 3);
    assert_eq!(three.iter().map(|i| i % 3).collect::<std::collections::BTreeSet<_>>().len(), 3, "three blanks hit three columns");
    assert_eq!(three.iter().map(|i| i / 3).collect::<std::collections::BTreeSet<_>>().len(), 3, "and three rows");
}

#[test]
fn default_shows_counts_and_totals() {
    let svg = render(&defaults());
    assert!(svg.contains(">24<") && svg.contains(">50<") && svg.contains(">43<") && svg.contains(">100<"), "{svg}");
    assert!(svg.contains(">Girls<") && svg.contains(">Pizza<"));
}

#[test]
fn totals_are_the_sums() {
    // worked by hand: rows 1 + 2 = 3 and 3 + 4 = 7, columns 1 + 3 = 4 and 2 + 4 = 6, all 10
    let p = with(&[("cells", "1, 2\n3, 4"), ("row_labels", "A, B"), ("col_labels", "X, Y")]);
    let svg = render(&p);
    for t in [">3<", ">7<", ">4<", ">6<", ">10<"] {
        assert!(svg.contains(t), "{t} missing");
    }
}

#[test]
fn student_copy_blanks_the_totals_only() {
    let (student, key) = crate::figures::answer_pair(&def(), &defaults()).unwrap();
    let n = |s: &str| s.matches("<text").count();
    // 2 rows x 3 cols = 6 counts stay; 2 + 3 + 1 totals go
    assert_eq!(n(&key) - n(&student), 6);
    assert!(!student.contains(">100<") && !student.contains(">50<"));
    assert!(student.contains(">24<"), "the given counts stay");
    let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
    assert_eq!(size(&student), size(&key));
}

#[test]
fn student_copy_can_blank_cells_instead() {
    let mut p = defaults();
    p.set_s("blank", "Some cells");
    p.set_i("blank_count", 2);
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert_eq!(key.matches("<text").count() - student.matches("<text").count(), 2);
    assert!(student.contains(">50<"), "totals are given when only cells are blank");
    let mut q = defaults();
    q.set_s("blank", "Totals and some cells");
    q.set_i("blank_count", 2);
    let (s2, k2) = crate::figures::answer_pair(&def(), &q).unwrap();
    assert_eq!(k2.matches("<text").count() - s2.matches("<text").count(), 8);
    assert!(relevant(ID, "blank_count", &q) && !relevant(ID, "blank_count", &defaults()));
    q.set_s("blank", CHOOSE);
    assert!(!relevant(ID, "blank_count", &q));
}

// ---------- old saves ----------

/// Every old figure (saved before the new keys existed) loads through the migration and draws byte
/// for byte what the frozen pre-change render drew.
#[test]
fn old_saves_render_byte_identically() {
    let old_keys = ["row_labels", "col_labels", "cells", "row_title", "col_title", "title", "blank", "blank_count", "style", "label_size", "show_answers"];
    let cells = [
        "24, 18, 8\n19, 27, 4",
        "",
        "abc",
        "1",
        "-5, 3",
        "1e12, 1e12\n1e12, 1e12",
        "0.5, 0.25",
        "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17",
        "NaN, inf",
        "9999999999999999999999",
        "1,,3\n4",
        "x, 5\n3, ?",
        "Total, 1, 2\n3 = 4",
        "12\t\t8\n1\t2\t3",
        "0.1, 0.2\n0.3, 0.4",
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10",
        "\tA\tB\nR1\t1\t2\nR2\t3\t4",
        "[12], 5\n-0, 7",
        "40, 22, 6\n35, 30, 14\n12, 18, 41",
        "1 2 3\n4 5 6\n7 8 9",
    ];
    let names = [("Girls, Boys", "Pizza, Tacos, Salad"), ("", ""), ("A", "B"), ("a,b,c,d,e,f,g,h,i,j", "1,2,3,4,5,6,7,8,9,10,11"), ("Freshmen, Sophomores, Juniors", "Bus, Walk, Car")];
    let def = def();
    let mut n = 0;
    for cell in cells {
        for (rl, cl) in names {
            for (blank, count) in [("Totals", 2), ("Some cells", 3), ("Totals and some cells", 1)] {
                for answers in [true, false] {
                    for (style, size, title, rt, ct) in [("Colour", "Medium", "", "", ""), ("Print", "Large", "Lunch survey", "Grade", "Gets to school by"), ("Colour", "Small", "T", "", "Col")] {
                        let mut old = defaults();
                        for (k, v) in [("cells", cell), ("row_labels", rl), ("col_labels", cl), ("blank", blank), ("style", style), ("label_size", size), ("title", title), ("row_title", rt), ("col_title", ct)] {
                            old.set_s(k, v);
                        }
                        old.set_i("blank_count", count);
                        old.set_b(SHOW_ANSWERS, answers);
                        let spec = crate::model::FigureSpec {
                            id: ID.into(),
                            params: old_keys.iter().map(|k| (k.to_string(), crate::figures::spec::value_to_json(old.0.get(*k).unwrap()))).collect(),
                        };
                        let loaded = crate::figures::spec::params_from_spec(&def, &spec);
                        assert_eq!(render(&loaded), legacy::render(&old), "cells {cell:?} names {rl:?}/{cl:?} {blank} answers {answers} {style} {size}");
                        n += 1;
                    }
                }
            }
        }
    }
    assert_eq!(n, 20 * 5 * 3 * 2 * 3);
    // a save made now keeps its marks: it has the new keys, so nothing is rewritten
    let p = travel();
    let spec = crate::figures::spec::spec_for(ID, &p);
    assert_eq!(crate::figures::spec::params_from_spec(&def, &spec).s("cells"), p.s("cells"));
}

#[test]
fn migration_writes_the_numbers_the_old_table_drew() {
    let mut p = with(&[("cells", "1,,3\n4\nabc, 2")]);
    migrate(&crate::model::FigureSpec { id: ID.into(), params: Default::default() }, &mut p);
    // three lines, so a third row; empty boxes and words drew 0, short lines were padded with 0
    assert_eq!(p.s("cells"), "1, 0, 3\n4, 0, 0\n0, 2, 0");
}

// ---------- typed totals, gaps, letters ----------

#[test]
fn gaps_are_worked_out_from_typed_totals() {
    let p = travel();
    let w = Worked::new(&p);
    // by hand: 30 - 12 - 8 = 10, 29 - 15 - 8 = 6, 28 - 14 - 5 = 9; columns check: 12 + 15 + 9 = 36,
    // 10 + 8 + 14 = 32, 8 + 6 + 5 = 19, and 30 + 29 + 28 = 87
    assert_eq!(w.truth(Slot::Cell(0, 1)), Some(10.0));
    assert_eq!(w.truth(Slot::Cell(1, 2)), Some(6.0));
    assert_eq!(w.truth(Slot::Cell(2, 0)), Some(9.0));
    assert_eq!(w.missing(), vec![Slot::Cell(0, 1), Slot::Cell(1, 2), Slot::Cell(2, 0)]);
    assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    let (st, kt) = (texts(&student), texts(&key));
    assert_eq!(kt.len() - st.len(), 3);
    let bold = bold_texts(&key);
    for v in ["10", "6", "9"] {
        assert!(bold.contains(&v.to_string()), "{v} bold in the key: {bold:?}");
        assert!(!st.contains(&v.to_string()), "{v} not on the student copy");
    }
    // the typed totals are given (not bold, shown to students)
    assert!(st.contains(&"87".to_string()) && st.contains(&"30".to_string()));
    assert!(!bold.contains(&"87".to_string()));
}

#[test]
fn a_letter_is_drawn_and_the_key_gives_its_value() {
    let p = with(&[("row_labels", "Year 10, Year 11"), ("col_labels", "School lunch, Packed lunch"), ("cells", "46, x = 80\n38, 41 = ?\nTotal, 84, 75 = 159"), ("blank", CHOOSE)]);
    let w = Worked::new(&p);
    // by hand: x = 80 - 46 = 34 (and 75 - 41 = 34); Year 11's total 38 + 41 = 79 (and 159 - 80 = 79)
    assert_eq!(w.truth(Slot::Cell(0, 1)), Some(34.0));
    assert_eq!(w.truth(Slot::RowTot(1)), Some(79.0));
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(texts(&student).contains(&"x".to_string()));
    assert!(!texts(&student).iter().any(|t| t.contains("34") || t.contains("79")), "{:?}", texts(&student));
    assert!(texts(&key).contains(&"x = 34".to_string()) && bold_texts(&key).contains(&"79".to_string()));
    assert!(student.contains(r#"font-style="italic""#), "the letter is set in italics");
}

#[test]
fn a_hidden_count_keeps_its_value_for_the_key() {
    let p = with(&[("cells", "24, [18], 8\n19, 27, 4"), ("blank", CHOOSE)]);
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(!texts(&student).contains(&"18".to_string()) && bold_texts(&key).contains(&"18".to_string()));
    assert!(texts(&student).contains(&"50".to_string()), "totals are given in Boxes I choose");
}

#[test]
fn a_gap_that_cannot_be_worked_out_is_named() {
    // totals left out are worked out from the counts, so two gaps in one row with no typed total can't be found
    let p = with(&[("cells", "24, , \n19, 27, 4"), ("blank", CHOOSE)]);
    let w = Worked::new(&p);
    assert_eq!(w.truth(Slot::Cell(0, 1)), None);
    assert_eq!(warnings(&p), vec!["Students can't work out any of the 2 blank boxes from what the table shows: Girls, Tacos; Girls, Salad.".to_string()]);
    // the key leaves them empty rather than guess (the one empty text is the untitled corner)
    let key = render(&p);
    assert_eq!(texts(&key).iter().filter(|t| t.is_empty()).count(), 1);
    // a 2 x 2 of four gaps with every total given is a cycle: none can be found
    let q = with(&[("row_labels", "A, B"), ("col_labels", "X, Y"), ("cells", "?, ? = 10\n?, ? = 20\nTotal, 12, 18 = 30"), ("blank", CHOOSE)]);
    assert!(warnings(&q)[0].starts_with("Students can't work out any of the 4 blank boxes"), "{:?}", warnings(&q));
    // with one count given the rest follow: b = 10 - 5 = 5, c = 12 - 5 = 7, d = 20 - 7 = 13
    let r = with(&[("row_labels", "A, B"), ("col_labels", "X, Y"), ("cells", "5, ? = 10\n?, ? = 20\nTotal, 12, 18 = 30"), ("blank", CHOOSE)]);
    let w = Worked::new(&r);
    assert_eq!((w.truth(Slot::Cell(0, 1)), w.truth(Slot::Cell(1, 0)), w.truth(Slot::Cell(1, 1))), (Some(5.0), Some(7.0), Some(13.0)));
    assert!(warnings(&r).is_empty());
    // the default student copy (totals blank) can always be worked out
    assert!(warnings(&defaults()).is_empty());
    // a gap and its row total both blank, nothing else to go on in its column: named
    let s = with(&[("cells", "24, , 8\n19, 27, 4"), ("blank", "Totals")]);
    assert!(warnings(&s)[0].contains("Girls, Tacos"), "{:?}", warnings(&s));
}

#[test]
fn a_bridge_between_two_loops_of_gaps_is_still_found() {
    // rows A, B and columns X, Y hold a loop of four gaps; so do rows C, D and columns Z, W; the gap at
    // B, Z joins the loops. Peeling boxes one at a time finds nothing, but the block sums fix the
    // bridge: rows A and B total 10 + 20 = 30 and their known boxes add to 1 + 2 + 3 + 4 = 10, the X and
    // Y columns' gaps hold the 4 + 6 ... worked out by the solver as 30 - 10 - (X, Y gaps) etc. Checked
    // against the full table it was made from.
    let full = [[1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0], [9.0, 10.0, 11.0, 12.0], [13.0, 14.0, 15.0, 16.0]];
    let gaps = [(0, 0), (0, 1), (1, 0), (1, 1), (2, 2), (2, 3), (3, 2), (3, 3), (1, 2)];
    let mut lines = Vec::new();
    for (r, row) in full.iter().enumerate() {
        let boxes: Vec<String> = row.iter().enumerate().map(|(c, v)| if gaps.contains(&(r, c)) { "?".into() } else { format!("{v}") }).collect();
        lines.push(format!("{} = {}", boxes.join(", "), row.iter().sum::<f64>()));
    }
    let cols: Vec<String> = (0..4).map(|c| format!("{}", full.iter().map(|r| r[c]).sum::<f64>())).collect();
    lines.push(format!("Total, {} = 136", cols.join(", ")));
    let p = with(&[("row_labels", "A, B, C, D"), ("col_labels", "X, Y, Z, W"), ("cells", &lines.join("\n")), ("blank", CHOOSE)]);
    let w = Worked::new(&p);
    assert_eq!(w.truth(Slot::Cell(1, 2)), Some(7.0), "the bridge");
    assert_eq!(w.truth(Slot::Cell(0, 0)), None, "a box on a loop");
    let note = &warnings(&p)[0];
    assert!(note.contains("8 of the 9 blank boxes") && !note.contains("B, Z"), "{note}");
}

#[test]
fn typed_totals_that_do_not_add_up_are_reported() {
    let p = with(&[("cells", "24, 18, 8 = 51\n19, 27, 4\nTotal, 43, 45, 12 = 100")]);
    // Girls' typed 51 is not 24 + 18 + 8 = 50, so the row totals 51 + 50 = 101 miss the grand total 100 too
    assert_eq!(warnings(&p), vec!["Girls: the counts add up to 50, but its total is 51.".to_string(), "The row totals add up to 101, but the grand total is 100.".to_string()]);
    let q = with(&[("cells", "24, 18, 8\n19, 27, 4\nTotal, 43, 45, 12 = 99")]);
    assert!(warnings(&q).iter().any(|w| w == "The row totals add up to 100, but the grand total is 99."), "{:?}", warnings(&q));
}

#[test]
fn the_total_switches_drop_the_column_and_row() {
    let total = |p: &Params| texts(&render(p)).iter().filter(|t| *t == "Total").count();
    assert_eq!(total(&defaults()), 2);
    let col_off = with(&[("total_col", "false")]);
    let row_off = with(&[("total_row", "false")]);
    let both = with(&[("total_col", "false"), ("total_row", "false")]);
    assert_eq!((total(&col_off), total(&row_off), total(&both)), (1, 1, 0));
    // the row totals 50 and the grand total 100 go with the column; the column totals 43, 45, 12 with the row
    assert!(!texts(&render(&col_off)).contains(&"100".to_string()) && texts(&render(&col_off)).contains(&"43".to_string()));
    assert!(!texts(&render(&row_off)).contains(&"43".to_string()) && texts(&render(&row_off)).contains(&"50".to_string()));
    let (w0, _) = crate::figures::svg_size(&render(&defaults()));
    let (w1, _) = crate::figures::svg_size(&render(&col_off));
    let (_, h0) = crate::figures::svg_size(&render(&defaults()));
    let (_, h2) = crate::figures::svg_size(&render(&row_off));
    assert!(w1 < w0 && h2 < h0);
    // a student copy without the Total column hides only the totals that are drawn
    let (st, key) = crate::figures::answer_pair(&def(), &col_off).unwrap();
    assert_eq!(texts(&key).len() - texts(&st).len(), 3);
}

// ---------- relative frequencies ----------

fn shown(p: &Params, s: Slot) -> String {
    match disp(&Worked::new(p), p, s).unwrap() {
        Disp::Text(t) => t,
        Disp::Frac(a, b) => format!("{a}/{b}"),
        Disp::Letter(l, v) => format!("{l} = {}", v.unwrap_or_default()),
    }
}

#[test]
fn relative_frequencies_by_hand() {
    // the default table: Girls 24, 18, 8 (50); Boys 19, 27, 4 (50); columns 43, 45, 12; 100 in all
    let joint = with(&[("values", OF_TOTAL)]);
    assert_eq!(shown(&joint, Slot::Cell(0, 0)), "0.24");
    assert_eq!(shown(&joint, Slot::RowTot(0)), "0.50");
    assert_eq!(shown(&joint, Slot::ColTot(2)), "0.12");
    assert_eq!(shown(&joint, Slot::Grand), "1.00");
    let row = with(&[("values", OF_ROW)]);
    // 24 / 50 = 0.48, 18 / 50 = 0.36, 8 / 50 = 0.16; the Total row is each column over the grand total
    assert_eq!([shown(&row, Slot::Cell(0, 0)), shown(&row, Slot::Cell(0, 1)), shown(&row, Slot::Cell(0, 2))], ["0.48", "0.36", "0.16"]);
    assert_eq!(shown(&row, Slot::RowTot(1)), "1.00");
    assert_eq!(shown(&row, Slot::ColTot(0)), "0.43");
    let col = with(&[("values", OF_COL)]);
    // 24 / 43 = 0.5581, 19 / 43 = 0.4419, 8 / 12 = 0.6667; the Total column is each row over the grand total
    assert_eq!([shown(&col, Slot::Cell(0, 0)), shown(&col, Slot::Cell(1, 0)), shown(&col, Slot::Cell(0, 2))], ["0.56", "0.44", "0.67"]);
    assert_eq!(shown(&col, Slot::ColTot(1)), "1.00");
    assert_eq!(shown(&col, Slot::RowTot(0)), "0.50");
    let pct = with(&[("values", OF_COL), ("rf_as", "Percent"), ("rf_places", "1")]);
    assert_eq!(shown(&pct, Slot::Cell(0, 0)), "55.8%");
    let pct0 = with(&[("values", OF_COL), ("rf_as", "Percent"), ("rf_places", "0")]);
    assert_eq!(shown(&pct0, Slot::Cell(0, 0)), "56%");
    let pct4 = with(&[("values", OF_TOTAL), ("rf_as", "Decimal"), ("rf_places", "4")]);
    assert_eq!(shown(&pct4, Slot::Cell(1, 2)), "0.0400");
    // fractions in lowest terms: 24/43 stays, 18/45 = 2/5, 27/45 = 3/5, 8/12 = 2/3, 50/100 = 1/2, 45/45 = 1
    let frac = with(&[("values", OF_COL), ("rf_as", "Fraction")]);
    let f = |s| shown(&frac, s);
    assert_eq!([f(Slot::Cell(0, 0)), f(Slot::Cell(0, 1)), f(Slot::Cell(1, 1)), f(Slot::Cell(0, 2)), f(Slot::RowTot(0)), f(Slot::ColTot(1))], ["24/43", "2/5", "3/5", "2/3", "1/2", "1"]);
    // joint: 24/100 = 6/25
    assert_eq!(shown(&with(&[("values", OF_TOTAL), ("rf_as", "Fraction")]), Slot::Cell(0, 0)), "6/25");
    // a fraction of counts that are not whole numbers falls back to a decimal
    let half = with(&[("cells", "0.5, 1.5\n1, 2"), ("col_labels", "A, B"), ("values", OF_TOTAL), ("rf_as", "Fraction")]);
    assert_eq!(shown(&half, Slot::Cell(0, 0)), "0.10");
    // a half rounds up as on paper: 34 / 80 = 0.425 is 0.43 and 42.5%, 1 / 8 = 0.125 is 0.13, though the floats sit just under
    let lunch = with(&[("row_labels", "Year 10, Year 11"), ("col_labels", "School lunch, Packed lunch"), ("cells", "46, 34\n38, 41"), ("values", OF_ROW)]);
    assert_eq!(shown(&lunch, Slot::Cell(0, 1)), "0.43");
    assert_eq!(shown(&lunch, Slot::Cell(0, 0)), "0.58", "46 / 80 = 0.575");
    let lunch_pct = with(&[("row_labels", "Year 10, Year 11"), ("col_labels", "School lunch, Packed lunch"), ("cells", "46, 34\n38, 41"), ("values", OF_ROW), ("rf_as", "Percent"), ("rf_places", "1")]);
    assert_eq!([shown(&lunch_pct, Slot::Cell(0, 1)), shown(&lunch_pct, Slot::Cell(1, 0))], ["42.5%", "48.1%"], "38 / 79 = 48.10%");
    let eighth = with(&[("row_labels", "A"), ("col_labels", "X, Y"), ("cells", "1, 7"), ("values", OF_ROW)]);
    assert_eq!(shown(&eighth, Slot::Cell(0, 0)), "0.13");
    // a zero total gives a dash, not NaN
    let zero = with(&[("cells", "0, 0, 0\n1, 2, 3"), ("values", OF_ROW)]);
    assert_eq!(shown(&zero, Slot::Cell(0, 0)), "\u{2013}");
}

#[test]
fn relative_frequencies_of_a_worked_out_gap() {
    // Year 7's Bus gap is 10 of its 30: 0.33 of the row; the travel table's Walk column is 36 of 87
    let mut p = travel();
    p.set_s("values", OF_ROW);
    assert_eq!(shown(&p, Slot::Cell(0, 1)), "0.33");
    p.set_s("values", OF_TOTAL);
    assert_eq!(shown(&p, Slot::ColTot(0)), "0.41");
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(texts(&key).contains(&"0.11".to_string()) && !texts(&student).contains(&"0.11".to_string()), "10/87 = 0.11 only on the key");
}

#[test]
fn fractions_are_stacked_and_the_rows_grow() {
    let p = with(&[("values", OF_COL), ("rf_as", "Fraction")]);
    let svg = render(&p);
    let t = texts(&svg);
    assert!(t.contains(&"24".to_string()) && t.contains(&"43".to_string()) && !t.iter().any(|s| s.contains('/')), "numerator and denominator are separate lines: {t:?}");
    let (_, h0) = crate::figures::svg_size(&render(&with(&[("values", OF_COL)])));
    let (_, h1) = crate::figures::svg_size(&svg);
    assert!(h1 > h0);
}

// ---------- titles and look ----------

#[test]
fn title_as_a_blank_line_and_split_corner() {
    let p = with(&[("title", "Lunch survey"), ("title_mode", "Blank line")]);
    let (student, key) = crate::figures::answer_pair(&def(), &p).unwrap();
    assert!(!texts(&student).contains(&"Lunch survey".to_string()) && texts(&key).contains(&"Lunch survey".to_string()));
    assert!(student.matches("<line").count() > render(&with(&[("title", "Lunch survey")])).matches("<line").count(), "a line for the title");
    // an untitled blank line still makes room
    let blank = render(&with(&[("title_mode", "Blank line")]));
    assert!(crate::figures::svg_size(&blank).1 > crate::figures::svg_size(&render(&defaults())).1);

    let q = with(&[("row_title", "Gender"), ("col_title", "Lunch"), ("corner_split", "true")]);
    let svg = render(&q);
    let t = texts(&svg);
    assert!(t.contains(&"Gender".to_string()) && t.contains(&"Lunch".to_string()));
    // the column variable is not drawn above the table any more: no spanning title row
    let plain = render(&with(&[("row_title", "Gender"), ("col_title", "Lunch")]));
    assert!(svg.contains(r#"text-anchor="end""#) && !plain.contains(r#"text-anchor="end""#));
}

/// The split corner's titles stay on their own side of the diagonal, at every size and for long names.
#[test]
fn split_corner_titles_clear_the_diagonal() {
    for size in ["Small", "Medium", "Large"] {
        for (rt, ct) in [("Gender", "Lunch"), ("Grade level of the student", "How they get to school"), ("A", "B")] {
            let p = with(&[("row_title", rt), ("col_title", ct), ("corner_split", "true"), ("label_size", size)]);
            let w = Worked::new(&p);
            let slots: Vec<Slot> = w.m.slots().into_iter().filter(|&s| w.drawn(s)).collect();
            let disps: Vec<(Slot, Option<Disp>)> = slots.iter().map(|&s| (s, disp(&w, &p, s))).collect();
            let l = layout(&w, &p, &disps);
            let cw = l.fs * 0.62;
            let line_y = |x: f64| l.y0 + l.head_h * (x - l.x0) / l.first_w;
            // column title: its bottom-left corner above the line
            let xl = l.x0 + l.first_w - 8.0 - ct.len() as f64 * cw;
            assert!(xl > l.x0 && l.y0 + l.fs * 1.4 < line_y(xl), "{size} {ct}");
            // row title: its top-right corner below the line
            let xr = l.x0 + 8.0 + rt.len() as f64 * cw;
            assert!(xr < l.x0 + l.first_w && l.y0 + l.head_h - l.fs * 1.2 > line_y(xr), "{size} {rt}");
        }
    }
}

#[test]
fn look_choices_change_the_drawing() {
    let base = render(&defaults());
    let plain = render(&with(&[("headings", "Plain")]));
    assert!(base.contains("#f0e9db") && !plain.contains("#f0e9db"));
    let rules = render(&with(&[("lines", "Header and totals")]));
    assert!(rules.matches("<line").count() < base.matches("<line").count() && rules.matches("<line").count() == 4);
    // Totals in bold: shows where the totals are given (Some cells), and on a student copy's typed totals
    let some = with(&[("blank", "Some cells")]);
    let mut bold = some.clone();
    bold.set_b("totals_bold", true);
    assert!(!bold_texts(&render(&some)).contains(&"100".to_string()) && bold_texts(&render(&bold)).contains(&"100".to_string()));
}

#[test]
fn print_style_is_grey() {
    for extra in [vec![], vec![("headings", "Plain")], vec![("lines", "Header and totals")], vec![("corner_split", "true"), ("row_title", "G"), ("col_title", "L")], vec![("values", OF_ROW), ("rf_as", "Fraction")], vec![("title_mode", "Blank line")]] {
        let mut kv = vec![("style", "Print"), ("title", "Lunch survey")];
        kv.extend(extra);
        let png = crate::figures::to_png(&render(&with(&kv)), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "{kv:?}");
    }
}

// ---------- pasting ----------

#[test]
fn a_pasted_table_with_headings_and_totals_is_split_up() {
    let paste = "\tWalk\tBus\tCar\tTotal\nYear 7\t12\t\t8\t30\nYear 8\t15\t8\t\t29\nYear 9\t\t14\t5\t28\nTotal\t36\t32\t19\t87\n";
    let mut p = defaults();
    let old = p.0["cells"].clone();
    p.set_s("cells", paste);
    // read as it is, before the panel tidies it
    let raw = render(&p);
    assert!(pasted(&old, &mut p));
    assert_eq!(p.s("row_labels"), "Year 7, Year 8, Year 9");
    assert_eq!(p.s("col_labels"), "Walk, Bus, Car");
    assert_eq!(p.s("cells"), "12, ?, 8 = 30\n15, 8, ? = 29\n?, 14, 5 = 28\nTotal, 36, 32, 19 = 87");
    assert_eq!(p.s("blank"), CHOOSE, "the gaps are what students fill in");
    assert_eq!(render(&p), { let mut q = p.clone(); q.set_s("cells", paste); q.set_s("row_labels", "Girls, Boys"); q.set_s("blank", CHOOSE); render(&q) });
    // and the gaps are worked out as typed
    assert_eq!(Worked::new(&p).truth(Slot::Cell(0, 1)), Some(10.0));
    assert_ne!(raw, render(&defaults()));
    // a corner heading becomes the row variable; spreadsheet thousands keep their value
    let mut q = defaults();
    q.set_s("cells", "Grade\tYes\tNo\nNinth\t1,200\t300\nTenth\t900\t600");
    assert!(pasted(&Value::S(String::new()), &mut q));
    assert_eq!((q.s("row_title"), q.s("cells")), ("Grade", "1200, 300\n900, 600"));
    assert_eq!(q.s("blank"), "Totals", "no gaps: the choice stays");
}

#[test]
fn a_pasted_block_with_its_sums_becomes_typed_totals() {
    assert_eq!(positional_totals("24\t18\t8\t50\n19\t27\t4\t50\n43\t45\t12\t100").as_deref(), Some("24, 18, 8 = 50\n19, 27, 4 = 50\nTotal, 43, 45, 12 = 100"));
    // gaps are fine where the sums can still be checked
    assert!(positional_totals("12\t\t8\t30\n15\t8\t6\t29\n9\t14\t5\t28\n36\t32\t19\t87").is_some());
    // a block whose last column is not the sums is just counts
    assert_eq!(positional_totals("24\t18\t8\t51\n19\t27\t4\t50\n43\t45\t12\t101"), None);
    assert_eq!(positional_totals("1\t2\n3\t4"), None);
    let mut p = defaults();
    p.set_s("cells", "24\t18\t8\t50\n19\t27\t4\t50\n43\t45\t12\t100");
    assert!(pasted(&Value::S("24, 18, 8\n19, 27, 4".into()), &mut p));
    assert_eq!(render(&p), render(&defaults()), "the same table, its totals now typed");
}

#[test]
fn typing_is_not_a_paste() {
    let mut p = defaults();
    p.set_s("cells", "24, 18, 8\n19, 27, 4\n1");
    assert!(!pasted(&Value::S("24, 18, 8\n19, 27, 4".into()), &mut p));
    assert_eq!(p.s("cells"), "24, 18, 8\n19, 27, 4\n1");
}

// ---------- clicking boxes on the table ----------

fn layouts() -> Vec<Params> {
    vec![
        defaults(),
        with(&[("label_size", "Large"), ("row_labels", "Students in the morning group, Boys"), ("col_labels", "Pizza with extra cheese, Tacos, Salad")]),
        with(&[("corner_split", "true"), ("row_title", "Gender"), ("col_title", "Favourite lunch"), ("title", "Lunch")]),
        with(&[("values", OF_COL), ("rf_as", "Fraction"), ("total_col", "false")]),
        travel(),
        with(&[("blank", "Some cells"), ("blank_count", "2"), ("total_row", "false"), ("label_size", "Small")]),
    ]
}

/// Every pixel over a value box, sent through the workspace's own hit test, reaches that box's
/// handle, and a click there flips the box between shown and blank on the student copy (and back).
#[test]
fn clicking_a_box_through_the_hit_test_blanks_it_and_back() {
    let mut hits = 0;
    for p0 in layouts() {
        let svg = render(&p0);
        let size = crate::figures::svg_size(&svg);
        let hs = interact::handles_for(ID, &p0, size);
        let w = Worked::new(&p0);
        let slots: Vec<Slot> = w.m.slots().into_iter().filter(|&s| w.drawn(s)).collect();
        assert_eq!(hs.len(), slots.len());
        let disps: Vec<(Slot, Option<Disp>)> = slots.iter().map(|&s| (s, disp(&w, &p0, s))).collect();
        let l = layout(&w, &p0, &disps);
        for k in [1.0, 0.6] {
            for &s in &slots {
                let (x, y, wid) = l.rect(w.m.nr(), w.m.nc(), s);
                for fx in [0.1, 0.5, 0.9] {
                    for fy in [0.15, 0.5, 0.85] {
                        let q = (x + wid * fx, y + l.rowh * fy);
                        let i = interact::pick(&hs, q, k).unwrap_or_else(|| panic!("nothing at {q:?} for {s:?}"));
                        let mut p = p0.clone();
                        (hs[i].click.as_ref().unwrap())(&mut p);
                        let after = Worked::new(&p);
                        let flipped = (w.seen(s) == Seen::Shown) == (after.seen(s) != Seen::Shown);
                        let stuck = w.seen(s) == Seen::Blank && w.truth(s).is_none();
                        assert!(flipped || stuck, "{s:?} at {q:?}: {:?} -> {:?}", w.seen(s), after.seen(s));
                        // nothing else changes on the student copy
                        for &t in &slots {
                            if t != s {
                                assert_eq!(after.seen(t) == Seen::Shown, w.seen(t) == Seen::Shown, "{t:?} changed when {s:?} was clicked");
                                assert_eq!(after.truth(t), w.truth(t), "{t:?}'s value");
                            }
                        }
                        hits += 1;
                    }
                }
            }
        }
        // the headings and names are not boxes
        assert_eq!(interact::pick(&hs, (l.x0 + l.first_w / 2.0, l.row_top(1) + l.rowh / 2.0), 1.0), None);
        assert_eq!(interact::pick(&hs, (l.xs[1] + l.col_w[0] / 2.0, l.y0 + l.head_h / 2.0), 1.0), None);
    }
    assert!(hits > 500, "{hits}");
}

#[test]
fn first_click_keeps_the_choice_blanks_and_a_second_click_undoes_it() {
    let p0 = defaults();
    let (student0, _) = crate::figures::answer_pair(&def(), &p0).unwrap();
    let mut p = p0.clone();
    toggle(&mut p, Slot::Cell(0, 1));
    assert_eq!(p.s("blank"), CHOOSE);
    assert_eq!(p.s("cells"), "24, [18], 8 = [50]\n19, 27, 4 = [50]\nTotal, [43], [45], [12] = [100]");
    let (student1, key1) = crate::figures::answer_pair(&def(), &p).unwrap();
    let mut gone = texts(&student0);
    gone.retain(|t| t != "18");
    assert_eq!(texts(&student1), gone, "only Girls, Tacos left the student copy");
    assert_eq!(key1.matches("<text").count(), render(&p0).matches("<text").count());
    toggle(&mut p, Slot::Cell(0, 1));
    assert_eq!(crate::figures::answer_pair(&def(), &p).unwrap().0, student0);
    // a worked-out gap clicked shows its value
    let mut t = travel();
    toggle(&mut t, Slot::Cell(0, 1));
    assert!(t.s("cells").starts_with("12, 10, 8 = 30"), "{}", t.s("cells"));
    // a table read from a pasted spreadsheet keeps its names when clicked
    let mut h = with(&[("cells", "\tWalk\tBus\nYear 7\t1\t2\nYear 8\t3\t4")]);
    toggle(&mut h, Slot::Cell(1, 1));
    assert_eq!((h.s("row_labels"), h.s("col_labels"), h.s("cells")), ("Year 7, Year 8", "Walk, Bus", "1, 2 = [3]\n3, [4] = [7]\nTotal, [4], [6] = [10]"));
}

#[test]
fn handle_tips_name_the_box() {
    let hs = handles(&defaults());
    assert!(hs.iter().any(|h| h.tip == "Leave Girls, Pizza blank for students"));
    assert!(hs.iter().any(|h| h.tip == "Show the grand total to students"));
    let gap = handles(&with(&[("cells", "24, , \n19, 27, 4"), ("blank", CHOOSE)]));
    assert!(gap.iter().any(|h| h.tip == "Girls, Tacos can't be worked out: type its count to show it"));
}

// ---------- student copy holds no answer ----------

#[test]
fn no_answer_on_any_student_copy() {
    let def = def();
    let mut cases = layouts();
    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in e.params {
            p.0.insert(k.to_string(), v);
        }
        cases.push(p);
    }
    for mut p in cases {
        for values in [COUNTS, OF_TOTAL, OF_ROW] {
            p.set_s("values", values);
            let w = Worked::new(&p);
            let (student, key) = crate::figures::answer_pair(&def, &p).unwrap();
            let hidden = w.m.slots().into_iter().filter(|&s| w.drawn(s) && w.seen(s) == Seen::Blank && disp(&w, &p, s).is_some()).count();
            let fracs = w.m.slots().into_iter().filter(|&s| w.drawn(s) && w.seen(s) == Seen::Blank && matches!(disp(&w, &p, s), Some(Disp::Frac(..)))).count();
            let title = (p.s("title_mode") == "Blank line" && !p.s("title").trim().is_empty()) as usize;
            assert_eq!(key.matches("<text").count() - student.matches("<text").count(), hidden + fracs + title, "{:?}", p.s("cells"));
            // the letters' values are never on the student copy
            assert!(!student.contains(" = "), "{student}");
            // the alt text is the figure's title and blurb only
            let full = def.svg(&{ let mut q = p.clone(); q.set_b(SHOW_ANSWERS, false); q });
            let desc = &full[full.find("<desc>").unwrap() + 6..full.find("</desc>").unwrap()];
            assert_eq!(desc, esc(def.blurb));
        }
    }
}

#[test]
fn odd_input_never_panics_or_prints_nan() {
    let mut p = defaults();
    let cells = ["", "abc", "1", "-5, 3", "1e12, 1e12\n1e12, 1e12", "0.5, 0.25", "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17", "NaN, inf", "9999999999999999999999", "= = =", "Total", "Total = 5", "[", "[]", "[x]", "?, ?\n?, ?", "Total, Total\nTotal", "\t\t\n\t", "a\tb\nc\td", "=\n=\n="];
    for cell in cells {
        for (rl, cl) in [("", ""), ("A", "B"), ("a,b,c,d,e,f,g,h,i,j", "1,2,3,4,5,6,7,8,9,10,11")] {
            for blank in ["Totals", "Some cells", "Totals and some cells", CHOOSE] {
                for values in [COUNTS, OF_ROW] {
                    for rf_as in ["Decimal", "Fraction"] {
                        p.set_s("cells", cell);
                        p.set_s("row_labels", rl);
                        p.set_s("col_labels", cl);
                        p.set_s("blank", blank);
                        p.set_s("values", values);
                        p.set_s("rf_as", rf_as);
                        for answers in [true, false] {
                            p.set_b(SHOW_ANSWERS, answers);
                            let svg = render(&p);
                            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{cell:?}");
                            assert!(!svg.contains("=\"NaN") && !svg.contains("NaN\"") && !svg.contains("inf\""), "{cell:?}: {svg}");
                            let _ = warnings(&p);
                            if !answers || values != COUNTS || rf_as != "Decimal" {
                                continue;
                            }
                            for h in handles(&p) {
                                let mut q = p.clone();
                                (h.click.as_ref().unwrap())(&mut q);
                                assert!(render(&q).ends_with("</svg>"));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn names_are_escaped() {
    let mut p = defaults();
    p.set_s("row_labels", "<b>&, Boys");
    let svg = render(&p);
    assert!(svg.contains("&lt;b&gt;&amp;") && !svg.contains("<b>"));
    let q = with(&[("cells", "<i>, 2\n3, 4"), ("blank", CHOOSE)]);
    assert!(!render(&q).contains("<i>"));
}

#[test]
fn examples_render() {
    for e in examples(ID).unwrap() {
        let mut p = defaults();
        for (k, v) in e.params {
            p.0.insert(k.to_string(), v);
        }
        assert!(render(&p).contains("Total"), "{}", e.name);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
    }
}

#[test]
fn every_new_control_changes_the_drawing() {
    let base = render(&defaults());
    for (k, v) in [("total_col", "false"), ("total_row", "false"), ("values", OF_TOTAL), ("title_mode", "Blank line"), ("headings", "Plain"), ("lines", "Header and totals")] {
        assert_ne!(render(&with(&[(k, v)])), base, "{k}");
    }
    let rf = with(&[("values", OF_TOTAL)]);
    assert_ne!(render(&with(&[("values", OF_TOTAL), ("rf_as", "Percent")])), render(&rf));
    assert_ne!(render(&with(&[("values", OF_TOTAL), ("rf_places", "3")])), render(&rf));
    let titled = with(&[("row_title", "G"), ("col_title", "L")]);
    assert_ne!(render(&with(&[("row_title", "G"), ("col_title", "L"), ("corner_split", "true")])), render(&titled));
    let some = with(&[("blank", "Some cells")]);
    assert_ne!(render(&with(&[("blank", "Some cells"), ("totals_bold", "true")])), render(&some));
}
