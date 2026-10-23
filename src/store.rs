//! Local-only persistence. JSON documents in the store dir; images under media/ keyed by sha256.
use std::path::PathBuf;
use web_time::{SystemTime, UNIX_EPOCH};
use serde::{de::DeserializeOwned, Serialize};
use crate::model::{Exam, Id, Question, QuestionBank};

fn data_base() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn store_dir_from_env() -> PathBuf {
    if let Ok(dir) = crate::env_var("VERSHION_STORE_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    data_base().join("vershion")
}

/// Legacy path helper. New code goes through the backend (`save_json`, `read_media`, …) so the
/// native filesystem and the browser IndexedDB store stay interchangeable.
pub fn store_dir() -> PathBuf {
    store_dir_from_env()
}

#[cfg(target_arch = "wasm32")]
pub mod web;

/// The storage seam. Native uses [`FsBackend`]; the web build installs an IndexedDB backend
/// before constructing the app.
pub trait Backend {
    fn read_doc(&self, name: &str) -> Option<Vec<u8>>;
    fn write_doc(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
    fn exists(&self, rel: &str) -> bool;
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    fn write(&self, rel: &str, bytes: &[u8]) -> Result<(), String>;
    fn list(&self, sub: &str) -> Vec<String>;
    fn label(&self) -> String;
}

#[cfg(not(target_arch = "wasm32"))]
static BACKEND: std::sync::OnceLock<Box<dyn Backend + Send + Sync>> = std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
pub fn init(backend: Box<dyn Backend + Send + Sync>) {
    let _ = BACKEND.set(backend);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_backend<R>(f: impl FnOnce(&dyn Backend) -> R) -> R {
    let backend = BACKEND.get_or_init(|| Box::new(FsBackend::from_env()));
    f(backend.as_ref())
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static BACKEND: std::cell::RefCell<Option<Box<dyn Backend>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub fn init(backend: Box<dyn Backend>) {
    BACKEND.with(|slot| *slot.borrow_mut() = Some(backend));
}

#[cfg(target_arch = "wasm32")]
pub fn with_backend<R>(f: impl FnOnce(&dyn Backend) -> R) -> R {
    BACKEND.with(|slot| {
        let slot = slot.borrow();
        let backend = slot.as_ref().expect("store backend: call store::init before use");
        f(backend.as_ref())
    })
}

/// Filesystem backend: JSON documents, media and backups under one root.
#[cfg(not(target_arch = "wasm32"))]
pub struct FsBackend {
    pub root: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FsBackend {
    pub fn from_env() -> FsBackend {
        FsBackend { root: store_dir_from_env() }
    }

    pub fn new(root: impl Into<PathBuf>) -> FsBackend {
        FsBackend { root: root.into() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Backend for FsBackend {
    fn read_doc(&self, name: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(name)).ok()
    }

    fn write_doc(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = self.root.join(format!("{name}.tmp"));
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).exists()
    }

    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(rel)).ok()
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Written beside the final name and renamed into place, so a crash or a full disk never
        // leaves a half-written picture under its real (content) name.
        let mut tmp = path.clone().into_os_string();
        tmp.push(".part");
        let tmp = PathBuf::from(tmp);
        let done = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &path));
        if done.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        done.map_err(|e| e.to_string())
    }

    fn list(&self, sub: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.join(sub))
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn label(&self) -> String {
        self.root.display().to_string()
    }
}

/// One-time, non-destructive copy of the pre-rename data dir (`…/queshion`).
/// Runs only when `…/vershion` is absent or empty; the old dir is left untouched.
#[cfg(not(target_arch = "wasm32"))]
pub fn migrate_legacy_dir() {
    let base = data_base();
    let old = base.join("queshion");
    let new = base.join("vershion");
    if !old.is_dir() {
        return;
    }
    let new_empty = std::fs::read_dir(&new)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if !new_empty {
        return;
    }
    if let Err(e) = copy_tree(&old, &new) {
        eprintln!("verSHion: could not copy legacy data from {}: {e}", old.display());
    } else {
        eprintln!("verSHion: copied legacy data from {} to {}", old.display(), new.display());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else if entry.file_type()?.is_file() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Serializes and writes a JSON document through the backend (atomic on the filesystem).
pub fn save_json<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    with_backend(|b| b.write_doc(name, &bytes))
}

pub fn load_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    let bytes = with_backend(|b| b.read_doc(name))?;
    serde_json::from_slice(&bytes).ok()
}

/// Most bytes a picture may have before it is even looked at.
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
/// Widest or tallest source picture that is decoded at all (a flood guard; checked from the header).
pub const MAX_SOURCE_SIDE: u32 = 16384;
/// Most pixels a source picture may have (64 Mpix is about 256 MB as RGBA).
pub const MAX_SOURCE_PIXELS: u64 = 64_000_000;
/// A stored picture is scaled down until neither side is longer than this (the pool's cap).
pub const MAX_STORED_SIDE: u32 = 4096;

/// Stores a picture under media/<sha256 of the stored bytes>.webp and returns the ImageRef.
///
/// Everything is checked before anything is written (see [`prepare_image`]); a refused picture
/// leaves nothing in `media/`. Older stores keep working: their `.png`/`.jpg`/`.gif` files are
/// still read by name.
pub fn put_image(bytes: &[u8]) -> Result<crate::model::ImageRef, String> {
    with_backend(|b| put_image_in(b, bytes))
}

/// [`put_image`] against a given backend.
pub fn put_image_in(backend: &dyn Backend, bytes: &[u8]) -> Result<crate::model::ImageRef, String> {
    let img = prepare_image(bytes)?;
    let file = format!("media/{}.webp", sha_hex(&img.bytes));
    if !backend.exists(&file) {
        backend.write(&file, &img.bytes)?;
    }
    Ok(crate::model::ImageRef { file, width: img.width, height: img.height })
}

/// Puts back a picture from a bank file under the name it had there (`media/<sha256><ext>` of
/// exactly these bytes), so pictures exported by older builds, whose names are the hash of a PNG
/// or JPEG, still match their questions. The bytes get the same checks as any picture and are
/// kept as they are; returns false (nothing written) if they fail or do not match the name.
pub fn restore_image(file: &str, bytes: &[u8]) -> bool {
    with_backend(|b| restore_image_in(b, file, bytes))
}

pub fn restore_image_in(backend: &dyn Backend, file: &str, bytes: &[u8]) -> bool {
    let Some(rest) = file.strip_prefix("media/") else { return false };
    let Some((stem, ext)) = rest.split_once('.') else { return false };
    if stem != sha_hex(bytes) || format!(".{ext}") != sniff_ext(bytes) || prepare_image(bytes).is_err() {
        return false;
    }
    backend.exists(file) || backend.write(file, bytes).is_ok()
}

fn sha_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

/// A picture that passed every check, in the form that is stored.
pub struct PreparedImage {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Validates a picture and returns what would be stored, touching no storage:
/// - the bytes must start with the magic of PNG, JPEG, GIF, WebP or BMP (a name or extension
///   never counts), and the decoder for *that* format must accept them, so a file whose magic
///   says one thing and whose body is another is refused;
/// - size, width, height and pixel count are capped before the pixels are decoded;
/// - the picture is decoded in full (a truncated or corrupt file fails here, not later), with
///   its EXIF orientation applied; an animated GIF or WebP contributes its first frame;
/// - the result is lossless WebP (no EXIF, ICC or comments), scaled down to fit
///   [`MAX_STORED_SIDE`]. A still WebP that carries only picture data is kept byte for byte.
pub fn prepare_image(bytes: &[u8]) -> Result<PreparedImage, String> {
    use image::{ImageDecoder, ImageFormat};
    if bytes.is_empty() {
        return Err("empty image".into());
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("image is larger than {} MB", MAX_IMAGE_BYTES / 1024 / 1024));
    }
    let format = match sniff_format(bytes) {
        Some(f) => f,
        None => return Err("not a PNG, JPEG, GIF, WebP or BMP picture".into()),
    };
    let limits = || {
        let mut l = image::Limits::default();
        l.max_image_width = Some(MAX_SOURCE_SIDE);
        l.max_image_height = Some(MAX_SOURCE_SIDE);
        l.max_alloc = Some(512 * 1024 * 1024);
        l
    };
    let open = || {
        let mut r = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        r.limits(limits());
        r.into_decoder().map_err(|e| format!("unreadable image: {e}"))
    };
    let mut decoder = open()?;
    let (w, h) = decoder.dimensions();
    if w == 0 || h == 0 || w > MAX_SOURCE_SIDE || h > MAX_SOURCE_SIDE || u64::from(w) * u64::from(h) > MAX_SOURCE_PIXELS {
        return Err(format!("image is too large ({w}x{h})"));
    }
    let orientation = decoder.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = image::DynamicImage::from_decoder(decoder).map_err(|e| format!("unreadable image: {e}"))?;
    img.apply_orientation(orientation);
    if format == ImageFormat::WebP && webp_is_plain_still(bytes) && img.width().max(img.height()) <= MAX_STORED_SIDE {
        return Ok(PreparedImage { bytes: bytes.to_vec(), width: img.width(), height: img.height() });
    }
    if img.width().max(img.height()) > MAX_STORED_SIDE {
        img = img.resize(MAX_STORED_SIDE, MAX_STORED_SIDE, image::imageops::FilterType::Lanczos3);
    }
    let (w, h) = (img.width(), img.height());
    let (raw, color) = if img.color().has_alpha() {
        (img.to_rgba8().into_raw(), image::ExtendedColorType::Rgba8)
    } else {
        (img.to_rgb8().into_raw(), image::ExtendedColorType::Rgb8)
    };
    let mut out = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut out)
        .encode(&raw, w, h, color)
        .map_err(|e| format!("could not store image: {e}"))?;
    Ok(PreparedImage { bytes: out, width: w, height: h })
}

fn sniff_format(bytes: &[u8]) -> Option<image::ImageFormat> {
    use image::ImageFormat as F;
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(F::Png)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(F::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(F::Gif)
    } else if is_webp(bytes) {
        Some(F::WebP)
    } else if bytes.starts_with(b"BM") {
        Some(F::Bmp)
    } else {
        None
    }
}

/// `RIFF`, a size that matches the file, and `WEBP` at byte 8 (any other RIFF, e.g. WAV, is not).
fn is_webp(b: &[u8]) -> bool {
    b.len() >= 20 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" && {
        let riff = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
        riff + 8 == b.len() || riff + 8 + (riff & 1) == b.len()
    }
}

/// True for a WebP whose chunks are only picture data: `VP8 `/`VP8L`/`ALPH`, or `VP8X` with the
/// animation, ICC, EXIF and XMP flags clear. Anything else is re-encoded.
fn webp_is_plain_still(b: &[u8]) -> bool {
    let mut i = 12;
    let mut n = 0;
    while i + 8 <= b.len() {
        let tag = &b[i..i + 4];
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        match tag {
            b"VP8X" => {
                // flags: ICC 0x20, alpha 0x10, EXIF 0x08, XMP 0x04, animation 0x02
                if len < 10 || i + 9 > b.len() || b[i + 8] & 0x2e != 0 {
                    return false;
                }
            }
            b"VP8 " | b"VP8L" | b"ALPH" => {}
            _ => return false,
        }
        n += 1;
        i = match i.checked_add(8 + len + (len & 1)) {
            Some(v) => v,
            None => return false,
        };
    }
    n > 0 && i == b.len()
}

/// Reads a stored media file (`ImageRef.file`) through the backend.
pub fn read_media(file: &str) -> Option<Vec<u8>> {
    with_backend(|b| b.read(file))
}

/// Whether a media file is already present in the backend.
pub fn media_exists(image: &crate::model::ImageRef) -> bool {
    with_backend(|b| b.exists(&image.file))
}

/// Lists the names in a store subdirectory (`media`, `backups`, …).
pub fn list_dir(sub: &str) -> Vec<String> {
    with_backend(|b| b.list(sub))
}

/// Human-readable location of the active backend, for Settings/Storage.
pub fn backend_label() -> String {
    with_backend(|b| b.label())
}

/// Where automatic backups go: `<store>/backups`.
pub fn backup_dir() -> PathBuf {
    store_dir().join("backups")
}

/// "20261017-235201" for a unix time (UTC), for backup file names.
pub fn backup_stamp(secs: i64) -> String {
    let (y, m, d) = crate::time::civil_from_days(secs.div_euclid(86400));
    let t = secs.rem_euclid(86400);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", t / 3600, t % 3600 / 60, t % 60)
}

/// Saves a copy of the banks before a bulk change: `banks.json` from the store as it is on
/// disk (or `banks` serialized, when there is no file yet) to `<backups>/banks-<stamp>.json`,
/// never overwriting an earlier backup. Returns the backup's path.
pub fn backup_banks_in(store: &std::path::Path, backups: &std::path::Path, banks: &[QuestionBank], secs: i64) -> Result<PathBuf, String> {
    std::fs::create_dir_all(backups).map_err(|e| e.to_string())?;
    let stamp = backup_stamp(secs);
    let mut path = backups.join(format!("banks-{stamp}.json"));
    let mut n = 2;
    while path.exists() {
        path = backups.join(format!("banks-{stamp}-{n}.json"));
        n += 1;
    }
    let bytes = match std::fs::read(store.join("banks.json")) {
        Ok(b) => b,
        Err(_) => serde_json::to_vec(banks).map_err(|e| e.to_string())?,
    };
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// [`backup_banks_in`] for the app's own store.
pub fn backup_banks(banks: &[QuestionBank]) -> Result<PathBuf, String> {
    backup_banks_in(&store_dir(), &backup_dir(), banks, now())
}

/// The store directory as text (headless hooks print it so tests can find media files).
pub fn store_dir_string() -> String {
    store_dir().to_string_lossy().into_owned()
}

pub fn media_path(image: &crate::model::ImageRef) -> PathBuf {
    store_dir().join(&image.file)
}

/// The file extension for a picture's bytes (names are by content, never by what a file was
/// called). Reads old stores' files too, so `.tiff` stays for them. A RIFF file is `.webp` only
/// when `WEBP` is at byte 8.
pub fn sniff_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") { ".png" }
    else if bytes.starts_with(b"\xff\xd8") { ".jpg" }
    else if bytes.starts_with(b"GIF8") { ".gif" }
    else if bytes.starts_with(&[0x49, 0x49, 0x2a]) || bytes.starts_with(&[0x4d, 0x4d, 0x00]) { ".tiff" }
    else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" { ".webp" }
    else if bytes.starts_with(b"BM") { ".bmp" }
    else { ".bin" }
}

/// One shared Store the whole app reads and mutates. Saves the affected file on every mutation.
#[derive(Default)]
pub struct Store {
    pub banks: Vec<QuestionBank>,
    pub exams: Vec<Exam>,
    pub imports: Vec<crate::model::Import>,
    pub exports: Vec<crate::model::ExportRecord>,
}

impl Store {
    /// Loads all four JSON documents; missing or corrupt files yield empty vectors.
    pub fn load() -> Store {
        Store {
            banks: load_json("banks.json").unwrap_or_default(),
            exams: load_json("exams.json").unwrap_or_default(),
            imports: load_json("imports.json").unwrap_or_default(),
            exports: load_json("exports.json").unwrap_or_default(),
        }
    }

    pub fn save_banks(&self) { let _ = save_json("banks.json", &self.banks); }
    pub fn save_exams(&self) { let _ = save_json("exams.json", &self.exams); }
    pub fn save_imports(&self) { let _ = save_json("imports.json", &self.imports); }
    pub fn save_exports(&self) { let _ = save_json("exports.json", &self.exports); }

    pub fn bank(&self, id: &Id) -> Option<&QuestionBank> {
        self.banks.iter().find(|b| b.id == *id)
    }

    pub fn bank_mut(&mut self, id: &Id) -> Option<&mut QuestionBank> {
        self.banks.iter_mut().find(|b| b.id == *id)
    }

    pub fn exam(&self, id: &Id) -> Option<&Exam> {
        self.exams.iter().find(|e| e.id == *id)
    }

    pub fn exam_mut(&mut self, id: &Id) -> Option<&mut Exam> {
        self.exams.iter_mut().find(|e| e.id == *id)
    }

    pub fn question(&self, bank_id: &Id, question_id: &Id) -> Option<&Question> {
        self.bank(bank_id)?.questions.iter().find(|q| q.id == *question_id)
    }

    pub fn question_mut(&mut self, bank_id: &Id, question_id: &Id) -> Option<&mut Question> {
        self.bank_mut(bank_id)?.questions.iter_mut().find(|q| q.id == *question_id)
    }
}
#[cfg(test)]
mod image_tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Mem(RefCell<BTreeMap<String, Vec<u8>>>);
    impl Backend for Mem {
        fn read_doc(&self, n: &str) -> Option<Vec<u8>> { self.0.borrow().get(n).cloned() }
        fn write_doc(&self, n: &str, b: &[u8]) -> Result<(), String> { self.0.borrow_mut().insert(n.into(), b.to_vec()); Ok(()) }
        fn exists(&self, r: &str) -> bool { self.0.borrow().contains_key(r) }
        fn read(&self, r: &str) -> Option<Vec<u8>> { self.0.borrow().get(r).cloned() }
        fn write(&self, r: &str, b: &[u8]) -> Result<(), String> { self.0.borrow_mut().insert(r.into(), b.to_vec()); Ok(()) }
        fn list(&self, _: &str) -> Vec<String> { self.0.borrow().keys().cloned().collect() }
        fn label(&self) -> String { "mem".into() }
    }
    impl Mem { fn n(&self) -> usize { self.0.borrow().len() } }

    fn pic(w: u32, h: u32, f: image::ImageFormat) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x * 7) as u8, (y * 5) as u8, 90]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, f).unwrap();
        out.into_inner()
    }

    #[test]
    fn each_format_is_stored_as_webp() {
        for f in [image::ImageFormat::Png, image::ImageFormat::Jpeg, image::ImageFormat::Gif, image::ImageFormat::WebP, image::ImageFormat::Bmp] {
            let m = Mem::default();
            let r = put_image_in(&m, &pic(37, 21, f)).unwrap_or_else(|e| panic!("{f:?}: {e}"));
            assert!(r.file.starts_with("media/") && r.file.ends_with(".webp"), "{f:?}: {}", r.file);
            assert_eq!((r.width, r.height), (37, 21));
            let stored = m.read(&r.file).unwrap();
            assert!(is_webp(&stored) && sniff_ext(&stored) == ".webp");
            assert_eq!(r.file, format!("media/{}.webp", sha_hex(&stored)));
            assert_eq!(m.n(), 1);
            // same picture again: same name, nothing new
            assert_eq!(put_image_in(&m, &pic(37, 21, f)).unwrap().file, r.file);
            assert_eq!(m.n(), 1);
        }
    }

    #[test]
    fn a_plain_webp_is_kept_byte_for_byte() {
        let m = Mem::default();
        let w = pic(10, 10, image::ImageFormat::WebP);
        let r = put_image_in(&m, &w).unwrap();
        assert_eq!(m.read(&r.file).unwrap(), w);
    }

    #[test]
    fn refused_pictures_write_nothing() {
        let m = Mem::default();
        let png = pic(20, 20, image::ImageFormat::Png);
        let mut wav = b"RIFF".to_vec();
        wav.extend(40u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend([0u8; 40]);
        let mut bad_magic_body = pic(20, 20, image::ImageFormat::Jpeg);
        bad_magic_body.splice(0..8, b"\x89PNG\r\n\x1a\n".iter().copied()); // PNG magic, JPEG body
        let mut svg = b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec();
        svg.resize(200, b' ');
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty", vec![]),
            ("text", b"hello world, not a picture".to_vec()),
            ("svg", svg),
            ("wav", wav),
            ("truncated header", png[..12].to_vec()),
            ("truncated body", png[..png.len() / 2].to_vec()),
            ("magic does not match body", bad_magic_body),
            ("tiff", b"II*\0\x08\0\0\0\0\0".to_vec()),
            ("too many bytes", vec![0x89; MAX_IMAGE_BYTES + 1]),
        ];
        for (name, bytes) in cases {
            assert!(put_image_in(&m, &bytes).is_err(), "{name} was accepted");
            assert_eq!(m.n(), 0, "{name} left a file behind");
        }
    }

    #[test]
    fn dimension_caps_are_checked_from_the_header() {
        // 20000x1 is a 20 KB-ish PNG that would decode fine but is over the side cap.
        let wide = pic(MAX_SOURCE_SIDE + 1, 1, image::ImageFormat::Png);
        let m = Mem::default();
        let e = put_image_in(&m, &wide).unwrap_err();
        assert!(e.contains("large") || e.contains("limit") || e.contains("unreadable"), "{e}");
        assert_eq!(m.n(), 0);
    }

    #[test]
    fn big_pictures_are_scaled_to_the_stored_cap() {
        let m = Mem::default();
        let r = put_image_in(&m, &pic(MAX_STORED_SIDE * 2, 100, image::ImageFormat::Png)).unwrap();
        assert_eq!(r.width, MAX_STORED_SIDE);
        assert_eq!(r.height, 50);
    }

    #[test]
    fn exif_orientation_is_applied_and_metadata_dropped() {
        // A JPEG with an APP1 EXIF segment (orientation 6 = rotate 90) and a comment.
        let base = pic(30, 10, image::ImageFormat::Jpeg);
        let exif: Vec<u8> = {
            let mut t = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0".to_vec();
            let mut seg = vec![0xff, 0xe1];
            seg.extend(((t.len() + 2) as u16).to_be_bytes());
            seg.append(&mut t);
            seg
        };
        let mut jpg = base[..2].to_vec();
        jpg.extend(exif);
        jpg.extend(&base[2..]);
        let m = Mem::default();
        let r = put_image_in(&m, &jpg).unwrap();
        assert_eq!((r.width, r.height), (10, 30), "orientation not applied");
        let stored = m.read(&r.file).unwrap();
        assert!(!stored.windows(4).any(|w| w == b"EXIF") && !stored.windows(4).any(|w| w == b"ICCP"));
    }

    #[test]
    fn alpha_survives() {
        let img = image::RgbaImage::from_fn(8, 8, |x, _| image::Rgba([10, 20, 30, if x < 4 { 0 } else { 255 }]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let m = Mem::default();
        let r = put_image_in(&m, &png.into_inner()).unwrap();
        let back = image::load_from_memory(&m.read(&r.file).unwrap()).unwrap().to_rgba8();
        assert_eq!(back.get_pixel(0, 0)[3], 0);
        assert_eq!(back.get_pixel(7, 0)[3], 255);
    }

    #[test]
    fn sniff_ext_wants_webp_at_byte_8() {
        assert_eq!(sniff_ext(b"RIFF\x24\0\0\0WAVEfmt "), ".bin");
        assert_eq!(sniff_ext(b"RIFF\x24\0\0\0WEBPVP8L"), ".webp");
        assert_eq!(sniff_ext(b"RIFF"), ".bin");
        assert_eq!(sniff_ext(b"\x89PNG\r\n"), ".png");
        assert_eq!(sniff_ext(b"\xff\xd8\xff\xe0"), ".jpg");
        assert_eq!(sniff_ext(b"GIF89a"), ".gif");
    }

    #[test]
    fn old_exports_restore_under_their_old_names() {
        let png = pic(12, 12, image::ImageFormat::Png);
        let name = format!("media/{}.png", sha_hex(&png));
        let m = Mem::default();
        assert!(restore_image_in(&m, &name, &png));
        assert_eq!(m.read(&name).unwrap(), png); // still readable by the old name
        // wrong name, wrong extension, corrupt bytes, path tricks: refused, nothing written
        let m = Mem::default();
        assert!(!restore_image_in(&m, "media/deadbeef.png", &png));
        assert!(!restore_image_in(&m, &format!("media/{}.jpg", sha_hex(&png)), &png));
        assert!(!restore_image_in(&m, "../x.png", &png));
        let bad = png[..png.len() / 2].to_vec();
        assert!(!restore_image_in(&m, &format!("media/{}.png", sha_hex(&bad)), &bad));
        assert_eq!(m.n(), 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn fs_backend_leaves_no_partial_files() {
        let dir = std::env::temp_dir().join(format!("vershion-img-{}-{}", std::process::id(), now()));
        let b = FsBackend::new(&dir);
        assert!(put_image_in(&b, b"not an image").is_err());
        assert!(!dir.join("media").exists() || std::fs::read_dir(dir.join("media")).unwrap().next().is_none());
        let r = put_image_in(&b, &pic(9, 9, image::ImageFormat::Png)).unwrap();
        let names = b.list("media");
        assert_eq!(names, vec![r.file.trim_start_matches("media/").to_string()], "{names:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
