//! VSEPR shapes: one molecule at a time as a rotatable, depth-sorted ball-and-stick model.
//!
//! **The chemistry.** A molecule's shape is set by the electron domains around its central atom:
//! each bond and each lone pair is one domain, and the domains spread out to keep as far apart as
//! possible. The electron-domain geometry counts every domain; the molecular shape counts only the
//! atoms. Four domains are tetrahedral, but one lone pair leaves a trigonal pyramid (NH3) and two
//! leave a bent molecule (H2O); six domains are octahedral, but two lone pairs leave a square plane
//! (XeF4); five domains are trigonal bipyramidal, but one lone pair leaves a see-saw (SF4). The
//! figure prints both names on the key and leaves both blank on the student copy.
//!
//! **The angles actually drawn**, measured from the coordinates in this module:
//!
//! | molecule | domains | shape | angle drawn |
//! | --- | --- | --- | --- |
//! | BeCl2 | 2 | linear | 180° |
//! | BF3 | 3 | trigonal planar | 120° |
//! | CH4 | 4 | tetrahedral | 109.5° (the exact tetrahedral angle 109.47°) |
//! | NH3 | 4 | trigonal pyramidal | 107° |
//! | H2O | 4 | bent | 104.5° |
//! | PCl5 | 5 | trigonal bipyramidal | 120° and 90° |
//! | SF6 | 6 | octahedral | 90° |
//! | XeF4 | 6 | square planar | 90° |
//! | SF4 | 5 | see-saw | about 102° and 90° |
//!
//! NH3 and H2O start from the tetrahedral framework and are compressed by lone-pair repulsion to the
//! angles drawn; the figure never claims 109.5° for water. A lone pair is held by only one nucleus,
//! so its cloud spreads more than a bonding pair. The repulsion order is lone pair–lone pair >
//! lone pair–bonding pair > bonding pair–bonding pair, which is why the H–N–H angle closes to about
//! 107° and the H–O–H angle to about 104.5°. SF4 is drawn with the lone pair in an equatorial
//! position; the real axial bonds bend a little away from it, which this simplified model does not
//! show, so the axial–equatorial angle drawn is 90°.
//!
//! **The model.** It is a simplified ball-and-stick drawing: atoms are circles, bonds are
//! single sticks (no double bonds, no resonance, no bond order), each lone pair is a pair of dots on
//! a short dashed stick pointing where the pair truly projects; a pair whose projection lands on a
//! bond, collapses to a point, sits behind a nearer atom or would put two pairs on top of each other
//! is left off and the panel says so, and bond lengths are not
//! to scale. No real bond length is measured or implied. The 3-D coordinates are exact for the
//! stated VSEPR geometry, with the two lone-pair compressions above written into the coordinates;
//! the view is an orthographic projection of the rotated model. A bond pointing almost straight at
//! the viewer is hidden behind its atom, a ligand label whose ball is hidden by a nearer ball is left
//! off rather than drawn through it, and the panel says when a bond or an atom is hidden this way.
//! The centre label is the reference: it stays on top of a nearer ball when it can, and the panel
//! says when a nearer ligand covers the centre.
//!
//! **The arc is the true angle.** An orthographic projection can foreshorten a bond and change the
//! angle between the sticks on the page, so an arc is drawn only when the page shows the printed
//! angle (within 0.4°). Each molecule opens at a view chosen so its angle is true: the opening views
//! are (0, 0) for BeCl2 and BF3, (202, 21) for CH4, (0, 27) for NH3, (44, 62) for H2O, (250, 0) for
//! PCl5, and (0, 20), (0, 0) and (296, 36) for SF6, XeF4 and SF4. At XeF4's opening the two axial lone
//! pairs point at and away from the viewer, so they are a panel note rather than a made-up stick;
//! a true 90° F–Xe–F arc in any view forces those pairs onto the F bonds. PCl5 and SF4 cannot show both their
//! 120°/90° and ~102°/90° angles true at once; they open on the 90° angle, and the panel names both.
//! Turning and tilting can foreshorten the sticks; then the arc is left off and the panel still
//! names the angle. The student copy draws no angle arc at all, because the angle is its blanked
//! answer and a foreshortened arc next to a blank would invite a wrong answer.
//!
//! **What is an answer.** The electron-domain geometry, the molecular shape and the bond angle are
//! worked out from the molecule, so they are printed only under Show answers. The student copy keeps
//! the molecule, the AX count, the number of electron domains, the number of lone pairs and the
//! drawing, blanks both geometry names and every angle, omits the explanation, and draws no angle
//! arc: an arc that reads a foreshortened angle next to a blank would invite a wrong answer. The key
//! draws an arc only when it is true on the page. The description names no shape and no angle.
//!
//! **Share links.** This figure is new and unreleased: it was authored at share version 4, and the
//! catalog has since reached 6. No saved specification or link can predate its `molecule`, `turn`,
//! `tilt`, `lone`, `angles` or `labels` controls, so it has no `start::added_in` entry. A migration
//! that could separate an already-made link of this figure from this one would need a global
//! share-version bump and a `was_overridden` value for every existing figure, so the note here is
//! the documented path.
use std::f64::consts::PI;

use crate::figures::chart::fmt;
use crate::figures::chemistry::ratesgraph::{text_w, wrap, FONT};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "vsepr-shapes";
pub const BLANK: &str = "______";
pub const TURN_LO: f64 = 0.0;
pub const TURN_HI: f64 = 360.0;
pub const TILT_LO: f64 = -90.0;
pub const TILT_HI: f64 = 90.0;
pub const TURN_DEFAULT: f64 = 0.0;
pub const TILT_DEFAULT: f64 = 0.0;
pub const NAMES: [&str; 9] = ["BeCl2", "BF3", "CH4", "NH3", "H2O", "PCl5", "SF6", "XeF4", "SF4"];
pub const DEFAULT_MOL: usize = 2;

pub type Pt = (f64, f64);
/// A box in canvas pixels: left, right, top, bottom.
pub type Bx = (f64, f64, f64, f64);

// ---------------------------------------------------------------------------------------------
// The molecules
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Atom {
    pub sym: &'static str,
    /// Position in the model, in bond lengths from the central atom.
    pub pos: [f64; 3],
    pub central: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AnglePair {
    /// Indices into `Mol::atoms`; the vertex is always atom 0.
    pub a: usize,
    pub b: usize,
    /// The angle printed for this pair, in degrees.
    pub angle: f64,
    pub text: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Mol {
    /// Plain formula, for matching and tests.
    pub name: &'static str,
    /// Formula as printed (ASCII).
    pub display: &'static str,
    /// The AX_nE_m count, e.g. "AX3E".
    pub ax: &'static str,
    pub domains: usize,
    pub domain_geo: &'static str,
    pub shape: &'static str,
    /// The absolute (turn, tilt) at which this molecule opens. The Turn and Tilt controls are
    /// relative to this view, so 0/0 is the opening view for every molecule.
    pub open: (f64, f64),
    pub atoms: &'static [Atom],
    /// Unit directions of the lone pairs, all on the central atom.
    pub lone: &'static [[f64; 3]],
    pub pairs: &'static [AnglePair],
    /// Key-only explanation; it names the shape and the angle, so the student copy omits it.
    pub why: &'static str,
    /// The same explanation with no angle, used on the key while Show bond angles is off.
    pub why_plain: &'static str,
}

impl Mol {
    pub fn lone_count(&self) -> usize { self.lone.len() }
    pub fn central(&self) -> &Atom { &self.atoms[0] }
    /// The distinct angle texts, e.g. "120° and 90°".
    pub fn angle_text(&self) -> String {
        let mut v: Vec<&str> = Vec::new();
        for p in self.pairs {
            if !v.contains(&p.text) { v.push(p.text); }
        }
        match v.len() {
            0 => "—".into(),
            1 => v[0].into(),
            _ => v.join(" and "),
        }
    }
}

const S3: f64 = 0.577_350_269_189_625_8; // 1/sqrt(3)
/// NH3: the H atoms are three of the four tetrahedral directions, compressed from 109.47° to 107°
/// by the lone pair on the fourth direction (+z). h^2 = (1 + 2 cos 107°)/3.
const NH3_H: f64 = 0.372_046_856_616_442_27;
const NH3_R: f64 = 0.928_213_949_734_555_7;
/// H2O: the two O–H bonds are 104.5° apart, with the two lone pairs in the perpendicular plane.
const H2O_R: f64 = 0.790_689_573_743_843_3;
const H2O_H: f64 = 0.612_217_280_034_449_3;
/// The two lone pairs of water are 115° apart, so the lone-pair–O–H angle is about 109.2°.
const H2O_LP_Y: f64 = 0.843_391_445_812_885_7;
const H2O_LP_Z: f64 = 0.537_299_608_346_823_9;
/// SF4: two equatorial bonds 102° apart, one lone pair in the third equatorial position.
const SF4_EQ_X: f64 = -0.629_320_391_049_837_3;
const SF4_EQ_Y: f64 = 0.777_145_961_456_971;

static EMPTY3: [[f64; 3]; 0] = [];
static NO_PAIRS: [AnglePair; 0] = [];

static BECL2_ATOMS: [Atom; 3] = [
    Atom { sym: "Be", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "Cl", pos: [1.0, 0.0, 0.0], central: false },
    Atom { sym: "Cl", pos: [-1.0, 0.0, 0.0], central: false },
];
static BECL2_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 2, angle: 180.0, text: "180°" }];

static BF3_ATOMS: [Atom; 4] = [
    Atom { sym: "B", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "F", pos: [0.0, 1.0, 0.0], central: false },
    Atom { sym: "F", pos: [-0.866_025_403_784_438_6, -0.5, 0.0], central: false },
    Atom { sym: "F", pos: [0.866_025_403_784_438_6, -0.5, 0.0], central: false },
];
static BF3_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 2, angle: 120.0, text: "120°" }];

static CH4_ATOMS: [Atom; 5] = [
    Atom { sym: "C", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "H", pos: [S3, S3, S3], central: false },
    Atom { sym: "H", pos: [S3, -S3, -S3], central: false },
    Atom { sym: "H", pos: [-S3, S3, -S3], central: false },
    Atom { sym: "H", pos: [-S3, -S3, S3], central: false },
];
static CH4_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 2, angle: 109.5, text: "109.5°" }];

static NH3_ATOMS: [Atom; 4] = [
    Atom { sym: "N", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "H", pos: [0.0, NH3_R, -NH3_H], central: false },
    Atom { sym: "H", pos: [-0.803_856_860_617_217_2, -0.464_106_974_867_278, -NH3_H], central: false },
    Atom { sym: "H", pos: [0.803_856_860_617_217_1, -0.464_106_974_867_278_25, -NH3_H], central: false },
];
static NH3_LONE: [[f64; 3]; 1] = [[0.0, 0.0, 1.0]];
static NH3_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 2, angle: 107.0, text: "107°" }];

static H2O_ATOMS: [Atom; 3] = [
    Atom { sym: "O", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "H", pos: [H2O_R, 0.0, -H2O_H], central: false },
    Atom { sym: "H", pos: [-H2O_R, 0.0, -H2O_H], central: false },
];
static H2O_LONE: [[f64; 3]; 2] = [[0.0, H2O_LP_Y, H2O_LP_Z], [0.0, -H2O_LP_Y, H2O_LP_Z]];
static H2O_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 2, angle: 104.5, text: "104.5°" }];

static PCL5_ATOMS: [Atom; 6] = [
    Atom { sym: "P", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "Cl", pos: [0.0, 1.0, 0.0], central: false },
    Atom { sym: "Cl", pos: [-0.866_025_403_784_438_6, -0.5, 0.0], central: false },
    Atom { sym: "Cl", pos: [0.866_025_403_784_438_6, -0.5, 0.0], central: false },
    Atom { sym: "Cl", pos: [0.0, 0.0, 1.0], central: false },
    Atom { sym: "Cl", pos: [0.0, 0.0, -1.0], central: false },
];
static PCL5_PAIRS: [AnglePair; 2] = [
    AnglePair { a: 1, b: 2, angle: 120.0, text: "120°" },
    AnglePair { a: 1, b: 4, angle: 90.0, text: "90°" },
];

static SF6_ATOMS: [Atom; 7] = [
    Atom { sym: "S", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "F", pos: [1.0, 0.0, 0.0], central: false },
    Atom { sym: "F", pos: [-1.0, 0.0, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, 1.0, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, -1.0, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, 0.0, 1.0], central: false },
    Atom { sym: "F", pos: [0.0, 0.0, -1.0], central: false },
];
static SF6_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 3, angle: 90.0, text: "90°" }];

static XEF4_ATOMS: [Atom; 5] = [
    Atom { sym: "Xe", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "F", pos: [1.0, 0.0, 0.0], central: false },
    Atom { sym: "F", pos: [-1.0, 0.0, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, 1.0, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, -1.0, 0.0], central: false },
];
static XEF4_LONE: [[f64; 3]; 2] = [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
static XEF4_PAIRS: [AnglePair; 1] = [AnglePair { a: 1, b: 3, angle: 90.0, text: "90°" }];

static SF4_ATOMS: [Atom; 5] = [
    Atom { sym: "S", pos: [0.0, 0.0, 0.0], central: true },
    Atom { sym: "F", pos: [SF4_EQ_X, SF4_EQ_Y, 0.0], central: false },
    Atom { sym: "F", pos: [SF4_EQ_X, -SF4_EQ_Y, 0.0], central: false },
    Atom { sym: "F", pos: [0.0, 0.0, 1.0], central: false },
    Atom { sym: "F", pos: [0.0, 0.0, -1.0], central: false },
];
static SF4_LONE: [[f64; 3]; 1] = [[1.0, 0.0, 0.0]];
static SF4_PAIRS: [AnglePair; 2] = [
    AnglePair { a: 1, b: 2, angle: 102.0, text: "~102°" },
    AnglePair { a: 1, b: 3, angle: 90.0, text: "90°" },
];

pub static MOLECULES: [Mol; 9] = [
    Mol {
        name: "BeCl2", display: "BeCl2", ax: "AX2", domains: 2,
        domain_geo: "Linear", shape: "Linear", open: (0.0, 0.0),
        atoms: &BECL2_ATOMS, lone: &EMPTY3, pairs: &BECL2_PAIRS,
        why: "Two bonding pairs and no lone pairs: the two bonds point in opposite directions, 180° apart.",
        why_plain: "Two bonding pairs and no lone pairs: the two bonds point in opposite directions.",
    },
    Mol {
        name: "BF3", display: "BF3", ax: "AX3", domains: 3,
        domain_geo: "Trigonal planar", shape: "Trigonal planar", open: (0.0, 0.0),
        atoms: &BF3_ATOMS, lone: &EMPTY3, pairs: &BF3_PAIRS,
        why: "Three bonding pairs and no lone pairs: the bonds spread evenly in one plane, 120° apart.",
        why_plain: "Three bonding pairs and no lone pairs: the bonds spread evenly in one plane.",
    },
    Mol {
        name: "CH4", display: "CH4", ax: "AX4", domains: 4,
        domain_geo: "Tetrahedral", shape: "Tetrahedral", open: (202.0, 21.0),
        atoms: &CH4_ATOMS, lone: &EMPTY3, pairs: &CH4_PAIRS,
        why: "Four bonding pairs and no lone pairs: a tetrahedron keeps all four as far apart as possible, at 109.5°.",
        why_plain: "Four bonding pairs and no lone pairs: a tetrahedron keeps all four as far apart as possible.",
    },
    Mol {
        name: "NH3", display: "NH3", ax: "AX3E", domains: 4,
        domain_geo: "Tetrahedral", shape: "Trigonal pyramidal", open: (0.0, 27.0),
        atoms: &NH3_ATOMS, lone: &NH3_LONE, pairs: &NH3_PAIRS,
        why: "Four electron domains, one of them a lone pair. A lone pair repels more strongly than a bonding pair, so the three N–H bonds close to about 107° and the atoms form a trigonal pyramid.",
        why_plain: "Four electron domains, one of them a lone pair. The lone pair repels more strongly than a bonding pair, so the three N–H bonds close up and the atoms form a trigonal pyramid.",
    },
    Mol {
        name: "H2O", display: "H2O", ax: "AX2E2", domains: 4,
        domain_geo: "Tetrahedral", shape: "Bent", open: (44.0, 62.0),
        atoms: &H2O_ATOMS, lone: &H2O_LONE, pairs: &H2O_PAIRS,
        why: "Four electron domains, two of them lone pairs. The lone pairs repel more strongly than the bonds, closing the H–O–H angle to about 104.5°, so the molecule is bent, not linear.",
        why_plain: "Four electron domains, two of them lone pairs. The lone pairs repel more strongly than the bonds, so the molecule is bent, not linear.",
    },
    Mol {
        name: "PCl5", display: "PCl5", ax: "AX5", domains: 5,
        domain_geo: "Trigonal bipyramidal", shape: "Trigonal bipyramidal", open: (250.0, 0.0),
        atoms: &PCL5_ATOMS, lone: &EMPTY3, pairs: &PCL5_PAIRS,
        why: "Five bonding pairs: three equatorial bonds 120° apart and two axial bonds at 90° to them, a trigonal bipyramid.",
        why_plain: "Five bonding pairs: three equatorial bonds and two axial bonds, a trigonal bipyramid.",
    },
    Mol {
        name: "SF6", display: "SF6", ax: "AX6", domains: 6,
        domain_geo: "Octahedral", shape: "Octahedral", open: (0.0, 20.0),
        atoms: &SF6_ATOMS, lone: &EMPTY3, pairs: &SF6_PAIRS,
        why: "Six bonding pairs: all six positions are equivalent, with 90° between neighbours, an octahedron.",
        why_plain: "Six bonding pairs: all six positions are equivalent, an octahedron.",
    },
    Mol {
        name: "XeF4", display: "XeF4", ax: "AX4E2", domains: 6,
        domain_geo: "Octahedral", shape: "Square planar", open: (0.0, 0.0),
        atoms: &XEF4_ATOMS, lone: &XEF4_LONE, pairs: &XEF4_PAIRS,
        why: "Six electron domains, two of them lone pairs. The lone pairs take the two axial positions, leaving four bonds in a square plane at 90°.",
        why_plain: "Six electron domains, two of them lone pairs. The lone pairs take the axial positions, leaving four bonds in a square plane.",
    },
    Mol {
        name: "SF4", display: "SF4", ax: "AX4E", domains: 5,
        domain_geo: "Trigonal bipyramidal", shape: "See-saw", open: (296.0, 36.0),
        atoms: &SF4_ATOMS, lone: &SF4_LONE, pairs: &SF4_PAIRS,
        why: "Five electron domains, one of them a lone pair in an equatorial position. The four bonds form a see-saw, with the equatorial pair at about 102° and the axial bonds at 90° to them.",
        why_plain: "Five electron domains, one of them a lone pair in an equatorial position. The four bonds form a see-saw.",
    },
];

pub fn molecules() -> &'static [Mol] { &MOLECULES }

// ---------------------------------------------------------------------------------------------
// The projection: one matrix, one orthographic projection
// ---------------------------------------------------------------------------------------------

pub type Mat3 = [[f64; 3]; 3];

/// The rotation matrix R = Rx(tilt) · Ry(turn), applied to column vectors. Turn is a yaw about the
/// y axis (0 to 360°); tilt is a pitch about the x axis (-90 to 90°). At (0, 0) this is the identity.
pub fn view_matrix(turn: f64, tilt: f64) -> Mat3 {
    // A full turn is exactly the identity: 360° is folded to 0° before the trigonometry, so the
    // floating-point sine of 2π cannot flip a depth tie and reorder the drawing.
    let (a, b) = (turn.rem_euclid(360.0).to_radians(), tilt.to_radians());
    let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
    [
        [ca, 0.0, sa],
        [sa * sb, cb, -ca * sb],
        [-cb * sa, sb, cb * ca],
    ]
}

pub fn apply(m: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// The orthographic projection of an already-rotated point: screen x is model x, screen y is model
/// -y (SVG y grows downward). The depth z is returned separately; larger z is nearer the viewer.
pub fn project_rotated(p: [f64; 3], c: Pt, scale: f64) -> Pt {
    (c.0 + scale * p[0], c.1 - scale * p[1])
}

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub turn: f64,
    pub tilt: f64,
}

impl View {
    pub fn new(turn: f64, tilt: f64) -> Self { View { turn, tilt } }
    pub fn matrix(&self) -> Mat3 { view_matrix(self.turn, self.tilt) }
    pub fn rotate(&self, v: [f64; 3]) -> [f64; 3] { apply(&self.matrix(), v) }
    pub fn screen(&self, v: [f64; 3], c: Pt, scale: f64) -> (Pt, f64) {
        let r = self.rotate(v);
        (project_rotated(r, c, scale), r[2])
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the controls
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Setup {
    pub mol: usize,
    pub lone: bool,
    pub angles: bool,
    pub labels: bool,
    pub turn: f64,
    pub tilt: f64,
    pub answers: bool,
}

fn quote(s: &str) -> String {
    let mut t: String = s.chars().take(40).collect();
    if s.chars().count() > 40 { t.push_str("..."); }
    t
}

fn typed(v: f64) -> String {
    if v.is_nan() {
        "not a number".into()
    } else if v == f64::INFINITY {
        "∞".into()
    } else if v == f64::NEG_INFINITY {
        "−∞".into()
    } else {
        fmt(v)
    }
}

fn norm_name(s: &str) -> String {
    let mut o = s.trim().to_string();
    for (i, c) in "₀₁₂₃₄₅₆₇₈₉".chars().enumerate() { o = o.replace(c, &i.to_string()); }
    o.chars().filter(|c| !c.is_whitespace()).collect()
}

fn molecule(p: &Params, n: &mut Vec<String>) -> usize {
    match p.0.get("molecule") {
        None => DEFAULT_MOL,
        Some(Value::S(s)) => {
            let q = norm_name(s);
            NAMES.iter().position(|x| *x == q).unwrap_or_else(|| {
                n.push(format!("Molecule \"{}\" is not one of the molecules offered, so CH4 is used.", quote(s)));
                DEFAULT_MOL
            })
        }
        Some(_) => {
            n.push("Molecule is not a name, so CH4 is used.".into());
            DEFAULT_MOL
        }
    }
}

fn flag(p: &Params, key: &str, name: &str, d: bool, n: &mut Vec<String>) -> bool {
    match p.0.get(key) {
        None => d,
        Some(Value::B(b)) => *b,
        Some(_) => {
            n.push(format!("{name} is not on or off, so it is {} by default.", if d { "on" } else { "off" }));
            d
        }
    }
}

fn angle(p: &Params, key: &str, name: &str, lo: f64, hi: f64, d: f64, n: &mut Vec<String>) -> f64 {
    let v = match p.0.get(key) {
        None => return d,
        Some(Value::F(v)) => *v,
        Some(Value::I(i)) => *i as f64,
        Some(_) => {
            n.push(format!("{name} is not a number, so {name} = {}° is used.", fmt(d)));
            return d;
        }
    };
    if v.is_nan() {
        n.push(format!("{name} is not a number, so {name} = {}° is used.", fmt(d)));
        return d;
    }
    if v == f64::INFINITY {
        n.push(format!("{name} = ∞° is above the {}° this figure draws, so {}° is used.", fmt(hi), fmt(hi)));
        return hi;
    }
    if v == f64::NEG_INFINITY {
        n.push(format!("{name} = −∞° is below the {}° this figure draws, so {}° is used.", fmt(lo), fmt(lo)));
        return lo;
    }
    if v < lo {
        n.push(format!("{name} = {}° is below the {}° this figure draws, so {}° is used.", typed(v), fmt(lo), fmt(lo)));
        return lo;
    }
    if v > hi {
        n.push(format!("{name} = {}° is above the {}° this figure draws, so {}° is used.", typed(v), fmt(hi), fmt(hi)));
        return hi;
    }
    v
}

pub fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n = Vec::new();
    let mol = molecule(p, &mut n);
    let lone = flag(p, "lone", "Show lone pairs", true, &mut n);
    let angles = flag(p, "angles", "Show bond angles", true, &mut n);
    let labels = flag(p, "labels", "Show atom labels", true, &mut n);
    let turn = angle(p, "turn", "Turn", TURN_LO, TURN_HI, TURN_DEFAULT, &mut n);
    let tilt = angle(p, "tilt", "Tilt", TILT_LO, TILT_HI, TILT_DEFAULT, &mut n);
    // A wrong-typed answer switch is safer as the student copy; a missing one keeps the key.
    let answers = match p.0.get(SHOW_ANSWERS) {
        None => true,
        Some(Value::B(b)) => *b,
        _ => false,
    };
    (Setup { mol, lone, angles, labels, turn, tilt, answers }, n)
}

pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

/// The accessible description, saying only what this copy actually draws (and no answer).
fn describe(lone: bool, labels: bool, angles: bool) -> String {
    let mut s = String::from("A rotatable three-dimensional ball-and-stick model of one molecule");
    let mut feats: Vec<&str> = Vec::new();
    if lone { feats.push("lone pairs"); }
    if labels { feats.push("atom labels"); }
    if angles { feats.push("bond angles"); }
    if !feats.is_empty() {
        s.push_str(" with ");
        s.push_str(&feats.join(", "));
    }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------------------------
// The figure
// ---------------------------------------------------------------------------------------------

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "VSEPR Shapes",
        blurb: "A rotatable ball-and-stick model of a simple molecule; the key can show lone pairs, atom labels and bond angles.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("molecule", "Molecule", &NAMES, "CH4")
            .hint("BeCl2, BF3, CH4, NH3, H2O, PCl5, SF6, XeF4 or SF4. The angle is the one drawn in this simplified model"),
        ParamDef::bool("lone", "Show lone pairs", true)
            .hint("A pair of dots on a short dashed stick pointing where the pair truly projects; a pair that projects on a bond or at the viewer, sits behind a nearer atom, or would overlap another pair is left off with a panel note"),
        ParamDef::bool("angles", "Show bond angles", true)
            .hint("An arc and the angle at the central atom, drawn only when the page shows the true 3-D angle; the number is an answer"),
        ParamDef::bool("labels", "Show atom labels", true),
        ParamDef::float("turn", "Turn (yaw, °)", TURN_LO, TURN_HI, 1.0, TURN_DEFAULT)
            .hint("Rotate the model about the vertical axis, 0 to 360°, relative to the molecule's opening view"),
        ParamDef::float("tilt", "Tilt (pitch, °)", TILT_LO, TILT_HI, 1.0, TILT_DEFAULT)
            .hint("Tip the model toward or away from you, -90 to 90°, relative to the molecule's opening view"),
        answers_param().hint("Off blanks the electron-domain geometry, the molecular shape and every bond angle, removes the angle arcs and omits the explanation"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// Small geometry and text helpers
// ---------------------------------------------------------------------------------------------

fn norm3(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-12 { [v[0] / l, v[1] / l, v[2] / l] } else { [0.0, 0.0, 1.0] }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The angle between two model directions, in degrees, 0 to 180.
pub fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (na, nb) = (norm3(a), norm3(b));
    dot(na, nb).clamp(-1.0, 1.0).acos().to_degrees()
}

/// A point `t` of the way along the shorter great-circle arc from `u` to `v` (both unit vectors).
/// At exactly 180° a fixed perpendicular is used, because the arc's plane is not unique.
pub fn slerp(u: [f64; 3], v: [f64; 3], t: f64) -> [f64; 3] {
    let (u, v) = (norm3(u), norm3(v));
    let c = dot(u, v).clamp(-1.0, 1.0);
    let th = c.acos();
    if th < 1e-6 { return u; }
    if (PI - th).abs() < 1e-6 {
        let axis = if u[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [0.0, 1.0, 0.0] };
        let w = norm3(cross(u, axis));
        let a = PI * t;
        return norm3([u[0] * a.cos() + w[0] * a.sin(), u[1] * a.cos() + w[1] * a.sin(), u[2] * a.cos() + w[2] * a.sin()]);
    }
    let s = th.sin();
    let (c1, c2) = (((1.0 - t) * th).sin() / s, (t * th).sin() / s);
    norm3([u[0] * c1 + v[0] * c2, u[1] * c1 + v[1] * c2, u[2] * c1 + v[2] * c2])
}

fn dist(a: Pt, b: Pt) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

/// The distance from a point to a line segment.
fn point_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-12 { return dist(p, a); }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

fn grey(v: f64) -> String {
    let c = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{c:02x}{c:02x}{c:02x}")
}

/// The estimated ink box of one line of text, the same estimator the wrapping uses.
fn est_box(x: f64, y: f64, fs: f64, anchor: &str, s: &str) -> Bx {
    let w = text_w(s, fs);
    let (l, r) = match anchor {
        "end" => (x - w, x),
        "middle" => (x - w / 2.0, x + w / 2.0),
        _ => (x, x + w),
    };
    (l, r, y - 0.86 * fs, y + 0.30 * fs)
}

fn pad(b: Bx, m: f64) -> Bx { (b.0 - m, b.1 + m, b.2 - m, b.3 + m) }

fn hits(a: Bx, b: Bx) -> bool { a.0 < b.1 && b.0 < a.1 && a.2 < b.3 && b.2 < a.3 }

/// Whether the segment a-b passes through the box (Liang-Barsky clipping).
fn seg_hits(a: Pt, b: Pt, r: Bx) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.1 - a.0), (-dy, a.1 - r.2), (dy, r.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
        }
    }
    t0 < t1
}

/// Depth order with a stable tie rule: depths within 1e-12 are the same depth, and the index
/// decides. Rotations produce differences of a few ulps where exact symmetry makes a tie, and a
/// tie must not reorder the drawing from run to run.
fn depth_cmp(a: (f64, usize), b: (f64, usize)) -> std::cmp::Ordering {
    if (a.0 - b.0).abs() < 1e-12 { a.1.cmp(&b.1) } else { a.0.total_cmp(&b.0) }
}

fn box_hits_disc(b: Bx, c: Pt, r: f64) -> bool {
    let nx = b.0.max(c.0).min(b.1);
    let ny = b.2.max(c.1).min(b.3);
    (nx - c.0).powi(2) + (ny - c.1).powi(2) < r * r
}

fn text_el(d: &mut Doc, x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, kind: &str, feat: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}" data-o="{kind}" data-f="{feat}">{}</text>"##,
        svg::esc(s)
    ));
}

/// A text element at a given font weight: the panel's values are set at 600, its labels at 400.
fn text_el_weighted(d: &mut Doc, x: f64, y: f64, fs: f64, anchor: &str, fill: &str, weight: &str, s: &str, kind: &str, feat: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" font-weight="{weight}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}" data-o="{kind}" data-f="{feat}">{}</text>"##,
        svg::esc(s)
    ));
}

fn text_halo(d: &mut Doc, x: f64, y: f64, fs: f64, anchor: &str, s: &str, feat: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="none" stroke="#ffffff" stroke-width="3" stroke-linejoin="round" data-o="anglehalo" data-f="{feat}">{}</text>"##,
        svg::esc(s)
    ));
}

// ---------------------------------------------------------------------------------------------
// What was drawn, for the tests and for the panel
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct AtomReport {
    pub index: usize,
    pub p: Pt,
    pub depth: f64,
    pub r: f64,
    pub labelled: bool,
    /// A ligand whose ball projects onto the central atom's: a bond pointing straight at the
    /// viewer. It is left out rather than drawn twice at the centre.
    pub hidden: bool,
}

#[derive(Clone, Debug)]
pub struct BondReport {
    pub index: usize,
    pub depth: f64,
    /// Depth at the trimmed start and end of the stick, for splitting it by depth.
    pub da: f64,
    pub db: f64,
    pub a: Pt,
    pub b: Pt,
    pub width: f64,
    /// False when the two balls overlap in projection and the stick would have no length.
    pub drawn: bool,
}

#[derive(Clone, Debug)]
pub struct LoneReport {
    pub pair: usize,
    pub p: Pt,
    pub dots: [Pt; 2],
    pub depth: f64,
}

#[derive(Clone, Debug)]
pub struct AngleReport {
    pub pair: usize,
    pub a: usize,
    pub b: usize,
    pub angle: f64,
    pub vertex: Pt,
    pub at: Pt,
    pub arc: Vec<Pt>,
    pub text: String,
}

pub struct Report {
    pub setup: Setup,
    pub mol_index: usize,
    pub view: (f64, f64),
    pub atoms: Vec<AtomReport>,
    pub bonds: Vec<BondReport>,
    pub lone: Vec<LoneReport>,
    pub angles: Vec<AngleReport>,
    /// The panel lines as drawn (key or student).
    pub panel: Vec<String>,
    pub view_notes: Vec<String>,
    pub footer: Vec<String>,
    pub notes: Vec<String>,
}

impl Report {
    pub fn mol(&self) -> &'static Mol { &MOLECULES[self.mol_index] }
}

/// The atom pair used for an angle label: among all pairs whose 3-D angle matches the stated one,
/// the one whose projected bonds are longest and most clearly separated. That keeps the arc and
/// its label out of an edge-on view; the 180° pair has only one choice.
fn best_pair(mol: &Mol, pair: &AnglePair, view: &View, c: Pt, scale: f64, hidden: &[bool]) -> Option<(usize, usize)> {
    if pair.angle >= 179.5 {
        if hidden.get(pair.a).copied().unwrap_or(false) || hidden.get(pair.b).copied().unwrap_or(false) { return None; }
        let (pa, _) = view.screen(mol.atoms[pair.a].pos, c, scale);
        let (pb, _) = view.screen(mol.atoms[pair.b].pos, c, scale);
        let (la, lb) = (dist(pa, c), dist(pb, c));
        return if la > 0.12 * scale && lb > 0.12 * scale { Some((pair.a, pair.b)) } else { None };
    }
    let mut best: Option<(usize, usize)> = None;
    let mut best_score = -1.0;
    for a in 1..mol.atoms.len() {
        for b in a + 1..mol.atoms.len() {
            if hidden.get(a).copied().unwrap_or(false) || hidden.get(b).copied().unwrap_or(false) { continue; }
            if (angle_between(mol.atoms[a].pos, mol.atoms[b].pos) - pair.angle).abs() > 1.0 { continue; }
            let (pa, _) = view.screen(mol.atoms[a].pos, c, scale);
            let (pb, _) = view.screen(mol.atoms[b].pos, c, scale);
            let va = (pa.0 - c.0, pa.1 - c.1);
            let vb = (pb.0 - c.0, pb.1 - c.1);
            let cross = (va.0 * vb.1 - va.1 * vb.0).abs();
            let dot = va.0 * vb.0 + va.1 * vb.1;
            let la = (va.0 * va.0 + va.1 * va.1).sqrt();
            let lb = (vb.0 * vb.0 + vb.1 * vb.1).sqrt();
            // The arc is drawn only when the page shows the same angle as the label. A 3-D
            // projection can foreshorten a bond and change the angle between the sticks; then
            // this pair is left off (the panel still names the angle) rather than mislabelled.
            let shown = cross.atan2(dot).to_degrees();
            if (shown - pair.angle).abs() > 0.4 { continue; }
            let score = cross * (0.25 + la.min(lb) / scale);
            if score > best_score + 1e-9 {
                best_score = score;
                best = Some((a, b));
            }
        }
    }
    best
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, notes) = read(p);
    let mol = &MOLECULES[st.mol];
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let key = st.answers;

    let ink = if print { "#000" } else { "#111" };
    let mute = if print { "#555" } else { "#5b6068" };
    let rule = if print { "#b0b0b0" } else { "#c9ced6" };

    let w = 760.0;
    // the model and the facts start this far down the sheet: a 64 px top margin left a wide blank
    // band between the preview's toolbar and the molecule
    const TOP_Y: f64 = 30.0;
    // The model is drawn larger and starts at the panel's top, so the drawing, the facts and the
    // caption fill the sheet together (the old 118 px model left a white band under the facts).
    let cx = 250.0;
    let geom = 1.0 + 0.18 * (lab - 1.0);
    let scale = 136.0 * geom;
    let r_atom = 16.0 * lab;
    let view = View::new(st.turn + mol.open.0, st.tilt + mol.open.1);
    // The model's band is as tall as this view of the molecule needs (its atoms and lone pairs,
    // with room for a label), not a sphere any rotation could fill: a view with nothing pointing
    // up left a wide blank band above the molecule.
    let (up, down) = mol.atoms.iter().map(|a| a.pos).chain(mol.lone.iter().copied()).fold((0.0f64, 0.0f64), |(u, d), v| {
        let dy = view.screen(v, (0.0, 0.0), scale).0 .1;
        (u.max(-dy), d.max(dy))
    });
    let (up, down) = (up.max(0.25 * scale), down.max(0.25 * scale));
    let cy = (TOP_Y + up + r_atom + 8.0).round();
    let fs_atom = 17.0 * lab;
    let fs_angle = 16.5 * lab;
    let fs_panel = 16.5 * lab;
    let fs_note = 16.5 * lab;
    let panel_x = 456.0;
    let panel_w = 282.0;
    let mol_index = st.mol;

    // ---- rotated atoms and their screen positions
    let mut atom_report: Vec<AtomReport> = mol.atoms.iter().enumerate().map(|(i, a)| {
        let (p, depth) = view.screen(a.pos, (cx, cy), scale);
        let depth = if depth == 0.0 { 0.0 } else { depth };
        AtomReport { index: i, p, depth, r: r_atom, labelled: false, hidden: false }
    }).collect();
    let central_p = atom_report[0].p;
    for a in atom_report.iter_mut().skip(1) {
        if dist(a.p, central_p) <= 0.6 { a.hidden = true; }
    }

    // ---- bonds, far first, thinner and lighter; ties by index
    let mut bonds: Vec<BondReport> = (1..mol.atoms.len()).map(|i| {
        let z = (atom_report[0].depth + atom_report[i].depth) / 2.0;
        let z = if z == 0.0 { 0.0 } else { z };
        let t = ((z + 1.0) / 2.0).clamp(0.0, 1.0);
        let width = (2.6 + 1.8 * t) * lab;
        let (pc, pl) = (atom_report[0].p, atom_report[i].p);
        let len = dist(pc, pl);
        let (z0, z1) = (atom_report[0].depth, atom_report[i].depth);
        let (a, b, da, db) = if len > 2.0 * r_atom + 1.0 {
            let u = ((pl.0 - pc.0) / len, (pl.1 - pc.1) / len);
            let t0 = r_atom / len;
            let t1 = 1.0 - r_atom / len;
            ((pc.0 + u.0 * r_atom, pc.1 + u.1 * r_atom), (pl.0 - u.0 * r_atom, pl.1 - u.1 * r_atom),
             z0 + (z1 - z0) * t0, z0 + (z1 - z0) * t1)
        } else {
            // The two balls overlap: keep the full stick in the SVG (behind the balls) so a
            // bond pointing at the viewer is still a bond, not a silently missing one.
            (pc, pl, z0, z1)
        };
        BondReport { index: i, depth: z, da, db, a, b, width, drawn: true }
    }).collect();
    bonds.sort_by(|a, b| depth_cmp((a.depth, a.index), (b.depth, b.index)));

    // ---- lone pairs: the stick is the real projected 3-D lone-pair direction. If that direction
    // collapses to a point, lands on a bond, or puts two pairs' dots on top of each other, the pair
    // is omitted and the panel says why; it is never turned to a made-up screen direction.
    let mut lone: Vec<LoneReport> = Vec::new();
    let mut lone_collapsed = 0usize;
    let mut lone_on_bond = 0usize;
    let mut lone_overlap = 0usize;
    let mut lone_hidden = 0usize;
    if st.lone {
        // The line of sight of each bond, not only the trimmed stick: a pair beyond a short
        // projected bond still reads as lying on that bond.
        let bond_segments: Vec<(Pt, Pt)> = atom_report.iter().skip(1)
            .filter(|a| !a.hidden)
            .filter_map(|a| {
                let v = (a.p.0 - cx, a.p.1 - cy);
                let l = (v.0 * v.0 + v.1 * v.1).sqrt();
                if l < 1e-9 { None } else { Some(((cx, cy), (cx + v.0 / l * scale, cy + v.1 / l * scale))) }
            }).collect();
        let mut chosen_dots: Vec<Pt> = Vec::new();
        for (k, d) in mol.lone.iter().enumerate() {
            let (pp, depth) = view.screen(*d, (cx, cy), scale);
            let v = (pp.0 - cx, pp.1 - cy);
            let l = (v.0 * v.0 + v.1 * v.1).sqrt();
            if l < 0.12 * scale { lone_collapsed += 1; continue; }
            let u = (v.0 / l, v.1 / l);
            let perp = (-u.1, u.0);
            let off = 3.8 * lab;
            let mut r = 0.30 * scale;
            let p0 = (cx + u.0 * r, cy + u.1 * r);
            if atom_report[1..].iter().any(|a| dist(p0, a.p) < a.r + off + 2.0) { r = 0.22 * scale; }
            let p = (cx + u.0 * r, cy + u.1 * r);
            let dots = [(p.0 + perp.0 * off, p.1 + perp.1 * off), (p.0 - perp.0 * off, p.1 - perp.1 * off)];
            let on_bond = bond_segments.iter().any(|(a, b)| {
                point_seg(p, *a, *b) < 4.2 || dots.iter().any(|dd| point_seg(*dd, *a, *b) < 4.2)
            });
            if on_bond { lone_on_bond += 1; continue; }
            // (the threshold is the drawing's own scale: measured at the 118 px model it was 5 px)
            if chosen_dots.iter().any(|q| dots.iter().any(|dd| dist(*dd, *q) < 5.0 * scale / (118.0 * geom))) {
                lone_overlap += 1;
                continue;
            }
            // A pair whose dots would be covered by a nearer ligand ball is not drawn.
            if dots.iter().any(|dd| atom_report.iter().skip(1).any(|a| {
                !a.hidden && a.depth > depth + 1e-9 && dist(*dd, a.p) < a.r
            })) {
                lone_hidden += 1;
                continue;
            }
            chosen_dots.extend(dots);
            lone.push(LoneReport { pair: k, p, dots, depth });
        }
    }

    // ---- which atom labels are visible: a nearer ball wins
    let atom_order: Vec<usize> = {
        let mut v: Vec<usize> = (0..atom_report.len()).collect();
        v.sort_by(|&a, &b| depth_cmp((atom_report[a].depth, a), (atom_report[b].depth, b)));
        v
    };
    let mut visible_labels: Vec<bool> = vec![false; atom_report.len()];
    let mut accepted: Vec<Bx> = Vec::new();
    if st.labels {
        // Every atom gets its label in depth order, nearest first: a nearer ball may cover a
        // farther label, and a far label that a nearer ball covers is left off. The central atom
        // has no special place in this order.
        for &i in atom_order.iter().rev() {
            let a = &atom_report[i];
            if a.hidden { continue; }
            let text = mol.atoms[i].sym;
            let y = a.p.1 + 0.35 * fs_atom;
            let bx = pad(est_box(a.p.0, y, fs_atom, "middle", text), 0.5);
            let in_canvas = bx.0 >= 0.0 && bx.1 <= w && bx.2 >= 0.0;
            let mut blocked = !in_canvas;
            if !blocked {
                for (j, other) in atom_report.iter().enumerate() {
                    if j == i || other.hidden { continue; }
                    let later = depth_cmp((other.depth, j), (a.depth, i)) == std::cmp::Ordering::Greater;
                    if later && box_hits_disc(bx, other.p, other.r + 0.5) { blocked = true; break; }
                }
            }
            if !blocked {
                for b in &accepted {
                    if hits(bx, *b) { blocked = true; break; }
                }
            }
            if !blocked {
                for l in &lone {
                    let db = (l.p.0 - 6.0, l.p.0 + 6.0, l.p.1 - 6.0, l.p.1 + 6.0);
                    if hits(bx, db) { blocked = true; break; }
                }
            }
            if !blocked {
                visible_labels[i] = true;
                accepted.push(bx);
            }
        }
        // The centre is the reference atom: if a nearer ball would cover its label, keep the
        // label anyway (it is drawn after every ball, so it stays readable) unless it would
        // collide with another label or a lone-pair dot.
        if !visible_labels[0] {
            let a = &atom_report[0];
            let text = mol.atoms[0].sym;
            let y = a.p.1 + 0.35 * fs_atom;
            let bx = pad(est_box(a.p.0, y, fs_atom, "middle", text), 0.5);
            let free = bx.0 >= 0.0 && bx.1 <= w && bx.2 >= 0.0
                && accepted.iter().all(|b| !hits(bx, *b))
                && lone.iter().all(|l| !hits(bx, (l.p.0 - 6.0, l.p.0 + 6.0, l.p.1 - 6.0, l.p.1 + 6.0)));
            visible_labels[0] = free;
        }
    }
    for (i, a) in atom_report.iter_mut().enumerate() { a.labelled = visible_labels[i]; }

    // ---- the angle arcs and their labels
    let mut angles: Vec<AngleReport> = Vec::new();
    if st.angles && key {
    let mut placed: Vec<Bx> = Vec::new();
    for (k, pair) in mol.pairs.iter().enumerate() {
        let hidden: Vec<bool> = atom_report.iter().map(|a| a.hidden).collect();
        let Some((ia, ib)) = best_pair(mol, pair, &view, (cx, cy), scale, &hidden) else { continue };
        let u = norm3(mol.atoms[ia].pos);
        let v = norm3(mol.atoms[ib].pos);
        let arc_r = 0.30 + 0.02 * k as f64;
        let mut arc: Vec<Pt> = Vec::new();
        for j in 0..=24 {
            let t = j as f64 / 24.0;
            let d = slerp(u, v, t);
            let (p, _) = view.screen(d, (cx, cy), scale * arc_r);
            arc.push(p);
        }
        // A 180° arc seen edge-on is a straight line under the two sticks and the centre ball;
        // leave it off rather than draw a broken arc with a floating label.
        if pair.angle >= 179.5 {
            let (p0, p1) = (arc[0], *arc.last().unwrap());
            let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
            let l = (dx * dx + dy * dy).sqrt();
            let dev = if l < 1e-9 { 0.0 } else {
                arc.iter().map(|p| (dx * (p.1 - p0.1) - dy * (p.0 - p0.0)).abs() / l).fold(0.0, f64::max)
            };
            if dev < 2.0 { continue; }
        }
        let mid = slerp(u, v, 0.5);
        let (mp, _) = view.screen(mid, (cx, cy), scale * 0.55);
        let vertex = atom_report[0].p;
        let mut base = (mp.0 - vertex.0, mp.1 - vertex.1);
        let bl = (base.0 * base.0 + base.1 * base.1).sqrt();
        if bl < 0.12 * scale {
            // Edge-on: use a screen direction square to the average bond direction.
            let (pa, _) = view.screen(u, (cx, cy), scale);
            let (pb, _) = view.screen(v, (cx, cy), scale);
            let mut ax = (pa.0 - vertex.0 + pb.0 - vertex.0, pa.1 - vertex.1 + pb.1 - vertex.1);
            let al = (ax.0 * ax.0 + ax.1 * ax.1).sqrt();
            if al < 1e-9 { ax = (1.0, 0.0); } else { ax = (ax.0 / al, ax.1 / al); }
            base = (-ax.1, ax.0);
            if base.1 < 0.0 { base = (-base.0, -base.1); }
            if base.0.abs() + base.1.abs() < 1e-9 { base = (0.0, 1.0); }
            let l = (base.0 * base.0 + base.1 * base.1).sqrt().max(1e-9);
            base = (base.0 / l, base.1 / l);
        } else {
            base = (base.0 / bl, base.1 / bl);
        }
        // Try a few radii and small angular offsets, clear of the atoms and the other labels.
        let text_half = text_w(pair.text, fs_angle) / 2.0;
        let r0 = (r_atom + text_half + 7.0).max(0.30 * scale + 0.86 * fs_angle + 4.0).max(0.38 * scale);
        let mut radii = vec![r0, r0 + 0.08 * scale, r0 + 0.16 * scale, r0 + 0.24 * scale];
        let mut rr = r0 + 0.32 * scale;
        while rr <= 0.82 * scale { radii.push(rr); rr += 0.06 * scale; }
        let mut best: Option<(Pt, Bx)> = None;
        let mut fallback = (vertex.0 + base.0 * r0, vertex.1 + base.1 * r0);
        'outer: for &rad in &radii {
            for &deg in &[0.0f64, 12.0, -12.0, 24.0, -24.0, 36.0, -36.0, 48.0, -48.0] {
                let a = deg.to_radians();
                let d = (base.0 * a.cos() - base.1 * a.sin(), base.0 * a.sin() + base.1 * a.cos());
                let p = (vertex.0 + d.0 * rad, vertex.1 + d.1 * rad);
                fallback = p;
                let text = pair.text.to_string();
                let bx = if text.is_empty() {
                    (p.0 - 2.0, p.0 + 2.0, p.1 - 2.0, p.1 + 2.0)
                } else {
                    pad(est_box(p.0, p.1, fs_angle, "middle", &text), 5.0)
                };
                if bx.0 < 0.0 || bx.1 > w || bx.2 < 0.0 { continue; }
                let mut bad = false;
                for a2 in &atom_report {
                    if a2.hidden { continue; }
                    if box_hits_disc(bx, a2.p, a2.r + 2.0) { bad = true; break; }
                }
                if !bad {
                    for b in &accepted {
                        if hits(bx, *b) { bad = true; break; }
                    }
                }
                if !bad {
                    for b in &placed {
                        if hits(bx, *b) { bad = true; break; }
                    }
                }
                if !bad {
                    for l in &lone {
                        let db = (l.p.0 - 8.0, l.p.0 + 8.0, l.p.1 - 8.0, l.p.1 + 8.0);
                        if hits(bx, db) { bad = true; break; }
                    }
                }
                if !bad {
                    for b in &bonds {
                        if b.drawn && seg_hits(b.a, b.b, bx) { bad = true; break; }
                    }
                }
                // the number stays off its own arc (and every arc drawn before it), so the
                // degree sign is not drawn across the curve it names
                if !bad {
                    bad = std::iter::once(&arc).chain(angles.iter().map(|a| &a.arc)).any(|c| c.windows(2).any(|sg| seg_hits(sg[0], sg[1], bx)));
                }
                if !bad { best = Some((p, bx)); break 'outer; }
            }
        }
        let (at, bx, text) = match best {
            Some((at, bx)) => (at, bx, pair.text.to_string()),
            // A crowded view has no clear place for the number: keep the true arc and let the panel
            // carry the value rather than draw a label through a ball.
            None => (fallback, (fallback.0 - 2.0, fallback.0 + 2.0, fallback.1 - 2.0, fallback.1 + 2.0), String::new()),
        };
        placed.push(bx);
        angles.push(AngleReport { pair: k, a: ia, b: ib, angle: pair.angle, vertex, at, arc, text });
    }
    }

    // ---- view notes: what an edge-on view hides
    let centre_depth = atom_report[0].depth;
    let hidden: Vec<&AtomReport> = atom_report.iter().skip(1).filter(|a| a.hidden).collect();
    let hidden_near = hidden.iter().filter(|a| a.depth > centre_depth + 1e-9).count();
    let hidden_far = hidden.len().saturating_sub(hidden_near);
    let near_centre = atom_report.iter().skip(1)
        .filter(|a| !a.hidden && dist(a.p, atom_report[0].p) < r_atom)
        .min_by(|a, b| dist(a.p, atom_report[0].p).total_cmp(&dist(b.p, atom_report[0].p)));
    let mut overlaps: Vec<(usize, usize)> = Vec::new();
    for a in 1..atom_report.len() {
        for b in a + 1..atom_report.len() {
            if atom_report[a].hidden || atom_report[b].hidden { continue; }
            if dist(atom_report[a].p, atom_report[b].p) < atom_report[a].r + atom_report[b].r {
                overlaps.push((a, b));
            }
        }
    }
    let mut view_notes: Vec<String> = Vec::new();
    if hidden_near > 0 && hidden_far > 0 {
        view_notes.push("Bonds point straight at and away from the viewer, so the atoms at their ends are hidden.".into());
    } else if hidden_near == 1 {
        view_notes.push("A bond points straight at the viewer, so the atom at its end is hidden.".into());
    } else if hidden_near == 2 {
        view_notes.push("Two bonds point straight at the viewer, so the atoms at their ends are hidden.".into());
    } else if hidden_near > 2 {
        view_notes.push(format!("{hidden_near} bonds point straight at the viewer, so the atoms at their ends are hidden."));
    } else if hidden_far == 1 {
        view_notes.push("A bond points straight away from the viewer, so the atom at its end is hidden behind the centre.".into());
    } else if hidden_far == 2 {
        view_notes.push("Two bonds point straight away from the viewer, so the atoms at their ends are hidden behind the centre.".into());
    } else if hidden_far > 2 {
        view_notes.push(format!("{hidden_far} bonds point straight away from the viewer, so the atoms at their ends are hidden behind the centre."));
    }
    if let Some(a) = near_centre {
        let behind = a.depth < centre_depth - 1e-9;
        view_notes.push(format!("The {} atom lies over the centre{} and hides the {} atom in this view.",
            mol.atoms[a.index].sym, if behind { " behind it" } else { " in front of it" }, mol.atoms[0].sym));
    }
    if let Some((a, b)) = overlaps.first().copied() {
        let nearer = if atom_report[a].depth >= atom_report[b].depth { a } else { b };
        let farther = if nearer == a { b } else { a };
        view_notes.push(format!("The {} atom and the {} atom overlap in this view; the nearer {} atom hides the farther.",
            mol.atoms[a].sym, mol.atoms[b].sym, mol.atoms[nearer].sym));
        let _ = farther;
    }
    if lone_collapsed == 1 {
        view_notes.push("A lone pair points straight at or away from the viewer, so it is not drawn.".into());
    } else if lone_collapsed == 2 {
        view_notes.push("Two lone pairs point straight at or away from the viewer, so they are not drawn.".into());
    } else if lone_collapsed > 2 {
        view_notes.push(format!("{lone_collapsed} lone pairs point straight at or away from the viewer, so they are not drawn."));
    }
    if lone_on_bond == 1 {
        view_notes.push("A lone pair projects along a bond in this view, so it is not drawn.".into());
    } else if lone_on_bond == 2 {
        view_notes.push("Two lone pairs project along bonds in this view, so they are not drawn.".into());
    } else if lone_on_bond > 2 {
        view_notes.push(format!("{lone_on_bond} lone pairs project along bonds in this view, so they are not drawn."));
    }
    if lone_overlap > 0 {
        view_notes.push("Two lone pairs project on top of each other in this view, so one is not drawn.".into());
    }
    if lone_hidden == 1 {
        view_notes.push("A lone pair sits behind a ligand atom in this view, so it is not drawn.".into());
    } else if lone_hidden > 1 {
        view_notes.push(format!("{lone_hidden} lone pairs sit behind ligand atoms in this view, so they are not drawn."));
    }

    // ---- the panel. The key and the student copy have the same facts, so the two exported copies
    // have the same height whatever the label size. Each fact is a label and a value set side by
    // side: the label in the page's grey, the value in ink at 600 weight, so the panel reads as a
    // list of readings rather than one block of text. (The labels are short enough to stay on the
    // value's line: a mid-phrase wrap under a value reads as a broken row.)
    let singular = mol.pairs.len() == 1;
    let angle_key = if st.angles { mol.angle_text() } else { "not shown".into() };
    let angle_student = if singular { BLANK.to_string() } else { format!("{BLANK} and {BLANK}") };
    let facts: Vec<(&str, String)> = vec![
        ("Molecule", format!("{} ({})", mol.display, mol.ax)),
        ("Electron domains", mol.domains.to_string()),
        ("Domain geometry", if key { mol.domain_geo } else { BLANK }.to_string()),
        ("Molecular shape", if key { mol.shape } else { BLANK }.to_string()),
        ("Lone pairs", mol.lone_count().to_string()),
        (if singular { "Bond angle" } else { "Bond angles" }, if key { angle_key.clone() } else { angle_student.clone() }),
    ];
    let panel_notes: Vec<String> = view_notes.iter().flat_map(|n| wrap(n, fs_panel, panel_w)).collect();
    let why_key = wrap(if st.angles { mol.why } else { mol.why_plain }, fs_panel, panel_w);
    let why_student = wrap("Explain the shape and the angle:", fs_panel, panel_w);
    let why_reserved = why_key.len().max(why_student.len()).max(2);
    let why_drawn: Vec<String> = if key { why_key } else { why_student };
    let panel: Vec<String> = facts.iter().map(|(l, v)| format!("{l}: {v}")).chain(panel_notes.iter().cloned()).collect();

    let footer_text = "A simplified ball-and-stick model: atoms are circles joined by single sticks. Each lone pair is a pair of dots on a short dashed stick pointing where the pair truly projects on the central atom; a pair that projects on a bond, at the viewer, behind a nearer atom or onto another pair is left off and the panel says so. Bond lengths are not to scale, and no double bonds, resonance or bond order is shown.";
    // The caption is set at a reading measure of about 72 characters in the sheet's ink (at 82 a
    // phone-width preview read it as ~100-character lines across the whole card).
    let footer = wrap(footer_text, fs_note, (w - 40.0).min(72.0 * 0.5 * fs_note));

    // ---- the facts' column layout. The label column is as wide as the widest label at this size;
    // a value that does not fit beside it goes on its own line under its label rather than being
    // wrapped into a ragged column (or, at a large label size, running into it).
    let line_h = 1.55 * fs_panel;
    let panel_gap = 0.45 * fs_panel;
    let label_texts: Vec<String> = facts.iter().map(|(l, _)| format!("{l}:")).collect();
    let max_label_w = label_texts.iter().map(|t| text_w(t, fs_panel)).fold(0.0, f64::max);
    let label_col = max_label_w + 10.0;
    let panel_right = w - 20.0;
    let value_x = panel_x + label_col;
    let beside = value_x + 70.0 <= panel_right;
    let value_w = (panel_right - value_x).max(70.0);
    let values_below: Vec<Vec<String>> = facts.iter().map(|(_, v)| wrap(v, fs_panel, panel_w)).collect();
    let value_beside = |i: usize| beside && text_w(&facts[i].1, fs_panel) <= value_w;
    let fact_lines_of = |v: &str| if beside && text_w(v, fs_panel) <= value_w { 1 } else { 1 + wrap(v, fs_panel, panel_w).len().max(1) };
    // the key's and the student's values differ in length; the panel reserves the longer of the
    // two, so both copies are the same height now that the panel, not the model, can set it
    let other: [String; 3] = [
        if key { BLANK.to_string() } else { mol.domain_geo.to_string() },
        if key { BLANK.to_string() } else { mol.shape.to_string() },
        if key { angle_student.clone() } else { angle_key.clone() },
    ];
    let fact_lines: usize = (0..facts.len())
        .map(|i| {
            let mine = if value_beside(i) { 1 } else { 1 + values_below[i].len().max(1) };
            let alt = match i {
                2 => fact_lines_of(&other[0]),
                3 => fact_lines_of(&other[1]),
                5 => fact_lines_of(&other[2]),
                _ => mine,
            };
            mine.max(alt)
        })
        .sum();

    // ---- height
    let panel_top = TOP_Y;
    let panel_h = (fact_lines + panel_notes.len()) as f64 * line_h + panel_gap + 0.8 * fs_panel + why_reserved as f64 * line_h;
    let panel_bottom = panel_top + panel_h;
    let mol_bottom = cy + down + r_atom + 24.0;
    let body_bottom = panel_bottom.max(mol_bottom);
    let footer_top = body_bottom + 16.0;
    let h = (footer_top + footer.len() as f64 * 1.5 * fs_note + 14.0).ceil();

    // ---- draw
    let mut d = Doc::new(w, h);
    let labels_drawn = st.labels && atom_report.iter().any(|a| a.labelled);
    let angles_drawn = key && !angles.is_empty();
    d.describe("VSEPR Shapes", &describe(!lone.is_empty(), labels_drawn, angles_drawn));
    d.defs(&fills.defs());

    // the panel rule
    d.raw(&format!(r##"<line x1="434.00" y1="{:.2}" x2="434.00" y2="{:.2}" stroke="{rule}" stroke-width="1.0" data-o="rule"/>"##, panel_top - 6.0, body_bottom));

    // angle arcs, behind the sticks
    for a in angles.iter() {
        if a.arc.len() < 2 { continue; }
        let pts: Vec<String> = a.arc.iter().map(|p| format!("{:.2},{:.2}", p.0, p.1)).collect();
        d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{mute}" stroke-width="{:.2}" stroke-linejoin="round" stroke-linecap="round" data-o="anglearc" data-f="p{}"/>"##, pts.join(" "), 2.2 * lab, a.pair));
    }

    // Sticks, balls and lone dots in ONE depth order, far to near. Each stick is split into
    // short pieces, each sorted by its own depth, so a far ball cannot paint over a near stick
    // and a near ball cannot be covered by a far stick.
    let draw_order: Vec<usize> = atom_order.iter().copied().filter(|&i| !atom_report[i].hidden).collect();
    const BOND_PIECES: usize = 12;
    const GUIDE_PIECES: usize = 6;
    let mut items: Vec<(f64, u8, usize, usize)> = Vec::new();
    for (bi, b) in bonds.iter().enumerate() {
        for s in 0..BOND_PIECES {
            let t = (s as f64 + 0.5) / BOND_PIECES as f64;
            items.push((b.da + (b.db - b.da) * t, 0, bi, s));
        }
    }
    for (li, l) in lone.iter().enumerate() {
        for s in 0..GUIDE_PIECES { items.push((l.depth * ((s as f64 + 0.5) / GUIDE_PIECES as f64), 1, li, s)); }
    }
    for &i in &draw_order { items.push((atom_report[i].depth, 2, i, 0)); }
    for (li, l) in lone.iter().enumerate() { items.push((l.depth, 3, li, 0)); }
    items.sort_by(|a, b| depth_cmp((a.0, a.1 as usize * 10_000_000 + a.2 * 100 + a.3), (b.0, b.1 as usize * 10_000_000 + b.2 * 100 + b.3)));
    for &(depth, kind, idx, seg) in &items {
        match kind {
            0 => {
                let b = &bonds[idx];
                let n = BOND_PIECES as f64;
                let (t0, t1) = (seg as f64 / n, (seg as f64 + 1.0) / n);
                let p0 = (b.a.0 + (b.b.0 - b.a.0) * t0, b.a.1 + (b.b.1 - b.a.1) * t0);
                let p1 = (b.a.0 + (b.b.0 - b.a.0) * t1, b.a.1 + (b.b.1 - b.a.1) * t1);
                let t = ((depth + 1.0) / 2.0).clamp(0.0, 1.0);
                let width = (3.0 + 2.0 * t) * lab;
                let shade = 0.36 - 0.20 * t;
                d.raw(&format!(
                    r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{:.2}" stroke-linecap="round" data-o="bond" data-f="b{}" data-d="{:.5}"/>"##,
                    p0.0, p0.1, p1.0, p1.1, grey(shade), width, b.index, depth
                ));
            }
            1 => {
                let l = &lone[idx];
                let pc = atom_report[0].p;
                let len = dist(pc, l.p);
                if len <= r_atom { continue; }
                let u = ((l.p.0 - pc.0) / len, (l.p.1 - pc.1) / len);
                let start = (pc.0 + u.0 * r_atom, pc.1 + u.1 * r_atom);
                let n = GUIDE_PIECES as f64;
                let (t0, t1) = (seg as f64 / n, (seg as f64 + 1.0) / n);
                let p0 = (start.0 + (l.p.0 - start.0) * t0, start.1 + (l.p.1 - start.1) * t0);
                let p1 = (start.0 + (l.p.0 - start.0) * t1, start.1 + (l.p.1 - start.1) * t1);
                d.raw(&format!(
                    r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{:.2}" stroke-dasharray="3 3" data-o="loneguide" data-f="l{}" data-d="{:.5}"/>"##,
                    p0.0, p0.1, p1.0, p1.1, mute, 1.4 * lab, l.pair, depth
                ));
            }
            2 => {
                let a = &atom_report[idx];
                let fill = fills.series(if mol.atoms[idx].central { 2 } else { 3 });
                d.raw(&format!(
                    r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{fill}" stroke="{ink}" stroke-width="{:.2}" data-o="atom" data-f="a{i}" data-d="{:.5}"/>"##,
                    a.p.0, a.p.1, a.r, 1.4 * lab, a.depth, i = idx
                ));
            }
            _ => {
                let l = &lone[idx];
                for (j, p) in l.dots.iter().enumerate() {
                    d.raw(&format!(
                        r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{ink}" stroke="{ink}" stroke-width="0.6" data-o="lone" data-f="l{}.{j}" data-d="{:.5}"/>"##,
                        p.0, p.1, 2.4 * lab, l.pair, l.depth
                    ));
                }
            }
        }
    }
    // Labels after every ball and dot: a nearer ball may cover a farther label (the label was
    // then left off), while the centre's label stays on top as the reference.
    if st.labels {
        for &i in &draw_order {
            let a = &atom_report[i];
            if a.labelled {
                text_el(&mut d, a.p.0, a.p.1 + 0.35 * fs_atom, fs_atom, "middle", ink, mol.atoms[i].sym, "alabel", &format!("a{i}"));
            }
        }
    }

    // angle labels, on top and with a white halo
    if key {
        for a in angles.iter() {
            if a.text.is_empty() { continue; }
            text_halo(&mut d, a.at.0, a.at.1, fs_angle, "middle", &a.text, &format!("p{}", a.pair));
            text_el(&mut d, a.at.0, a.at.1, fs_angle, "middle", ink, &a.text, "angle", &format!("p{}", a.pair));
        }
    }

    // the panel: a label column in the page's grey, a 600-weight value beside it, then the
    // view's own notes under them
    let mut y = panel_top;
    for (i, (_, value)) in facts.iter().enumerate() {
        text_el(&mut d, panel_x, y, fs_panel, "start", mute, &label_texts[i], "panel", &format!("L{i}"));
        if value_beside(i) {
            text_el_weighted(&mut d, value_x, y, fs_panel, "start", ink, "600", value, "panel", &format!("V{i}"));
            y += line_h;
        } else {
            y += line_h;
            for (j, line) in values_below[i].iter().enumerate() {
                text_el_weighted(&mut d, panel_x, y, fs_panel, "start", ink, "600", line, "panel", &format!("V{i}.{j}"));
                y += line_h;
            }
        }
        if i == 0 { y += panel_gap; }
    }
    for (i, line) in panel_notes.iter().enumerate() {
        text_el(&mut d, panel_x, y, fs_panel, "start", mute, line, "panel", &format!("N{i}"));
        y += line_h;
    }
    if !why_drawn.is_empty() {
        y += 0.8 * fs_panel;
        for (i, line) in why_drawn.iter().enumerate() {
            text_el(&mut d, panel_x, y, fs_panel, "start", if key { ink } else { mute }, line, "why", &format!("W{i}"));
            y += line_h;
        }
    }

    // the model note, across the full width
    for (i, line) in footer.iter().enumerate() {
        text_el(&mut d, 20.0, footer_top + 0.86 * fs_note + i as f64 * 1.5 * fs_note, fs_note, "start", ink, line, "note", &format!("N{i}"));
    }

    let rep = Report {
        setup: st,
        mol_index,
        view: (view.turn, view.tilt),
        atoms: atom_report,
        bonds,
        lone,
        angles,
        panel,
        view_notes,
        footer,
        notes,
    };
    (d.finish(), rep)
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Methane: the tetrahedron", params: vec![s("molecule", "CH4")] },
        Example { name: "Ammonia: one lone pair makes a pyramid", params: vec![s("molecule", "NH3"), f("turn", 30.0), f("tilt", 20.0)] },
        Example { name: "Water: two lone pairs bend it", params: vec![s("molecule", "H2O"), f("turn", 40.0), f("tilt", 14.0)] },
        Example { name: "Sulfur hexafluoride: an octahedron", params: vec![s("molecule", "SF6"), f("turn", 30.0), f("tilt", 22.0)] },
        Example { name: "Xenon tetrafluoride: square planar", params: vec![s("molecule", "XeF4"), f("turn", 20.0), f("tilt", 10.0)] },
        Example { name: "Sulfur tetrafluoride: a see-saw", params: vec![s("molecule", "SF4"), f("turn", 35.0), f("tilt", 16.0)] },
        Example { name: "Student sheet: name the shape and the angle", params: vec![s("molecule", "CH4"), b(SHOW_ANSWERS, false)] },
    ])
}

#[cfg(test)]
mod tests;
