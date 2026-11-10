//! The ray diagram's and the motion map's entries (their code is in `ray_diagram.rs` and `motion_map.rs`): the
//! reference physics site's two figures with a one-line setup.
use crate::figures::{FigureDef, ParamDef, Subject};

/// Adds the L/M/S label-size param (the reference toolbar toggle).
fn ls(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(crate::figures::svg::label_size_param());
    v
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "motion-map", subject: Subject::Physics, title: "Motion Map",
            blurb: "Dots along a number line, one per tick of the clock, each moving dot with its velocity arrow.",
            params: || ls(super::motion_map::params()), render: |p| crate::figures::svg::with_label_size(super::motion_map::render(p), p) },
        FigureDef { id: "ray-diagram", subject: Subject::Physics, title: "Ray Diagram",
            blurb: "Principal rays and the image for a lens or a mirror, with the focal points marked.",
            params: || ls(super::ray_diagram::params()), render: |p| crate::figures::svg::with_label_size(super::ray_diagram::render(p), p) },
    ]
}
