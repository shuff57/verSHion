//! Structural image checks for PUT /blobs (worker/src/pool/image.ts; docs/shared-pool.md "Images"). The format is sniffed
//! from the bytes; PNG, JPEG, GIF or WebP only, as ONE still image, each side 1..4096, a strict allow-list of
//! chunks/segments/blocks (no metadata, no animation), every length field consistent with the file, and nothing after the
//! format's end marker. A refusal is a code, never input text.
use super::sqlfrag::hex;
use flate2::bufread::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub content_type: &'static str,
    pub width: usize,
    pub height: usize,
}

pub const MAX_SIDE: usize = 4096;
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// At most this many pixels (16 Mpix: a 4096 x 4096 image).
pub const MAX_PIXELS: usize = 16 * 1024 * 1024;
/// A PNG's pixel data may inflate to at most this many bytes (filter bytes included).
pub const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

type R<T> = Result<T, &'static str>;

fn u16be(b: &[u8], i: usize) -> usize {
    (b[i] as usize) << 8 | b[i + 1] as usize
}
fn u16le(b: &[u8], i: usize) -> usize {
    b[i] as usize | (b[i + 1] as usize) << 8
}
fn u24le(b: &[u8], i: usize) -> usize {
    b[i] as usize | (b[i + 1] as usize) << 8 | (b[i + 2] as usize) << 16
}
fn u32be(b: &[u8], i: usize) -> usize {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}
fn u32le(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize
}
fn ascii(b: &[u8], i: usize, n: usize) -> String {
    b[i..i + n].iter().map(|&c| c as char).collect()
}
fn need(b: &[u8], end: usize) -> R<()> {
    if end > b.len() { Err("truncated") } else { Ok(()) }
}
fn side(w: usize, h: usize) -> R<()> {
    if !((1..=MAX_SIDE).contains(&w) && (1..=MAX_SIDE).contains(&h)) {
        return Err("dimensions");
    }
    // Decompression amplification is bounded by pixels and decoded bytes.
    if w * h > MAX_PIXELS { Err("dimensions") } else { Ok(()) }
}

// ---- PNG ----

const PNG_SIG: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const PNG_ALLOWED: [&str; 9] = ["IHDR", "PLTE", "tRNS", "gAMA", "cHRM", "sRGB", "pHYs", "IDAT", "IEND"];

fn channels(color: u8) -> usize {
    match color {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        _ => 4,
    }
}

/// Bytes of the filtered, uncompressed image data (Adam7 passes when interlaced).
pub fn png_raw_size(w: usize, h: usize, depth: usize, color: u8, interlace: u8) -> usize {
    let bits = channels(color) * depth;
    let row = |cols: usize| if cols == 0 { 0 } else { 1 + (cols * bits).div_ceil(8) };
    if interlace == 0 {
        return h * row(w);
    }
    const PASSES: [[usize; 4]; 7] = [[0, 0, 8, 8], [4, 0, 8, 8], [0, 4, 4, 8], [2, 0, 4, 4], [0, 2, 2, 4], [1, 0, 2, 2], [0, 1, 1, 2]];
    let mut n = 0;
    for [x0, y0, dx, dy] in PASSES {
        let cols = (w.saturating_sub(x0)).div_ceil(dx);
        let rows = (h.saturating_sub(y0)).div_ceil(dy);
        if cols > 0 {
            n += rows * row(cols);
        }
    }
    n
}

/// Inflates `z` (zlib) and checks it is exactly `expected` bytes: no trailing data, no bomb.
fn inflates_to(z: &[u8], expected: usize) -> bool {
    let mut input: &[u8] = z;
    let mut n = 0usize;
    let mut buf = [0u8; 16 * 1024];
    {
        let mut d = ZlibDecoder::new(&mut input);
        loop {
            match d.read(&mut buf) {
                Ok(0) => break,
                Ok(k) => {
                    n += k;
                    if n > expected {
                        return false;
                    }
                }
                Err(_) => return false, // corrupt or truncated
            }
        }
    }
    n == expected && input.is_empty() // data after the end of the zlib stream is refused
}

struct PngParsed {
    width: usize,
    height: usize,
    idat: Vec<(usize, usize)>,
    raw: usize,
}

fn png(b: &[u8]) -> R<PngParsed> {
    let (mut i, mut w, mut h, mut color, mut raw, mut plte) = (8usize, 0usize, 0usize, 0u8, 0usize, 0usize);
    let mut idat: Vec<(usize, usize)> = Vec::new();
    let (mut seen_idat, mut after_idat, mut first) = (false, false, true);
    let mut seen: Vec<String> = Vec::new();
    loop {
        need(b, i + 12)?;
        let len = u32be(b, i);
        let ty = ascii(b, i + 4, 4);
        if len > b.len() {
            return Err("png_length");
        }
        need(b, i + 12 + len)?;
        if !PNG_ALLOWED.contains(&ty.as_str()) {
            return Err("png_chunk");
        }
        if crc32fast::hash(&b[i + 4..i + 8 + len]) as usize != u32be(b, i + 8 + len) {
            return Err("png_crc");
        }
        if first && ty != "IHDR" {
            return Err("png_ihdr_first");
        }
        if !first && ty == "IHDR" {
            return Err("png_ihdr_twice");
        }
        if ty != "IDAT" && ty != "IEND" && seen.contains(&ty) {
            return Err("png_chunk_repeated");
        }
        seen.push(ty.clone());
        if ty == "IHDR" {
            if len != 13 {
                return Err("png_ihdr");
            }
            w = u32be(b, i + 8);
            h = u32be(b, i + 12);
            let (depth, ct, comp, filter, interlace) = (b[i + 16], b[i + 17], b[i + 18], b[i + 19], b[i + 20]);
            color = ct;
            let ok_depth: &[u8] = match ct {
                0 => &[1, 2, 4, 8, 16],
                2 | 4 | 6 => &[8, 16],
                3 => &[1, 2, 4, 8],
                _ => &[],
            };
            if !ok_depth.contains(&depth) || comp != 0 || filter != 0 || interlace > 1 {
                return Err("png_ihdr");
            }
            side(w, h)?;
            raw = png_raw_size(w, h, depth as usize, color, interlace);
            if raw > MAX_DECODED_BYTES {
                return Err("png_decoded_size");
            }
        }
        // Ancillary chunks have fixed sizes: nothing can ride along in them.
        let fixed = match ty.as_str() {
            "gAMA" => Some(4),
            "cHRM" => Some(32),
            "sRGB" => Some(1),
            "pHYs" => Some(9),
            _ => None,
        };
        if fixed.is_some_and(|f| f != len) {
            return Err("png_chunk_size");
        }
        if ty == "PLTE" {
            // A palette only where it means something: required for indexed colour, refused for greyscale and truecolour.
            if color == 0 || color == 4 {
                return Err("png_plte_greyscale");
            }
            if color == 2 || color == 6 {
                return Err("png_plte_truecolor");
            }
            if len == 0 || len % 3 != 0 || len > 768 {
                return Err("png_chunk_size");
            }
            plte = len / 3;
        }
        if ty == "tRNS" {
            let ok = match color {
                0 => len == 2,
                2 => len == 6,
                3 => len >= 1 && len <= plte,
                _ => false,
            };
            if !ok {
                return Err("png_trns");
            }
        }
        if ty == "IDAT" {
            idat.push((i + 8, i + 8 + len));
            if after_idat {
                return Err("png_idat_split");
            }
            seen_idat = true;
        } else if seen_idat {
            after_idat = true;
        }
        if ty == "PLTE" && seen_idat {
            return Err("png_order");
        }
        first = false;
        i += 12 + len;
        if ty == "IEND" {
            if len != 0 {
                return Err("png_iend");
            }
            if i != b.len() {
                return Err("trailing_bytes");
            }
            break;
        }
    }
    if !seen_idat {
        return Err("png_no_idat");
    }
    if color == 3 && plte == 0 {
        return Err("png_no_plte");
    }
    side(w, h)?;
    // Compressed data can be at most the raw size plus deflate's stored-block overhead.
    let z: usize = idat.iter().map(|(s, e)| e - s).sum();
    if z > raw + 6 + 5 * raw.div_ceil(16383) + 64 {
        return Err("png_idat_size");
    }
    Ok(PngParsed { width: w, height: h, idat, raw })
}

fn idat_bytes(b: &[u8], p: &PngParsed) -> Vec<u8> {
    p.idat.iter().flat_map(|(s, e)| b[*s..*e].iter().copied()).collect()
}

// ---- JPEG ----

fn jpeg(b: &[u8]) -> R<ImageInfo> {
    let mut i = 2usize; // after SOI
    let (mut w, mut h, mut frames, mut scans, mut dqt, mut dht, mut comps) = (0usize, 0usize, 0, 0, 0, 0, 3usize);
    loop {
        need(b, i + 2)?;
        if b[i] != 0xff {
            return Err("jpeg_marker");
        }
        // Fill bytes (0xFF) may precede a marker.
        while b[i + 1] == 0xff {
            i += 1;
            need(b, i + 2)?;
        }
        let m = b[i + 1];
        i += 2;
        if m == 0xd9 {
            if scans == 0 || frames != 1 {
                return Err("jpeg_structure");
            }
            if i != b.len() {
                return Err("trailing_bytes");
            }
            break;
        }
        if m == 0xd8 || m == 0x01 || (0xd0..=0xd7).contains(&m) {
            return Err("jpeg_marker");
        }
        need(b, i + 2)?;
        let len = u16be(b, i);
        if len < 2 {
            return Err("jpeg_length");
        }
        need(b, i + len)?;
        let seg = &b[i + 2..i + len];
        match m {
            0xe0 => {
                // Exactly the 14-byte JFIF header with a 0x0 thumbnail: no JFXX, no embedded thumbnail, no payload.
                if seg.len() != 14 || &seg[..5] != b"JFIF\0" || seg[12] != 0 || seg[13] != 0 {
                    return Err("jpeg_app0");
                }
            }
            0xee => {
                if seg.len() != 12 || &seg[..5] != b"Adobe" {
                    return Err("jpeg_app14");
                }
            }
            0xc0..=0xc2 => {
                frames += 1;
                if frames > 1 || seg.len() < 6 {
                    return Err("jpeg_sof");
                }
                h = u16be(seg, 1);
                w = u16be(seg, 3);
                comps = seg[5] as usize;
                if seg.len() != 6 + 3 * comps || !matches!(comps, 1 | 3 | 4) {
                    return Err("jpeg_sof");
                }
            }
            0xdb => {
                // DQT: whole tables only (precision, id, 64 or 128 bytes), at most 4 per file.
                let mut k = 0;
                while k < seg.len() {
                    let pq = (seg[k] >> 4) as usize;
                    if pq > 1 || (seg[k] & 15) > 3 {
                        return Err("jpeg_dqt");
                    }
                    k += 1 + 64 * (pq + 1);
                    dqt += 1;
                    if dqt > 4 {
                        return Err("jpeg_dqt");
                    }
                }
                if k != seg.len() {
                    return Err("jpeg_dqt");
                }
            }
            0xc4 => {
                // DHT: whole tables only (class, id, 16 counts, at most 256 symbols).
                let mut k = 0;
                while k < seg.len() {
                    if seg[k] >> 4 > 1 || (seg[k] & 15) > 3 || k + 17 > seg.len() {
                        return Err("jpeg_dht");
                    }
                    let sum: usize = (1..=16).map(|t| seg[k + t] as usize).sum();
                    if sum > 256 {
                        return Err("jpeg_dht");
                    }
                    k += 17 + sum;
                    dht += 1;
                    if dht > 32 {
                        return Err("jpeg_dht");
                    }
                }
                if k != seg.len() {
                    return Err("jpeg_dht");
                }
            }
            0xdd => {
                if seg.len() != 2 {
                    return Err("jpeg_dri");
                }
            }
            0xda => {
                if frames != 1 {
                    return Err("jpeg_structure");
                }
                let ns = seg.first().copied().unwrap_or(0) as usize;
                if !(1..=4).contains(&ns) || seg.len() != 4 + 2 * ns {
                    return Err("jpeg_sos");
                }
                scans += 1;
                i += len;
                // Entropy-coded data: until a marker other than a stuffed 0x00 or a restart marker.
                loop {
                    need(b, i + 2)?;
                    if b[i] == 0xff {
                        let n = b[i + 1];
                        if n == 0x00 || (0xd0..=0xd7).contains(&n) {
                            i += 2;
                            continue;
                        }
                        if n == 0xff {
                            i += 1;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            // APP1 (EXIF/XMP), APP2 (ICC), APP13 (IPTC), COM, every other APPn and marker.
            _ => return Err("jpeg_segment"),
        }
        i += len;
    }
    side(w, h)?;
    // Entropy-coded data cannot be checked without decoding; its size is bounded by the pixel count.
    if b.len() > 4096 + 2 * w * h * comps {
        return Err("jpeg_size");
    }
    Ok(ImageInfo { content_type: "image/jpeg", width: w, height: h })
}

// ---- GIF ----

fn gif(b: &[u8]) -> R<ImageInfo> {
    need(b, 13)?;
    let (sw, sh) = (u16le(b, 6), u16le(b, 8));
    side(sw, sh)?;
    let packed = b[10];
    let mut i = 13usize;
    if packed & 0x80 != 0 {
        i += 3 * (1 << ((packed & 7) + 1));
    }
    need(b, i)?;
    let (mut images, mut gce) = (0, 0);
    loop {
        need(b, i + 1)?;
        let t = b[i];
        if t == 0x3b {
            if images != 1 {
                return Err("gif_structure");
            }
            if i + 1 != b.len() {
                return Err("trailing_bytes");
            }
            break;
        }
        if t == 0x21 {
            need(b, i + 2)?;
            if b[i + 1] != 0xf9 {
                return Err("gif_extension"); // comment, application (looping, XMP), plain text
            }
            gce += 1;
            if images > 0 || gce > 1 {
                return Err("gif_extension");
            }
            need(b, i + 8)?;
            if b[i + 2] != 4 || b[i + 7] != 0 {
                return Err("gif_gce");
            }
            i += 8;
            continue;
        }
        if t == 0x2c {
            images += 1;
            if images > 1 {
                return Err("gif_animated");
            }
            need(b, i + 10)?;
            let (l, tp, w, h) = (u16le(b, i + 1), u16le(b, i + 3), u16le(b, i + 5), u16le(b, i + 7));
            if w < 1 || h < 1 || l + w > sw || tp + h > sh {
                return Err("gif_frame");
            }
            let p = b[i + 9];
            i += 10;
            if p & 0x80 != 0 {
                i += 3 * (1 << ((p & 7) + 1));
            }
            need(b, i + 1)?;
            let lzw = b[i] as usize;
            if !(2..=8).contains(&lzw) {
                return Err("gif_lzw");
            }
            i += 1;
            let start = i;
            loop {
                need(b, i + 1)?;
                let n = b[i] as usize;
                i += 1;
                if n == 0 {
                    break;
                }
                need(b, i + n)?;
                i += n;
            }
            if !lzw_exact(&b[start..i], lzw, w * h) {
                return Err("gif_lzw_data");
            }
            continue;
        }
        return Err("gif_block");
    }
    Ok(ImageInfo { content_type: "image/gif", width: sw, height: sh })
}

/// Decodes a GIF LZW stream (sub-blocks still framed) far enough to count pixels: it must end with the end-of-information
/// code, decode to exactly `pixels`, and leave no whole byte unused after it.
fn lzw_exact(blocks: &[u8], min: usize, pixels: usize) -> bool {
    let mut data: Vec<u8> = Vec::new();
    let mut k = 0;
    while k < blocks.len() {
        let n = blocks[k] as usize;
        if n == 0 {
            break;
        }
        for t in 1..=n {
            data.push(*blocks.get(k + t).unwrap_or(&0));
        }
        k += 1 + n;
    }
    let clear = 1usize << min;
    let eoi = clear + 1;
    let mut len = [0usize; 4096];
    let (mut size, mut next, mut prev, mut count, mut bit) = (min + 1, eoi + 1, usize::MAX, 0usize, 0usize);
    len[..clear].fill(1);
    let total = data.len() * 8;
    while bit + size <= total {
        let mut code = 0usize;
        for k in 0..size {
            code |= (((data[bit >> 3] >> (bit & 7)) & 1) as usize) << k;
            bit += 1;
        }
        if code == clear {
            size = min + 1;
            next = eoi + 1;
            prev = usize::MAX;
            continue;
        }
        if code == eoi {
            return count == pixels && total - bit < 8;
        }
        let l = if code < next && (code < clear || code > eoi) {
            len[code]
        } else if code == next && prev != usize::MAX {
            len[prev] + 1
        } else {
            return false;
        };
        count += l;
        if count > pixels {
            return false;
        }
        if prev != usize::MAX && next < 4096 {
            len[next] = len[prev] + 1;
            next += 1;
            if next == 1 << size && size < 12 {
                size += 1;
            }
        }
        prev = code;
    }
    false // no end-of-information code
}

// ---- WebP ----

fn vp8_size(d: &[u8]) -> R<(usize, usize)> {
    if d.len() < 10 {
        return Err("webp_vp8");
    }
    if d[0] & 1 != 0 {
        return Err("webp_vp8_keyframe");
    }
    if d[3] != 0x9d || d[4] != 0x01 || d[5] != 0x2a {
        return Err("webp_vp8_start");
    }
    Ok((u16le(d, 6) & 0x3fff, u16le(d, 8) & 0x3fff))
}
fn vp8l_size(d: &[u8]) -> R<(usize, usize)> {
    if d.len() < 5 || d[0] != 0x2f {
        return Err("webp_vp8l");
    }
    let bits = u32le(d, 1);
    if bits >> 29 != 0 {
        return Err("webp_vp8l_version");
    }
    Ok(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
}

fn webp(b: &[u8]) -> R<ImageInfo> {
    need(b, 12)?;
    if u32le(b, 4) != b.len() - 8 {
        return Err("webp_riff_size");
    }
    let mut i = 12usize;
    let mut chunks: Vec<(String, &[u8])> = Vec::new();
    while i < b.len() {
        need(b, i + 8)?;
        let ty = ascii(b, i, 4);
        let len = u32le(b, i + 4);
        need(b, i + 8 + len)?;
        chunks.push((ty, &b[i + 8..i + 8 + len]));
        i += 8 + len;
        if len & 1 == 1 {
            need(b, i + 1)?;
            i += 1;
        }
        if chunks.len() > 3 {
            return Err("webp_layout");
        }
    }
    if i != b.len() {
        return Err("trailing_bytes");
    }
    let types: Vec<&str> = chunks.iter().map(|c| c.0.as_str()).collect();
    let (w, h);
    match types.as_slice() {
        ["VP8 "] => (w, h) = vp8_size(chunks[0].1)?,
        ["VP8L"] => (w, h) = vp8l_size(chunks[0].1)?,
        ["VP8X", rest @ ..] if matches!(rest, ["VP8 "] | ["VP8L"] | ["ALPH", "VP8 "] | ["ALPH", "VP8L"]) => {
            let x = chunks[0].1;
            if x.len() != 10 {
                return Err("webp_vp8x");
            }
            // ICC 0x20, EXIF 0x08, XMP 0x04, animation 0x02 must be clear (alpha 0x10 may be set).
            if x[0] & (0x20 | 0x08 | 0x04 | 0x02) != 0 {
                return Err("webp_vp8x_flags");
            }
            let (cw, ch) = (u24le(x, 4) + 1, u24le(x, 7) + 1);
            let last = chunks.last().unwrap();
            (w, h) = if last.0 == "VP8 " { vp8_size(last.1)? } else { vp8l_size(last.1)? };
            if cw != w || ch != h {
                return Err("webp_canvas");
            }
        }
        _ => return Err("webp_layout"),
    }
    side(w, h)?;
    // Bitstreams and alpha bounded by the pixel count (uncompressed alpha is exactly 1 + w*h bytes).
    for (ty, d) in &chunks {
        if (ty == "VP8 " || ty == "VP8L") && d.len() > 4 * w * h + 1024 {
            return Err("webp_size");
        }
        if ty == "ALPH" {
            let first = d.first().copied().unwrap_or(0xff);
            let method = first & 3;
            if d.is_empty() || method > 1 || (first >> 6) != 0 {
                return Err("webp_alph");
            }
            if if method == 0 { d.len() != 1 + w * h } else { d.len() > 1 + w * h + 1024 } {
                return Err("webp_alph_size");
            }
        }
    }
    Ok(ImageInfo { content_type: "image/webp", width: w, height: h })
}

/// A hash of a PNG's decoded pixels (canonical RGBA, dimensions included), so the same picture re-encoded (another deflate,
/// other filters, palette or truecolour) is the same image for licence evidence. Streamed row by row (never the whole
/// decoded image in memory). None for what it does not decode (interlaced images; any other format: their bytes' hash is
/// the key).
pub fn png_pixel_hash(b: &[u8]) -> Option<String> {
    if b.len() < 8 || b[..8] != PNG_SIG {
        return None;
    }
    let p = png(b).ok()?;
    // IHDR fields and palette (png() checked them).
    let (depth, color, interlace) = (b[24] as usize, b[25], b[28]);
    if interlace != 0 {
        return None;
    }
    let (mut palette, mut trns): (&[u8], Option<&[u8]>) = (&[], None);
    let mut i = 8;
    while i < b.len() {
        let len = u32be(b, i);
        match &b[i + 4..i + 8] {
            b"PLTE" => palette = &b[i + 8..i + 8 + len],
            b"tRNS" => trns = Some(&b[i + 8..i + 8 + len]),
            _ => {}
        }
        i += 12 + len;
    }
    let w = p.width;
    let ch = channels(color);
    let bits = ch * depth;
    let row_bytes = (w * bits).div_ceil(8);
    let bpp = bits.div_ceil(8).max(1);
    let out16 = depth == 16;
    let max_v = (1usize << depth) - 1;
    let sample = |row: &[u8], x: usize, c: usize| -> usize {
        match depth {
            8 => row[x * ch + c] as usize,
            16 => (row[(x * ch + c) * 2] as usize) << 8 | row[(x * ch + c) * 2 + 1] as usize,
            _ => {
                let bit = (x * ch + c) * depth;
                ((row[bit >> 3] as usize) >> (8 - depth - (bit & 7))) & max_v
            }
        }
    };
    let t16 = |t: &[u8], i: usize| (t[i] as usize) << 8 | t[i + 1] as usize;
    let scale = |v: usize| if out16 || depth == 8 { v } else { ((v * 255) as f64 / max_v as f64).round() as usize };
    let full = if out16 { 65535 } else { 255 };
    let emit = |row: &[u8]| -> Vec<u8> {
        let mut px = vec![0u8; w * 4 * if out16 { 2 } else { 1 }];
        let mut put = |x: usize, k: usize, v: usize| {
            if out16 {
                px[(x * 4 + k) * 2] = (v >> 8) as u8;
                px[(x * 4 + k) * 2 + 1] = (v & 255) as u8;
            } else {
                px[x * 4 + k] = v as u8;
            }
        };
        for x in 0..w {
            if color == 3 {
                let idx = sample(row, x, 0);
                let at = |o: usize| palette.get(idx * 3 + o).copied().unwrap_or(0) as usize;
                put(x, 0, at(0));
                put(x, 1, at(1));
                put(x, 2, at(2));
                put(x, 3, trns.and_then(|t| t.get(idx)).map_or(255, |&v| v as usize));
                continue;
            }
            let g = |c: usize| sample(row, x, c);
            let (r, gg, bb) = if color == 0 || color == 4 { (g(0), g(0), g(0)) } else { (g(0), g(1), g(2)) };
            let mut a = if color == 4 { g(1) } else if color == 6 { g(3) } else { full };
            if let (Some(t), true) = (trns, color == 0 || color == 2) {
                let matches = if color == 0 { t16(t, 0) == g(0) } else { t16(t, 0) == g(0) && t16(t, 2) == g(1) && t16(t, 4) == g(2) };
                if matches {
                    a = 0;
                }
            }
            put(x, 0, scale(r));
            put(x, 1, scale(gg));
            put(x, 2, scale(bb));
            put(x, 3, if color == 4 || color == 6 { scale(a) } else if a == 0 { 0 } else { full });
        }
        px
    };
    let z = idat_bytes(b, &p);
    let mut input: &[u8] = &z;
    let mut d = ZlibDecoder::new(&mut input);
    let (mut prev, mut cur) = (vec![0u8; row_bytes], vec![0u8; row_bytes]);
    let mut pending = vec![0u8; row_bytes + 1];
    let (mut have, mut rows) = (0usize, 0usize);
    let mut row_digests: Vec<[u8; 32]> = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = match d.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return None,
        };
        for &byte in &buf[..n] {
            pending[have] = byte;
            have += 1;
            if have == row_bytes + 1 {
                cur.copy_from_slice(&pending[1..]);
                let f = pending[0];
                for k in 0..row_bytes {
                    let a = if k >= bpp { cur[k - bpp] as i32 } else { 0 };
                    let up = prev[k] as i32;
                    let c = if k >= bpp { prev[k - bpp] as i32 } else { 0 };
                    let mut v = cur[k] as i32;
                    match f {
                        0 => {}
                        1 => v += a,
                        2 => v += up,
                        3 => v += (a + up) >> 1,
                        4 => {
                            let pp = a + up - c;
                            let (pa, pb, pc) = ((pp - a).abs(), (pp - up).abs(), (pp - c).abs());
                            v += if pa <= pb && pa <= pc { a } else if pb <= pc { up } else { c };
                        }
                        _ => return None,
                    }
                    cur[k] = (v & 255) as u8;
                }
                row_digests.push(Sha256::digest(emit(&cur)).into());
                std::mem::swap(&mut prev, &mut cur);
                have = 0;
                rows += 1;
                if rows > p.height {
                    return None;
                }
            }
        }
    }
    if rows != p.height || have != 0 {
        return None;
    }
    let mut h = Sha256::new();
    h.update(format!("vershion/pixels/v1|{w}|{}|{}|", p.height, if out16 { 16 } else { 8 }));
    for d in &row_digests {
        h.update(d);
    }
    Some(format!("px:{}", hex(&h.finalize())))
}

/// The verified format and size, or the reason the bytes are refused (a code, never input text).
pub fn check_image(b: &[u8]) -> Result<ImageInfo, &'static str> {
    if b.is_empty() || b.len() > MAX_IMAGE_BYTES {
        return Err("size");
    }
    if b.len() >= 8 && b[..8] == PNG_SIG {
        let p = png(b)?;
        // The pixel data must inflate to exactly the image's size: nothing after the zlib stream.
        if !inflates_to(&idat_bytes(b, &p), p.raw) {
            return Err("png_idat_data");
        }
        return Ok(ImageInfo { content_type: "image/png", width: p.width, height: p.height });
    }
    if b.len() >= 3 && b[0] == 0xff && b[1] == 0xd8 && b[2] == 0xff {
        return jpeg(b);
    }
    if b.len() >= 6 && (&b[..6] == b"GIF87a" || &b[..6] == b"GIF89a") {
        return gif(b);
    }
    if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return webp(b);
    }
    Err("format")
}

pub fn ext(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "webp",
    }
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
