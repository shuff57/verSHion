//! A cumulative table typed as it is printed: one line per upper class boundary with the running
//! total up to it ("50, 3" is "3 values are below 50"). A first line with total 0 is where the first
//! class starts; without one the first class is taken as wide as the next. The frequencies are the
//! differences between neighbouring totals, so a total that falls is refused with a fix.
use super::{cell_number, cells, exact_text, Class, MAX_CLASSES};

pub const DEFAULT_CUM: &str = "40, 0\n50, 3\n60, 10\n70, 22\n80, 37\n90, 46\n100, 50";

/// The numbers a line holds, with inequality marks taken off ("< 50", "≤50", "140 < h ≤ 150").
fn numbers(line: &str) -> Vec<f64> {
    cells(line)
        .iter()
        .filter_map(|c| cell_number(c.trim_matches(|ch: char| matches!(ch, '<' | '>' | '=' | '\u{2264}' | '\u{2265}'))))
        .collect()
}

fn line_text(u: f64, c: f64) -> String { format!("{}, {}", exact_text(u), exact_text(c)) }

/// The classes, and a note when the first class's start was worked out rather than typed.
pub fn parse_cum_table(text: &str) -> Result<(Vec<Class>, Option<String>), String> {
    let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty()).collect();
    if lines.len() > MAX_CLASSES + 2 {
        return Err(format!(
            "The table has {} lines; an ogive here can have at most {MAX_CLASSES} classes. Merge neighbouring classes (keep every other upper boundary) until there are {MAX_CLASSES} or fewer.",
            lines.len()
        ));
    }
    // (line, upper boundary, running total)
    let mut rows: Vec<(usize, f64, f64)> = Vec::new();
    for (k, (n, line)) in lines.iter().enumerate() {
        let v = numbers(line);
        if v.is_empty() && k == 0 {
            continue; // a header
        }
        if v.len() < 2 {
            let ex = match (v.first(), rows.last()) {
                (Some(&u), Some(&(_, _, c))) => line_text(u, c),
                (Some(&u), None) => line_text(u, 0.0),
                (None, Some(&(_, u, c))) => line_text(u + 10.0, c),
                (None, None) => line_text(50.0, 3.0),
            };
            let shown: String = line.chars().take(40).collect();
            return Err(format!(
                "Line {n} (\"{shown}\") is not a boundary and a running total. Type each line as two numbers, the upper class boundary and the cumulative frequency up to it, for example \"{ex}\"."
            ));
        }
        rows.push((*n, v[v.len() - 2], v[v.len() - 1]));
    }
    if rows.is_empty() {
        return Err("Type one line per class: its upper boundary and the cumulative frequency up to it, for example \"50, 3\". A first line with 0, such as \"40, 0\", says where the first class starts.".into());
    }
    for &(n, u, c) in &rows {
        if c < 0.0 || c.fract() != 0.0 {
            let fix = if c < 0.0 { 0.0 } else { c.round() };
            return Err(format!(
                "Line {n}: the cumulative frequency {} is not a count. A running total counts values, so it is a whole number, 0 or more: for example \"{}\".",
                exact_text(c), line_text(u, fix)
            ));
        }
    }
    rows.sort_by(|a, b| a.1.total_cmp(&b.1));
    for w in rows.windows(2) {
        let ((n1, u1, c1), (n2, u2, c2)) = (w[0], w[1]);
        if u2 == u1 {
            return Err(format!(
                "Lines {n1} and {n2} both end at {}. Each line is a different upper class boundary: delete one of them, or change it to the boundary it should be.",
                exact_text(u1)
            ));
        }
        if c2 < c1 {
            return Err(format!(
                "The cumulative frequency falls from {} (up to {}, line {n1}) to {} (up to {}, line {n2}). A running total counts every value below the boundary, so it can stay the same or rise but never fall: type the total up to {}, at least {}, for example \"{}\". If line {n2} is a frequency rather than a total, choose Data: Grouped table instead.",
                exact_text(c1), exact_text(u1), exact_text(c2), exact_text(u2), exact_text(u2), exact_text(c1), line_text(u2, c1 + c2)
            ));
        }
    }
    let mut note = None;
    let (start, rest) = if rows[0].2 == 0.0 && rows.len() >= 2 {
        (rows[0].1, &rows[1..])
    } else if rows.len() >= 2 {
        let w = rows[1].1 - rows[0].1;
        let s = rows[0].1 - w;
        note = Some(format!(
            "The first class is taken to start at {}, as wide as the next class. To start it somewhere else, type the boundary with total 0 as the first line, for example \"{}\".",
            exact_text(s), line_text(s, 0.0)
        ));
        (s, &rows[..])
    } else {
        let (n, u, c) = rows[0];
        return Err(if c == 0.0 {
            format!("Line {n} says no values are below {}, and there are no more lines. Add a line for each upper boundary with its running total, for example \"{}\".", exact_text(u), line_text(u + 10.0, 1.0))
        } else {
            format!(
                "There is one class, and nothing says where it starts. Type the lower boundary with total 0 as the first line, for example \"{}\" above line {n}.",
                line_text(u - 10.0, 0.0)
            )
        });
    };
    if rest.len() > MAX_CLASSES {
        return Err(format!("The table has {} classes; an ogive here can have at most {MAX_CLASSES}. Merge neighbouring classes (keep every other upper boundary) until there are {MAX_CLASSES} or fewer.", rest.len()));
    }
    if rest[rest.len() - 1].2 == 0.0 {
        return Err("Every cumulative frequency is 0, so no values were counted. Type the running total up to each boundary, for example \"50, 3\".".into());
    }
    let mut out = Vec::new();
    let (mut lo, mut prev) = (start, 0.0);
    for &(_, u, c) in rest {
        out.push(Class { lo, hi: u, f: (c - prev) as u64 });
        lo = u;
        prev = c;
    }
    Ok((out, note))
}
