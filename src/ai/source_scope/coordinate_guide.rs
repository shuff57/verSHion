//! A scope-only coordinate aid. The clean original remains a separate request image.
//! Grid lines and rulers are application overlays, never source facts or crop proposals.
use image::{GenericImageView, ImageFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;

const MAX_SOURCE_SIDE: u32 = 4096;
const MAX_SOURCE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_GUIDE_PIXELS: u64 = 10 * 1024 * 1024;
const MAX_PNG_BYTES: usize = 4 * 1024 * 1024;
const SIDE_MARGIN: u32 = 72;
const TOP_MARGIN: u32 = 120;

#[derive(Debug, Serialize)]
pub(super) struct GridPosition {
    pub normalized: u32,
    /// Edge coordinate in the unchanged decoded source page, not a pixel index.
    pub source_pixel_edge: u32,
    pub guide_pixel_edge: u32,
}

#[derive(Debug, Serialize)]
pub(super) struct Derivation {
    pub protocol: &'static str,
    pub source_image: usize,
    pub output_image: usize,
    pub source_byte_sha256: String,
    pub source_size: [u32; 2],
    pub guide_sha256: String,
    pub guide_size: [u32; 2],
    pub source_rect_xywh: [u32; 4],
    pub page_roi_xywh: [u32; 4],
    pub integer_scale: u32,
    pub x_grid_positions: Vec<GridPosition>,
    pub y_grid_positions: Vec<GridPosition>,
    pub overlay: &'static str,
    pub coordinate_mapping: &'static str,
}

pub(super) struct Guide {
    pub png: Vec<u8>,
    pub derivation: Derivation,
}

pub(super) struct Build {
    pub source_byte_sha256: String,
    pub source_size: Option<[u32; 2]>,
    pub guide: Option<Guide>,
    pub error: Option<String>,
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn dimensions(bytes: &[u8]) -> Result<[u32; 2], String> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Coordinate guide source format could not be read: {e}"))?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("Coordinate guide source dimensions could not be read: {e}"))?;
    Ok([w, h])
}

fn positions(length: u32, offset: u32) -> Vec<GridPosition> {
    (0..=10)
        .map(|step| {
            let normalized = step * 100;
            let source_pixel_edge = (u64::from(length) * u64::from(normalized) / 1000) as u32;
            GridPosition {
                normalized,
                source_pixel_edge,
                guide_pixel_edge: offset + source_pixel_edge,
            }
        })
        .collect()
}

fn render(
    bytes: &[u8],
    size: [u32; 2],
    source_byte_sha256: &str,
    output_image: usize,
) -> Result<Guide, String> {
    let [w, h] = size;
    if w < 160
        || h < 160
        || w > MAX_SOURCE_SIDE
        || h > MAX_SOURCE_SIDE
        || u64::from(w) * u64::from(h) > MAX_SOURCE_PIXELS
    {
        return Err("Coordinate guide source dimensions exceed the bounded readable range (160..4096 per side, at most 8 megapixels); clean source images retained.".into());
    }
    let guide_size = [(w + 2 * SIDE_MARGIN).max(560), h + TOP_MARGIN + SIDE_MARGIN];
    if u64::from(guide_size[0]) * u64::from(guide_size[1]) > MAX_GUIDE_PIXELS {
        return Err(
            "Coordinate guide canvas exceeds the 10 megapixel limit; clean source images retained."
                .into(),
        );
    }
    let source = image::load_from_memory(bytes)
        .map_err(|e| format!("Coordinate guide source could not be decoded: {e}"))?;
    if source.dimensions() != (w, h) {
        return Err("Coordinate guide source dimensions changed during decoding; clean source images retained.".into());
    }
    let left = (guide_size[0] - w) / 2;
    let top = TOP_MARGIN;
    let xs = positions(w, left);
    let ys = positions(h, top);
    let mut doc = crate::figures::svg::Doc::new(guide_size[0] as f64, guide_size[1] as f64);
    doc.rect(
        0.0,
        0.0,
        guide_size[0] as f64,
        guide_size[1] as f64,
        "#fff",
        "none",
        0.0,
    );
    doc.text(
        16.0,
        24.0,
        16.0,
        "start",
        "APPLICATION COORDINATE GUIDE - NOT SOURCE",
    );
    doc.text(
        16.0,
        48.0,
        12.0,
        "start",
        "X increases right; Y increases down; origin (0,0) is upper left.",
    );
    doc.text(
        16.0,
        68.0,
        12.0,
        "start",
        "0..1000 refers to the SOURCE PAGE FRAME inside the rulers.",
    );
    doc.text(
        16.0,
        86.0,
        12.0,
        "start",
        "Coloured lines are application guides. Read ink in the clean images.",
    );
    for p in &xs {
        let x = p.guide_pixel_edge as f64;
        doc.text(
            x,
            top as f64 - 16.0,
            11.0,
            "middle",
            &p.normalized.to_string(),
        );
    }
    for p in &ys {
        let y = p.guide_pixel_edge as f64;
        doc.text(
            left as f64 - 12.0,
            y + 4.0,
            11.0,
            "end",
            &p.normalized.to_string(),
        );
    }
    let captions = crate::figures::to_png(&doc.finish(), 1.0)
        .map_err(|e| format!("Coordinate guide rulers could not be rendered: {e}"))?;
    let mut canvas = image::load_from_memory(&captions)
        .map_err(|e| format!("Coordinate guide canvas could not be decoded: {e}"))?
        .to_rgba8();
    // Copy at 1:1 before drawing the conspicuously coloured, non-source overlay.
    image::imageops::replace(
        &mut canvas,
        &source.to_rgba8(),
        i64::from(left),
        i64::from(top),
    );
    let colour = image::Rgba([208, 68, 163, 255]);
    // Use integer page-edge coordinates; the 1000 frame edge is outside the last source pixel.
    for p in &xs {
        for y in top..=top + h {
            canvas.put_pixel(p.guide_pixel_edge, y, colour);
        }
    }
    for p in &ys {
        for x in left..=left + w {
            canvas.put_pixel(x, p.guide_pixel_edge, colour);
        }
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| format!("Coordinate guide could not be encoded: {e}"))?;
    if png.len() > MAX_PNG_BYTES {
        return Err(
            "Coordinate guide PNG exceeds the 4 MiB limit; clean source images retained.".into(),
        );
    }
    let derivation = Derivation {
        protocol: "scope_coordinate_guide_v1",
        source_image: 1,
        output_image,
        source_byte_sha256: source_byte_sha256.into(),
        source_size: size,
        guide_sha256: sha256(&png),
        guide_size,
        source_rect_xywh: [0, 0, w, h],
        page_roi_xywh: [left, top, w, h],
        integer_scale: 1,
        x_grid_positions: xs,
        y_grid_positions: ys,
        overlay: "Opaque magenta 1-pixel grid/frame every 100 normalized units; ruler numbers and legend are outside the source page ROI. Overlay is application data, not source ink. No owner, crop, suggested rectangle or interpretation is drawn.",
        coordinate_mapping: "Source page edge coordinates: x=1000*source_x/source_width, y=1000*source_y/source_height; grid pixel edges use floor(normalized*dimension/1000). Output bounds remain [ymin,xmin,ymax,xmax] relative to the source page ROI, excluding guide margins. Source aspect ratio and 1:1 scale are unchanged.",
    };
    Ok(Guide { png, derivation })
}

pub(super) fn build(bytes: &[u8], output_image: usize) -> Build {
    let source_byte_sha256 = sha256(bytes);
    let size = dimensions(bytes);
    let source_size = size.as_ref().ok().copied();
    let result = size.and_then(|size| render(bytes, size, &source_byte_sha256, output_image));
    let (guide, error) = match result {
        Ok(guide) => (Some(guide), None),
        Err(error) => (None, Some(error)),
    };
    Build {
        source_byte_sha256,
        source_size,
        guide,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asymmetric_page(w: u32, h: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([(x % 251) as u8, (y % 251) as u8, ((x + y) % 251) as u8, 255])
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn guide_preserves_aspect_orientation_hashes_and_unoverlaid_source_pixels() {
        let bytes = asymmetric_page(263, 417);
        let original_bytes = bytes.clone();
        let first = build(&bytes, 3);
        let guide = first.guide.unwrap();
        let again = build(&bytes, 3).guide.unwrap();
        assert_eq!(
            bytes, original_bytes,
            "original request bytes remain untouched"
        );
        assert_eq!(guide.png, again.png);
        assert_eq!(
            guide.png,
            build(&bytes, 2).guide.unwrap().png,
            "image index changes no page/grid pixels"
        );
        assert_eq!(
            serde_json::to_value(&guide.derivation).unwrap(),
            serde_json::to_value(&again.derivation).unwrap()
        );
        let d = &guide.derivation;
        assert_eq!(d.source_byte_sha256, sha256(&bytes));
        assert_eq!(d.guide_sha256, sha256(&guide.png));
        assert_eq!(d.source_image, 1);
        assert_eq!(d.output_image, 3);
        assert_eq!(d.source_size, [263, 417]);
        assert_eq!(d.source_rect_xywh, [0, 0, 263, 417]);
        assert_eq!(d.integer_scale, 1);
        let [left, top, w, h] = d.page_roi_xywh;
        assert_eq!([w, h], [263, 417]);
        assert_eq!(d.x_grid_positions[0].guide_pixel_edge, left);
        assert_eq!(d.x_grid_positions[10].guide_pixel_edge, left + w);
        assert_eq!(d.y_grid_positions[0].guide_pixel_edge, top);
        assert_eq!(d.y_grid_positions[10].guide_pixel_edge, top + h);
        assert_eq!(d.x_grid_positions[5].source_pixel_edge, 131);
        assert_eq!(d.y_grid_positions[5].source_pixel_edge, 208);
        let source = image::load_from_memory(&bytes).unwrap().to_rgba8();
        let output = image::load_from_memory(&guide.png).unwrap().to_rgba8();
        assert_eq!(output.dimensions(), (d.guide_size[0], d.guide_size[1]));
        // These four inset corner pixels avoid overlay lines and have distinct source colours.
        for (x, y) in [(1, 1), (w - 2, 1), (1, h - 2), (w - 2, h - 2)] {
            assert_eq!(output.get_pixel(left + x, top + y), source.get_pixel(x, y));
        }
        for p in &d.x_grid_positions {
            assert_eq!(
                output.get_pixel(p.guide_pixel_edge, top + 1),
                &image::Rgba([208, 68, 163, 255])
            );
        }
        for p in &d.y_grid_positions {
            assert_eq!(
                output.get_pixel(left + 1, p.guide_pixel_edge),
                &image::Rgba([208, 68, 163, 255])
            );
        }
    }

    #[test]
    fn guide_failure_is_bounded_and_preserves_source_identity() {
        let bytes = asymmetric_page(MAX_SOURCE_SIDE + 1, 160);
        let result = build(&bytes, 2);
        assert!(result.guide.is_none());
        assert_eq!(result.source_size, Some([MAX_SOURCE_SIDE + 1, 160]));
        assert_eq!(result.source_byte_sha256, sha256(&bytes));
        assert!(result.error.unwrap().contains("bounded readable range"));
        let invalid = build(b"not an image", 2);
        assert!(invalid.guide.is_none() && invalid.error.is_some());
        assert_eq!(invalid.source_size, None);
    }
}
