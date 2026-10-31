//! Tests for `image.rs`, replaying worker/test/pool/image.test.ts (one valid and one invalid file per rule, the real
//! Pillow / ImageMagick fixtures) and round17's pixel-hash case.
use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

const OK_PNG: &str = "image/png";

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}
fn deflate(raw: &[u8], level: u32) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::new(level));
    e.write_all(raw).unwrap();
    e.finish().unwrap()
}
fn chunk(ty: &str, data: &[u8]) -> Vec<u8> {
    let mut out = (data.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(ty.as_bytes());
    out.extend_from_slice(data);
    let crc = crc32fast::hash(&out[4..]);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}
/// A real w x h greyscale PNG whose pixels depend on `seed`.
fn png_at(w: usize, h: usize, seed: usize, extra: &[Vec<u8>], level: u32) -> Vec<u8> {
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    let mut raw = vec![0u8; (w + 1) * h];
    for y in 0..h {
        for x in 0..w {
            raw[y * (w + 1) + 1 + x] = ((x * 7 + y * 13 + seed) & 0xff) as u8;
        }
    }
    let mut out = PNG_SIG.to_vec();
    out.extend(chunk("IHDR", &ihdr));
    for e in extra {
        out.extend_from_slice(e);
    }
    out.extend(chunk("IDAT", &deflate(&raw, level)));
    out.extend(chunk("IEND", &[]));
    out
}
fn png_(w: usize, h: usize) -> Vec<u8> {
    png_at(w, h, 0, &[], 6)
}
fn code(b: &[u8]) -> Option<&'static str> {
    check_image(b).err()
}

#[test]
fn png_chunk_allow_list_crcs_order_and_sides() {
    assert_eq!(check_image(&png_(10, 20)), Ok(ImageInfo { content_type: OK_PNG, width: 10, height: 20 }));
    for t in ["tEXt", "zTXt", "iTXt", "eXIf", "iCCP", "tIME", "acTL", "fcTL", "abcd"] {
        assert_eq!(code(&png_at(4, 4, 0, &[chunk(t, b"x")], 6)), Some("png_chunk"), "{t}");
    }
    assert_eq!(code(&png_at(4, 4, 0, &[chunk("gAMA", &[0; 4]), chunk("pHYs", &[0; 9])], 6)), None, "allowed ancillaries");
    assert_eq!(code(&cat(&[&png_(4, 4), &[0]])), Some("trailing_bytes"));
    let mut bad = png_(4, 4);
    let n = bad.len();
    bad[n - 13] ^= 1; // a byte inside the IDAT data: its CRC no longer matches
    assert_eq!(code(&bad), Some("png_crc"));
    assert_eq!(code(&png_(4097, 1)), Some("dimensions"));
    assert_eq!(code(&png_(1, 0)), Some("dimensions"));
    assert_eq!(code(&png_(4, 4)[..40]), Some("truncated"));
    let full = png_(4, 4);
    assert_eq!(code(&full[..full.len() - 12]), Some("truncated"));
}

#[test]
fn png_idat_must_inflate_to_exactly_the_image_size() {
    // Pixel data for a 4x4 image, but the IDAT holds one row too few: the zlib stream is valid, the size is wrong.
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&4u32.to_be_bytes());
    ihdr.extend_from_slice(&4u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    let short = cat(&[&PNG_SIG, &chunk("IHDR", &ihdr), &chunk("IDAT", &deflate(&[0u8; 15], 6)), &chunk("IEND", &[])]);
    assert_eq!(code(&short), Some("png_idat_data"));
    let long = cat(&[&PNG_SIG, &chunk("IHDR", &ihdr), &chunk("IDAT", &deflate(&[0u8; 25], 6)), &chunk("IEND", &[])]);
    assert_eq!(code(&long), Some("png_idat_data"));
    // Bytes after the zlib stream, inside the IDAT chunk.
    let mut z = deflate(&[0u8; 20], 6);
    z.push(0);
    let trailing = cat(&[&PNG_SIG, &chunk("IHDR", &ihdr), &chunk("IDAT", &z), &chunk("IEND", &[])]);
    assert_eq!(code(&trailing), Some("png_idat_data"));
    let exact = cat(&[&PNG_SIG, &chunk("IHDR", &ihdr), &chunk("IDAT", &deflate(&[0u8; 20], 6)), &chunk("IEND", &[])]);
    assert_eq!(code(&exact), None);
}

fn seg(m: u8, data: &[u8]) -> Vec<u8> {
    let mut v = vec![0xff, m, ((data.len() + 2) >> 8) as u8, ((data.len() + 2) & 255) as u8];
    v.extend_from_slice(data);
    v
}
fn jpeg_of(w: usize, h: usize, extra: &[Vec<u8>], tail: &[u8]) -> Vec<u8> {
    let sof = [8, (h >> 8) as u8, (h & 255) as u8, (w >> 8) as u8, (w & 255) as u8, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1];
    let mut jfif = b"JFIF\0".to_vec();
    jfif.extend_from_slice(&[1, 1, 0, 0, 1, 0, 1, 0, 0]);
    let mut out = vec![0xff, 0xd8];
    out.extend(seg(0xe0, &jfif));
    for e in extra {
        out.extend_from_slice(e);
    }
    out.extend(seg(0xdb, &[0; 65]));
    out.extend(seg(0xc0, &sof));
    out.extend(seg(0xc4, &[0; 17])); // one Huffman table with no codes
    out.extend(seg(0xda, &[3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]));
    out.extend_from_slice(&[0x12, 0xff, 0x00, 0x34, 0xff, 0xd0, 0x56]); // entropy data with a stuffed byte and a restart marker
    out.extend_from_slice(&[0xff, 0xd9]);
    out.extend_from_slice(tail);
    out
}

#[test]
fn jpeg_segments_allow_list() {
    assert_eq!(check_image(&jpeg_of(640, 480, &[], &[])), Ok(ImageInfo { content_type: "image/jpeg", width: 640, height: 480 }));
    let mut adobe = b"Adobe".to_vec();
    adobe.extend_from_slice(&[0; 7]);
    assert_eq!(code(&jpeg_of(8, 8, &[seg(0xee, &adobe)], &[])), None);
    let mut exif = b"Exif\0\0".to_vec();
    exif.extend_from_slice(&[0; 10]);
    for (m, d, why) in [(0xe1, &exif[..], "EXIF"), (0xe2, b"ICC_PROFILE\0", "ICC"), (0xed, b"Photoshop 3.0\0", "IPTC"), (0xfe, b"a comment", "COM"), (0xe5, b"x", "APP5")] {
        assert_eq!(code(&jpeg_of(8, 8, &[seg(m, d)], &[])), Some("jpeg_segment"), "{why}");
    }
    assert_eq!(code(&jpeg_of(8, 8, &[], b"PK")), Some("trailing_bytes"));
    assert_eq!(code(&jpeg_of(5000, 8, &[], &[])), Some("dimensions"));
    assert_eq!(code(&jpeg_of(8, 0, &[], &[])), Some("dimensions"), "DNL-deferred height");
    let two_sof = jpeg_of(8, 8, &[seg(0xc0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0])], &[]);
    assert_eq!(code(&two_sof), Some("jpeg_sof"));
}

/// A valid GIF LZW stream of `pixels` zero pixels (literal codes only), in sub-blocks.
fn lzw_encode(min: usize, pixels: usize, extra: &[usize]) -> Vec<u8> {
    let clear = 1usize << min;
    let eoi = clear + 1;
    let mut codes: Vec<(usize, usize)> = Vec::new();
    let (mut size, mut next, mut first) = (min + 1, eoi + 1, true);
    codes.push((clear, size));
    for _ in 0..pixels {
        codes.push((0, size));
        if !first {
            next += 1;
            if next == 1 << size && size < 12 {
                size += 1;
            }
            if next >= 4095 {
                codes.push((clear, size));
                size = min + 1;
                next = eoi + 1;
                first = true;
                continue;
            }
        }
        first = false;
    }
    codes.push((eoi, size));
    for &x in extra {
        codes.push((x, size));
    }
    let (mut bytes, mut acc, mut nb) = (Vec::new(), 0u64, 0usize);
    for (c, n) in codes {
        acc |= (c as u64) << nb;
        nb += n;
        while nb >= 8 {
            bytes.push((acc & 255) as u8);
            acc >>= 8;
            nb -= 8;
        }
    }
    if nb > 0 {
        bytes.push((acc & 255) as u8);
    }
    let mut out = Vec::new();
    for part in bytes.chunks(255) {
        out.push(part.len() as u8);
        out.extend_from_slice(part);
    }
    out.push(0);
    out
}
struct G {
    w: usize,
    h: usize,
    frames: usize,
    ext: Vec<Vec<u8>>,
    frame_w: Option<usize>,
    tail: Vec<u8>,
}
fn g() -> G {
    G { w: 10, h: 10, frames: 1, ext: vec![], frame_w: None, tail: vec![] }
}
fn gif_of(o: G) -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend_from_slice(&[(o.w & 255) as u8, (o.w >> 8) as u8, (o.h & 255) as u8, (o.h >> 8) as u8, 0x80, 0, 0]);
    out.extend_from_slice(&[0; 6]);
    for e in &o.ext {
        out.extend_from_slice(e);
    }
    let fw = o.frame_w.unwrap_or(o.w);
    for _ in 0..o.frames {
        out.extend_from_slice(&[0x2c, 0, 0, 0, 0, (fw & 255) as u8, (fw >> 8) as u8, (o.h & 255) as u8, (o.h >> 8) as u8, 0, 2]);
        out.extend(lzw_encode(2, fw * o.h, &[]));
    }
    out.push(0x3b);
    out.extend_from_slice(&o.tail);
    out
}
const GCE: [u8; 8] = [0x21, 0xf9, 4, 1, 0, 0, 0, 0];

#[test]
fn gif_one_image_one_gce_nothing_else() {
    assert_eq!(check_image(&gif_of(g())), Ok(ImageInfo { content_type: "image/gif", width: 10, height: 10 }));
    assert_eq!(code(&gif_of(G { ext: vec![GCE.to_vec()], ..g() })), None);
    assert_eq!(code(&gif_of(G { ext: vec![GCE.to_vec(), GCE.to_vec()], ..g() })), Some("gif_extension"));
    assert_eq!(code(&gif_of(G { frames: 2, ..g() })), Some("gif_animated"));
    let mut looping = vec![0x21, 0xff, 11];
    looping.extend_from_slice(b"NETSCAPE2.0");
    looping.extend_from_slice(&[3, 1, 0, 0, 0]);
    assert_eq!(code(&gif_of(G { ext: vec![looping], ..g() })), Some("gif_extension"), "looping");
    let mut comment = vec![0x21, 0xfe, 3];
    comment.extend_from_slice(b"hey");
    comment.push(0);
    assert_eq!(code(&gif_of(G { ext: vec![comment], ..g() })), Some("gif_extension"), "comment");
    assert_eq!(code(&gif_of(G { ext: vec![vec![0x21, 0x01, 1, 0, 0]], ..g() })), Some("gif_extension"), "plain text");
    assert_eq!(code(&gif_of(G { frame_w: Some(11), ..g() })), Some("gif_frame"), "frame outside the logical screen");
    assert_eq!(code(&gif_of(G { tail: vec![0], ..g() })), Some("trailing_bytes"));
    assert_eq!(code(&gif_of(G { w: 4097, ..g() })), Some("dimensions"));
    // Data hidden after the end-of-information code, or too few pixels, is refused (LZW decoded).
    let mut head = b"GIF89a".to_vec();
    head.extend_from_slice(&[10, 0, 10, 0, 0x80, 0, 0]);
    head.extend_from_slice(&[0; 6]);
    head.extend_from_slice(&[0x2c, 0, 0, 0, 0, 10, 0, 10, 0, 0, 2]);
    assert_eq!(code(&cat(&[&head, &lzw_encode(2, 100, &[]), &[0x3b]])), None);
    assert_eq!(code(&cat(&[&head, &lzw_encode(2, 99, &[]), &[0x3b]])), Some("gif_lzw_data"));
    assert_eq!(code(&cat(&[&head, &lzw_encode(2, 100, &[0; 8]), &[0x3b]])), Some("gif_lzw_data"));
}

fn riff(chunks: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (t, d) in chunks {
        body.extend_from_slice(t.as_bytes());
        body.extend_from_slice(&(d.len() as u32).to_le_bytes());
        body.extend_from_slice(d);
        if d.len() & 1 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend(body);
    out
}
fn vp8l(w: usize, h: usize) -> Vec<u8> {
    let mut d = vec![0u8; 10];
    d[0] = 0x2f;
    d[1..5].copy_from_slice(&((((w - 1) & 0x3fff) | (((h - 1) & 0x3fff) << 14)) as u32).to_le_bytes());
    d
}
fn vp8(w: usize, h: usize) -> Vec<u8> {
    let mut d = vec![0u8; 20];
    d[..6].copy_from_slice(&[0x10, 0, 0, 0x9d, 0x01, 0x2a]);
    d[6..8].copy_from_slice(&(w as u16).to_le_bytes());
    d[8..10].copy_from_slice(&(h as u16).to_le_bytes());
    d
}
fn vp8x(flags: u8, w: usize, h: usize) -> Vec<u8> {
    let mut d = vec![0u8; 10];
    d[0] = flags;
    d[4..6].copy_from_slice(&(((w - 1) & 0xffff) as u16).to_le_bytes());
    d[6] = (((w - 1) >> 16) & 255) as u8;
    d[7..9].copy_from_slice(&(((h - 1) & 0xffff) as u16).to_le_bytes());
    d[9] = (((h - 1) >> 16) & 255) as u8;
    d
}

#[test]
fn webp_layouts_flags_canvas_and_riff_size() {
    assert_eq!(check_image(&riff(&[("VP8L", vp8l(30, 20))])), Ok(ImageInfo { content_type: "image/webp", width: 30, height: 20 }));
    assert_eq!(check_image(&riff(&[("VP8 ", vp8(31, 21))])), Ok(ImageInfo { content_type: "image/webp", width: 31, height: 21 }));
    assert_eq!(code(&riff(&[("VP8X", vp8x(0x10, 30, 20)), ("ALPH", vec![1, 0, 0]), ("VP8 ", vp8(30, 20))])), None);
    assert_eq!(code(&riff(&[("VP8X", vp8x(0x08, 30, 20)), ("VP8L", vp8l(30, 20))])), Some("webp_vp8x_flags"), "EXIF flag");
    assert_eq!(code(&riff(&[("VP8X", vp8x(0x02, 30, 20)), ("VP8L", vp8l(30, 20))])), Some("webp_vp8x_flags"), "animation flag");
    assert_eq!(code(&riff(&[("VP8X", vp8x(0, 99, 20)), ("VP8L", vp8l(30, 20))])), Some("webp_canvas"));
    assert_eq!(code(&riff(&[("VP8X", vp8x(0, 30, 20)), ("VP8L", vp8l(30, 20)), ("EXIF", vec![0; 4])])), Some("webp_layout"));
    assert_eq!(code(&riff(&[("VP8X", vp8x(0, 30, 20)), ("ANIM", vec![0; 6]), ("ANMF", vec![0; 16])])), Some("webp_layout"));
    assert_eq!(code(&riff(&[("VP8L", vp8l(30, 20)), ("XMP ", b"<x/>".to_vec())])), Some("webp_layout"));
    let sized = riff(&[("VP8L", vp8l(30, 20))]);
    assert_eq!(code(&cat(&[&sized, &[0, 0]])), Some("webp_riff_size"));
    let mut wav = b"RIFF".to_vec();
    wav.extend_from_slice(&[4, 0, 0, 0]);
    wav.extend_from_slice(b"WAVE");
    assert_eq!(code(&wav), Some("format"), "a non-WebP RIFF");
}

#[test]
fn anything_else_is_refused_by_its_bytes() {
    for b in [&b"<svg/>"[..], b"BM\0\0\0\0", b"II*\0", b"", b"%PDF-1.7", &vec![0u8; 2 * 1024 * 1024 + 1]] {
        assert!(check_image(b).is_err());
    }
    assert_eq!(code(b""), Some("size"));
    assert_eq!(code(b"<svg/>"), Some("format"));
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../worker/test/pool/fixtures").join(name)).unwrap()
}

#[test]
fn real_encoder_output_stripped_stills_pass_and_exif_animation_and_metadata_are_refused() {
    for (n, ct) in [("pil.jpg", "image/jpeg"), ("pil_prog.jpg", "image/jpeg"), ("pil.gif", "image/gif"), ("pil_lossless.webp", "image/webp"), ("pil_lossy.webp", "image/webp"), ("pil_alpha_lossy.webp", "image/webp")] {
        assert_eq!(check_image(&fixture(n)), Ok(ImageInfo { content_type: ct, width: 64, height: 48 }), "{n}");
    }
    assert_eq!(code(&fixture("pil_exif.jpg")), Some("jpeg_segment"));
    assert_eq!(code(&fixture("anim.gif")), Some("gif_extension"));
    assert_eq!(code(&fixture("anim.webp")), Some("webp_layout"));
    assert_eq!(code(&fixture("im_nostrip.png")), Some("png_chunk"));
}

#[test]
fn the_pixel_hash_is_the_same_for_a_re_encoded_picture_and_differs_for_other_pixels() {
    // round17: the same picture under another deflate level is the same image; other pixels are not.
    let h1 = png_pixel_hash(&png_at(9, 9, 31, &[], 6)).expect("a plain greyscale PNG decodes");
    assert!(h1.starts_with("px:") && h1.len() == 3 + 64);
    assert_eq!(Some(h1.clone()), png_pixel_hash(&png_at(9, 9, 31, &[], 1)));
    assert_ne!(Some(h1), png_pixel_hash(&png_at(9, 9, 32, &[], 6)));
    // Anything it does not decode has no pixel hash.
    assert_eq!(png_pixel_hash(&fixture("pil.jpg")), None);
    assert_eq!(png_pixel_hash(&fixture("im_nostrip.png")), None, "refused by the structural check");
    assert_eq!(png_pixel_hash(b"not an image"), None);
    let mut interlaced = png_(4, 4);
    interlaced[28] = 1;
    assert_eq!(png_pixel_hash(&interlaced), None);
    assert_eq!(ext("image/png"), "png");
    assert_eq!(ext("image/jpeg"), "jpg");
}
