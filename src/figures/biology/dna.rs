//! DNA and protein synthesis: a sequence typed as the DNA template strand, the coding strand or the mRNA, drawn as
//! rows of base boxes (DNA coding strand, DNA template strand, mRNA, tRNA anticodons) with the amino acids under
//! the codons, and the standard codon chart beside, below or alone.
//!
//! The biology, as every row is worked out:
//! - The two DNA strands are antiparallel and complementary: A pairs with T and G with C. The coding strand is
//!   drawn 5′ to 3′ left to right, so the template strand under it reads 3′ to 5′.
//! - Transcription copies the template strand: the mRNA is complementary to the template, which makes it the coding
//!   strand with U in place of T, drawn 5′ to 3′.
//! - Translation reads the mRNA 5′ to 3′ in codons of three, from the first AUG (the start codon, methionine) or from
//!   the first base, with the standard genetic code, up to and including the first stop codon (UAA, UAG or UGA).
//!   A stop codon has no tRNA and no amino acid; it ends the chain. Codons after it are not read.
//! - Each tRNA's anticodon pairs with its codon, antiparallel: written base for base under the codon it reads
//!   3′ to 5′ left to right, so its first base pairs with the codon's first base (A with U, G with C).
//!
//! The figure also does what the reference generator offers: any row shown, blank, partly blank (a share set with a
//! slider and a shuffle) or off; row names as written, typed, blank lines or off; 5′ and 3′ ends shown, blank or off;
//! amino acids by three-letter or one-letter code or full name, as a chain of beads or words; gaps between codons,
//! a bracket under each mRNA codon, hydrogen bonds between the DNA strands (2 for A–T, 3 for G–C) and bases filled
//! to read apart in grey; up to four deliberate wrong bases in one row for multiple choice; the codon chart with the
//! start and stop codons in bold; a chart title and an answer key. Show answers fills each blank in the key style,
//! rings the deliberate mistakes and prints the key under the figure; off is the student copy.
//!
//! Sources checked: OpenStax, Biology 2e (2018), 15.1 "The Genetic Code" (figure 15.4, the 64 codons; "AUG ... also
//! serves as the start codon"; the three stop codons UAA, UAG and UGA), 15.2 "Prokaryotic Transcription" ("the mRNA
//! product is complementary to the template strand and is almost identical to the other DNA strand, called the
//! nontemplate strand, or the coding strand. The only nucleotide difference is that in mRNA, all of the T nucleotides
//! are replaced with U nucleotides") and 15.5 "Ribosomes and Protein Synthesis" (tRNA anticodons base-pair with the
//! mRNA codons). The drawing and the code are original.

use super::cladogram::text_w;
use super::*;

pub const ID: &str = "dna-protein-synthesis";

/// The sequence the figure opens with: a template strand from TAC that reads Met–Glu–Pro–Ala–Phe and a stop.
pub const DEFAULT_SEQ: &str = "TACCTTGGACGTAAAATT";

/// At most this many bases (12 codons) are drawn; the rest of a longer sequence is left off with a note.
pub const MAX_BASES: usize = 36;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "DNA and Protein Synthesis",
        blurb: "A sequence typed as the template strand, the coding strand or the mRNA, worked through to the tRNA anticodons and the amino acids, with the codon chart. Any row blank or partly blank for students, and wrong bases for multiple choice.",
        params,
        render,
    }]
}

const ROW_MODES: [&str; 4] = ["Shown", "Blank", "Some", "Off"];
const ROW_KEYS: [&str; 5] = ["row_coding", "row_template", "row_mrna", "row_trna", "row_amino"];
const NAME_KEYS: [&str; 5] = ["name_coding", "name_template", "name_mrna", "name_trna", "name_amino"];
pub const MISTAKE_ROWS: [&str; 4] = ["DNA coding strand", "DNA template strand", "mRNA", "tRNA anticodons"];

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("typed", "You’re typing the", &["Template", "Coding", "mRNA"], "Template")
            .hint("The DNA template strand is typed 3′ to 5′ (it usually starts TAC); the coding strand and the mRNA 5′ to 3′"),
        ParamDef::text("seq", "Sequence", DEFAULT_SEQ)
            .hint("Up to 36 bases (12 codons). Lower case, spaces, hyphens and written 5′ and 3′ ends are fine"),
        ParamDef::choice("frame", "Reading frame", &["From the first AUG", "From the first base"], "From the first AUG")
            .hint("From the first AUG leaves any bases before the start codon untranslated"),
        ParamDef::choice("row_coding", "DNA coding strand", &ROW_MODES, "Shown"),
        ParamDef::choice("row_template", "DNA template strand", &ROW_MODES, "Shown"),
        ParamDef::choice("row_mrna", "mRNA", &ROW_MODES, "Shown"),
        ParamDef::choice("row_trna", "tRNA anticodons", &ROW_MODES, "Shown"),
        ParamDef::choice("row_amino", "Amino acids", &ROW_MODES, "Shown"),
        ParamDef::int("some_share", "Blank in a Some row (%)", 10, 90, 50).hint("The share of a Some row left blank; Shuffle the blanks picks other ones"),
        ParamDef::choice("row_names", "Row names", &["Shown", "Blank", "Off"], "Shown").hint("Blank draws a line to write each row's name on"),
        ParamDef::text("name_coding", "DNA coding strand row name", "").hint("Empty keeps “DNA coding strand”"),
        ParamDef::text("name_template", "DNA template strand row name", "").hint("Empty keeps “DNA template strand”"),
        ParamDef::text("name_mrna", "mRNA row name", "").hint("Empty keeps “mRNA”"),
        ParamDef::text("name_trna", "tRNA anticodons row name", "").hint("Empty keeps “tRNA anticodons”"),
        ParamDef::text("name_amino", "Amino acids row name", "").hint("Empty keeps “Amino acids”"),
        ParamDef::choice("aa_names", "Amino acid names", &["Met", "M", "Methionine"], "Met").hint("Three-letter code, one-letter code or the full name"),
        ParamDef::choice("aa_style", "Amino acids drawn as", &["Chain of beads", "Words"], "Chain of beads"),
        ParamDef::choice("ends", "5′ and 3′ ends", &["Shown", "Blank", "Off"], "Shown"),
        ParamDef::bool("gaps", "Gaps between codons", true),
        ParamDef::bool("brackets", "Bracket under each codon", false).hint("Under the mRNA, one per codon read"),
        ParamDef::bool("hbonds", "Hydrogen bonds", false).hint("Between the two DNA strands: 2 for A–T, 3 for G–C"),
        ParamDef::bool("color_bases", "Colour the bases", false).hint("A, T or U, G and C each get a fill; Print uses four greys that stay apart on a copier"),
        ParamDef::choice("mistakes", "How many mistakes", &["None", "1", "2", "3", "4"], "None")
            .hint("Wrong bases in one row, for multiple choice: make choice A with none, then B to D with some and shuffle"),
        ParamDef::choice("mistake_row", "Mistakes in the row", &MISTAKE_ROWS, "mRNA"),
        ParamDef::choice("chart", "Show", &["Strands", "Strands and codon chart", "Codon chart only"], "Strands").hint("The standard codon chart, start and stop codons in bold"),
        ParamDef::choice("chart_pos", "Codon chart", &["On the right", "Below"], "On the right"),
        ParamDef::text("title", "Chart title (blank = none)", ""),
        ParamDef::bool(SHOW_ANSWERS, "Answer key", false)
            .hint("Prints the strands, the amino acids and any mistakes under the figure, fills each blank in grey and rings the mistakes. Off is the student copy"),
        ParamDef::int("blank_seed", "Blank pattern", 1, 99_999, 1),
        ParamDef::int("mistake_seed", "Mistake pattern", 1, 99_999, 1),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

/// Which panel controls apply to the current params (hidden ones keep their values).
pub fn relevant(key: &str, p: &Params) -> bool {
    let off = |k: &str| p.s(k) == "Off";
    match key {
        "some_share" => ROW_KEYS.iter().any(|k| p.s(k) == "Some"),
        "name_coding" | "name_template" | "name_mrna" | "name_trna" | "name_amino" => {
            let i = NAME_KEYS.iter().position(|k| *k == key).unwrap_or(0);
            p.s("row_names") != "Off" && !off(ROW_KEYS[i]) && p.s("chart") != "Codon chart only"
        }
        "aa_style" => !off("row_amino") && p.s("chart") != "Codon chart only",
        "hbonds" => !off("row_coding") && !off("row_template"),
        "brackets" => !off("row_mrna"),
        "mistake_row" => p.s("mistakes") != "None",
        "chart_pos" => p.s("chart") == "Strands and codon chart",
        // the seeds are set by the Shuffle buttons
        "blank_seed" | "mistake_seed" => false,
        _ => true,
    }
}

// ================================================================== bases and the genetic code

/// The DNA partner of a base (T for A and A for T or U, C for G, G for C).
pub fn dna_pair(b: u8) -> u8 {
    match b {
        b'A' => b'T',
        b'T' | b'U' => b'A',
        b'G' => b'C',
        _ => b'G',
    }
}

/// The RNA partner of a base (U for A, A for U or T, C for G, G for C).
pub fn rna_pair(b: u8) -> u8 {
    match b {
        b'A' => b'U',
        b'T' | b'U' => b'A',
        b'G' => b'C',
        _ => b'G',
    }
}

fn to_rna(b: u8) -> u8 { if b == b'T' { b'U' } else { b } }
fn to_dna(b: u8) -> u8 { if b == b'U' { b'T' } else { b } }

/// The standard genetic code, one-letter amino acid per codon, `*` for stop. Index = 16·first + 4·second + third
/// with the bases in the order U, C, A, G (the order of the codon chart).
const CODE: &[u8; 64] = b"FFLLSSSSYY**CC*WLLLLPPPPHHQQRRRRIIIMTTTTNNKKSSRRVVVVAAAADDEEGGGG";
const ORDER: [u8; 4] = [b'U', b'C', b'A', b'G'];

fn base_index(b: u8) -> usize {
    match b {
        b'U' | b'T' => 0,
        b'C' => 1,
        b'A' => 2,
        _ => 3,
    }
}

/// The amino acid an mRNA codon codes for, as its one-letter code (`*` = stop).
pub fn translate(codon: &[u8]) -> u8 {
    CODE[16 * base_index(codon[0]) + 4 * base_index(codon[1]) + base_index(codon[2])]
}

/// (one-letter, three-letter, full name) of the 20 amino acids and stop.
const NAMES: [(u8, &str, &str); 21] = [
    (b'A', "Ala", "Alanine"),
    (b'R', "Arg", "Arginine"),
    (b'N', "Asn", "Asparagine"),
    (b'D', "Asp", "Aspartic acid"),
    (b'C', "Cys", "Cysteine"),
    (b'Q', "Gln", "Glutamine"),
    (b'E', "Glu", "Glutamic acid"),
    (b'G', "Gly", "Glycine"),
    (b'H', "His", "Histidine"),
    (b'I', "Ile", "Isoleucine"),
    (b'L', "Leu", "Leucine"),
    (b'K', "Lys", "Lysine"),
    (b'M', "Met", "Methionine"),
    (b'F', "Phe", "Phenylalanine"),
    (b'P', "Pro", "Proline"),
    (b'S', "Ser", "Serine"),
    (b'T', "Thr", "Threonine"),
    (b'W', "Trp", "Tryptophan"),
    (b'Y', "Tyr", "Tyrosine"),
    (b'V', "Val", "Valine"),
    (b'*', "Stop", "Stop"),
];

/// How amino acids are named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Naming { Three, One, Full }

/// The name of the amino acid with one-letter code `aa` (`*` = "Stop" in every naming).
pub fn aa_name(aa: u8, n: Naming) -> &'static str {
    let Some(row) = NAMES.iter().find(|r| r.0 == aa) else { return "?" };
    if aa == b'*' {
        return "Stop";
    }
    match n {
        Naming::Three => row.1,
        Naming::Full => row.2,
        Naming::One => ONE_LETTER[NAMES.iter().position(|r| r.0 == aa).unwrap_or(0)],
    }
}

const ONE_LETTER: [&str; 21] = ["A", "R", "N", "D", "C", "Q", "E", "G", "H", "I", "L", "K", "M", "F", "P", "S", "T", "W", "Y", "V", "Stop"];

// ================================================================== the sequence, worked out

/// What the typed sequence says once read.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// The DNA coding strand, 5′ to 3′ (A, C, G, T).
    pub coding: Vec<u8>,
    /// The DNA template strand under it, base for base (so it reads 3′ to 5′ left to right).
    pub template: Vec<u8>,
    /// The mRNA, 5′ to 3′: the coding strand with U for T.
    pub mrna: Vec<u8>,
    /// Index of the first base of the first codon read (None: no AUG when reading from the first AUG).
    pub start: Option<usize>,
    /// Full codons from `start` to the end of the sequence.
    pub codons: usize,
    /// Codons translated: up to and including the first stop codon.
    pub read: usize,
    /// Whether translation ended at a stop codon.
    pub stopped: bool,
    /// Letters that are not bases, left out.
    pub ignored: Vec<char>,
    /// Bases past the 36th, left off.
    pub cut: usize,
    /// U typed in DNA (read as T) or T typed in an mRNA (read as U).
    pub swapped: bool,
}

impl Model {
    pub fn from_params(p: &Params) -> Model { Model::new(p.s("seq"), p.s("typed"), p.s("frame") != "From the first base") }

    pub fn new(text: &str, typed: &str, from_aug: bool) -> Model {
        let mut bases = Vec::new();
        let mut ignored = Vec::new();
        let mut cut = 0;
        let chars: Vec<char> = text.chars().collect();
        for (i, &c) in chars.iter().enumerate() {
            let u = c.to_ascii_uppercase();
            match u {
                'A' | 'C' | 'G' | 'T' | 'U' => {
                    if bases.len() < MAX_BASES { bases.push(u as u8) } else { cut += 1 }
                }
                // written ends (5′, 3', 5’), spaces and separators are not bases
                '5' | '3' if chars.get(i + 1).is_some_and(|n| matches!(n, '\'' | '′' | '’' | 'ʹ')) => {}
                '\'' | '′' | '’' | 'ʹ' | '-' | '–' | '—' | '.' | ',' | ';' | ':' | '_' | '/' | '|' => {}
                c if c.is_whitespace() => {}
                _ => ignored.push(c),
            }
        }
        let dna_typed = typed != "mRNA";
        let swapped = bases.iter().any(|&b| if dna_typed { b == b'U' } else { b == b'T' });
        let coding: Vec<u8> = match typed {
            "Coding" | "mRNA" => bases.iter().map(|&b| to_dna(b)).collect(),
            _ => bases.iter().map(|&b| dna_pair(b)).collect(),
        };
        let template: Vec<u8> = coding.iter().map(|&b| dna_pair(b)).collect();
        let mrna: Vec<u8> = coding.iter().map(|&b| to_rna(b)).collect();
        let start = if from_aug { mrna.windows(3).position(|w| w == b"AUG") } else if mrna.is_empty() { None } else { Some(0) };
        let codons = start.map_or(0, |s| (mrna.len() - s) / 3);
        let mut read = 0;
        let mut stopped = false;
        if let Some(s) = start {
            for k in 0..codons {
                read += 1;
                if translate(&mrna[s + 3 * k..s + 3 * k + 3]) == b'*' {
                    stopped = true;
                    break;
                }
            }
        }
        Model { coding, template, mrna, start, codons, read, stopped, ignored, cut, swapped }
    }

    pub fn len(&self) -> usize { self.coding.len() }

    /// The mRNA codon `k` of the reading frame.
    pub fn codon(&self, k: usize) -> &[u8] {
        let s = self.start.unwrap_or(0) + 3 * k;
        &self.mrna[s..s + 3]
    }

    /// The one-letter amino acids of the codons read (`*` for the stop codon that ends it).
    pub fn protein(&self) -> Vec<u8> { (0..self.read).map(|k| translate(self.codon(k))).collect() }

    /// The anticodon of codon `k`, base for base under it (so written 3′ to 5′).
    pub fn anticodon(&self, k: usize) -> Vec<u8> { self.codon(k).iter().map(|&b| rna_pair(b)).collect() }

    /// Codons that get a tRNA: every codon read except a stop codon.
    pub fn trnas(&self) -> usize { self.read - usize::from(self.stopped) }

    /// The group each base sits in, for the gaps between codons: the bases before the start codon, each codon from
    /// the start on, then the bases left after the last full codon. With no reading frame it is one group.
    pub fn groups(&self) -> Vec<(usize, usize)> {
        let n = self.len();
        let Some(s) = self.start else { return if n == 0 { vec![] } else { vec![(0, n)] } };
        let mut g = Vec::new();
        if s > 0 {
            g.push((0, s));
        }
        for k in 0..self.codons {
            g.push((s + 3 * k, 3));
        }
        let end = s + 3 * self.codons;
        if end < n {
            g.push((end, n - end));
        }
        g
    }

    /// The protein in words: "Met–Glu–Pro–Ala–Phe, then Stop".
    pub fn chain(&self, n: Naming) -> String {
        let prot = self.protein();
        let names: Vec<&str> = prot.iter().filter(|&&a| a != b'*').map(|&a| aa_name(a, n)).collect();
        let mut s = names.join("\u{2013}");
        if self.stopped {
            if s.is_empty() { s = "Stop".into() } else { s.push_str(", then Stop") }
        }
        s
    }
}

// ================================================================== rows, blanks and mistakes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row { Coding, Template, Mrna, Trna, Amino }

pub const ROWS: [Row; 5] = [Row::Coding, Row::Template, Row::Mrna, Row::Trna, Row::Amino];

impl Row {
    fn i(self) -> usize { ROWS.iter().position(|r| *r == self).unwrap_or(0) }
    pub fn default_name(self) -> &'static str {
        match self {
            Row::Coding => "DNA coding strand",
            Row::Template => "DNA template strand",
            Row::Mrna => "mRNA",
            Row::Trna => "tRNA anticodons",
            Row::Amino => "Amino acids",
        }
    }
    /// The ends written left and right of the row.
    fn ends(self) -> (&'static str, &'static str) {
        match self {
            Row::Template | Row::Trna => ("3\u{2032}", "5\u{2032}"),
            _ => ("5\u{2032}", "3\u{2032}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Shown, Blank, Some, Off }

fn mode(p: &Params, r: Row) -> Mode {
    match p.s(ROW_KEYS[r.i()]) {
        "Blank" => Mode::Blank,
        "Some" => Mode::Some,
        "Off" => Mode::Off,
        _ => Mode::Shown,
    }
}

pub fn row_name(p: &Params, r: Row) -> String {
    let t = p.s(NAME_KEYS[r.i()]).trim();
    if t.is_empty() { r.default_name().to_string() } else { t.to_string() }
}

/// The cells of a row: base indices for the strands and the tRNA row (only under codons that get a tRNA), codon
/// numbers for the amino acids.
pub fn cells(m: &Model, r: Row) -> Vec<usize> {
    match r {
        Row::Coding | Row::Template | Row::Mrna => (0..m.len()).collect(),
        Row::Trna => {
            let s = m.start.unwrap_or(0);
            (0..m.trnas()).flat_map(|k| s + 3 * k..s + 3 * k + 3).collect()
        }
        Row::Amino => (0..m.read).collect(),
    }
}

/// The correct letter of strand-row cell `i`.
fn letter(m: &Model, r: Row, i: usize) -> u8 {
    match r {
        Row::Coding => m.coding[i],
        Row::Template => m.template[i],
        Row::Mrna => m.mrna[i],
        _ => rna_pair(m.mrna[i]),
    }
}

fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut rng = Rng::new(seed);
    for i in (1..n).rev() {
        let j = rng.below(i + 1);
        v.swap(i, j);
    }
    v
}

/// The cells of row `r` left blank (a Blank row: all of them; Some: the set share, picked by the blank pattern).
pub fn blanks(p: &Params, m: &Model, r: Row) -> Vec<usize> {
    let c = cells(m, r);
    match mode(p, r) {
        Mode::Blank => c,
        Mode::Some if !c.is_empty() => {
            let n = c.len();
            let share = p.i("some_share").clamp(1, 99) as f64 / 100.0;
            let k = ((n as f64 * share).round() as usize).clamp(1, (n - 1).max(1));
            let seed = (p.i("blank_seed").max(0) as u64).wrapping_mul(7919).wrapping_add(r.i() as u64 * 104_729 + 17);
            let mut pick: Vec<usize> = shuffled(n, seed).into_iter().take(k).map(|j| c[j]).collect();
            pick.sort_unstable();
            pick
        }
        _ => vec![],
    }
}

/// One deliberate wrong base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mistake {
    pub row: Row,
    /// Base index along the strands.
    pub at: usize,
    pub drawn: u8,
    pub right: u8,
}

pub fn mistake_row(p: &Params) -> Row {
    match p.s("mistake_row") {
        "DNA coding strand" => Row::Coding,
        "DNA template strand" => Row::Template,
        "tRNA anticodons" => Row::Trna,
        _ => Row::Mrna,
    }
}

/// How many mistakes were asked for.
pub fn mistakes_asked(p: &Params) -> usize { p.s("mistakes").parse::<usize>().unwrap_or(0).min(4) }

/// The wrong bases: in the chosen row, among the cells drawn and not blank, picked by the mistake pattern.
pub fn mistakes(p: &Params, m: &Model) -> Vec<Mistake> {
    let want = mistakes_asked(p);
    let r = mistake_row(p);
    if want == 0 || mode(p, r) == Mode::Off || p.s("chart") == "Codon chart only" {
        return vec![];
    }
    let bl = blanks(p, m, r);
    let cand: Vec<usize> = cells(m, r).into_iter().filter(|i| !bl.contains(i)).collect();
    let seed = (p.i("mistake_seed").max(0) as u64).wrapping_mul(6007).wrapping_add(r.i() as u64 * 31 + 5);
    let mut rng = Rng::new(seed ^ 0x5bd1_e995);
    let mut out: Vec<Mistake> = shuffled(cand.len(), seed)
        .into_iter()
        .take(want)
        .map(|j| {
            let at = cand[j];
            let right = letter(m, r, at);
            let alphabet: &[u8; 4] = if matches!(r, Row::Coding | Row::Template) { b"ACGT" } else { b"ACGU" };
            let wrong: Vec<u8> = alphabet.iter().copied().filter(|&b| b != right).collect();
            Mistake { row: r, at, drawn: wrong[rng.below(wrong.len())], right }
        })
        .collect();
    out.sort_by_key(|x| x.at);
    out
}

/// A mistake in words, numbered as a student counts along the row.
pub fn mistake_text(p: &Params, m: &Model, x: &Mistake) -> String {
    let (drawn, right) = (x.drawn as char, x.right as char);
    match x.row {
        Row::Trna => {
            let s = m.start.unwrap_or(0);
            format!("tRNA anticodon {}, base {} is {drawn}; it should be {right}", (x.at - s) / 3 + 1, (x.at - s) % 3 + 1)
        }
        r => format!("{} base {} is {drawn}; it should be {right}", row_name(p, r), x.at + 1),
    }
}

/// The editor's readout under Reading frame (the protein, or why nothing is read).
pub fn readout(p: &Params) -> String {
    let m = Model::from_params(p);
    if m.len() == 0 {
        return "Type a sequence of bases.".into();
    }
    if m.start.is_none() {
        return "No AUG in this mRNA, so nothing is translated. Choose From the first base to read it anyway.".into();
    }
    if m.read == 0 {
        return "Fewer than three bases from the start: no codon to read.".into();
    }
    let mut s = m.chain(naming(p));
    if !m.stopped {
        s.push_str(" (no stop codon yet)");
    }
    s
}

/// Notes under the sequence box: what was left out or read differently.
pub fn seq_note(p: &Params) -> Option<String> {
    let m = Model::from_params(p);
    let mut out = Vec::new();
    if m.cut > 0 {
        out.push(format!("Only the first {MAX_BASES} bases are drawn; {} more left off.", m.cut));
    }
    if !m.ignored.is_empty() {
        let mut v: Vec<String> = m.ignored.iter().map(|c| c.to_string()).collect();
        v.dedup();
        out.push(format!("Not bases, left out: {}.", v.join(" ")));
    }
    if m.swapped {
        out.push(if p.s("typed") == "mRNA" { "T read as U in the mRNA.".to_string() } else { "U read as T in the DNA.".to_string() });
    }
    (!out.is_empty()).then(|| out.join("\n"))
}

/// The mistakes listed under the Mistakes controls (what the answer key prints).
pub fn mistakes_note(p: &Params) -> Option<String> {
    let want = mistakes_asked(p);
    if want == 0 {
        return None;
    }
    let m = Model::from_params(p);
    let xs = mistakes(p, &m);
    if xs.is_empty() {
        return Some("No base to change: that row is off, blank or empty.".into());
    }
    let mut lines: Vec<String> = xs.iter().map(|x| mistake_text(p, &m, x)).collect();
    if xs.len() < want {
        lines.push(format!("Only {} could be placed.", xs.len()));
    }
    Some(lines.join("\n"))
}

fn naming(p: &Params) -> Naming {
    match p.s("aa_names") {
        "M" => Naming::One,
        "Methionine" => Naming::Full,
        _ => Naming::Three,
    }
}

// ================================================================== drawing

const INK: &str = "#111111";
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const MARGIN: f64 = 16.0;

struct Look {
    print: bool,
    key: bool,
    colour: bool,
    lab: f64,
}

impl Look {
    /// The ink of a filled-in blank in the answer key.
    fn answer(&self) -> &'static str { if self.print { "#5a5a5a" } else { "#1d5fb8" } }
    fn mistake(&self) -> &'static str { if self.print { "#000000" } else { "#c62828" } }
    fn base_fill(&self, b: u8) -> &'static str {
        if !self.colour {
            return "#ffffff";
        }
        match (self.print, b) {
            (false, b'A') => "#f4a3a3",
            (false, b'T' | b'U') => "#f6dc78",
            (false, b'G') => "#96b6ee",
            (false, _) => "#a3dcae",
            (true, b'A') => "#b4b4b4",
            (true, b'T' | b'U') => "#ffffff",
            (true, b'G') => "#8a8a8a",
            (true, _) => "#dcdcdc",
        }
    }
}

fn text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, fill: &str, extra: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}"{extra}>{}</text>"##,
        svg::esc(s)
    ));
}

/// Sizes of the strand block for the label size.
struct Dims {
    fs: f64,
    fl: f64,
    cell: f64,
    gap: f64,
    name_w: f64,
    end_w: f64,
}

/// One row's band: its top and height.
struct Band {
    row: Row,
    y: f64,
}

struct Strands {
    w: f64,
    h: f64,
}

/// Lines of a row name, wrapped at about 15 characters into at most two lines.
fn name_lines(s: &str) -> Vec<String> {
    let mut v = super::wrap(s, 15);
    if v.len() > 2 {
        let rest = v.split_off(1).join(" ");
        v.push(rest);
    }
    v
}

fn dims(p: &Params, lab: f64, rows: &[Row]) -> Dims {
    let fs = 14.0 * lab;
    let fl = 11.5 * lab;
    let cell = (fs * 1.65).max(24.0);
    let gap = if p.b("gaps") { (cell * 0.42).max(8.0) } else { 0.0 };
    let name_w = match p.s("row_names") {
        "Off" => 0.0,
        "Blank" => 104.0 * lab + 12.0,
        _ => rows
            .iter()
            .flat_map(|&r| name_lines(&row_name(p, r)))
            .map(|l| text_w(&l, fl) * 1.08)
            .fold(0.0, f64::max)
            + 14.0,
    };
    let end_w = if p.s("ends") == "Off" { 4.0 } else { fl * 1.6 + 6.0 };
    Dims { fs, fl, cell, gap, name_w, end_w }
}

/// Draws the strands at (x0, y0); returns the block's size.
fn draw_strands(d: &mut Doc, p: &Params, m: &Model, look: &Look, x0: f64, y0: f64, measure_only: bool) -> Strands {
    let rows: Vec<Row> = ROWS.iter().copied().filter(|&r| mode(p, r) != Mode::Off).collect();
    let dm = dims(p, look.lab, &rows);
    let groups = m.groups();
    let group_of: Vec<usize> = {
        let mut g = vec![0; m.len()];
        for (gi, &(s, n)) in groups.iter().enumerate() {
            for v in g.iter_mut().skip(s).take(n) {
                *v = gi;
            }
        }
        g
    };
    let xs = x0 + dm.name_w + dm.end_w;
    let bx = |i: usize| xs + i as f64 * dm.cell + dm.gap * group_of.get(i).copied().unwrap_or(0) as f64;
    let width_bases = if m.len() == 0 { 6.0 * dm.cell } else { bx(m.len() - 1) + dm.cell - xs };
    let naming = naming(p);
    let beads = p.s("aa_style") != "Words";

    // vertical bands
    let mut bands = Vec::new();
    let mut y = y0;
    for (n, &r) in rows.iter().enumerate() {
        let h = if r == Row::Amino { dm.cell * if beads { 1.05 } else { 0.9 } } else { dm.cell };
        bands.push(Band { row: r, y });
        y += h;
        let next = rows.get(n + 1).copied();
        y += match (r, next) {
            (_, None) => 0.0,
            (Row::Coding, Some(Row::Template)) if p.b("hbonds") => dm.cell * 0.95,
            (Row::Mrna, Some(_)) if p.b("brackets") => dm.cell * 0.85,
            (Row::Mrna | Row::Trna, Some(_)) => dm.cell * 0.55,
            _ => dm.cell * 0.4,
        };
    }
    // a bracket under the last row still needs room
    if rows.last() == Some(&Row::Mrna) && p.b("brackets") {
        y += dm.cell * 0.45;
    }
    let total_h = (y - y0).max(dm.cell);
    let total_w = dm.name_w + dm.end_w * 2.0 + width_bases;
    if measure_only {
        return Strands { w: total_w, h: total_h };
    }

    let err = mistakes(p, m);
    let sw = 1.3;
    for band in &bands {
        let r = band.row;
        let yy = band.y;
        let bl = blanks(p, m, r);
        let mid = yy + dm.cell / 2.0;
        // row name
        match p.s("row_names") {
            "Off" => {}
            "Blank" => {
                let ly = mid + dm.fl * 0.5;
                d.line(x0, ly, x0 + dm.name_w - 14.0, ly, INK, 1.0);
                if look.key {
                    text(d, x0, ly - 3.0, dm.fl * 0.9, "start", look.answer(), r#" font-style="italic""#, &row_name(p, r));
                }
            }
            _ => {
                let lines = name_lines(&row_name(p, r));
                let lh = dm.fl * 1.15;
                let top = mid - lh * (lines.len() as f64 - 1.0) / 2.0 + dm.fl * 0.36;
                for (k, l) in lines.iter().enumerate() {
                    text(d, x0, top + k as f64 * lh, dm.fl, "start", INK, r#" font-weight="bold""#, l);
                }
            }
        }
        if r == Row::Amino {
            draw_amino(d, m, look, &dm, &bx, yy, &bl, naming, beads);
            continue;
        }
        let cs = cells(m, r);
        if cs.is_empty() {
            continue;
        }
        // backbone between codon groups (not between separate tRNAs)
        if r != Row::Trna && dm.gap > 0.0 {
            for w in groups.windows(2) {
                let a = bx(w[0].0 + w[0].1 - 1) + dm.cell;
                let b = bx(w[1].0);
                d.line(a, mid, b, mid, INK, 1.4);
            }
        }
        for &i in &cs {
            let x = bx(i);
            let blank = bl.contains(&i);
            let right = letter(m, r, i);
            let mis = err.iter().find(|e| e.row == r && e.at == i);
            let shown = mis.map_or(right, |e| e.drawn);
            let fill = if blank { "#ffffff" } else { look.base_fill(shown) };
            d.rect(x, yy, dm.cell, dm.cell, fill, INK, sw);
            let ty = mid + dm.fs * 0.36;
            if !blank {
                text(d, x + dm.cell / 2.0, ty, dm.fs, "middle", INK, r#" font-weight="bold""#, &(shown as char).to_string());
            } else if look.key {
                text(d, x + dm.cell / 2.0, ty, dm.fs, "middle", look.answer(), r#" font-style="italic""#, &(right as char).to_string());
            }
            if mis.is_some() && look.key {
                let dash = if look.print { r#" stroke-dasharray="4 2""# } else { "" };
                d.raw(&format!(
                    r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="4" fill="none" stroke="{}" stroke-width="2.6"{dash}/>"#,
                    x - 3.0, yy - 3.0, dm.cell + 6.0, dm.cell + 6.0, look.mistake()
                ));
            }
        }
        // 5′ and 3′ ends
        let (l, rt) = r.ends();
        let (first, last) = (cs[0], *cs.last().unwrap_or(&cs[0]));
        let (lx, rx) = (bx(first) - 5.0, bx(last) + dm.cell + 5.0);
        match p.s("ends") {
            "Off" => {}
            "Blank" => {
                let ly = mid + dm.fl * 0.5;
                let w = dm.fl * 1.3;
                d.line(lx - w, ly, lx, ly, INK, 1.0);
                d.line(rx, ly, rx + w, ly, INK, 1.0);
                if look.key {
                    text(d, lx - w / 2.0, ly - 3.0, dm.fl, "middle", look.answer(), r#" font-style="italic""#, l);
                    text(d, rx + w / 2.0, ly - 3.0, dm.fl, "middle", look.answer(), r#" font-style="italic""#, rt);
                }
            }
            _ => {
                text(d, lx, mid + dm.fl * 0.36, dm.fl, "end", INK, "", l);
                text(d, rx, mid + dm.fl * 0.36, dm.fl, "start", INK, "", rt);
            }
        }
        // a bracket under each codon of the mRNA
        if r == Row::Mrna && p.b("brackets") {
            if let Some(s) = m.start {
                for k in 0..m.codons {
                    let a = bx(s + 3 * k) + 2.0;
                    let b = bx(s + 3 * k + 2) + dm.cell - 2.0;
                    let t = yy + dm.cell + dm.cell * 0.18;
                    let bt = t + dm.cell * 0.22;
                    d.path(&format!("M{a:.2} {t:.2}V{bt:.2}H{b:.2}V{t:.2}"), "none", INK, 1.3);
                }
            }
        }
    }
    // hydrogen bonds between the DNA strands
    if p.b("hbonds") {
        let c = bands.iter().find(|b| b.row == Row::Coding);
        let t = bands.iter().find(|b| b.row == Row::Template);
        if let (Some(c), Some(t)) = (c, t) {
            let (ya, yb) = (c.y + dm.cell + 3.0, t.y - 3.0);
            for i in 0..m.len() {
                let x = bx(i);
                let fr: &[f64] = if matches!(m.coding[i], b'G' | b'C') { &[0.3, 0.5, 0.7] } else { &[0.37, 0.63] };
                for f in fr {
                    let xx = x + dm.cell * f;
                    d.raw(&format!(r#"<line x1="{xx:.2}" y1="{ya:.2}" x2="{xx:.2}" y2="{yb:.2}" stroke="{INK}" stroke-width="1.1" stroke-dasharray="2.2 1.8"/>"#));
                }
            }
        }
    }
    if m.len() == 0 {
        text(d, xs, y0 + dm.cell * 0.65, dm.fl, "start", "#6b6b6b", r#" font-style="italic""#, "Type a sequence of bases");
    }
    Strands { w: total_w, h: total_h }
}

#[allow(clippy::too_many_arguments)]
fn draw_amino(d: &mut Doc, m: &Model, look: &Look, dm: &Dims, bx: &dyn Fn(usize) -> f64, yy: f64, bl: &[usize], naming: Naming, beads: bool) {
    let Some(s) = m.start else { return };
    let prot = m.protein();
    let h = dm.cell * 0.88;
    let cy = yy + dm.cell * 0.52;
    let room = 3.0 * dm.cell + dm.gap - 6.0;
    let mut prev: Option<f64> = None; // right edge of the previous bead in the chain
    for (k, &aa) in prot.iter().enumerate() {
        let cx = (bx(s + 3 * k) + bx(s + 3 * k + 2) + dm.cell) / 2.0;
        let name = aa_name(aa, naming);
        let blank = bl.contains(&k);
        let stop = aa == b'*';
        let mut fsz = dm.fl * 1.02;
        let tw = text_w(name, fsz) * 1.06;
        let bw = (tw + 14.0).max(dm.cell * 1.55).min(room);
        if tw + 10.0 > bw {
            fsz *= (bw - 10.0) / tw;
        }
        let ink = if blank { look.answer() } else { INK };
        let style = if blank { r#" font-style="italic""# } else { r#" font-weight="bold""# };
        if beads && !(stop && !blank) {
            let x = cx - bw / 2.0;
            if let (Some(px), false) = (prev, stop) {
                d.line(px, cy, x, cy, INK, 1.4);
            }
            d.raw(&format!(
                r##"<rect x="{x:.2}" y="{:.2}" width="{bw:.2}" height="{h:.2}" rx="{:.2}" fill="#ffffff" stroke="{INK}" stroke-width="1.5"/>"##,
                cy - h / 2.0,
                h / 2.0
            ));
            prev = if stop { None } else { Some(x + bw) };
        }
        if !blank || look.key {
            text(d, cx, cy + fsz * 0.36, fsz, "middle", ink, style, name);
        }
        if blank && !beads {
            let ly = cy + dm.fl * 0.55;
            d.line(cx - bw / 2.0 + 4.0, ly, cx + bw / 2.0 - 4.0, ly, INK, 1.0);
        }
    }
}

/// The standard codon chart at (x, y); returns its size. With `measure_only` nothing is drawn.
fn draw_chart(d: &mut Doc, look: &Look, naming: Naming, x: f64, y: f64, measure_only: bool) -> (f64, f64) {
    let fc = 10.5 * look.lab;
    let fh = 12.0 * look.lab;
    let nm = |aa: u8| if naming == Naming::One { aa_name(aa, Naming::One) } else { aa_name(aa, Naming::Three) };
    let entry = |c: &[u8; 3]| {
        let aa = translate(c);
        let mut s = format!("{} {}", String::from_utf8_lossy(c), nm(aa));
        if c == b"AUG" {
            s.push_str(" (Start)");
        }
        s
    };
    let widest = ["AUG Met (Start)", "UUU Phe"].iter().map(|s| text_w(s, fc) * 1.1).fold(0.0, f64::max);
    let cw = widest + 14.0;
    let lh = fc * 1.5;
    let ch = 4.0 * lh + 8.0;
    let left = fh * 3.0; // "First base" turned, then the row letters
    let top = fh * 3.0; // "Second base", then the column letters
    let right = fh * 3.0;
    let w = left + 4.0 * cw + right;
    let h = top + 4.0 * ch;
    if measure_only {
        return (w, h);
    }
    let gx = x + left;
    let gy = y + top;
    text(d, gx + 2.0 * cw, y + fh, fh * 0.95, "middle", INK, r#" font-weight="bold""#, "Second base");
    let side = |d: &mut Doc, cx: f64, s: &str| {
        let cy = gy + 2.0 * ch;
        d.raw(&format!(
            r##"<text x="{cx:.2}" y="{cy:.2}" font-size="{:.2}" text-anchor="middle" font-family="{FONT}" fill="{INK}" font-weight="bold" transform="rotate(-90 {cx:.2} {cy:.2})">{s}</text>"##,
            fh * 0.95
        ));
    };
    side(d, x + fh * 0.9, "First base");
    side(d, gx + 4.0 * cw + right - fh * 0.5, "Third base");
    for (ci, &b2) in ORDER.iter().enumerate() {
        text(d, gx + (ci as f64 + 0.5) * cw, gy - fh * 0.55, fh * 1.1, "middle", INK, r#" font-weight="bold""#, &(b2 as char).to_string());
    }
    for (ri, &b1) in ORDER.iter().enumerate() {
        let ry = gy + ri as f64 * ch;
        text(d, gx - fh * 0.7, ry + ch / 2.0 + fh * 0.4, fh * 1.1, "middle", INK, r#" font-weight="bold""#, &(b1 as char).to_string());
        for (k, &b3) in ORDER.iter().enumerate() {
            let ly = ry + 4.0 + (k as f64 + 0.5) * lh;
            text(d, gx + 4.0 * cw + fh * 0.7, ly + fc * 0.36, fc * 1.05, "middle", INK, r#" font-weight="bold""#, &(b3 as char).to_string());
        }
        for (ci, &b2) in ORDER.iter().enumerate() {
            let cx = gx + ci as f64 * cw;
            for (k, &b3) in ORDER.iter().enumerate() {
                let c = [b1, b2, b3];
                let aa = translate(&c);
                let ly = ry + 4.0 + (k as f64 + 0.5) * lh;
                let special = aa == b'*' || &c == b"AUG";
                if special {
                    d.rect(cx + 3.0, ly - lh / 2.0 + 1.0, cw - 6.0, lh - 2.0, "#e4e4e4", "none", 0.0);
                }
                let extra = if special { r#" font-weight="bold""# } else { "" };
                text(d, cx + 7.0, ly + fc * 0.36, fc, "start", INK, extra, &entry(&c));
            }
        }
    }
    // the grid: heavy outside and between first-base rows, light between columns
    for i in 0..=4 {
        let yy = gy + i as f64 * ch;
        d.line(gx, yy, gx + 4.0 * cw, yy, INK, if i == 0 || i == 4 { 1.8 } else { 1.2 });
        let xx = gx + i as f64 * cw;
        d.line(xx, gy, xx, gy + 4.0 * ch, INK, if i == 0 || i == 4 { 1.8 } else { 1.0 });
    }
    (w, h)
}

/// The answer key's lines (rows as drawn, correct bases grouped by codon).
pub fn key_lines(p: &Params, m: &Model) -> Vec<String> {
    let groups = m.groups();
    let grouped = |v: &[u8]| groups.iter().map(|&(s, n)| String::from_utf8_lossy(&v[s..s + n]).into_owned()).collect::<Vec<_>>().join(" ");
    let mut out = Vec::new();
    let chart_only = p.s("chart") == "Codon chart only";
    if !chart_only && m.len() > 0 {
        for r in ROWS {
            if mode(p, r) == Mode::Off {
                continue;
            }
            let name = row_name(p, r);
            match r {
                Row::Coding => out.push(format!("{name}: 5\u{2032}-{}-3\u{2032}", grouped(&m.coding))),
                Row::Template => out.push(format!("{name}: 3\u{2032}-{}-5\u{2032}", grouped(&m.template))),
                Row::Mrna => out.push(format!("{name}: 5\u{2032}-{}-3\u{2032}", grouped(&m.mrna))),
                Row::Trna if m.trnas() > 0 => {
                    let a: Vec<String> = (0..m.trnas()).map(|k| String::from_utf8_lossy(&m.anticodon(k)).into_owned()).collect();
                    out.push(format!("{name} (each 3\u{2032} to 5\u{2032}): {}", a.join(" ")));
                }
                Row::Amino if m.read > 0 => out.push(format!("{name}: {}", m.chain(naming(p)))),
                _ => {}
            }
        }
        let xs = mistakes(p, m);
        if !xs.is_empty() {
            let t: Vec<String> = xs.iter().map(|x| mistake_text(p, m, x)).collect();
            out.push(format!("{}: {}", if xs.len() == 1 { "Mistake" } else { "Mistakes" }, t.join("; ")));
        }
    }
    out
}

fn wrap_px(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in s.split(' ') {
        let t = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if !cur.is_empty() && text_w(&t, size) > max {
            lines.push(std::mem::take(&mut cur));
            cur = w.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn render(p: &Params) -> String {
    let m = Model::from_params(p);
    let print = p.s("style") == "Print";
    let look = Look { print, key: p.b(SHOW_ANSWERS), colour: p.b("color_bases"), lab: svg::label_factor(p.s("label_size")) };
    let naming = naming(p);
    let chart = p.s("chart");
    let show_strands = chart != "Codon chart only";
    let show_chart = chart != "Strands";
    let below = p.s("chart_pos") == "Below";

    let mut scratch = Doc::new(1.0, 1.0);
    let st = if show_strands { draw_strands(&mut scratch, p, &m, &look, 0.0, 0.0, true) } else { Strands { w: 0.0, h: 0.0 } };
    let (cw, chh) = if show_chart { draw_chart(&mut scratch, &look, naming, 0.0, 0.0, true) } else { (0.0, 0.0) };
    let sep = 28.0 * look.lab;
    let (body_w, body_h) = match (show_strands, show_chart) {
        (true, true) if below => (st.w.max(cw), st.h + sep + chh),
        (true, true) => (st.w + sep + cw, st.h.max(chh)),
        (true, false) => (st.w, st.h),
        _ => (cw, chh),
    };

    let title = p.s("title").trim().to_string();
    let ft = 16.0 * look.lab;
    let title_h = if title.is_empty() { 0.0 } else { ft * 1.9 };
    let fk = 12.0 * look.lab;
    let key_raw = if look.key { key_lines(p, &m) } else { vec![] };
    let key_max = body_w.max(420.0 * look.lab);
    let key: Vec<String> = key_raw.iter().flat_map(|l| wrap_px(l, key_max, fk)).collect();
    let key_w = key.iter().map(|l| text_w(l, fk) * 1.04).fold(0.0, f64::max);
    let key_h = if key.is_empty() { 0.0 } else { fk * 1.0 + key.len() as f64 * fk * 1.45 };

    let inner_w = body_w.max(key_w).max(if title.is_empty() { 0.0 } else { text_w(&title, ft) * 1.08 });
    let w = (inner_w + 2.0 * MARGIN).ceil();
    let h = (title_h + body_h + key_h + 2.0 * MARGIN).ceil();
    let mut d = Doc::new(w, h);
    let cx = w / 2.0;
    if !title.is_empty() {
        text(&mut d, cx, MARGIN + ft, ft, "middle", INK, r#" font-weight="bold""#, &title);
    }
    let top = MARGIN + title_h;
    let bx0 = (w - body_w) / 2.0;
    match (show_strands, show_chart) {
        (true, true) if below => {
            draw_strands(&mut d, p, &m, &look, (w - st.w) / 2.0, top, false);
            draw_chart(&mut d, &look, naming, (w - cw) / 2.0, top + st.h + sep, false);
        }
        (true, true) => {
            draw_strands(&mut d, p, &m, &look, bx0, top + (body_h - st.h) / 2.0, false);
            draw_chart(&mut d, &look, naming, bx0 + st.w + sep, top + (body_h - chh) / 2.0, false);
        }
        (true, false) => {
            draw_strands(&mut d, p, &m, &look, bx0, top, false);
        }
        _ => {
            draw_chart(&mut d, &look, naming, bx0, top, false);
        }
    }
    let ky = top + body_h + fk * 1.0;
    for (i, l) in key.iter().enumerate() {
        text(&mut d, cx, ky + (i as f64 + 0.75) * fk * 1.45, fk, "middle", INK, "", l);
    }

    let mut desc = Vec::new();
    if show_strands {
        let rows: Vec<String> = ROWS.iter().filter(|&&r| mode(p, r) != Mode::Off).map(|&r| row_name(p, r)).collect();
        desc.push(format!("Rows: {}.", rows.join(", ")));
        if m.len() > 0 {
            desc.push(format!("mRNA 5\u{2032}-{}-3\u{2032}, {} codons read.", String::from_utf8_lossy(&m.mrna), m.read));
        }
    }
    if show_chart {
        desc.push("The standard codon chart: first base down the left, second across the top, third down the right.".into());
    }
    d.describe("DNA and protein synthesis", &desc.join(" "));
    d.finish()
}

// ================================================================== examples and classic setups

fn sv(v: &str) -> Value { Value::S(v.to_string()) }

/// The reference's "Classic setups" dropdown, with our own sequences.
pub fn classics() -> Vec<Example> {
    let off = |k: &'static str| (k, sv("Off"));
    let mc = |name: &'static str, n: &str, seed: i64, title: &str| Example {
        name,
        params: vec![
            ("typed", sv("Template")),
            ("seq", sv("TACGGTCTTAAACCG")),
            off("row_trna"),
            off("row_amino"),
            ("mistakes", sv(n)),
            ("mistake_row", sv("mRNA")),
            ("mistake_seed", Value::I(seed)),
            ("title", sv(title)),
        ],
    };
    vec![
        Example { name: "Template strand starting TAC", params: vec![("typed", sv("Template")), ("seq", sv("TACGGTCTTAAACCG"))] },
        Example { name: "Coding strand with bases before the ATG", params: vec![("typed", sv("Coding")), ("seq", sv("GCTAATGTCCAAGGGCTAGTA"))] },
        Example { name: "An mRNA, translated", params: vec![("typed", sv("mRNA")), ("seq", sv("AUGGCAUUCCGAUAA")), off("row_coding"), off("row_template")] },
        Example {
            name: "Fill in the other DNA strand",
            params: vec![("typed", sv("Coding")), ("seq", sv("ATGAGCTTCGGATCCAAA")), ("row_template", sv("Blank")), off("row_mrna"), off("row_trna"), off("row_amino"), ("hbonds", Value::B(true))],
        },
        Example { name: "Ending at a stop codon", params: vec![("typed", sv("Template")), ("seq", sv("TACAAACGGTTGACT"))] },
        mc("Multiple choice A: every base right", "None", 1, "A."),
        mc("Multiple choice B: one wrong base", "1", 11, "B."),
        mc("Multiple choice C: two wrong bases", "2", 12, "C."),
        mc("Multiple choice D: three wrong bases", "3", 13, "D."),
    ]
}

/// "Start from an example": the start of the β-globin gene and four kinds of point mutation in it.
pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let coding = |name: &'static str, seq: &str| Example { name, params: vec![("typed", sv("Coding")), ("seq", sv(seq)), ("row_trna", sv("Off"))] };
    Some(vec![
        coding("β-globin gene start, normal (Met-Val-His-Leu-Thr-Pro-Glu-Glu-Lys)", BETA_NORMAL),
        coding("Sickle-cell β-globin: one base changed, Glu becomes Val", BETA_SICKLE),
        coding("Silent mutation: CTG to CTC, still Leu", BETA_SILENT),
        coding("Nonsense mutation: GAG to TAG, an early stop", BETA_NONSENSE),
        coding("Frameshift: one base deleted from codon 3", BETA_FRAMESHIFT),
        Example {
            name: "Transcribe and translate: student copy with blank rows",
            params: vec![("row_mrna", sv("Blank")), ("row_trna", sv("Blank")), ("row_amino", sv("Blank")), ("chart", sv("Strands and codon chart")), ("chart_pos", sv("Below"))],
        },
    ])
}

pub const BETA_NORMAL: &str = "ATGGTGCATCTGACTCCTGAGGAGAAG";
pub const BETA_SICKLE: &str = "ATGGTGCATCTGACTCCTGTGGAGAAG";
pub const BETA_SILENT: &str = "ATGGTGCATCTCACTCCTGAGGAGAAG";
pub const BETA_NONSENSE: &str = "ATGGTGCATCTGACTCCTGAGTAGAAG";
pub const BETA_FRAMESHIFT: &str = "ATGGTGATCTGACTCCTGAGGAGAAG";

#[cfg(test)]
mod tests;
