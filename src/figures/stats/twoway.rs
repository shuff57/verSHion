//! Two-way (contingency) table: counts for two categorical variables with row and column totals.
//! The totals (the marginals) and chosen boxes are what a student is asked to work out, so the
//! student copy (Show answers off) leaves those boxes empty and the key fills them in, in bold.
//!
//! The Counts text is one line per row. On a line, `= 30` after the counts types that row's total,
//! and a line starting `Total` types the Total row (its last number, or one after `=`, the grand
//! total); totals left out are worked out. A box left empty or written `?` is blank for students
//! (the key shows it worked out from the rest), `[12]` is blank for students with 12 in the key,
//! and a letter like `x` is drawn as the letter (the key adds `x = 34`). A table pasted with its
//! headings (and Total row and column) is read as such.
use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "two-way-table";
pub const MAX_LINES: usize = 8;
/// The Blank for students choice that hides only the boxes marked in the counts (or clicked on the table).
pub const CHOOSE: &str = "Boxes I choose";
pub const COUNTS: &str = "Counts";
pub const OF_TOTAL: &str = "Relative frequency of the grand total";
pub const OF_ROW: &str = "Relative frequency of each row";
pub const OF_COL: &str = "Relative frequency of each column";
/// Longest letter or word kept for an unknown box.
const MAX_LETTER: usize = 12;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Two-Way Table",
        blurb: "A two-way table of counts with row and column totals; hide the totals or chosen cells for students to fill in.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        (ID, "blank_count") => matches!(p.s("blank"), "Some cells" | "Totals and some cells"),
        (ID, "rf_as") => p.s("values") != COUNTS,
        (ID, "rf_places") => p.s("values") != COUNTS && p.s("rf_as") != "Fraction",
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "Favourite lunch", params: vec![] },
            Example {
                name: "Missing cells",
                params: vec![
                    ("row_labels", s("Freshmen, Sophomores, Juniors")),
                    ("col_labels", s("Bus, Walk, Car")),
                    ("cells", s("40, 22, 6\n35, 30, 14\n12, 18, 41")),
                    ("row_title", s("Grade")),
                    ("col_title", s("Gets to school by")),
                    ("blank", s("Some cells")),
                    ("blank_count", Value::I(3)),
                ],
            },
            Example {
                name: "Gaps to fill from the totals",
                params: vec![
                    ("row_labels", s("Year 7, Year 8, Year 9")),
                    ("col_labels", s("Walk, Bus, Car")),
                    ("cells", s("12, , 8 = 30\n15, 8, = 29\n, 14, 5 = 28\nTotal, 36, 32, 19 = 87")),
                    ("title", s("How students travel to school")),
                    ("blank", s(CHOOSE)),
                    ("show_answers", Value::B(false)),
                ],
            },
            Example {
                name: "An unknown, x",
                params: vec![
                    ("row_labels", s("Year 10, Year 11")),
                    ("col_labels", s("School lunch, Packed lunch")),
                    ("cells", s("46, x = 80\n38, 41 = ?\nTotal, 84, 75 = 159")),
                    ("blank", s(CHOOSE)),
                    ("show_answers", Value::B(false)),
                ],
            },
            Example {
                name: "Row percents",
                params: vec![
                    ("row_labels", s("Grade 7, Grade 8")),
                    ("col_labels", s("Soccer, Basketball, Swimming")),
                    ("cells", s("28, 14, 18\n22, 26, 12")),
                    ("values", s(OF_ROW)),
                    ("rf_as", s("Percent")),
                    ("rf_places", Value::I(1)),
                    ("blank", s(CHOOSE)),
                ],
            },
            Example {
                name: "Fractions of the total",
                params: vec![
                    ("row_labels", s("Left-handed, Right-handed")),
                    ("col_labels", s("Boys, Girls")),
                    ("cells", s("6, 4\n24, 26")),
                    ("values", s(OF_TOTAL)),
                    ("rf_as", s("Fraction")),
                    ("blank", s(CHOOSE)),
                    ("row_title", s("Hand")),
                    ("col_title", s("Student")),
                    ("corner_split", Value::B(true)),
                ],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("row_labels", "Row names", "Girls, Boys").hint("Separate with commas, up to 8"),
        ParamDef::text("col_labels", "Column names", "Pizza, Tacos, Salad").hint("Separate with commas, up to 8"),
        ParamDef::long("cells", "Counts", "24, 18, 8\n19, 27, 4").hint(
            "One line per row, counts separated by commas, or paste a table from a spreadsheet (headings and totals too). \
             = 30 at the end of a line types that row's total; a line starting Total types the Total row. \
             Leave a count empty or type ? for students to find, [12] to hide 12 from students, or a letter like x",
        ),
        ParamDef::bool("total_col", "Total column", true),
        ParamDef::bool("total_row", "Total row", true),
        ParamDef::choice("values", "In the cells", &[COUNTS, OF_TOTAL, OF_ROW, OF_COL], COUNTS),
        ParamDef::choice("rf_as", "Written as", &["Decimal", "Percent", "Fraction"], "Decimal").hint("Fractions are in lowest terms"),
        ParamDef::int("rf_places", "Decimal places", 0, 4, 2),
        ParamDef::text("row_title", "Row variable", ""),
        ParamDef::text("col_title", "Column variable", ""),
        ParamDef::bool("corner_split", "Variable names in a split corner", false).hint("Both variable names go in the corner box, either side of a diagonal"),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("title_mode", "Table title", &["Text", "Blank line"], "Text").hint("Blank line leaves a line for students to write the title on; the key writes it there"),
        ParamDef::choice("blank", "Blank for students", &["Totals", "Some cells", "Totals and some cells", CHOOSE], "Totals")
            .hint("What Show answers off leaves empty. Boxes I choose: only the boxes left empty in the counts, or clicked on the table"),
        ParamDef::int("blank_count", "How many cells", 1, 20, 2),
        ParamDef::choice("headings", "Headings", &["Shaded", "Plain"], "Shaded"),
        ParamDef::choice("lines", "Lines", &["Every cell", "Header and totals"], "Every cell"),
        ParamDef::bool("totals_bold", "Totals in bold", false),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off leaves the chosen boxes empty for students; on fills them in bold"),
    ]
}

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

// ---------- the table as typed ----------

/// One box of the table as the teacher typed it.
#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    /// A count, shown.
    Num(f64),
    /// `[12]`: a count hidden from students, in the key.
    Hidden(f64),
    /// Empty or `?`: hidden from students, the key works it out from the rest.
    Empty,
    /// A letter like `x`, drawn as itself; the key adds its value.
    Letter(String),
    /// A total that was not typed: worked out from the counts.
    Absent,
}

/// A box of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Cell(usize, usize),
    RowTot(usize),
    ColTot(usize),
    Grand,
}

/// A number typed in a box (the whole box, so "Year 7" is a name): `1,000` from a spreadsheet keeps
/// its thousands, a trailing `%` and a typographic minus are read as a spreadsheet writes them.
fn number(t: &str) -> Option<f64> {
    let t = t.trim().replace('\u{2212}', "-");
    let t = t.trim_end_matches('%').trim();
    let plain = if t.contains(',') {
        let groups: Vec<&str> = t.trim_start_matches('-').split(',').collect();
        let thousands = groups.len() > 1
            && (1..=3).contains(&groups[0].len())
            && groups[0].chars().all(|c| c.is_ascii_digit())
            && groups[1..].iter().enumerate().all(|(i, g)| {
                let int = if i == groups.len() - 2 { g.split('.').next().unwrap_or("") } else { g };
                int.len() == 3 && int.chars().all(|c| c.is_ascii_digit())
            });
        if !thousands {
            return None;
        }
        t.replace(',', "")
    } else {
        t.to_string()
    };
    // only digits, a sign, a point and an exponent: "inf" and "NaN" are words here
    if !plain.chars().all(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')) || !plain.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    plain.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= data::MAX_ABS)
}

/// What one typed box means.
pub fn entry(s: &str) -> Entry {
    let t = s.trim();
    if t.is_empty() || t == "?" {
        return Entry::Empty;
    }
    if let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return number(inner).map_or(Entry::Empty, Entry::Hidden);
    }
    match number(t) {
        Some(v) => Entry::Num(v),
        None => Entry::Letter(t.chars().take(MAX_LETTER).collect()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub rows: Vec<String>,
    pub cols: Vec<String>,
    pub cells: Vec<Vec<Entry>>,
    pub row_tot: Vec<Entry>,
    pub col_tot: Vec<Entry>,
    pub grand: Entry,
    /// Read from a pasted table with headings (its names win over the name boxes).
    pub headed: bool,
    /// A pasted table's corner heading (the row variable).
    pub corner: String,
}

impl Model {
    pub fn nr(&self) -> usize { self.rows.len() }
    pub fn nc(&self) -> usize { self.cols.len() }
    pub fn at(&self, s: Slot) -> &Entry {
        match s {
            Slot::Cell(r, c) => &self.cells[r][c],
            Slot::RowTot(r) => &self.row_tot[r],
            Slot::ColTot(c) => &self.col_tot[c],
            Slot::Grand => &self.grand,
        }
    }
    fn at_mut(&mut self, s: Slot) -> &mut Entry {
        match s {
            Slot::Cell(r, c) => &mut self.cells[r][c],
            Slot::RowTot(r) => &mut self.row_tot[r],
            Slot::ColTot(c) => &mut self.col_tot[c],
            Slot::Grand => &mut self.grand,
        }
    }
    /// Every box in variable order (cells row-major, row totals, column totals, grand total).
    fn slots(&self) -> Vec<Slot> {
        let (nr, nc) = (self.nr(), self.nc());
        let mut v: Vec<Slot> = (0..nr).flat_map(|r| (0..nc).map(move |c| Slot::Cell(r, c))).collect();
        v.extend((0..nr).map(Slot::RowTot));
        v.extend((0..nc).map(Slot::ColTot));
        v.push(Slot::Grand);
        v
    }
    fn index(&self, s: Slot) -> usize {
        let (nr, nc) = (self.nr(), self.nc());
        match s {
            Slot::Cell(r, c) => r * nc + c,
            Slot::RowTot(r) => nr * nc + r,
            Slot::ColTot(c) => nr * nc + nr + c,
            Slot::Grand => nr * nc + nr + nc,
        }
    }
    /// The box's name in a note: "Girls, Pizza", "Girls total", "the grand total".
    pub fn name(&self, s: Slot) -> String {
        match s {
            Slot::Cell(r, c) => format!("{}, {}", self.rows[r], self.cols[c]),
            Slot::RowTot(r) => format!("{} total", self.rows[r]),
            Slot::ColTot(c) => format!("{} total", self.cols[c]),
            Slot::Grand => "the grand total".into(),
        }
    }
}

/// A line starting with "Total" (or "Totals"): the rest of it, else None.
fn total_line(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let low = t.to_ascii_lowercase();
    let n = if low.starts_with("totals") { 6 } else if low.starts_with("total") { 5 } else { return None };
    let rest = &t[n..];
    if rest.chars().next().is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    let rest = rest.trim_start_matches(' ');
    Some(rest.strip_prefix([':', ',', ';', '\t']).unwrap_or(rest))
}

/// A line's boxes before any `=`, and what follows the `=` (a typed total).
fn split_total(line: &str) -> (Vec<String>, Option<String>) {
    let (left, right) = match line.split_once('=') {
        Some((a, b)) => (a, Some(b.trim().to_string())),
        None => (line, None),
    };
    // a tab line keeps its empty boxes; parse_table drops a line that is only separators, so split here
    let cells = if left.trim().is_empty() {
        Vec::new()
    } else if left.contains('\t') {
        left.trim_end_matches([' ', '\t']).split('\t').map(|c| c.trim().to_string()).collect()
    } else {
        data::parse_table(left).into_iter().next().unwrap_or_default()
    };
    (cells, right)
}

fn is_word(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t != "?" && !t.starts_with('[') && number(t).is_none()
}

fn is_total_name(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "total" | "totals")
}

/// A table pasted with its headings: column headings across the first line, a row name at the start
/// of every other line, and "Total" as the last heading or row name for typed totals.
pub fn headed(text: &str) -> Option<Model> {
    let lines: Vec<Vec<String>> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| if l.contains('\t') { l.trim_end_matches([' ', '\t', '\r']).split('\t').map(|c| c.trim().to_string()).collect() } else { data::parse_table(l).into_iter().next().unwrap_or_default() })
        .collect();
    if lines.len() < 2 || lines[0].len() < 2 || text.contains('=') {
        return None;
    }
    if !lines[0][1..].iter().all(|s| is_word(s)) || !lines[1..].iter().all(|l| l.first().is_some_and(|s| is_word(s))) || !lines[1..].iter().any(|l| l.len() >= 2) {
        return None;
    }
    // single letters all round are unknowns in a block of counts, not headings
    if !lines[0][1..].iter().chain(lines[1..].iter().map(|l| &l[0])).any(|s| s.trim().chars().count() > 1) {
        return None;
    }
    let tidy = |s: &str| s.trim().replace([',', ';'], " ");
    let mut cols: Vec<String> = lines[0][1..].iter().map(|s| tidy(s)).collect();
    let tot_col = cols.len() >= 2 && is_total_name(cols.last().unwrap());
    let tot_idx = cols.len(); // index in a line of the Total column (after the row name)
    if tot_col {
        cols.pop();
    }
    cols.truncate(MAX_LINES);
    let nc = cols.len();
    let mut body: Vec<&Vec<String>> = lines[1..].iter().collect();
    let tot_row = body.len() >= 2 && is_total_name(&body.last().unwrap()[0]);
    let total = if tot_row { body.pop() } else { None };
    let get = |l: &Vec<String>, i: usize| l.get(i).map_or(Entry::Empty, |s| entry(s));
    let rows: Vec<String> = body.iter().map(|l| tidy(&l[0])).collect();
    let cells = body.iter().map(|l| (0..nc).map(|c| get(l, 1 + c)).collect()).collect();
    let row_tot = body.iter().map(|l| if tot_col { get(l, tot_idx) } else { Entry::Absent }).collect();
    let (col_tot, grand) = match total {
        Some(l) => ((0..nc).map(|c| get(l, 1 + c)).collect(), if tot_col { get(l, tot_idx) } else { Entry::Absent }),
        None => (vec![Entry::Absent; nc], Entry::Absent),
    };
    Some(Model { rows, cols, cells, row_tot, col_tot, grand, headed: true, corner: lines[0][0].trim().to_string() })
}

/// The table from the name boxes and the Counts text.
pub fn model(p: &Params) -> Model {
    let text = p.s("cells");
    if let Some(m) = headed(text) {
        return m;
    }
    let mut cell_lines: Vec<(Vec<String>, Option<String>)> = Vec::new();
    let mut total: Option<(Vec<String>, Option<String>)> = None;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match total_line(line) {
            Some(rest) => {
                if total.is_none() {
                    total = Some(split_total(rest));
                }
            }
            None => cell_lines.push(split_total(line)),
        }
    }
    let typed_cols = cell_lines.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    let rows = labels(p.s("row_labels"), "Row", cell_lines.len());
    let cols = labels(p.s("col_labels"), "Column", typed_cols.min(MAX_LINES));
    let (nr, nc) = (rows.len(), cols.len());
    let cells = (0..nr).map(|r| (0..nc).map(|c| cell_lines.get(r).and_then(|(l, _)| l.get(c)).map_or(Entry::Empty, |s| entry(s))).collect()).collect();
    let row_tot = (0..nr).map(|r| cell_lines.get(r).and_then(|(_, t)| t.as_deref()).map_or(Entry::Absent, entry)).collect();
    let (col_tot, grand) = match &total {
        Some((l, eq)) => (
            (0..nc).map(|c| l.get(c).map_or(Entry::Absent, |s| entry(s))).collect(),
            match eq {
                Some(g) => entry(g),
                None => l.get(nc).map_or(Entry::Absent, |s| entry(s)),
            },
        ),
        None => (vec![Entry::Absent; nc], Entry::Absent),
    };
    Model { rows, cols, cells, row_tot, col_tot, grand, headed: false, corner: String::new() }
}

// ---------- working out the missing boxes ----------

/// The sums that hold in any two-way table, as lists of (box, coefficient) adding to 0: each row's
/// counts less its total, each column's counts less its total, and both sets of totals less the
/// grand total.
fn equations(m: &Model) -> Vec<Vec<(Slot, f64)>> {
    let (nr, nc) = (m.nr(), m.nc());
    let mut eqs = Vec::new();
    for r in 0..nr {
        let mut e: Vec<(Slot, f64)> = (0..nc).map(|c| (Slot::Cell(r, c), 1.0)).collect();
        e.push((Slot::RowTot(r), -1.0));
        eqs.push(e);
    }
    for c in 0..nc {
        let mut e: Vec<(Slot, f64)> = (0..nr).map(|r| (Slot::Cell(r, c), 1.0)).collect();
        e.push((Slot::ColTot(c), -1.0));
        eqs.push(e);
    }
    let mut a: Vec<(Slot, f64)> = (0..nr).map(|r| (Slot::RowTot(r), 1.0)).collect();
    a.push((Slot::Grand, -1.0));
    eqs.push(a);
    let mut b: Vec<(Slot, f64)> = (0..nc).map(|c| (Slot::ColTot(c), 1.0)).collect();
    b.push((Slot::Grand, -1.0));
    eqs.push(b);
    eqs
}

/// Every box whose value the `known` ones fix (Gauss-Jordan on the table's sums: a box is fixed when
/// its pivot row has nothing left in a free column). Values are rounded to 1e-9.
pub fn solve(m: &Model, known: &[Option<f64>]) -> Vec<Option<f64>> {
    let unk: Vec<usize> = (0..known.len()).filter(|&i| known[i].is_none()).collect();
    let k = unk.len();
    let mut col_of = vec![usize::MAX; known.len()];
    for (j, &i) in unk.iter().enumerate() {
        col_of[i] = j;
    }
    let mut a: Vec<Vec<f64>> = equations(m)
        .iter()
        .map(|eq| {
            let mut row = vec![0.0; k + 1];
            for &(s, coef) in eq {
                let i = m.index(s);
                match known[i] {
                    Some(v) => row[k] -= coef * v,
                    None => row[col_of[i]] += coef,
                }
            }
            row
        })
        .collect();
    let mut pivot = vec![None; k];
    let mut next = 0;
    for col in 0..k {
        let Some(best) = (next..a.len()).max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs())) else { break };
        if a[best][col].abs() < 1e-9 {
            continue;
        }
        a.swap(next, best);
        let pv = a[next][col];
        for x in a[next].iter_mut() {
            *x /= pv;
        }
        for r in 0..a.len() {
            if r != next && a[r][col].abs() > 0.0 {
                let f = a[r][col];
                let src = a[next].clone();
                for (x, s) in a[r].iter_mut().zip(src) {
                    *x -= f * s;
                }
            }
        }
        pivot[col] = Some(next);
        next += 1;
    }
    let mut out = known.to_vec();
    for col in 0..k {
        if let Some(r) = pivot[col]
            && (0..k).all(|c| c == col || pivot[c].is_some() || a[r][c].abs() < 1e-9)
        {
            out[unk[col]] = Some((a[r][k] * 1e9).round() / 1e9);
        }
    }
    out
}

/// How a box looks on the student copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    Shown,
    Blank,
    Letter,
}

/// What a box holds when drawn.
#[derive(Clone, Debug, PartialEq)]
enum Disp {
    Text(String),
    Frac(String, String),
    /// The letter, and its value for the key.
    Letter(String, Option<String>),
}

impl Disp {
    fn chars(&self) -> usize {
        match self {
            Disp::Text(s) => s.chars().count(),
            Disp::Frac(a, b) => a.chars().count().max(b.chars().count()),
            Disp::Letter(l, v) => l.chars().count() + v.as_ref().map_or(0, |v| 3 + v.chars().count()),
        }
    }
}

/// The table worked out: the model, every box's true value, and how each looks to students.
pub struct Worked {
    pub m: Model,
    pub truth: Vec<Option<f64>>,
    pub tc: bool,
    pub tr: bool,
    auto_tot: bool,
    picks: Vec<usize>,
}

impl Worked {
    pub fn new(p: &Params) -> Self {
        Self::from_model(model(p), p)
    }
    pub fn from_model(m: Model, p: &Params) -> Self {
        let mode = p.s("blank");
        let auto_tot = matches!(mode, "Totals" | "Totals and some cells");
        let picks = if matches!(mode, "Some cells" | "Totals and some cells") { blanked(m.nr(), m.nc(), p.i("blank_count").max(0) as usize) } else { Vec::new() };
        let slots = m.slots();
        let known: Vec<Option<f64>> = slots
            .iter()
            .map(|&s| match m.at(s) {
                Entry::Num(v) | Entry::Hidden(v) => Some(*v),
                _ => None,
            })
            .collect();
        let mut truth = solve(&m, &known);
        // totals that were not typed are the plain sums of the counts (added in the same order as ever,
        // so an old table's totals print exactly as they did)
        let (nr, nc) = (m.nr(), m.nc());
        let cell = |t: &[Option<f64>], r: usize, c: usize| t[r * nc + c];
        for r in 0..nr {
            if m.row_tot[r] == Entry::Absent
                && let Some(v) = (0..nc).map(|c| cell(&truth, r, c)).collect::<Option<Vec<f64>>>()
            {
                truth[nr * nc + r] = Some(v.iter().sum());
            }
        }
        for c in 0..nc {
            if m.col_tot[c] == Entry::Absent
                && let Some(v) = (0..nr).map(|r| cell(&truth, r, c)).collect::<Option<Vec<f64>>>()
            {
                truth[nr * nc + nr + c] = Some(v.iter().sum());
            }
        }
        if m.grand == Entry::Absent
            && let Some(v) = (0..nr).map(|r| truth[nr * nc + r]).collect::<Option<Vec<f64>>>()
        {
            truth[nr * nc + nr + nc] = Some(v.iter().sum());
        }
        Worked { m, truth, tc: p.b("total_col"), tr: p.b("total_row"), auto_tot, picks }
    }
    pub fn truth(&self, s: Slot) -> Option<f64> { self.truth[self.m.index(s)] }
    pub fn drawn(&self, s: Slot) -> bool {
        match s {
            Slot::Cell(..) => true,
            Slot::RowTot(_) => self.tc,
            Slot::ColTot(_) => self.tr,
            Slot::Grand => self.tc && self.tr,
        }
    }
    pub fn seen(&self, s: Slot) -> Seen {
        match self.m.at(s) {
            Entry::Letter(_) => Seen::Letter,
            Entry::Hidden(_) | Entry::Empty => Seen::Blank,
            Entry::Num(_) | Entry::Absent => {
                let auto = match s {
                    Slot::Cell(r, c) => self.picks.contains(&(r * self.m.nc() + c)),
                    _ => self.auto_tot,
                };
                if auto { Seen::Blank } else { Seen::Shown }
            }
        }
    }
    /// The drawn boxes a student has to fill in or find (blank ones and letters).
    pub fn missing(&self) -> Vec<Slot> {
        self.m.slots().into_iter().filter(|&s| self.drawn(s) && self.seen(s) != Seen::Shown).collect()
    }
    /// What a student can work out from the boxes shown to them.
    pub fn student(&self) -> Vec<Option<f64>> {
        let known: Vec<Option<f64>> = self.m.slots().into_iter().map(|s| if self.drawn(s) && self.seen(s) == Seen::Shown { self.truth(s) } else { None }).collect();
        solve(&self.m, &known)
    }
}

/// `v` to `places` decimals, without a "-0".
fn dec(v: f64, places: usize) -> String {
    let s = format!("{v:.places$}");
    if s.starts_with('-') && s.trim_start_matches(['-', '0', '.']).is_empty() { s[1..].to_string() } else { s.replace('-', "\u{2212}") }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

/// The relative frequency's denominator for box `s`: the grand total, the box's row total or its column total.
fn denominator(values: &str, s: Slot) -> Slot {
    match (values, s) {
        (OF_ROW, Slot::Cell(r, _) | Slot::RowTot(r)) => Slot::RowTot(r),
        (OF_COL, Slot::Cell(_, c) | Slot::ColTot(c)) => Slot::ColTot(c),
        _ => Slot::Grand,
    }
}

/// Box `s` as drawn in the key: a count, a relative frequency, or the letter with its value.
fn disp(w: &Worked, p: &Params, s: Slot) -> Option<Disp> {
    let v = w.truth(s);
    let shown = v.and_then(|v| value_disp(w, p, s, v));
    match w.m.at(s) {
        Entry::Letter(l) => Some(Disp::Letter(
            l.clone(),
            shown.map(|d| match d {
                Disp::Frac(a, b) => format!("{a}/{b}"),
                Disp::Text(t) | Disp::Letter(t, _) => t,
            }),
        )),
        _ => shown,
    }
}

fn value_disp(w: &Worked, p: &Params, s: Slot, v: f64) -> Option<Disp> {
    let values = p.s("values");
    if values == COUNTS || !matches!(values, OF_TOTAL | OF_ROW | OF_COL) {
        return Some(Disp::Text(chart::fmt(v)));
    }
    let d = w.truth(denominator(values, s))?;
    if d == 0.0 {
        return Some(Disp::Text("\u{2013}".into()));
    }
    let places = p.i("rf_places").clamp(0, 4) as usize;
    Some(match p.s("rf_as") {
        "Percent" => Disp::Text(format!("{}%", ratio_text(v * 100.0, d, places))),
        "Fraction" => {
            let whole = |x: f64| (x - x.round()).abs() < 1e-9 && x.abs() < 1e15;
            if !(whole(v) && whole(d)) {
                return Some(Disp::Text(ratio_text(v, d, places)));
            }
            let (a, b) = (v.round() as i64, d.round() as i64);
            let neg = (a < 0) != (b < 0);
            let g = gcd(a.unsigned_abs(), b.unsigned_abs());
            let (a, b) = (a.unsigned_abs() / g, b.unsigned_abs() / g);
            let sign = if neg && a != 0 { "\u{2212}" } else { "" };
            if a == 0 {
                Disp::Text("0".into())
            } else if b == 1 {
                Disp::Text(format!("{sign}{a}"))
            } else {
                Disp::Frac(format!("{sign}{a}"), b.to_string())
            }
        }
        _ => Disp::Text(ratio_text(v, d, places)),
    })
}

/// `v / d` to `places` decimals, a half rounded up as on paper: 34 / 80 = 0.425 is 0.43 (the float
/// 0.425 sits just under the half, so plain formatting would print 0.42). Whole counts are rounded
/// exactly in integers; anything else rounds the scaled float.
fn ratio_text(v: f64, d: f64, places: usize) -> String {
    let scale = 10f64.powi(places as i32);
    let whole = |x: f64| (x - x.round()).abs() < 1e-9 && x.abs() < 1e12;
    let units: f64 = if whole(v) && whole(d) && d != 0.0 {
        let (a, b) = ((v.round() as i128).abs() * 10i128.pow(places as u32), (d.round() as i128).abs());
        let r = (2 * a + b) / (2 * b);
        let neg = (v < 0.0) != (d < 0.0);
        (if neg { -r } else { r }) as f64
    } else {
        (v / d * scale).round()
    };
    dec(units / scale, places)
}

// ---------- layout ----------

/// Where everything goes; shared by the render and the click handles.
struct Lay {
    fs: f64,
    rowh: f64,
    head_h: f64,
    split: bool,
    x0: f64,
    y0: f64,
    first_w: f64,
    col_w: Vec<f64>,
    xs: Vec<f64>,
    table_w: f64,
    table_h: f64,
    top_pad: f64,
    span_h: f64,
    w: f64,
    h: f64,
}

impl Lay {
    /// Top of table row `i`: 0 the header, 1..=nr the rows, nr + 1 the Total row (and the bottom).
    fn row_top(&self, i: usize) -> f64 {
        if self.split {
            if i == 0 { self.y0 } else { self.y0 + self.head_h + self.rowh * (i - 1) as f64 }
        } else {
            self.y0 + self.rowh * i as f64
        }
    }
    /// The box of slot `s`: (x, y, width).
    fn rect(&self, nr: usize, nc: usize, s: Slot) -> (f64, f64, f64) {
        let (r, c) = match s {
            Slot::Cell(r, c) => (r + 1, c),
            Slot::RowTot(r) => (r + 1, nc),
            Slot::ColTot(c) => (nr + 1, c),
            Slot::Grand => (nr + 1, nc),
        };
        (self.xs[c + 1], self.row_top(r), self.col_w[c])
    }
}

fn layout(w: &Worked, p: &Params, disps: &[(Slot, Option<Disp>)]) -> Lay {
    let (nr, nc) = (w.m.nr(), w.m.nc());
    let k = svg::label_factor(p.s("label_size"));
    let fs = 17.0 * k;
    let char_w = fs * 0.62;
    let (title, row_title, col_title) = (p.s("title").trim(), row_title(w, p), p.s("col_title").trim());
    let split = p.b("corner_split");
    let any_frac = disps.iter().any(|(_, d)| matches!(d, Some(Disp::Frac(..))));
    let rowh = if any_frac { fs * 3.1 } else { fs * 2.5 };
    let head_h = if split { rowh.max(fs * 3.4) } else { rowh };
    let wid = |s: Slot| disps.iter().find(|(t, _)| *t == s).and_then(|(_, d)| d.as_ref()).map_or(1, Disp::chars);
    let mut first_w = (w.m.rows.iter().map(|s| s.chars().count()).chain([if split { 0 } else { row_title.chars().count() }, 5]).max().unwrap() as f64 * char_w + 28.0).max(90.0);
    if split {
        // the column variable sits above the diagonal at the top right, the row variable under it at the bottom left
        let above = (8.0 + col_title.chars().count() as f64 * char_w + 6.0) / (1.0 - (fs * 1.4 + 3.0) / head_h);
        let below = head_h * (8.0 + row_title.chars().count() as f64 * char_w + 6.0) / (head_h - fs * 1.2 - 3.0);
        first_w = first_w.max(above).max(below);
    }
    let ncols = nc + w.tc as usize;
    let col_w: Vec<f64> = (0..ncols)
        .map(|c| {
            let name = if c < nc { w.m.cols[c].chars().count() } else { 5 };
            let mut widest = (0..nr).map(|r| wid(if c < nc { Slot::Cell(r, c) } else { Slot::RowTot(r) })).max().unwrap_or(1);
            if w.tr {
                widest = widest.max(wid(if c < nc { Slot::ColTot(c) } else { Slot::Grand }));
            }
            (name.max(widest) as f64 * char_w + 28.0).max(80.0)
        })
        .collect();
    let has_title = !title.is_empty() || p.s("title_mode") == "Blank line";
    let top_pad = if has_title { 36.0 + fs * 1.1 } else { 14.0 };
    let span_h = if col_title.is_empty() || split { 0.0 } else { rowh * 0.8 };
    let table_h = if split { head_h + rowh * (nr + w.tr as usize) as f64 } else { rowh * (nr as f64 + 1.0 + w.tr as usize as f64) };
    let (cw, ch) = (20.0 + first_w + col_w.iter().sum::<f64>() + 20.0, top_pad + span_h + table_h + 16.0);
    let x0 = 20.0;
    let y0 = top_pad + span_h;
    let xs: Vec<f64> = std::iter::once(x0).chain(col_w.iter().scan(x0 + first_w, |x, cw| { let v = *x; *x += cw; Some(v) })).collect();
    let table_w = first_w + col_w.iter().sum::<f64>();
    Lay { fs, rowh, head_h, split, x0, y0, first_w, col_w, xs, table_w, table_h, top_pad, span_h, w: cw, h: ch }
}

/// The row variable: typed, else a pasted table's corner heading.
fn row_title<'a>(w: &'a Worked, p: &'a Params) -> &'a str {
    let t = p.s("row_title").trim();
    if t.is_empty() && w.m.headed { w.m.corner.as_str() } else { t }
}

fn render(p: &Params) -> String {
    let w = Worked::new(p);
    let (nr, nc) = (w.m.nr(), w.m.nc());
    let answers = p.b(SHOW_ANSWERS);
    let slots: Vec<Slot> = w.m.slots().into_iter().filter(|&s| w.drawn(s)).collect();
    let disps: Vec<(Slot, Option<Disp>)> = slots.iter().map(|&s| (s, disp(&w, p, s))).collect();
    let l = layout(&w, p, &disps);
    let (fs, rowh, x0, y0) = (l.fs, l.rowh, l.x0, l.y0);
    let fills = Fills::new(p);
    let tint = if fills.is_print() { "#ececec" } else { "#f0e9db" };
    let (title, col_title) = (p.s("title").trim(), p.s("col_title").trim());
    let row_title = row_title(&w, p).to_string();
    let char_w = fs * 0.62;
    let plain = p.s("headings") == "Plain";
    let every = p.s("lines") != "Header and totals";
    let totals_bold = p.b("totals_bold");

    let mut d = Doc::new(l.w, l.h);
    if !title.is_empty() || p.s("title_mode") == "Blank line" {
        let ty = 14.0 + fs * 1.1;
        if p.s("title_mode") == "Blank line" {
            let half = (title.chars().count() as f64 * fs * 0.4).max(fs * 6.0).min(l.w / 2.0 - 24.0);
            d.line(l.w / 2.0 - half, ty + fs * 0.3, l.w / 2.0 + half, ty + fs * 0.3, INK, 1.4);
            if answers && !title.is_empty() {
                chart::text(&mut d, l.w / 2.0, ty, fs * 1.15, "middle", title);
            }
        } else {
            chart::text(&mut d, l.w / 2.0, ty, fs * 1.15, "middle", title);
        }
    }
    let cell_x = |c: usize| l.xs[c + 1];
    // a line of text centred in a box `hgt` tall (a row keeps its old baseline exactly)
    let text_at = |d: &mut Doc, x: f64, y: f64, wid: f64, hgt: f64, s: &str, bold: bool| {
        let base = if hgt == rowh { y + rowh * 0.64 } else { y + hgt / 2.0 + fs * 0.35 };
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{fs}" text-anchor="middle" font-weight="{}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
            x + wid / 2.0,
            base,
            if bold { 700 } else { 400 },
            esc(s)
        ));
    };
    if !col_title.is_empty() && !l.split {
        let (a, b) = (cell_x(0), cell_x(nc));
        chart::text(&mut d, (a + b) / 2.0, l.top_pad + l.span_h * 0.7, fs * 1.05, "middle", col_title);
    }
    let total_y = l.row_top(nr + 1);
    let edge = if every { INK } else { "none" };
    if !plain {
        d.rect(x0, y0, l.table_w, l.head_h, tint, edge, 1.6);
        if w.tr {
            d.rect(x0, total_y, l.table_w, rowh, tint, edge, 1.6);
        }
        d.rect(x0, y0 + l.head_h, l.first_w, rowh * nr as f64, tint, edge, 1.6);
        if w.tc {
            d.rect(cell_x(nc), y0 + l.head_h, l.col_w[nc], rowh * nr as f64, tint, edge, 1.6);
        }
    }
    let ncols = nc + w.tc as usize;
    let nrows = nr + 1 + w.tr as usize;
    if every {
        for i in 0..nrows {
            let y = l.row_top(i);
            d.line(x0, y, x0 + l.table_w, y, INK, 1.6);
        }
        for c in 0..=ncols {
            let x = l.xs[c];
            d.line(x, y0, x, y0 + l.table_h, INK, 1.6);
        }
        d.line(x0 + l.table_w, y0, x0 + l.table_w, y0 + l.table_h, INK, 1.6);
        d.rect(x0, y0, l.table_w, l.table_h, "none", INK, 2.4);
    } else {
        // only a rule under the headings, one before the totals, one after the row names and one before the Total column
        d.line(x0, y0 + l.head_h, x0 + l.table_w, y0 + l.head_h, INK, 2.0);
        if w.tr {
            d.line(x0, total_y, x0 + l.table_w, total_y, INK, 2.0);
        }
        d.line(x0 + l.first_w, y0, x0 + l.first_w, y0 + l.table_h, INK, 2.0);
        if w.tc {
            d.line(cell_x(nc), y0, cell_x(nc), y0 + l.table_h, INK, 2.0);
        }
    }
    if l.split {
        d.line(x0, y0, x0 + l.first_w, y0 + l.head_h, INK, 1.4);
        let font = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
        for (x, y, anchor, s) in [(x0 + l.first_w - 8.0, y0 + fs * 1.15, "end", col_title), (x0 + 8.0, y0 + l.head_h - fs * 0.45, "start", row_title.as_str())] {
            if !s.is_empty() {
                d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs}" text-anchor="{anchor}" font-weight="700" font-family="{font}" fill="{INK}">{}</text>"##, esc(s)));
            }
        }
    } else {
        text_at(&mut d, x0, y0, l.first_w, l.head_h, &row_title, false);
    }
    for c in 0..nc {
        text_at(&mut d, cell_x(c), y0, l.col_w[c], l.head_h, &w.m.cols[c], true);
    }
    if w.tc {
        text_at(&mut d, cell_x(nc), y0, l.col_w[nc], l.head_h, "Total", true);
    }
    let draw = |d: &mut Doc, s: Slot| {
        let (x, y, wid) = l.rect(nr, nc, s);
        let seen = w.seen(s);
        if seen == Seen::Blank && !answers {
            return;
        }
        let Some(item) = disps.iter().find(|(t, _)| *t == s).and_then(|(_, v)| v.clone()) else { return };
        let is_total = !matches!(s, Slot::Cell(..));
        let bold = (answers && seen != Seen::Shown) || (totals_bold && is_total);
        match item {
            Disp::Text(t) => text_at(d, x, y, wid, rowh, &t, bold),
            Disp::Frac(a, b) => {
                let (cx, cy) = (x + wid / 2.0, y + rowh / 2.0);
                let half = a.chars().count().max(b.chars().count()) as f64 * char_w * 0.5 + 3.0;
                for (t, by) in [(&a, cy - fs * 0.25), (&b, cy + fs * 0.98)] {
                    d.raw(&format!(
                        r##"<text x="{cx:.2}" y="{by:.2}" font-size="{fs}" text-anchor="middle" font-weight="{}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}">{}</text>"##,
                        if bold { 700 } else { 400 },
                        esc(t)
                    ));
                }
                d.line(cx - half, cy, cx + half, cy, INK, if bold { 2.2 } else { 1.5 });
            }
            Disp::Letter(name, value) => {
                let value = value.filter(|_| answers);
                let bold = bold && value.is_some();
                d.raw(&format!(
                    r##"<text x="{:.2}" y="{:.2}" font-size="{fs}" text-anchor="middle" font-weight="{}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{INK}"><tspan font-style="italic" font-family="Noto Serif, DejaVu Serif, Liberation Serif, serif">{}</tspan>{}</text>"##,
                    x + wid / 2.0,
                    y + rowh * 0.64,
                    if bold { 700 } else { 400 },
                    esc(&name),
                    value.map(|v| format!(" = {}", esc(&v))).unwrap_or_default()
                ));
            }
        }
    };
    for r in 0..nr {
        let y = l.row_top(r + 1);
        text_at(&mut d, x0, y, l.first_w, rowh, &w.m.rows[r], true);
        for c in 0..nc {
            draw(&mut d, Slot::Cell(r, c));
        }
        if w.tc {
            draw(&mut d, Slot::RowTot(r));
        }
    }
    if w.tr {
        text_at(&mut d, x0, total_y, l.first_w, rowh, "Total", true);
        for c in 0..nc {
            draw(&mut d, Slot::ColTot(c));
        }
        if w.tc {
            draw(&mut d, Slot::Grand);
        }
    }
    d.finish()
}

// ---------- editing: clicks on the table, pastes, old saves ----------

/// A count as typed text (rounded to 1e-9, so a worked-out sum reads 0.3, not 0.30000000000000004).
fn num_tok(v: f64) -> String {
    format!("{}", (v * 1e9).round() / 1e9)
}

fn tok(e: &Entry) -> String {
    match e {
        Entry::Num(v) => num_tok(*v),
        Entry::Hidden(v) => format!("[{}]", num_tok(*v)),
        Entry::Empty | Entry::Absent => "?".into(),
        Entry::Letter(s) => s.clone(),
    }
}

/// The Counts text for `m`: one line per row, ` = total` where a total is typed, a Total line when any is.
pub fn write_counts(w: &Worked, m: &Model) -> String {
    let mut lines: Vec<String> = (0..m.nr())
        .map(|r| {
            let mut s = m.cells[r].iter().map(tok).collect::<Vec<_>>().join(", ");
            if m.row_tot[r] != Entry::Absent {
                s.push_str(&format!(" = {}", tok(&m.row_tot[r])));
            }
            s
        })
        .collect();
    if m.col_tot.iter().any(|e| *e != Entry::Absent) || m.grand != Entry::Absent {
        // a total left to be worked out is written as its value, so the line keeps its places
        let cols: Vec<String> = (0..m.nc()).map(|c| match &m.col_tot[c] {
            Entry::Absent => w.truth(Slot::ColTot(c)).map_or("?".into(), num_tok),
            e => tok(e),
        }).collect();
        let mut s = format!("Total, {}", cols.join(", "));
        if m.grand != Entry::Absent {
            s.push_str(&format!(" = {}", tok(&m.grand)));
        }
        lines.push(s);
    }
    lines.join("\n")
}

fn write_model(p: &mut Params, w: &Worked, m: &Model) {
    if m.headed {
        p.set_s("row_labels", &m.rows.join(", "));
        p.set_s("col_labels", &m.cols.join(", "));
        if p.s("row_title").trim().is_empty() && !m.corner.is_empty() {
            p.set_s("row_title", &m.corner.replace([',', ';'], " "));
        }
    }
    p.set_s("cells", &write_counts(w, m));
}

/// A click on box `s` of the table: shown to students becomes blank for them and back. The first click
/// on a table blanked by a choice (Totals, Some cells) writes those blanks into the counts and turns
/// Blank for students to Boxes I choose, so the table looks the same until the clicked box changes.
pub fn toggle(p: &mut Params, s: Slot) {
    let w = Worked::new(p);
    let mut m = w.m.clone();
    if p.s("blank") != CHOOSE {
        for t in m.slots() {
            if w.drawn(t) && w.seen(t) == Seen::Blank && matches!(m.at(t), Entry::Num(_) | Entry::Absent) {
                *m.at_mut(t) = w.truth(t).map_or(Entry::Empty, Entry::Hidden);
            }
        }
        p.set_s("blank", CHOOSE);
    }
    let truth = w.truth(s);
    let e = m.at_mut(s);
    *e = match e.clone() {
        Entry::Num(v) => Entry::Hidden(v),
        Entry::Hidden(v) => Entry::Num(v),
        Entry::Absent | Entry::Letter(_) => truth.map_or(Entry::Empty, Entry::Hidden),
        Entry::Empty => truth.map_or(Entry::Empty, Entry::Num),
    };
    write_model(p, &w, &m);
}

/// Click handles: one over every value box of the table.
pub fn handles(p: &Params) -> Vec<Handle> {
    let w = Worked::new(p);
    let (nr, nc) = (w.m.nr(), w.m.nc());
    let slots: Vec<Slot> = w.m.slots().into_iter().filter(|&s| w.drawn(s)).collect();
    let disps: Vec<(Slot, Option<Disp>)> = slots.iter().map(|&s| (s, disp(&w, p, s))).collect();
    let l = layout(&w, p, &disps);
    slots
        .into_iter()
        .map(|s| {
            let (x, y, wid) = l.rect(nr, nc, s);
            let tip = match (w.seen(s), w.truth(s)) {
                (Seen::Blank, Some(_)) => format!("Show {} to students", w.m.name(s)),
                (Seen::Blank, None) => format!("{} can't be worked out: type its count to show it", w.m.name(s)),
                _ => format!("Leave {} blank for students", w.m.name(s)),
            };
            Handle::zone((x + wid / 2.0, y + l.rowh / 2.0), (wid - 4.0, l.rowh - 4.0), &tip, move |q| toggle(q, s))
        })
        .collect()
}

/// After the Counts box changed: a table pasted from a spreadsheet with its headings is split into the
/// name boxes and the counts, and a pasted block whose last column and line are its sums becomes typed
/// totals. A paste with gaps (or letters) turns Blank for students to Boxes I choose: the gaps are what
/// students fill in. Returns whether anything changed.
pub fn pasted(old: &Value, p: &mut Params) -> bool {
    let text = p.s("cells").to_string();
    let old = match old { Value::S(s) => s.as_str(), _ => "" };
    // a paste, not typing: tabs, or two or more lines arriving at once
    if !(text.contains('\t') || text.lines().count() >= old.lines().count() + 2) {
        return false;
    }
    let m = if let Some(m) = headed(&text) {
        m
    } else if let Some(t) = positional_totals(&text) {
        let mut q = p.clone();
        q.set_s("cells", &t);
        model(&q)
    } else {
        return false;
    };
    let tc = m.row_tot.iter().any(|e| *e != Entry::Absent);
    let tr = m.col_tot.iter().any(|e| *e != Entry::Absent) || m.grand != Entry::Absent;
    let gaps = m.cells.iter().flatten().chain(&m.row_tot).chain(&m.col_tot).chain([&m.grand]).any(|e| matches!(e, Entry::Empty | Entry::Letter(_) | Entry::Hidden(_)));
    let w = Worked::from_model(m.clone(), p);
    write_model(p, &w, &m);
    if tc {
        p.set_b("total_col", true);
    }
    if tr {
        p.set_b("total_row", true);
    }
    if gaps {
        p.set_s("blank", CHOOSE);
    }
    true
}

/// A pasted block of counts whose last column holds each line's sum and whose last line holds each
/// column's sum (where the boxes are all filled in to check), written with ` = ` and a Total line.
pub fn positional_totals(text: &str) -> Option<String> {
    let lines: Vec<Vec<String>> = text.lines().filter(|l| !l.trim().is_empty()).map(|l| l.trim_end_matches(['\r', ' ', '\t']).split('\t').map(|c| c.trim().to_string()).collect()).collect();
    if lines.len() < 3 || text.contains('=') || lines.iter().any(|l| total_line(&l.join("\t")).is_some()) {
        return None;
    }
    let k = lines.iter().map(Vec::len).max()?;
    if k < 3 {
        return None;
    }
    let num = |r: usize, c: usize| lines[r].get(c).and_then(|s| number(s));
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0);
    let (nr, nc) = (lines.len() - 1, k - 1);
    let mut checked = (0, 0);
    for r in 0..nr {
        if let (Some(v), Some(t)) = ((0..nc).map(|c| num(r, c)).collect::<Option<Vec<f64>>>(), num(r, nc)) {
            if !close(v.iter().sum(), t) {
                return None;
            }
            checked.0 += 1;
        }
    }
    for c in 0..=nc {
        if let (Some(v), Some(t)) = ((0..nr).map(|r| num(r, c)).collect::<Option<Vec<f64>>>(), num(nr, c)) {
            if !close(v.iter().sum(), t) {
                return None;
            }
            checked.1 += 1;
        }
    }
    if checked.0 == 0 || checked.1 == 0 {
        return None;
    }
    let cell = |r: usize, c: usize| lines[r].get(c).cloned().unwrap_or_default();
    let mut out: Vec<String> = (0..nr).map(|r| format!("{} = {}", (0..nc).map(|c| cell(r, c)).collect::<Vec<_>>().join(", "), cell(r, nc))).collect();
    out.push(format!("Total, {} = {}", (0..nc).map(|c| cell(nr, c)).collect::<Vec<_>>().join(", "), cell(nr, nc)));
    Some(out.join("\n"))
}

/// Upgrades a two-way table saved before typed totals, blank boxes and letters (its spec has no
/// `total_col`): the counts are rewritten as the numbers it drew, every box filled in (a box that was
/// empty or a word drew 0), so nothing it typed reads as one of the new marks.
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    if spec.params.contains_key("total_col") {
        return;
    }
    let text = p.s("cells").to_string();
    let lines = data::parse_table(&text);
    let typed_cols = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let nr = labels(p.s("row_labels"), "Row", lines.len()).len();
    let nc = labels(p.s("col_labels"), "Column", typed_cols.min(MAX_LINES)).len();
    let mut g = vec![vec![0.0; nc]; nr];
    for (r, line) in lines.iter().take(nr).enumerate() {
        for (c, cell) in line.iter().take(nc).enumerate() {
            g[r][c] = data::numbers(cell).first().copied().unwrap_or(0.0);
        }
    }
    if lines.is_empty() {
        g = grid(&text, nr, nc);
    }
    let canon = g.iter().map(|row| row.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("\n");
    p.set_s("cells", &canon);
}

/// Editor notes: blank boxes students can't work out from what is shown, and typed totals that
/// don't match their counts.
pub fn warnings(p: &Params) -> Vec<String> {
    let w = Worked::new(p);
    let mut out = Vec::new();
    let missing = w.missing();
    let student = w.student();
    let stuck: Vec<Slot> = missing.iter().copied().filter(|&s| student[w.m.index(s)].is_none()).collect();
    if !stuck.is_empty() {
        let mut names: Vec<String> = stuck.iter().take(4).map(|&s| w.m.name(s)).collect();
        if stuck.len() > 4 {
            names.push(format!("{} more", stuck.len() - 4));
        }
        let of = if stuck.len() == missing.len() { if missing.len() == 1 { "the blank box".to_string() } else { format!("any of the {} blank boxes", missing.len()) } } else { format!("{} of the {} blank boxes", stuck.len(), missing.len()) };
        out.push(format!("Students can't work out {of} from what the table shows: {}.", names.join("; ")));
    }
    // typed totals against the counts they total
    let typed = |e: &Entry| matches!(e, Entry::Num(_) | Entry::Hidden(_));
    let val = |s: Slot| w.truth(s);
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0);
    let (nr, nc) = (w.m.nr(), w.m.nc());
    let given = |s: Slot| match w.m.at(s) { Entry::Num(v) | Entry::Hidden(v) => Some(*v), _ => None };
    for r in 0..nr {
        if typed(&w.m.row_tot[r])
            && let Some(v) = (0..nc).map(|c| given(Slot::Cell(r, c))).collect::<Option<Vec<f64>>>()
            && !close(v.iter().sum(), val(Slot::RowTot(r)).unwrap_or(f64::NAN))
        {
            out.push(format!("{}: the counts add up to {}, but its total is {}.", w.m.rows[r], chart::fmt(v.iter().sum()), chart::fmt(given(Slot::RowTot(r)).unwrap())));
        }
    }
    for c in 0..nc {
        if typed(&w.m.col_tot[c])
            && let Some(v) = (0..nr).map(|r| given(Slot::Cell(r, c))).collect::<Option<Vec<f64>>>()
            && !close(v.iter().sum(), given(Slot::ColTot(c)).unwrap())
        {
            out.push(format!("{}: the counts add up to {}, but its total is {}.", w.m.cols[c], chart::fmt(v.iter().sum()), chart::fmt(given(Slot::ColTot(c)).unwrap())));
        }
    }
    if let Some(g) = given(Slot::Grand) {
        for (what, parts) in [("row totals", (0..nr).map(|r| val(Slot::RowTot(r))).collect::<Option<Vec<f64>>>()), ("column totals", (0..nc).map(|c| val(Slot::ColTot(c))).collect::<Option<Vec<f64>>>())] {
            if let Some(v) = parts
                && !close(v.iter().sum(), g)
            {
                out.push(format!("The {what} add up to {}, but the grand total is {}.", chart::fmt(v.iter().sum()), chart::fmt(g)));
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "twoway_legacy.rs"]
mod legacy;

#[cfg(test)]
#[path = "twoway_tests.rs"]
mod tests;
