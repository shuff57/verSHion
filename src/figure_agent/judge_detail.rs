//! Pixel-preserving edge details for a judge's original images. No model evidence or
//! rendering is used here: every tile pixel is an integer repetition of decoded source RGBA.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Cursor;

const GUTTER: u32 = 12;
const MAX_PANEL_SIDE: u32 = 4096;
const MAX_PANEL_PIXELS: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImageDerivation {
    pub protocol: &'static str,
    /// Image positions in the judge input, starting at one.
    pub source_image: usize,
    pub output_image: usize,
    pub original_size: [u32; 2],
    pub panel_size: [u32; 2],
    pub png_sha256: String,
    pub tiles: Vec<TileDerivation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TileDerivation {
    pub region: &'static str,
    /// x, y, width, height, in the decoded original image.
    pub source_rect: [u32; 4],
    pub integer_scale: u32,
    /// x, y, width, height, in this panel.
    pub output_rect: [u32; 4],
}

#[derive(Debug)]
pub(super) struct DetailPanel {
    pub png: Vec<u8>,
    pub derivation: ImageDerivation,
}

/// Prefer a longest edge in 768..=1024, otherwise the closest integer enlargement
/// to 896. Ties use the smaller scale. Large sources are never downsampled.
fn scale(width: u32, height: u32) -> u32 {
    let long = u64::from(width.max(height));
    (1..=4)
        .min_by_key(|s| {
            let edge = long * u64::from(*s);
            (!(768..=1024).contains(&edge), edge.abs_diff(896), *s)
        })
        .expect("four scale choices")
}

fn tiles(width: u32, height: u32) -> Result<([u32; 2], Vec<TileDerivation>), String> {
    if width == 0 || height == 0 {
        return Err("Judge detail needs a nonempty source image.".into());
    }
    // Round up so fractional edge pixels, and one-pixel images, are never omitted.
    let left = (u64::from(width) * 3).div_ceil(10) as u32;
    let bottom = (u64::from(height) * 3).div_ceil(10) as u32;
    let first_scale = scale(left, height);
    let second_scale = scale(width, bottom);
    let dimensions = |w: u32, h: u32, s: u32| -> Result<(u32, u32), String> {
        w.checked_mul(s)
            .zip(h.checked_mul(s))
            .ok_or_else(|| "Judge detail dimensions overflowed.".into())
    };
    let (first_w, first_h) = dimensions(left, height, first_scale)?;
    let (second_w, second_h) = dimensions(width, bottom, second_scale)?;
    let second_y = first_h
        .checked_add(GUTTER)
        .ok_or("Judge detail dimensions overflowed.")?;
    let panel = [
        first_w.max(second_w),
        second_y
            .checked_add(second_h)
            .ok_or("Judge detail dimensions overflowed.")?,
    ];
    if panel[0] > MAX_PANEL_SIDE
        || panel[1] > MAX_PANEL_SIDE
        || u64::from(panel[0]) * u64::from(panel[1]) > MAX_PANEL_PIXELS
    {
        return Err("Source image exceeds the bounded, lossless judge detail panel; it was not downsampled or clipped.".into());
    }
    Ok((
        panel,
        vec![
            TileDerivation {
                region: "left_edge",
                source_rect: [0, 0, left, height],
                integer_scale: first_scale,
                output_rect: [0, 0, first_w, first_h],
            },
            TileDerivation {
                region: "bottom_edge",
                source_rect: [0, height - bottom, width, bottom],
                integer_scale: second_scale,
                output_rect: [0, second_y, second_w, second_h],
            },
        ],
    ))
}

pub(super) fn make(
    bytes: &[u8],
    source_image: usize,
    output_image: usize,
) -> Result<DetailPanel, String> {
    if source_image == 0 || output_image == 0 {
        return Err("Judge detail image positions must start at one.".into());
    }
    let reader = || {
        image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| "Judge detail source format could not be read.".to_string())
    };
    // Bound the allocation before decoding an unreasonably large input.
    let (width, height) = reader()?
        .into_dimensions()
        .map_err(|_| "Judge detail source image could not be decoded.".to_string())?;
    let (panel_size, tiles) = tiles(width, height)?;
    let decoded = reader()?
        .decode()
        .map_err(|_| "Judge detail source image could not be decoded.".to_string())?;
    if decoded.width() != width || decoded.height() != height {
        return Err("Judge detail source dimensions changed while decoding.".into());
    }
    // Higher-depth samples cannot be copied into this RGBA8 panel without losing data.
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
            "Judge detail requires 8-bit source pixels; higher-depth samples were not converted."
                .into(),
        );
    }
    let source = decoded.to_rgba8();
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
        .map_err(|_| "Judge detail panel could not be encoded.".to_string())?;
    let derivation = ImageDerivation {
        protocol: "judge_edge_detail_v1",
        source_image,
        output_image,
        original_size: [width, height],
        panel_size,
        png_sha256: format!("{:x}", Sha256::digest(&png)),
        tiles,
    };
    Ok(DetailPanel { png, derivation })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(image: &image::RgbaImage) -> Vec<u8> {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image.clone())
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    #[test]
    fn every_panel_pixel_is_a_source_pixel_or_white_gutter_with_exact_metadata() {
        for (width, height) in [(1, 1), (17, 11), (63, 31), (256, 256)] {
            let original = image::RgbaImage::from_fn(width, height, |x, y| {
                image::Rgba([x as u8, y as u8, (x * 7 + y * 3) as u8, (x + y * 5) as u8])
            });
            let bytes = encode(&original);
            let before = bytes.clone();
            let detail = make(&bytes, 2, 4).unwrap();
            let meta = &detail.derivation;
            assert_eq!(meta.protocol, "judge_edge_detail_v1");
            assert_eq!((meta.source_image, meta.output_image), (2, 4));
            assert_eq!(meta.original_size, [width, height]);
            assert_eq!(
                meta.png_sha256,
                format!("{:x}", Sha256::digest(&detail.png))
            );
            let panel = image::load_from_memory(&detail.png).unwrap().to_rgba8();
            assert_eq!([panel.width(), panel.height()], meta.panel_size);
            assert_eq!(meta.tiles.len(), 2);
            let left = &meta.tiles[0];
            let bottom = &meta.tiles[1];
            assert_eq!(left.region, "left_edge");
            assert_eq!(bottom.region, "bottom_edge");
            assert_eq!(left.source_rect, [0, 0, (width * 3).div_ceil(10), height]);
            assert_eq!(
                bottom.source_rect,
                [
                    0,
                    height - (height * 3).div_ceil(10),
                    width,
                    (height * 3).div_ceil(10)
                ]
            );
            assert_eq!(bottom.output_rect[1], left.output_rect[3] + GUTTER);
            for tile in &meta.tiles {
                assert!((1..=4).contains(&tile.integer_scale));
                assert_eq!(
                    tile.output_rect[2],
                    tile.source_rect[2] * tile.integer_scale
                );
                assert_eq!(
                    tile.output_rect[3],
                    tile.source_rect[3] * tile.integer_scale
                );
            }
            for y in 0..panel.height() {
                for x in 0..panel.width() {
                    if let Some(tile) = meta.tiles.iter().find(|t| {
                        let [ox, oy, w, h] = t.output_rect;
                        x >= ox && x < ox + w && y >= oy && y < oy + h
                    }) {
                        let sx =
                            tile.source_rect[0] + (x - tile.output_rect[0]) / tile.integer_scale;
                        let sy =
                            tile.source_rect[1] + (y - tile.output_rect[1]) / tile.integer_scale;
                        assert_eq!(panel.get_pixel(x, y), original.get_pixel(sx, sy));
                    } else {
                        assert_eq!(*panel.get_pixel(x, y), image::Rgba([255, 255, 255, 255]));
                    }
                }
            }
            assert_eq!(bytes, before, "Original encoded input is unchanged.");
            assert_eq!(
                image::load_from_memory(&bytes).unwrap().to_rgba8(),
                original
            );
        }
    }

    #[test]
    fn deterministic_integer_scales_never_shrink_large_sources() {
        assert_eq!(scale(300, 100), 3);
        assert_eq!(
            scale(256, 200),
            3,
            "Equal-distance targets prefer smaller scale."
        );
        assert_eq!(scale(20, 10), 4);
        assert_eq!(
            scale(1100, 80),
            1,
            "No downsampling to reach the preferred range."
        );
        let original = image::RgbaImage::from_pixel(1100, 900, image::Rgba([10, 90, 200, 255]));
        let bytes = encode(&original);
        let first = make(&bytes, 1, 3).unwrap();
        let second = make(&bytes, 1, 3).unwrap();
        assert_eq!(first.png, second.png);
        assert_eq!(first.derivation, second.derivation);
        assert!(first.derivation.tiles.iter().all(|t| t.integer_scale == 1));
        assert_eq!(first.derivation.tiles[0].source_rect, [0, 0, 330, 900]);
        assert_eq!(first.derivation.tiles[1].source_rect, [0, 630, 1100, 270]);
        assert_eq!(first.derivation.panel_size, [1100, 1182]);
    }

    #[test]
    fn invalid_or_unbounded_input_returns_an_error_instead_of_inventing_a_panel() {
        assert!(make(b"not an image", 1, 3).is_err());
        let png = encode(&image::RgbaImage::new(2, 2));
        assert!(make(&png, 0, 3).is_err());
        assert!(make(&png, 1, 0).is_err());
        let large = encode(&image::RgbaImage::new(5000, 1));
        assert!(
            make(&large, 1, 3)
                .unwrap_err()
                .contains("not downsampled or clipped")
        );
        let mut deep = Vec::new();
        image::DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            2,
            2,
            image::Rgba([1u16, 2, 3, 65535]),
        ))
        .write_to(&mut Cursor::new(&mut deep), image::ImageFormat::Png)
        .unwrap();
        assert!(make(&deep, 1, 3).unwrap_err().contains("higher-depth"));
    }
}
