//! Segment addition's Points and Spans cards, driven through the workspace as a teacher uses them:
//! removing a point in the middle, adding a point at the end, adding and removing a brace.
use super::last_b11::press_button;
use super::*;
use crate::figures::segment as seg;

fn names(ws: &Workspace) -> Vec<String> { (0..seg::n_points(&ws.params)).map(|i| ws.params.s(seg::NAME_KEYS[i]).to_string()).collect() }

/// cap-01b, cap-01a: Remove B joins A to C (one unlabeled part; the brace AC still spans the
/// line), as one undo step; + Point then adds a point at the right end with the first free letter.
#[test]
fn the_points_card_removes_a_middle_point_and_adds_one_at_the_end() {
    let (ctx, mut ws) = open_tall(seg::ID);
    assert_eq!(seg::ID, "segment-addition");
    let before = ws.output().unwrap().svg.unwrap();
    let undo0 = ws.undo.len();
    press_button(&ctx, &mut ws, "Remove B");
    assert_eq!(names(&ws), ["A", "C"]);
    assert_eq!(ws.params.s("p1_lab"), "", "the joined part AC starts unlabeled");
    assert_eq!((seg::n_spans(&ws.params), ws.params.i("s1_from"), ws.params.i("s1_to")), (1, 1, 2), "the brace AC, over B, still spans the line");
    assert_eq!(ws.undo.len(), undo0 + 1, "one undo step");
    assert_ne!(ws.output().unwrap().svg.unwrap(), before, "drawn");
    press_button(&ctx, &mut ws, "Point");
    assert_eq!(names(&ws), ["A", "C", "B"], "a new point at the right end, with the first free letter");
    assert_eq!(ws.params.s("p2_lab"), "", "its part starts unlabeled");
    // two points are the fewest
    press_button(&ctx, &mut ws, "Remove C");
    let w = super::last_b11::panel_widgets(&ctx, &mut ws);
    assert!(w.iter().any(|w| w.1 == "Remove A" && w.3), "Remove is off at two points");
}

/// cap-03c, cap-03g: + Span adds a brace no other brace has, and Remove span takes it away.
#[test]
fn the_spans_card_adds_and_removes_braces() {
    let (ctx, mut ws) = open_tall(seg::ID);
    open_section(&ctx, &mut ws, "Spans");
    press_button(&ctx, &mut ws, "Span");
    assert_eq!(seg::n_spans(&ws.params), 2);
    assert_eq!((ws.params.i("s2_from"), ws.params.i("s2_to")), (1, 2), "the first part, since AC has a brace");
    press_button(&ctx, &mut ws, "Remove span 1");
    assert_eq!(seg::n_spans(&ws.params), 1);
    assert_eq!((ws.params.i("s1_from"), ws.params.i("s1_to")), (1, 2), "the later brace moves up");
    press_button(&ctx, &mut ws, "Remove span 1");
    assert_eq!(seg::n_spans(&ws.params), 0, "no braces at all");
}
