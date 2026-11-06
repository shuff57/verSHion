//! Student work intake for the Grade screen: choose photos or scans for an exam, read the
//! handwriting with the AI the person set up, and save each student's pages where the review tray
//! already looks (`Store::student_submissions`).
//!
//! The logic (what to ask the model, how to read its answer, who a photo belongs to, which pages
//! still need reading) is plain functions below and is tested without a screen. The window and the
//! two floating buttons are drawn from `screens` while the Grade screen is open; results are
//! collected every frame whatever screen is open, so leaving the page does not lose a page that
//! finishes.
//!
//! What leaves the machine: each photo is sent to the AI the person chose in Settings, to be read.
//! The window says so before anything is sent. The pictures and transcripts are saved only here.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};

use egui::{Align2, Color32, CornerRadius, Margin, RichText, Stroke, Vec2};

use crate::ai::AiCfg;
use crate::model::{self, Correction, Illegible, StudentSubmission, SubmittedPage};
use crate::{pageshell, theme};

/// Photo and scan types the picker offers.
const EXTS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

/// Most pages in one go: a class set is read in several goes, not one runaway job.
const MAX_STAGED: usize = 40;

/// Most low-confidence words kept from one page.
const MAX_UNCERTAIN: usize = 20;

/// How the model is asked to read one page. The reply is JSON because every provider here is
/// asked for a JSON object.
pub fn prompt() -> String {
    "You are reading one photographed page of a student's handwritten exam work. Copy what the student wrote, in \
reading order, keeping their question numbers, line breaks, spelling and mistakes. Do not correct, solve or finish \
anything, and do not add words that are not on the page. Write math as plain text, or as LaTeX between $...$ when it \
has fractions or exponents. Where a run of writing cannot be read, write [illegible] there. Reply with JSON only: \
{\"text\": \"the whole page as text\", \"uncertain\": [{\"word\": \"a word exactly as you wrote it in text\", \
\"guess\": \"the other reading you considered\", \"confidence\": 0.4}]}. List only words you are truly unsure of, at \
most 20. If the page is blank or is not student work, reply {\"text\": \"\", \"uncertain\": []}."
        .to_string()
}

/// What one page read gave back.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub text: String,
    /// (word as written in `text`, the other reading, confidence 0..1)
    pub uncertain: Vec<(String, String, f32)>,
}

/// The model's reply as a page read. Fenced or chatty replies are accepted; a reply with no text
/// field, or with nothing readable in it, is an error the person is shown, not an empty page.
pub fn parse_reply(raw: &str) -> Result<Read, String> {
    let (Some(a), Some(b)) = (raw.find('{'), raw.rfind('}')) else {
        return Err("The AI's answer had no text in it.".to_string());
    };
    if b <= a {
        return Err("The AI's answer had no text in it.".to_string());
    }
    let v: serde_json::Value = serde_json::from_str(&raw[a..=b]).map_err(|_| "The AI's answer could not be read.".to_string())?;
    let text = v.get("text").and_then(|t| t.as_str()).ok_or_else(|| "The AI's answer had no text in it.".to_string())?;
    if text.trim().is_empty() {
        return Err("No handwriting was found on this page. Is it the right way up and in focus?".to_string());
    }
    let mut uncertain = Vec::new();
    if let Some(list) = v.get("uncertain").and_then(|u| u.as_array()) {
        for u in list {
            let word = u.get("word").and_then(|w| w.as_str()).unwrap_or("").trim();
            let guess = u.get("guess").and_then(|w| w.as_str()).unwrap_or("").trim();
            if word.is_empty() || guess.is_empty() || word == guess || !text.contains(word) {
                continue;
            }
            let c = u.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.5).clamp(0.0, 1.0) as f32;
            uncertain.push((word.to_string(), guess.to_string(), c));
            if uncertain.len() == MAX_UNCERTAIN {
                break;
            }
        }
    }
    Ok(Read { text: text.trim_end().to_string(), uncertain })
}

/// Puts a read onto a page, the way the review tray expects it.
pub fn apply_read(page: &mut SubmittedPage, r: Read) {
    page.illegible = (0..r.text.matches("[illegible]").count()).map(|_| Illegible { word: "[illegible]".to_string() }).collect();
    page.corrections = r.uncertain.into_iter().map(|(word, term, confidence)| Correction { word, term, confidence }).collect();
    page.transcript = r.text;
}

/// A student's name guessed from a photo's file name: "maria_lopez-p1.jpg" -> "maria lopez p1".
pub fn name_from_file(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let spaced: String = stem.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn mime_of(file: &str) -> &'static str {
    match file.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

/// One photo the person chose.
#[derive(Clone)]
pub struct Staged {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Who each staged photo belongs to: all of them to one named student, or one student per photo
/// named from its file. Returns (student, indexes into the staged list), in the order chosen.
pub fn plan(student: &str, per_file: bool, staged: &[Staged]) -> Vec<(String, Vec<usize>)> {
    if staged.is_empty() {
        return Vec::new();
    }
    if !per_file {
        return vec![(student.trim().to_string(), (0..staged.len()).collect())];
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    staged
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let mut n = name_from_file(&s.name);
            if n.is_empty() {
                n = format!("Student {}", i + 1);
            }
            // Two photos with one name stay two students rather than silently merging.
            let k = seen.entry(n.to_lowercase()).or_insert(0);
            *k += 1;
            if *k > 1 {
                n = format!("{n} ({})", *k);
            }
            (n, vec![i])
        })
        .collect()
}

/// A page waiting to be read.
#[derive(Clone, Debug)]
pub struct Job {
    pub page_id: model::Id,
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// Saves the pictures and builds the submissions for a plan: nothing is read yet, so every page
/// starts with an empty transcript and the review tray shows it as waiting.
pub fn build_submissions(
    backend: &dyn crate::store::Backend,
    exam_id: &str,
    plan: &[(String, Vec<usize>)],
    staged: &[Staged],
) -> Result<(Vec<StudentSubmission>, Vec<Job>), String> {
    let mut subs = Vec::new();
    let mut jobs = Vec::new();
    for (student, idxs) in plan {
        let mut pages = Vec::new();
        for &i in idxs {
            let s = &staged[i];
            let image = crate::store::put_image_in(backend, &s.bytes).map_err(|e| format!("{}: {e}", s.name))?;
            let id = model::new_id();
            jobs.push(Job { page_id: id.clone(), bytes: s.bytes.clone(), mime: mime_of(&s.name).to_string() });
            pages.push(SubmittedPage { id, image, ..Default::default() });
        }
        subs.push(StudentSubmission { id: model::new_id(), exam_id: exam_id.to_string(), student_name: student.clone(), pages, ..Default::default() });
    }
    Ok((subs, jobs))
}

/// Pages of this exam that have no transcript and are not being read now: a read that failed, or
/// one cut short when the app was closed.
pub fn unread_pages(subs: &[StudentSubmission], exam_id: &str, in_flight: &HashSet<String>) -> Vec<(String, model::ImageRef)> {
    subs.iter()
        .filter(|s| s.exam_id == exam_id)
        .flat_map(|s| s.pages.iter())
        .filter(|p| p.transcript.trim().is_empty() && !in_flight.contains(&p.id))
        .map(|p| (p.id.clone(), p.image.clone()))
        .collect()
}

/// Why a read cannot start with this AI set-up, in words for the person; None when it can.
pub fn ai_problem(cfg: &AiCfg) -> Option<String> {
    if cfg.provider == "account" && !crate::ai::account_ai_ready() {
        return Some("Sign in on the Account screen first: the hosted AI runs on your account.".to_string());
    }
    if !cfg.has_key_or_needs_none() {
        return Some("The AI needs a key. Add one in Settings.".to_string());
    }
    None
}

fn provider_words(cfg: &AiCfg) -> &'static str {
    match cfg.provider.as_str() {
        "ollama" => "the Ollama on this machine",
        "account" | "workers" => "the verSHion AI relay",
        "cloud" => "Ollama Cloud",
        "openrouter" => "OpenRouter",
        _ => "your custom AI endpoint",
    }
}

// ---------------------------------------------------------------- running reads

enum Msg {
    Read(model::Id, Result<Read, String>),
    /// The whole batch could not start (not signed in, out of credits).
    Refused(Vec<model::Id>, String),
}

#[derive(Default)]
struct State {
    open: bool,
    student: String,
    per_file: bool,
    staged: Vec<Staged>,
    /// A browser file picker is open: files arrive later through `platform::take_picked`.
    awaiting_pick: bool,
    in_flight: HashSet<String>,
    errors: HashMap<String, String>,
    note: Option<(String, bool)>,
    chan: Option<(Sender<Msg>, Receiver<Msg>)>,
}

thread_local! {
    static ST: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}

/// Starts reading `jobs` one after another in the background.
fn start(jobs: Vec<Job>, cfg: AiCfg, ctx: &egui::Context) {
    if jobs.is_empty() {
        return;
    }
    let tx = with(|st| {
        for j in &jobs {
            st.in_flight.insert(j.page_id.clone());
            st.errors.remove(&j.page_id);
        }
        st.chan.get_or_insert_with(channel).0.clone()
    });
    let ctx = ctx.clone();
    let prompt = prompt();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let ids: Vec<_> = jobs.iter().map(|j| j.page_id.clone()).collect();
        if let Err(e) = crate::ai::begin_service(&cfg, "handwriting", jobs.len()) {
            let _ = tx.send(Msg::Refused(ids, e));
            ctx.request_repaint();
            return;
        }
        for j in jobs {
            let r = crate::ai::transcribe_checked(&j.bytes, &j.mime, &cfg, &prompt).and_then(|raw| parse_reply(&raw));
            let _ = tx.send(Msg::Read(j.page_id, r));
            ctx.request_repaint();
        }
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let ids: Vec<_> = jobs.iter().map(|j| j.page_id.clone()).collect();
        if let Err(e) = crate::ai::begin_service_async(&cfg, "handwriting", jobs.len()).await {
            let _ = tx.send(Msg::Refused(ids, e));
            ctx.request_repaint();
            return;
        }
        for j in jobs {
            let r = match crate::ai::transcribe_checked_async(&j.bytes, &j.mime, &cfg, &prompt).await {
                Ok(raw) => parse_reply(&raw),
                Err(e) => Err(e),
            };
            let _ = tx.send(Msg::Read(j.page_id, r));
            ctx.request_repaint();
        }
    });
}

fn plural(n: usize, w: &str) -> String {
    format!("{n} {w}{}", if n == 1 { "" } else { "s" })
}

impl crate::App {
    /// Collects finished page reads into the saved submissions. Runs every frame.
    pub(crate) fn poll_grade_work(&mut self) {
        let msgs: Vec<Msg> = with(|st| st.chan.as_ref().map(|(_, rx)| rx.try_iter().collect()).unwrap_or_default());
        if msgs.is_empty() {
            return;
        }
        let mut changed = false;
        for m in msgs {
            match m {
                Msg::Read(id, res) => {
                    with(|st| st.in_flight.remove(&id));
                    match res {
                        Ok(r) => {
                            let page = self.store.student_submissions.iter_mut().flat_map(|s| s.pages.iter_mut()).find(|p| p.id == id);
                            if let Some(p) = page {
                                apply_read(p, r);
                                changed = true;
                            }
                            with(|st| st.errors.remove(&id));
                        }
                        Err(e) => with(|st| {
                            st.errors.insert(id, e);
                        }),
                    }
                }
                Msg::Refused(ids, e) => with(|st| {
                    for id in ids {
                        st.in_flight.remove(&id);
                        st.errors.insert(id, e.clone());
                    }
                }),
            }
        }
        if changed {
            self.store.save_student_submissions();
        }
    }

    /// The Grade screen's two floating pieces: the "Add student work" button (or the progress of a
    /// read) and the window it opens.
    pub(crate) fn draw_grade_intake(&mut self, ctx: &egui::Context) {
        let exam_id = self.grade.as_ref().and_then(|g| g.exam_id.clone());
        let exam_name = exam_id.as_ref().and_then(|id| self.store.exams.iter().find(|e| &e.id == id)).map(|e| e.name.clone());
        let cfg = AiCfg::from(&self.settings);
        let (busy, errors) = with(|st| (st.in_flight.len(), st.errors.len()));
        let unread = match &exam_id {
            Some(id) => with(|st| unread_pages(&self.store.student_submissions, id, &st.in_flight)).len(),
            None => 0,
        };
        if busy > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(400));
        }

        let (mut open_it, mut retry) = (false, false);
        let last_error = with(|st| st.errors.values().next().cloned());
        egui::Area::new(egui::Id::new("grade_intake_button"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, [-16.0, -16.0 - 44.0])
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    if busy > 0 {
                        pill_frame(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(theme::text(format!("Reading {}…", plural(busy, "page"))));
                            });
                        });
                    } else {
                        if unread > 0 {
                            let label = if errors > 0 {
                                format!("Try {} again", plural(unread, "unread page"))
                            } else {
                                format!("Read {}", plural(unread, "unread page"))
                            };
                            retry = pageshell::outline(ui, &label).clicked();
                            if let Some(e) = &last_error {
                                ui.add(egui::Label::new(theme::muted(e.clone()).size(12.0).color(theme::bad())).wrap());
                            }
                        }
                        let can = exam_id.is_some();
                        let r = ui.add_enabled(can, egui::Button::new(
                            RichText::new(format!("{}  Add student work", egui_phosphor::regular::UPLOAD_SIMPLE))
                                .family(theme::bold_family())
                                .size(14.0)
                                .color(theme::on_accent_col()),
                        )
                        .fill(theme::accent_col())
                        .corner_radius(CornerRadius::same(255))
                        .min_size(Vec2::new(0.0, 38.0)));
                        let r = if can { r } else { r.on_disabled_hover_text("Choose an exam first.") };
                        open_it = r.clicked();
                    }
                });
            });

        if retry {
            if let Some(msg) = ai_problem(&cfg) {
                with(|st| st.note = Some((msg, true)));
            } else if let Some(id) = &exam_id {
                let jobs: Vec<Job> = with(|st| unread_pages(&self.store.student_submissions, id, &st.in_flight))
                    .into_iter()
                    .filter_map(|(pid, img)| crate::store::read_media(&img.file).map(|bytes| Job { page_id: pid, bytes, mime: "image/webp".to_string() }))
                    .collect();
                start(jobs, cfg.clone(), ctx);
            }
        }
        if open_it {
            with(|st| {
                st.open = true;
                st.note = None;
            });
        }
        if with(|st| st.open) {
            self.draw_intake_window(ctx, exam_id.as_deref(), exam_name.as_deref(), &cfg);
        }
    }

    fn draw_intake_window(&mut self, ctx: &egui::Context, exam_id: Option<&str>, exam_name: Option<&str>, cfg: &AiCfg) {
        // Photos chosen in a browser arrive here a frame or more after the picker closed.
        if with(|st| st.awaiting_pick) {
            let got = crate::platform::take_picked();
            if !got.is_empty() {
                with(|st| {
                    st.awaiting_pick = false;
                    for f in got {
                        add_staged(st, f.name, f.bytes);
                    }
                });
            }
        }
        let mut open = true;
        let (mut pick, mut go, mut cancel) = (false, false, false);
        let mut remove: Option<usize> = None;
        egui::Window::new("Add student work")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_width(430.0_f32.min(ctx.content_rect().width() - 60.0).max(260.0));
                with(|st| {
                    ui.label(theme::muted(format!("Exam: {}", exam_name.unwrap_or("none chosen"))).size(12.5));
                    ui.add_space(8.0);
                    ui.checkbox(&mut st.per_file, "Each photo is a different student");
                    ui.add_space(4.0);
                    if st.per_file {
                        ui.label(theme::muted("Names come from the file names, like maria_lopez.jpg. You can fix them afterwards.").size(12.0));
                    } else {
                        ui.label(theme::text("Student name"));
                        ui.add(egui::TextEdit::singleline(&mut st.student).desired_width(f32::INFINITY).hint_text("All the photos below are this student's pages"));
                    }
                    ui.add_space(8.0);
                    pick = pageshell::outline(ui, &format!("{}  Choose photos or scans", egui_phosphor::regular::IMAGES)).clicked();
                    if st.awaiting_pick {
                        ui.label(theme::muted("Waiting for the files…").size(12.0));
                    }
                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(160.0).auto_shrink([true, true]).show(ui, |ui| {
                        for (i, s) in st.staged.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(theme::text(format!("{}. {}", i + 1, s.name)).size(13.0));
                                ui.label(theme::muted(format!("{} KB", s.bytes.len().div_ceil(1024))).size(11.5));
                                if ui.small_button("Remove").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                    });
                    ui.add_space(8.0);
                    ui.add(
                        egui::Label::new(
                            theme::muted(format!(
                                "Each photo is sent to {} to be read. The photos and the text are saved only on this machine.",
                                provider_words(cfg)
                            ))
                            .size(12.0),
                        )
                        .wrap(),
                    );
                    if let Some((msg, bad)) = &st.note {
                        ui.add_space(4.0);
                        let t = theme::muted(msg.clone()).size(12.5);
                        ui.add(egui::Label::new(if *bad { t.color(theme::bad()) } else { t }).wrap());
                    }
                    ui.add_space(10.0);
                    let ready = exam_id.is_some() && !st.staged.is_empty() && (st.per_file || !st.student.trim().is_empty());
                    ui.horizontal(|ui| {
                        let label = if st.staged.is_empty() { "Read pages".to_string() } else { format!("Read {}", plural(st.staged.len(), "page")) };
                        go = pageshell::primary_on(ui, &label, ready).clicked();
                        cancel = pageshell::outline(ui, "Cancel").clicked();
                    });
                    if !ready && !st.staged.is_empty() && !st.per_file && st.student.trim().is_empty() {
                        ui.label(theme::muted("Type the student's name first.").size(12.0));
                    }
                });
            });
        if let Some(i) = remove {
            with(|st| {
                if i < st.staged.len() {
                    st.staged.remove(i);
                }
            });
        }
        if pick {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(paths) = crate::platform::pick_files("Photos and scans", &EXTS) {
                with(|st| {
                    for p in paths {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        match std::fs::read(&p) {
                            Ok(bytes) => add_staged(st, name, bytes),
                            Err(e) => st.note = Some((format!("{name} could not be opened: {e}"), true)),
                        }
                    }
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = crate::platform::pick_files("Photos and scans", &EXTS);
                with(|st| st.awaiting_pick = true);
            }
        }
        if go {
            self.begin_reading(ctx, exam_id, cfg);
        }
        if cancel || !open {
            with(|st| {
                st.open = false;
                st.staged.clear();
                st.awaiting_pick = false;
            });
        }
    }

    /// "Read pages" pressed: save the pictures and the students, then start the reads.
    fn begin_reading(&mut self, ctx: &egui::Context, exam_id: Option<&str>, cfg: &AiCfg) {
        let Some(exam_id) = exam_id else { return };
        if let Some(msg) = ai_problem(cfg) {
            with(|st| st.note = Some((msg, true)));
            return;
        }
        let (student, per_file, staged) = with(|st| (st.student.clone(), st.per_file, st.staged.clone()));
        let p = plan(&student, per_file, &staged);
        let built = crate::store::with_backend(|b| build_submissions(b, exam_id, &p, &staged));
        match built {
            Ok((subs, jobs)) => {
                let (n_students, n_pages) = (subs.len(), jobs.len());
                self.store.student_submissions.extend(subs);
                self.store.save_student_submissions();
                start(jobs, cfg.clone(), ctx);
                with(|st| {
                    st.open = false;
                    st.staged.clear();
                    st.student.clear();
                });
                self.toast(format!("Reading {} for {}. Open them on the Grade screen as they finish.", plural(n_pages, "page"), plural(n_students, "student")));
            }
            Err(e) => with(|st| st.note = Some((format!("Could not save that picture: {e}"), true))),
        }
    }
}

fn add_staged(st: &mut State, name: String, bytes: Vec<u8>) {
    if st.staged.len() >= MAX_STAGED {
        st.note = Some((format!("That is the most for one go ({MAX_STAGED}). Read these, then add more."), true));
        return;
    }
    st.staged.push(Staged { name, bytes });
}

fn pill_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::card())
        .stroke(Stroke::new(1.0, theme::border()))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(14, 8))
        .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(40) })
        .show(ui, add);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Backend;
    use std::cell::RefCell as Cell;
    use std::collections::HashMap as Map;

    #[derive(Default)]
    struct Mem(Cell<Map<String, Vec<u8>>>);
    impl crate::store::Backend for Mem {
        fn read_doc(&self, n: &str) -> Option<Vec<u8>> {
            self.0.borrow().get(n).cloned()
        }
        fn write_doc(&self, n: &str, b: &[u8]) -> Result<(), String> {
            self.0.borrow_mut().insert(n.to_string(), b.to_vec());
            Ok(())
        }
        fn exists(&self, r: &str) -> bool {
            self.0.borrow().contains_key(r)
        }
        fn read(&self, r: &str) -> Option<Vec<u8>> {
            self.0.borrow().get(r).cloned()
        }
        fn write(&self, r: &str, b: &[u8]) -> Result<(), String> {
            self.0.borrow_mut().insert(r.to_string(), b.to_vec());
            Ok(())
        }
        fn list(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
        fn label(&self) -> String {
            "mem".into()
        }
    }

    fn png(shade: u8) -> Vec<u8> {
        let mut img = image::RgbImage::new(8, 8);
        for p in img.pixels_mut() {
            *p = image::Rgb([shade, 10, 10]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn staged(names: &[&str]) -> Vec<Staged> {
        names.iter().enumerate().map(|(i, n)| Staged { name: n.to_string(), bytes: png(i as u8 * 40) }).collect()
    }

    #[test]
    fn a_plain_reply_is_a_read() {
        let r = parse_reply(r#"{"text": "1. B\n2. x = 4", "uncertain": []}"#).unwrap();
        assert_eq!(r.text, "1. B\n2. x = 4");
        assert!(r.uncertain.is_empty());
    }

    #[test]
    fn a_fenced_reply_with_chatter_is_still_read() {
        let raw = "Sure! Here it is:\n```json\n{\"text\": \"x = 4\", \"uncertain\": [{\"word\": \"x\", \"guess\": \"y\", \"confidence\": 0.3}]}\n```";
        let r = parse_reply(raw).unwrap();
        assert_eq!(r.uncertain, vec![("x".to_string(), "y".to_string(), 0.3)]);
    }

    #[test]
    fn a_blank_page_is_an_error_not_an_empty_transcript() {
        let e = parse_reply(r#"{"text": "  ", "uncertain": []}"#).unwrap_err();
        assert!(e.contains("No handwriting"), "{e}");
    }

    #[test]
    fn replies_with_no_text_are_errors() {
        assert!(parse_reply("I cannot read this").is_err());
        assert!(parse_reply(r#"{"answer": "x"}"#).is_err());
        assert!(parse_reply("{ not json }").is_err());
    }

    #[test]
    fn uncertain_words_not_in_the_text_or_without_a_different_reading_are_dropped() {
        let raw = r#"{"text": "the cat sat", "uncertain": [
            {"word": "cat", "guess": "cot", "confidence": 2.0},
            {"word": "dog", "guess": "log"},
            {"word": "sat", "guess": "sat"},
            {"word": "", "guess": "x"}]}"#;
        let r = parse_reply(raw).unwrap();
        assert_eq!(r.uncertain, vec![("cat".to_string(), "cot".to_string(), 1.0)], "confidence is clamped");
    }

    #[test]
    fn at_most_twenty_uncertain_words_are_kept() {
        let words: Vec<String> = (0..30).map(|i| format!("w{i}")).collect();
        let list: Vec<String> = words.iter().map(|w| format!(r#"{{"word": "{w}", "guess": "{w}x"}}"#)).collect();
        let raw = format!(r#"{{"text": "{}", "uncertain": [{}]}}"#, words.join(" "), list.join(","));
        assert_eq!(parse_reply(&raw).unwrap().uncertain.len(), MAX_UNCERTAIN);
    }

    #[test]
    fn applying_a_read_fills_what_the_tray_reads() {
        let mut p = SubmittedPage::default();
        let r = parse_reply(r#"{"text": "a [illegible] b [illegible]", "uncertain": [{"word": "a", "guess": "o", "confidence": 0.4}]}"#).unwrap();
        apply_read(&mut p, r);
        assert_eq!(p.transcript, "a [illegible] b [illegible]");
        assert_eq!(p.illegible.len(), 2);
        assert_eq!((p.corrections[0].word.as_str(), p.corrections[0].term.as_str()), ("a", "o"));
    }

    #[test]
    fn names_come_from_file_names() {
        assert_eq!(name_from_file("maria_lopez.jpg"), "maria lopez");
        assert_eq!(name_from_file("/home/t/scans/Sam-Ng p1.PNG"), "Sam Ng p1");
        assert_eq!(name_from_file("C:\\x\\IMG_2041.jpeg"), "IMG 2041");
        assert_eq!(name_from_file(".jpg"), "");
    }

    #[test]
    fn one_student_gets_every_photo_in_the_order_chosen() {
        let p = plan("  Ana Ruiz ", false, &staged(&["b.jpg", "a.jpg", "c.jpg"]));
        assert_eq!(p, vec![("Ana Ruiz".to_string(), vec![0, 1, 2])]);
    }

    #[test]
    fn per_file_makes_a_student_each_and_keeps_same_names_apart() {
        let p = plan("ignored", true, &staged(&["ana.jpg", "ben.jpg", "Ana.png", ".jpg"]));
        let names: Vec<_> = p.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["ana", "ben", "Ana (2)", "Student 4"]);
        assert!(p.iter().all(|(_, i)| i.len() == 1));
    }

    #[test]
    fn an_empty_choice_plans_nothing() {
        assert!(plan("Ana", false, &[]).is_empty());
        assert!(plan("", true, &[]).is_empty());
    }

    #[test]
    fn building_saves_the_pictures_and_leaves_pages_unread() {
        let mem = Mem::default();
        let s = staged(&["ana-1.png", "ana-2.png"]);
        let (subs, jobs) = build_submissions(&mem, "exam1", &plan("Ana", false, &s), &s).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!((subs[0].exam_id.as_str(), subs[0].student_name.as_str()), ("exam1", "Ana"));
        assert_eq!(subs[0].pages.len(), 2);
        assert!(subs[0].pages.iter().all(|p| p.transcript.is_empty() && mem.exists(&p.image.file)));
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].page_id, subs[0].pages[0].id);
        assert_eq!(jobs[0].mime, "image/png");
    }

    #[test]
    fn a_file_that_is_not_a_picture_saves_nothing_and_names_the_file() {
        let mem = Mem::default();
        let s = vec![Staged { name: "notes.png".into(), bytes: b"not an image".to_vec() }];
        let e = build_submissions(&mem, "e", &plan("Ana", false, &s), &s).unwrap_err();
        assert!(e.starts_with("notes.png:"), "{e}");
        assert!(mem.0.borrow().is_empty());
    }

    #[test]
    fn unread_pages_are_this_exams_empty_pages_not_being_read() {
        let mem = Mem::default();
        let s = staged(&["a.png", "b.png", "c.png"]);
        let (mut subs, _) = build_submissions(&mem, "e1", &plan("", true, &s), &s).unwrap();
        let (other, _) = build_submissions(&mem, "e2", &plan("Zed", false, &staged(&["z.png"])), &staged(&["z.png"])).unwrap();
        subs.extend(other);
        subs[0].pages[0].transcript = "done".into();
        let flying: HashSet<String> = [subs[1].pages[0].id.clone()].into();
        let un = unread_pages(&subs, "e1", &flying);
        assert_eq!(un.len(), 1, "one read, one in flight, one waiting; the other exam is left alone");
        assert_eq!(un[0].0, subs[2].pages[0].id);
    }

    /// A one-answer-per-request fake Ollama: serves `replies` in order, then stops.
    fn fake_ollama(replies: Vec<String>) -> String {
        use std::io::{Read as _, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for reply in replies {
                let Ok((mut c, _)) = listener.accept() else { return };
                // Read the request head and the body it announces.
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let (mut head_end, mut want) = (None, 0usize);
                loop {
                    let n = c.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if head_end.is_none() {
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            head_end = Some(i + 4);
                            let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                            want = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                        }
                    }
                    if let Some(h) = head_end {
                        if buf.len() >= h + want {
                            break;
                        }
                    }
                }
                let body = serde_json::json!({"message": {"content": reply}}).to_string();
                let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            }
        });
        base
    }

    #[test]
    fn pages_are_read_in_the_background_and_a_blank_one_is_an_error_not_lost() {
        let base = fake_ollama(vec![
            r#"{"text": "1. B\n2. x = 4", "uncertain": [{"word": "B", "guess": "8", "confidence": 0.3}]}"#.to_string(),
            r#"{"text": "", "uncertain": []}"#.to_string(),
        ]);
        let cfg = AiCfg { provider: "ollama".into(), base, model: "test".into() };
        let mem = Mem::default();
        let s = staged(&["ana-1.png", "ana-2.png"]);
        let (subs, jobs) = build_submissions(&mem, "e1", &plan("Ana", false, &s), &s).unwrap();
        let (p1, p2) = (subs[0].pages[0].id.clone(), subs[0].pages[1].id.clone());
        start(jobs, cfg, &egui::Context::default());
        assert_eq!(with(|st| st.in_flight.len()), 2, "both pages are marked as being read at once");

        let mut got: HashMap<String, Result<Read, String>> = HashMap::new();
        let t0 = std::time::Instant::now();
        while got.len() < 2 && t0.elapsed() < std::time::Duration::from_secs(20) {
            let msgs: Vec<Msg> = with(|st| st.chan.as_ref().unwrap().1.try_iter().collect());
            for m in msgs {
                if let Msg::Read(id, r) = m {
                    got.insert(id, r);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let first = got.remove(&p1).expect("page 1 came back").expect("page 1 read");
        assert_eq!(first.text, "1. B\n2. x = 4");
        assert_eq!(first.uncertain, vec![("B".to_string(), "8".to_string(), 0.3)]);
        let second = got.remove(&p2).expect("page 2 came back");
        assert!(second.unwrap_err().contains("No handwriting"));
        with(|st| st.in_flight.clear());
    }

    #[test]
    fn the_prompt_asks_for_json_and_for_no_correcting() {
        let p = prompt();
        assert!(p.contains("JSON") && p.contains("[illegible]") && p.contains("Do not correct"));
    }
}
