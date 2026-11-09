//! The two-way table's render as it was before typed totals, blank boxes, letters and relative frequencies (frozen
//! 2026-11-08, test only): old saved figures, migrated at load, must still draw byte for byte what this draws.
use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{Params, SHOW_ANSWERS};

pub const MAX_LINES: usize = 8;

/// Names split at commas (or semicolons), trimmed, at most [`MAX_LINES`]; unnamed ones are
/// "Row 1" / "Column 2" style.
pub fn labels(text: &str, fallback: &str, want_at_least: usize) -> Vec<String> {
    let mut v: Vec<String> = text.split([',', ';']).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    v.truncate(MAX_LINES);
    while v.len() < want_at_least.max(1) {
        v.push(format!("{fallback} {}", v.len() + 1));
    }
    v
}

/// The counts row-major, `nr` by `nc`; short input is padded with 0 and extra is ignored.
pub fn grid(text: &str, nr: usize, nc: usize) -> Vec<Vec<f64>> {
    let nums = data::numbers(text);
    (0..nr).map(|r| (0..nc).map(|c| nums.get(r * nc + c).copied().unwrap_or(0.0)).collect()).collect()
}

/// Which interior cells (row-major index) a student fills in: `count` of them taken along
/// diagonals, so the blanks fall in different rows and columns instead of emptying one line.
pub fn blanked(nr: usize, nc: usize, count: usize) -> Vec<usize> {
    let n = nr * nc;
    (0..count.min(n)).map(|j| (j % nr) * nc + (j / nr + j % nr) % nc).collect()
}

pub fn render(p: &Params) -> String {
    let text = p.s("cells");
    // the table has as many columns and rows as there are names; names fall back to counts of the
    // lines and columns actually typed
    let lines = data::parse_table(text);
    let typed_cols = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let named_r = labels(p.s("row_labels"), "Row", lines.len());
    let named_c = labels(p.s("col_labels"), "Column", typed_cols.min(MAX_LINES));
    let (nr, nc) = (named_r.len(), named_c.len());
    // read per line, so a blank count in the middle keeps its place
    let mut g = vec![vec![0.0; nc]; nr];
    for (r, line) in lines.iter().take(nr).enumerate() {
        for (c, cell) in line.iter().take(nc).enumerate() {
            g[r][c] = data::numbers(cell).first().copied().unwrap_or(0.0);
        }
    }
    if lines.is_empty() {
        g = grid(text, nr, nc);
    }
    let row_tot: Vec<f64> = g.iter().map(|r| r.iter().sum()).collect();
    let col_tot: Vec<f64> = (0..nc).map(|c| g.iter().map(|r| r[c]).sum()).collect();
    let grand: f64 = row_tot.iter().sum();

    let mode = p.s("blank");
    let answers = p.b(SHOW_ANSWERS);
    let hide_totals = !answers && mode != "Some cells";
    let hidden: Vec<usize> = if mode == "Totals" { Vec::new() } else { blanked(nr, nc, p.i("blank_count").max(0) as usize) };
    let is_blank_cell = |r: usize, c: usize| hidden.contains(&(r * nc + c));

    let fills = Fills::new(p);
    let tint = if fills.is_print() { "#ececec" } else { "#f0e9db" };
    let k = svg::label_factor(p.s("label_size"));
    let fs = 17.0 * k;
    let (title, row_title, col_title) = (p.s("title").trim(), p.s("row_title").trim(), p.s("col_title").trim());
    let char_w = fs * 0.62;
    let num = |v: f64| chart::fmt(v);
    let first_w = (named_r.iter().map(|s| s.chars().count()).chain([row_title.chars().count(), 5]).max().unwrap() as f64 * char_w + 28.0).max(90.0);
    let col_w: Vec<f64> = (0..=nc)
        .map(|c| {
            let name = if c < nc { named_c[c].chars().count() } else { 5 };
            let widest_num = (0..nr).map(|r| num(if c < nc { g[r][c] } else { row_tot[r] }).chars().count()).max().unwrap_or(1).max(num(if c < nc { col_tot[c] } else { grand }).chars().count());
            (name.max(widest_num) as f64 * char_w + 28.0).max(80.0)
        })
        .collect();
    let rowh = fs * 2.5;
    let top_pad = if title.is_empty() { 14.0 } else { 36.0 + fs * 1.1 };
    let span_h = if col_title.is_empty() { 0.0 } else { rowh * 0.8 };
    let (w, h) = (20.0 + first_w + col_w.iter().sum::<f64>() + 20.0, top_pad + span_h + rowh * (nr as f64 + 2.0) + 16.0);

    let mut d = Doc::new(w, h);
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.1, fs * 1.15, "middle", title);
    }
    let x0 = 20.0;
    let y0 = top_pad + span_h;
    let xs: Vec<f64> = std::iter::once(x0).chain(col_w.iter().scan(x0 + first_w, |x, cw| { let v = *x; *x += cw; Some(v) })).collect();
    // xs[0] = first column, xs[1..=nc+1] = data columns and the Total column
    let cell_x = |c: usize| xs[c + 1];
    let cell_text = |d: &mut Doc, x: f64, y: f64, wid: f64, s: &str, bold: bool| {
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{fs}" text-anchor="middle" font-weight="{}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            x + wid / 2.0,
            y + rowh * 0.64,
            if bold { 700 } else { 400 },
            esc(s)
        ));
    };
    // spanning column variable, then the header row
    if !col_title.is_empty() {
        let (a, b) = (cell_x(0), cell_x(nc));
        chart::text(&mut d, (a + b) / 2.0, top_pad + span_h * 0.7, fs * 1.05, "middle", col_title);
    }
    let table_w = first_w + col_w.iter().sum::<f64>();
    // header row and total row are tinted
    d.rect(x0, y0, table_w, rowh, tint, INK, 1.6);
    d.rect(x0, y0 + rowh * (nr as f64 + 1.0), table_w, rowh, tint, INK, 1.6);
    d.rect(x0, y0 + rowh, first_w, rowh * nr as f64, tint, INK, 1.6);
    d.rect(cell_x(nc), y0 + rowh, col_w[nc], rowh * nr as f64, tint, INK, 1.6);
    for r in 0..=nr + 1 {
        let y = y0 + rowh * r as f64;
        d.line(x0, y, x0 + table_w, y, INK, 1.6);
    }
    for c in 0..=nc + 1 {
        let x = xs[c];
        d.line(x, y0, x, y0 + rowh * (nr as f64 + 2.0), INK, 1.6);
    }
    d.line(x0 + table_w, y0, x0 + table_w, y0 + rowh * (nr as f64 + 2.0), INK, 1.6);
    // outline
    d.rect(x0, y0, table_w, rowh * (nr as f64 + 2.0), "none", INK, 2.4);
    cell_text(&mut d, x0, y0, first_w, row_title, false);
    for c in 0..nc {
        cell_text(&mut d, cell_x(c), y0, col_w[c], &named_c[c], true);
    }
    cell_text(&mut d, cell_x(nc), y0, col_w[nc], "Total", true);
    for r in 0..nr {
        let y = y0 + rowh * (r as f64 + 1.0);
        cell_text(&mut d, x0, y, first_w, &named_r[r], true);
        for c in 0..nc {
            if is_blank_cell(r, c) && !answers {
                continue;
            }
            cell_text(&mut d, cell_x(c), y, col_w[c], &num(g[r][c]), is_blank_cell(r, c));
        }
        if !hide_totals {
            cell_text(&mut d, cell_x(nc), y, col_w[nc], &num(row_tot[r]), !answers_given_totals(mode));
        }
    }
    let ty = y0 + rowh * (nr as f64 + 1.0);
    cell_text(&mut d, x0, ty, first_w, "Total", true);
    if !hide_totals {
        for c in 0..nc {
            cell_text(&mut d, cell_x(c), ty, col_w[c], &num(col_tot[c]), !answers_given_totals(mode));
        }
        cell_text(&mut d, cell_x(nc), ty, col_w[nc], &num(grand), !answers_given_totals(mode));
    }
    d.finish()
}

/// Totals are given (so plain) when the student copy only blanks cells; otherwise they are answers.
fn answers_given_totals(mode: &str) -> bool {
    mode == "Some cells"
}

