//! The hand-drawn icon set (ported from iniSHial), painted from its SVG sprite.
//!
//! `assets/icons/inishial/icons.svg` is compiled in and read once. Every `<symbol>` is a 24 x 24
//! drawing of stroked paths, each form gone over twice: a solid pass and a ghost pass at opacity
//! .55 (see the header of the sprite). The paths are flattened with the same routine the Lucide
//! icons use, so no SVG renderer is involved and the icons are plain egui line shapes tinted by
//! whatever colour the caller passes.
//!
//! Callers that already hold an icon as a `&str` glyph (the nav lists, the empty states, the
//! account perks) pass one of the constants below instead of a Phosphor character:
//! [`paint_glyph`] draws either kind, so a surface moves to the hand-drawn set by changing the
//! constant it names.
//!
//! The doubling is meant for 20 px and up (the sprite's own note); below that the two passes close
//! into one heavy line, so the stroke has a floor in pixels and small chrome (the close X, carets)
//! stays Phosphor.

use std::collections::HashMap;
use std::sync::OnceLock;

use egui::{Color32, Painter, Pos2, Shape, Stroke};

const SPRITE: &str = include_str!("../assets/icons/inishial/icons.svg");

/// Marks a glyph string as a sprite symbol id rather than a font character.
const PREFIX: &str = "hand:";

macro_rules! glyphs {
    ($($name:ident = $id:literal),* $(,)?) => {
        $(pub const $name: &str = concat!("hand:", $id);)*
        /// Every sprite id the app names, for the test that each one exists.
        #[cfg(test)]
        const NAMED: &[&str] = &[$($id),*];
    };
}

glyphs! {
    HOME = "i-home",
    BANKS = "i-banks",
    EXAMS = "i-page",
    GRADE = "i-check",
    IMPORT = "i-import",
    FIGURES = "i-figures",
    HELP = "i-why",
    SETTINGS = "i-settings",
    EDIT = "i-edit-square",
    ADD = "i-add",
    INFO = "i-info",
    SHARED = "i-shared",
    PERSON = "i-person",
    SUN = "i-sun",
    MOON = "i-moon",
    DOWNLOAD = "i-download",
    SPARKLE = "i-sparkle",
}

/// Ink width of the drawing in 24-grid units, unless the symbol or the path says otherwise.
const DEFAULT_WIDTH: f32 = 1.5;
/// Thinnest line the painter will lay down, in points. Below it the doubled strokes read as a
/// smear on a 1x screen.
const MIN_STROKE_PX: f32 = 1.15;
/// The drawings fill about 20 of their 24 units while a font glyph of the same size fills a little
/// less, so a hand icon is drawn this much larger than the font size it stands in for.
const SIZE_FACTOR: f32 = 1.12;

struct Pass {
    lines: Vec<Vec<(f32, f32)>>,
    alpha: f32,
    width: f32,
}

type Sprite = HashMap<String, Vec<Pass>>;

fn sprite() -> &'static Sprite {
    static CELL: OnceLock<Sprite> = OnceLock::new();
    CELL.get_or_init(|| parse(SPRITE))
}

fn parse(text: &str) -> Sprite {
    let mut out = Sprite::new();
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return out;
    };
    for sym in doc.descendants().filter(|n| n.has_tag_name("symbol")) {
        let Some(id) = sym.attribute("id") else { continue };
        let base: f32 = sym.attribute("stroke-width").and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_WIDTH);
        let passes = sym
            .children()
            .filter(|n| n.has_tag_name("path"))
            .filter_map(|n| {
                let d = n.attribute("d")?;
                Some(Pass {
                    // An "open" subpath stays open: the hand-drawn forms overshoot rather than close.
                    lines: crate::figuresui::lucide::flatten(d).into_iter().map(|(pts, _)| pts).collect(),
                    alpha: n.attribute("opacity").and_then(|v| v.parse().ok()).unwrap_or(1.0),
                    width: n.attribute("stroke-width").and_then(|v| v.parse().ok()).unwrap_or(base),
                })
            })
            .collect();
        out.insert(id.to_string(), passes);
    }
    out
}

/// True when `glyph` names a sprite symbol (one of the constants in this module).
pub fn is_hand(glyph: &str) -> bool {
    glyph.starts_with(PREFIX)
}

/// Paints the glyph centred on `c` as a `size`-point icon: a hand-drawn symbol if `glyph` is one of
/// this module's constants, else the Phosphor character it holds.
pub fn paint_glyph(p: &Painter, c: Pos2, glyph: &str, size: f32, col: Color32) {
    match glyph.strip_prefix(PREFIX) {
        Some(id) => paint(p, c, id, size * SIZE_FACTOR, col),
        None => {
            p.text(c, egui::Align2::CENTER_CENTER, glyph, crate::theme::icon_font(size), col);
        }
    }
}

/// Paints sprite symbol `id` into the `side`-point square centred on `c`.
pub fn paint(p: &Painter, c: Pos2, id: &str, side: f32, col: Color32) {
    let Some(passes) = sprite().get(id) else { return };
    let k = side / 24.0;
    let origin = Pos2::new(c.x - side / 2.0, c.y - side / 2.0);
    for pass in passes {
        let width = (pass.width * k).max(MIN_STROKE_PX);
        let tint = if pass.alpha < 1.0 { col.gamma_multiply(pass.alpha) } else { col };
        let stroke = Stroke::new(width, tint);
        for line in &pass.lines {
            let pts: Vec<Pos2> = line.iter().map(|&(x, y)| Pos2::new(origin.x + x * k, origin.y + y * k)).collect();
            let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else { continue };
            if pts.len() > 1 {
                p.add(Shape::line(pts, stroke));
            }
            // Round caps: egui's line shape ends square.
            p.circle_filled(first, width / 2.0, tint);
            p.circle_filled(last, width / 2.0, tint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_icon_is_in_the_sprite_and_draws_something() {
        let s = sprite();
        assert!(!s.is_empty(), "the sprite did not parse");
        for id in NAMED {
            let passes = s.get(*id).unwrap_or_else(|| panic!("{id} is not in icons.svg"));
            let strokes: usize = passes.iter().map(|p| p.lines.iter().filter(|l| l.len() > 1).count()).sum();
            assert!(strokes >= 2, "{id} has {strokes} drawn strokes; every form is drawn twice");
        }
    }

    #[test]
    fn every_sprite_path_uses_only_commands_the_flattener_knows() {
        // `flatten` stops at a command it does not know and silently returns what it had, which
        // would leave an icon half drawn. M L H V C A and Z, either case, are the supported set.
        let doc = roxmltree::Document::parse(SPRITE).unwrap();
        for n in doc.descendants().filter(|n| n.has_tag_name("path")) {
            let d = n.attribute("d").unwrap();
            for ch in d.chars().filter(|c| c.is_ascii_alphabetic()) {
                assert!("MmLlHhVvCcAaZz".contains(ch), "unsupported path command {ch:?} in {d}");
            }
        }
    }

    #[test]
    fn flattened_icons_stay_inside_the_grid() {
        for (id, passes) in sprite() {
            for pass in passes {
                for &(x, y) in pass.lines.iter().flatten() {
                    assert!((0.0..=24.0).contains(&x) && (0.0..=24.0).contains(&y), "{id}: point ({x}, {y}) is outside 24 x 24");
                }
            }
        }
    }

    #[test]
    fn a_hand_glyph_is_told_apart_from_a_font_glyph() {
        assert!(is_hand(HOME));
        assert!(!is_hand(egui_phosphor::regular::HOUSE));
    }
}
