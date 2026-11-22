//! Optional, source-only second-reading views. Every tile pixel is an integer
//! repetition of decoded vision RGBA; layout and provenance are application data.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;

const GUTTER: u32 = 12;
const MAX_SOURCE_SIDE: u32 = 4096;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_PANEL_SIDE: u32 = 3072;
const MAX_PANEL_PIXELS: u64 = 4 * 1024 * 1024;
const PREFERRED_PANEL_SIDE: u32 = 2048;
const MAX_PNG_BYTES: usize = 4 * 1024 * 1024;
const MAX_REQUEST_IMAGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ImageDerivation {
    pub protocol: &'static str,
    pub source_image: usize,
    pub output_image: usize,
    pub source_sha256: String,
    pub source_size: [u32; 2],
    pub panel_size: [u32; 2],
    pub png_sha256: String,
    pub tiles: Vec<TileDerivation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct TileDerivation {
    pub region: &'static str,
    /// x, y, width, height in the unchanged decoded full vision image.
    pub source_rect: [u32; 4],
    pub integer_scale: u32,
    pub output_rect: [u32; 4],
}
#[derive(Debug)]
pub(super) struct DetailPanel {
    pub png: Vec<u8>,
    pub derivation: ImageDerivation,
}

fn dimensions(width: u32, height: u32, scale: u32) -> Option<[u32; 2]> {
    Some([
        width
            .checked_mul(scale)?
            .checked_mul(2)?
            .checked_add(GUTTER)?,
        height.checked_mul(scale)?,
    ])
}
fn bounded_panel(size: [u32; 2]) -> bool {
    size.iter().all(|v| *v > 0 && *v <= MAX_PANEL_SIDE)
        && u64::from(size[0]) * u64::from(size[1]) <= MAX_PANEL_PIXELS
}
fn layout(width: u32, height: u32) -> Result<([u32; 2], [Vec<TileDerivation>; 2]), String> {
    if width == 0
        || height == 0
        || width > MAX_SOURCE_SIDE
        || height > MAX_SOURCE_SIDE
        || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS
    {
        return Err(
            "Source reinspection details exceed bounded source dimensions; full source retained."
                .into(),
        );
    }
    // ceil(.6 * dimension) == dimension - floor(.4 * dimension), including tiny images.
    let tile_w = (u64::from(width) * 3).div_ceil(5) as u32;
    let tile_h = (u64::from(height) * 3).div_ceil(5) as u32;
    let twice = dimensions(tile_w, tile_h, 2);
    let scale = if twice
        .is_some_and(|s| bounded_panel(s) && s.iter().all(|v| *v <= PREFERRED_PANEL_SIDE))
    {
        2
    } else {
        1
    };
    let panel_size = dimensions(tile_w, tile_h, scale).filter(|s| bounded_panel(*s))
        .ok_or("Source reinspection details exceed bounded panel dimensions; no source pixels downsampled or clipped.".to_string())?;
    let row = |bottom: bool| {
        [false, true]
            .into_iter()
            .map(|right| TileDerivation {
                region: match (bottom, right) {
                    (false, false) => "upper_left",
                    (false, true) => "upper_right",
                    (true, false) => "lower_left",
                    (true, true) => "lower_right",
                },
                source_rect: [
                    if right { width - tile_w } else { 0 },
                    if bottom { height - tile_h } else { 0 },
                    tile_w,
                    tile_h,
                ],
                integer_scale: scale,
                output_rect: [
                    if right { tile_w * scale + GUTTER } else { 0 },
                    0,
                    tile_w * scale,
                    tile_h * scale,
                ],
            })
            .collect()
    };
    Ok((panel_size, [row(false), row(true)]))
}
fn check_payload_bytes(full: usize, panels: [usize; 2]) -> Result<(), String> {
    if panels.iter().any(|n| *n > MAX_PNG_BYTES) {
        return Err("Optional source detail PNG exceeds 4 MiB; optional details omitted.".into());
    }
    let total = full
        .checked_add(panels[0])
        .and_then(|n| n.checked_add(panels[1]));
    if total.is_none_or(|n| n > MAX_REQUEST_IMAGE_BYTES) {
        return Err("Full source plus optional details exceed 8 MiB raw image budget; optional details omitted.".into());
    }
    Ok(())
}

/// Returns both ordered panels or an error, never one partial detail. The caller
/// keeps the unchanged full vision image on any optional-detail failure.
pub(super) fn make(bytes: &[u8]) -> Result<[DetailPanel; 2], String> {
    if bytes.len() > MAX_REQUEST_IMAGE_BYTES {
        return Err(
            "Full source already exceeds optional-detail raw image budget; optional details omitted."
                .into(),
        );
    }
    let reader = || {
        image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| "Source detail image format could not be read.".to_string())
    };
    // Reject excessive dimensions before allocating the decoded raster.
    let (width, height) = reader()?
        .into_dimensions()
        .map_err(|_| "Source detail image dimensions could not be decoded.".to_string())?;
    let (panel_size, rows) = layout(width, height)?;
    let mut bounded_reader = reader()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    bounded_reader.limits(limits);
    let decoded = bounded_reader.decode().map_err(|_| {
        "Source detail image could not be decoded within allocation limits.".to_string()
    })?;
    if decoded.width() != width || decoded.height() != height {
        return Err("Source detail image dimensions changed during decoding.".into());
    }
    if matches!(
        decoded.color(),
        image::ColorType::L16
            | image::ColorType::La16
            | image::ColorType::Rgb16
            | image::ColorType::Rgba16
            | image::ColorType::Rgb32F
            | image::ColorType::Rgba32F
    ) {
        return Err(
            "Optional source details require 8-bit pixels; higher-depth source was not converted."
                .into(),
        );
    }
    let source = decoded.to_rgba8();
    let source_hash = format!("{:x}", Sha256::digest(bytes));
    let build = |tiles: Vec<TileDerivation>, output_image| -> Result<DetailPanel, String> {
        let mut panel = image::RgbaImage::from_pixel(
            panel_size[0],
            panel_size[1],
            image::Rgba([255, 255, 255, 255]),
        );
        for tile in &tiles {
            let [sx, sy, _, _] = tile.source_rect;
            let [ox, oy, ow, oh] = tile.output_rect;
            for y in 0..oh {
                for x in 0..ow {
                    panel.put_pixel(
                        ox + x,
                        oy + y,
                        *source.get_pixel(sx + x / tile.integer_scale, sy + y / tile.integer_scale),
                    );
                }
            }
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(panel)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|_| "Optional source detail PNG could not be encoded.".to_string())?;
        if png.len() > MAX_PNG_BYTES {
            return Err(
                "Optional source detail PNG exceeds 4 MiB; optional details omitted.".into(),
            );
        }
        let derivation = ImageDerivation {
            protocol: "source_reinspection_detail_v1",
            source_image: 1,
            output_image,
            source_sha256: source_hash.clone(),
            source_size: [width, height],
            panel_size,
            png_sha256: format!("{:x}", Sha256::digest(&png)),
            tiles,
        };
        Ok(DetailPanel { png, derivation })
    };
    let [upper, lower] = rows;
    let panels = [build(upper, 2)?, build(lower, 3)?];
    check_payload_bytes(bytes.len(), [panels[0].png.len(), panels[1].png.len()])?;
    Ok(panels)
}

/// The model's box is only an attention request. Invalid requests never become
/// source facts; the caller retains the existing quadrant/full-only fallback.
pub(super) fn make_focus(bytes: &[u8], focus: &serde_json::Value) -> Result<DetailPanel, String> {
    let values = focus
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or("Focus attention request needs four coordinates.".to_string())?;
    let mut b = [0.0; 4];
    for (i, v) in values.iter().enumerate() {
        b[i] = v
            .as_f64()
            .filter(|v| v.is_finite() && (0.0..=1000.0).contains(v))
            .ok_or("Focus attention coordinates must be finite and within 0..1000.".to_string())?;
    }
    if b[0] >= b[2] || b[1] >= b[3] {
        return Err("Focus attention rectangle must have positive width and height; no endpoints reordered.".into());
    }
    if bytes.len() > MAX_REQUEST_IMAGE_BYTES {
        return Err("Full source exceeds optional focus raw image budget.".into());
    }
    let reader = || {
        image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| "Focus source format could not be read.".to_string())
    };
    let (w, h) = reader()?
        .into_dimensions()
        .map_err(|_| "Focus source dimensions could not be decoded.".to_string())?;
    if w == 0
        || h == 0
        || w > MAX_SOURCE_SIDE
        || h > MAX_SOURCE_SIDE
        || u64::from(w) * u64::from(h) > MAX_SOURCE_PIXELS
    {
        return Err("Focus source exceeds bounded source dimensions.".into());
    }
    let x0 = (b[0] * f64::from(w) / 1000.0).floor() as u32;
    let y0 = (b[1] * f64::from(h) / 1000.0).floor() as u32;
    let x1 = ((b[2] * f64::from(w) / 1000.0).ceil() as u32).min(w);
    let y1 = ((b[3] * f64::from(h) / 1000.0).ceil() as u32).min(h);
    let (cw, ch) = (x1 - x0, y1 - y0);
    let scale = (1..=8u32)
        .filter(|s| bounded_panel([cw * s, ch * s]))
        .min_by_key(|s| {
            let long = u64::from(cw.max(ch)) * u64::from(*s);
            (!(768..=1024).contains(&long), long.abs_diff(896), *s)
        })
        .ok_or(
            "Focus detail exceeds bounded dimensions; no pixels downsampled or clipped."
                .to_string(),
        )?;
    let mut bounded_reader = reader()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    bounded_reader.limits(limits);
    let decoded = bounded_reader
        .decode()
        .map_err(|_| "Focus source could not be decoded within allocation limits.".to_string())?;
    if decoded.width() != w || decoded.height() != h {
        return Err("Focus source dimensions changed during decoding.".into());
    }
    if matches!(
        decoded.color(),
        image::ColorType::L16
            | image::ColorType::La16
            | image::ColorType::Rgb16
            | image::ColorType::Rgba16
            | image::ColorType::Rgb32F
            | image::ColorType::Rgba32F
    ) {
        return Err(
            "Optional focus requires 8-bit pixels; higher-depth source was not converted.".into(),
        );
    }
    let source = decoded.into_rgba8();
    let size = [cw * scale, ch * scale];
    let panel = image::RgbaImage::from_fn(size[0], size[1], |x, y| {
        *source.get_pixel(x0 + x / scale, y0 + y / scale)
    });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(panel)
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|_| "Optional focus PNG could not be encoded.".to_string())?;
    check_payload_bytes(bytes.len(), [png.len(), 0])?;
    let derivation = ImageDerivation {
        protocol: "source_reinspection_focus_v1",
        source_image: 1,
        output_image: 2,
        source_sha256: format!("{:x}", Sha256::digest(bytes)),
        source_size: [w, h],
        panel_size: size,
        png_sha256: format!("{:x}", Sha256::digest(&png)),
        tiles: vec![TileDerivation {
            region: "requested_focus",
            source_rect: [x0, y0, cw, ch],
            integer_scale: scale,
            output_rect: [0, 0, size[0], size[1]],
        }],
    };
    Ok(DetailPanel { png, derivation })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encoded(source: &image::RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(source.clone())
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }
    #[test]
    fn focused_detail_preserves_exact_outward_rect_pixels_alpha_hash_and_integer_scale() {
        let source = image::RgbaImage::from_fn(101, 79, |x, y| {
            image::Rgba([x as u8, y as u8, (x + y) as u8, (3 * x + 5 * y) as u8])
        });
        let bytes = encoded(&source);
        let unchanged = bytes.clone();
        let request = serde_json::json!([200, 250, 600, 750]);
        let p = make_focus(&bytes, &request).unwrap();
        let again = make_focus(&bytes, &request).unwrap();
        assert_eq!(p.png, again.png);
        assert_eq!(p.derivation, again.derivation);
        let d = &p.derivation;
        assert_eq!(d.protocol, "source_reinspection_focus_v1");
        assert_eq!((d.source_image, d.output_image), (1, 2));
        assert_eq!(d.source_size, [101, 79]);
        assert_eq!(d.source_sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(d.png_sha256, format!("{:x}", Sha256::digest(&p.png)));
        assert_eq!(d.tiles.len(), 1);
        let tile = &d.tiles[0];
        assert_eq!(tile.region, "requested_focus");
        assert_eq!(tile.source_rect, [20, 19, 41, 41]);
        assert_eq!(tile.integer_scale, 8);
        assert_eq!(tile.output_rect, [0, 0, 328, 328]);
        assert_eq!(d.panel_size, [328, 328]);
        let got = image::load_from_memory(&p.png).unwrap().to_rgba8();
        for y in 0..328 {
            for x in 0..328 {
                assert_eq!(
                    *got.get_pixel(x, y),
                    *source.get_pixel(20 + x / 8, 19 + y / 8)
                );
            }
        }
        assert_eq!(bytes, unchanged);
        check_payload_bytes(bytes.len(), [p.png.len(), 0]).unwrap();
        let big = encoded(&image::RgbaImage::new(1200, 200));
        let p = make_focus(&big, &serde_json::json!([0, 0, 1000, 1000])).unwrap();
        assert_eq!(
            p.derivation.tiles[0].integer_scale, 1,
            "Large details never downsample or needlessly repeat pixels."
        );
    }
    #[test]
    fn focus_attention_rejects_invalid_rectangles_or_excess_without_coercion() {
        let bytes = encoded(&image::RgbaImage::new(20, 16));
        for request in [
            serde_json::Value::Null,
            serde_json::json!([0, 0, 0, 1]),
            serde_json::json!([500, 0, 400, 100]),
            serde_json::json!([-1, 0, 400, 100]),
            serde_json::json!([0, 0, 1001, 1000]),
            serde_json::json!([0, 0, 1]),
            serde_json::json!("not a box"),
        ] {
            assert!(make_focus(&bytes, &request).is_err());
        }
        assert!(make_focus(b"bad image", &serde_json::json!([0, 0, 100, 100])).is_err());
        let big = encoded(&image::RgbaImage::new(4096, 1));
        assert!(
            make_focus(&big, &serde_json::json!([0, 0, 1000, 1000]))
                .unwrap_err()
                .contains("bounded dimensions")
        );
    }
    #[test]
    fn panels_exactly_repeat_original_rgba_with_complete_overlapping_coverage_and_provenance() {
        for (w, h) in [(1, 1), (17, 11), (64, 80), (903, 679)] {
            let source = image::RgbaImage::from_fn(w, h, |x, y| {
                image::Rgba([x as u8, y as u8, (7 * x + 3 * y) as u8, (x + 5 * y) as u8])
            });
            let bytes = encoded(&source);
            let unchanged = bytes.clone();
            let panels = make(&bytes).unwrap();
            let again = make(&bytes).unwrap();
            let mut covered = vec![false; (w * h) as usize];
            for (index, p) in panels.iter().enumerate() {
                assert_eq!(p.png, again[index].png);
                assert_eq!(p.derivation, again[index].derivation);
                let meta = &p.derivation;
                assert_eq!(meta.source_image, 1);
                assert_eq!(meta.output_image, index + 2);
                assert_eq!(meta.source_size, [w, h]);
                assert_eq!(meta.source_sha256, format!("{:x}", Sha256::digest(&bytes)));
                assert_eq!(meta.png_sha256, format!("{:x}", Sha256::digest(&p.png)));
                assert_eq!(meta.protocol, "source_reinspection_detail_v1");
                assert!(p.png.len() <= MAX_PNG_BYTES);
                let got = image::load_from_memory(&p.png).unwrap().to_rgba8();
                assert_eq!([got.width(), got.height()], meta.panel_size);
                assert_eq!(meta.tiles.len(), 2);
                for (col, tile) in meta.tiles.iter().enumerate() {
                    assert!((1..=2).contains(&tile.integer_scale));
                    assert_eq!(
                        tile.source_rect,
                        [
                            if col == 0 { 0 } else { w - (w * 3).div_ceil(5) },
                            if index == 0 {
                                0
                            } else {
                                h - (h * 3).div_ceil(5)
                            },
                            (w * 3).div_ceil(5),
                            (h * 3).div_ceil(5)
                        ]
                    );
                    assert_eq!(
                        tile.output_rect[2],
                        tile.source_rect[2] * tile.integer_scale
                    );
                    assert_eq!(
                        tile.output_rect[3],
                        tile.source_rect[3] * tile.integer_scale
                    );
                    for y in tile.source_rect[1]..tile.source_rect[1] + tile.source_rect[3] {
                        for x in tile.source_rect[0]..tile.source_rect[0] + tile.source_rect[2] {
                            covered[(y * w + x) as usize] = true;
                        }
                    }
                }
                assert!(
                    meta.tiles[0].source_rect[0] + meta.tiles[0].source_rect[2]
                        > meta.tiles[1].source_rect[0]
                );
                assert_eq!(
                    meta.tiles[1].output_rect[0],
                    meta.tiles[0].output_rect[2] + GUTTER
                );
                for y in 0..got.height() {
                    for x in 0..got.width() {
                        let tile = meta.tiles.iter().find(|t| {
                            let [ox, oy, tw, th] = t.output_rect;
                            x >= ox && x < ox + tw && y >= oy && y < oy + th
                        });
                        let expected = if let Some(t) = tile {
                            let [sx, sy, _, _] = t.source_rect;
                            *source.get_pixel(
                                sx + (x - t.output_rect[0]) / t.integer_scale,
                                sy + (y - t.output_rect[1]) / t.integer_scale,
                            )
                        } else {
                            image::Rgba([255, 255, 255, 255])
                        };
                        assert_eq!(*got.get_pixel(x, y), expected);
                    }
                }
            }
            assert!(covered.into_iter().all(|v| v));
            assert_eq!(bytes, unchanged);
            check_payload_bytes(bytes.len(), [panels[0].png.len(), panels[1].png.len()]).unwrap();
        }
    }
    #[test]
    fn invalid_oversized_or_lossy_optional_details_fail_without_partial_panels() {
        assert!(make(b"bad image").is_err());
        assert!(
            make(&encoded(&image::RgbaImage::new(MAX_SOURCE_SIDE + 1, 1)))
                .unwrap_err()
                .contains("source dimensions")
        );
        assert!(layout(0, 10).is_err());
        assert!(layout(4096, 4096).is_err());
        assert!(layout(3000, 1000).unwrap_err().contains("panel dimensions"));
        let mut high = Vec::new();
        image::DynamicImage::ImageLuma16(image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::new(
            10, 10,
        ))
        .write_to(&mut Cursor::new(&mut high), image::ImageFormat::Png)
        .unwrap();
        assert!(make(&high).unwrap_err().contains("higher-depth"));
        assert!(check_payload_bytes(0, [MAX_PNG_BYTES, MAX_PNG_BYTES]).is_ok());
        assert!(
            check_payload_bytes(0, [MAX_PNG_BYTES + 1, 0])
                .unwrap_err()
                .contains("4 MiB")
        );
        assert!(
            check_payload_bytes(1, [MAX_PNG_BYTES, MAX_PNG_BYTES])
                .unwrap_err()
                .contains("8 MiB")
        );
        assert!(check_payload_bytes(usize::MAX, [1, 1]).is_err());
        assert!(
            make(&vec![0; MAX_REQUEST_IMAGE_BYTES + 1])
                .unwrap_err()
                .contains("raw image budget")
        );
    }
}
