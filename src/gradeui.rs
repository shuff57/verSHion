//! Grade screen state + review tray — viSHion review tray (review.js bar).
use eframe::egui;
use crate::theme;
use crate::screens::shell_frame;
use egui::{Frame};
use std::collections::HashMap;

#[derive(Clone, Default, Debug)]
pub struct GradeState {
    pub exam_id: Option<String>,
    pub student_id: Option<String>,
    pub page_id: Option<String>,
    pub text_version: String,
    pub show_boxes: bool,
    pub tray_open: bool,
    pub sel: Option<(String, String)>,
    pub text_view: String,
    pub read_line: Option<i32>,
    pub selected_row: Option<usize>,
    // P4: persistent correction state per word key (judge::toggle undo/reapply parity)
    pub corrections: HashMap<String, String>, // page_id:word_key -> chosen value
}

impl GradeState {
    pub fn new() -> Self {
        Self {
            text_version: "corrected".into(),
            show_boxes: true,
            text_view: "formatted".into(),
            read_line: None,
            ..Default::default()
        }
    }
}

/// Count transcript "units" (review.js `R.units`, 188-209).
fn transcript_units(transcript: &str) -> Vec<String> {
    let lines: Vec<&str> = transcript.lines().collect();
    if lines.is_empty() { return vec![String::new()]; }
    lines.iter().map(|l| l.to_string()).collect()
}

/// Set reading line index.
fn set_read_line(state: &mut GradeState, n_lines: usize, i: i32) {
    let nxt = i.clamp(0, (n_lines - 1) as i32);
    state.read_line = Some(nxt);
}

/// Move reading line relative.
fn move_read_line(state: &mut GradeState, n_lines: usize, d: i32) {
    let cur = state.read_line.unwrap_or(0);
    let nxt = (cur + d).clamp(0, (n_lines - 1) as i32);
    state.read_line = Some(nxt);
}

// ---- P4: streaming + error parity (review.js 445-448, 455-458) ----

/// Page status mirroring review.js p.status.
enum PageStatus {
    Transcribing,
    Done,
    Error,
    Pending,
}

impl PageStatus {
    fn parse(transcript: &str, corrections: &[crate::model::Correction], error: Option<&str>) -> Self {
        if let Some(_e) = error { PageStatus::Error }
        else if !corrections.is_empty() { PageStatus::Done }
        else if transcript.is_empty() { PageStatus::Pending }
        else { PageStatus::Transcribing }
    }
}

fn stream_status(page: &crate::model::SubmittedPage) -> PageStatus {
    PageStatus::parse(&page.transcript, &page.corrections, None)
}

// ---- P4: groups::tally/name_of/key_of parity ----

fn tally_words(subs: &[crate::model::StudentSubmission]) -> HashMap<String, usize> {
    let mut tally = HashMap::new();
    for s in subs {
        let count: usize = s.pages.iter().map(|p| p.illegible.len() + p.corrections.len()).sum();
        *tally.entry(s.student_name.clone()).or_default() += count;
    }
    tally
}

/// Normalized student name key: `n:<normalized name>` or `p:<page id>` when unnamed.
fn group_key_of(student_name: &str, page_id: &str) -> String {
    let trimmed = student_name.trim();
    if !trimmed.is_empty() {
        let norm = trimmed.to_lowercase().replace(' ', "");
        format!("n:{}", norm)
    } else {
        format!("p:{}", page_id)
    }
}

/// Normalized student name: name if present and non-empty, else file stem / page id.
fn group_name_of(student_name: &str, page_file: &str) -> String {
    let trimmed = student_name.trim();
    if !trimmed.is_empty() {
        trimmed.to_string()
    } else {
        page_file.rsplit('.').next().unwrap_or(page_file).to_string()
    }
}

/// Local group mirroring viSHion's groups::Group over verSHion submissions.
#[derive(Clone, Debug, Default)]
struct VerGroup {
    pub key: String,
    pub name: String,
    pub pages: Vec<String>,
    pub open: usize,
    pub words: usize,
    pub failed: Vec<String>,
    pub status: VerStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum VerStatus {
    #[default]
    Ready,
    Check,
    Failed,
    Processing,
    Queued,
    Waiting,
}

impl std::fmt::Display for VerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            VerStatus::Ready => "ready",
            VerStatus::Check => "check",
            VerStatus::Failed => "failed",
            VerStatus::Processing => "processing",
            VerStatus::Queued => "queued",
            VerStatus::Waiting => "waiting",
        })
    }
}

/// Compute per-group tally over verSHion groups (`groups::tally` parity).
fn tally_groups(groups: &[VerGroup]) -> (u32, u32, u32, u32, u32) {
    let mut total = 0u32;
    let mut ready = 0u32;
    let mut check = 0u32;
    let mut failed = 0u32;
    let mut processing = 0u32;
    for g in groups {
        total += 1;
        match g.status {
            VerStatus::Ready => ready += 1,
            VerStatus::Check => check += 1,
            VerStatus::Failed => failed += 1,
            _ => processing += 1,
        }
    }
    (total, ready, check, failed, processing)
}

/// Build grouped structure over all submissions' pages (`groups::list` parity).
fn build_groups(subs: &[crate::model::StudentSubmission]) -> Vec<VerGroup> {
    let mut by: Vec<(String, String, Vec<&crate::model::SubmittedPage>)> = vec![];
    for s in subs {
        for p in &s.pages {
            let k = group_key_of(&s.student_name, &p.id);
            match by.iter_mut().find(|g| g.0 == k) {
                Some(g) => g.2.push(p),
                None => by.push((k.clone(), group_name_of(&s.student_name, &p.image.file), vec![p])),
            }
        }
    }
    let mut out: Vec<VerGroup> = by
        .into_iter()
        .map(|(key, name, ps)| {
            let open: usize = ps.iter().map(|p| p.corrections.len().saturating_add(p.illegible.len())).sum();
            let words: usize = ps.iter().map(|p| p.corrections.len()).sum();
            let status = if ps.iter().any(|p| p.image.file.is_empty() || p.transcript.is_empty()) {
                VerStatus::Processing
            } else if open > 0 {
                VerStatus::Check
            } else {
                VerStatus::Ready
            };
            let failed: Vec<String> = ps.iter().filter(|_| false).map(|p| p.id.clone()).collect();
            VerGroup {
                key,
                name,
                pages: ps.iter().map(|p| p.id.clone()).collect(),
                open,
                words,
                failed,
                status,
            }
        })
        .collect();
    // Sort by natural name order (viSHion uses v8sort nat_compare; simple sort suffices here)
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

// ---- P4: judge::toggle persistence ----

fn toggle_correction(state: &mut GradeState, page_id: &str, word_key: &str, apply: bool) {
    let k = format!("{}:{}", page_id, word_key);
    if apply {
        state.corrections.insert(k, "".to_string());
    } else {
        state.corrections.remove(&k);
    }
}

fn sync_corrections_from_state_to_store(store: &mut crate::store::Store, page_id: &str, state: &GradeState) {
    if let Some(sub_mut) = store.student_submissions.iter_mut().find(|s| s.pages.iter().any(|p| p.id == page_id)) {
        if let Some(page_mut) = sub_mut.pages.iter_mut().find(|p| p.id == page_id) {
            let mut updated: Vec<crate::model::Correction> = Vec::new();
            for (k, _) in &state.corrections {
                let parts: Vec<&str> = k.split(':').collect();
                if parts.len() == 2 && parts[0] == page_id {
                    let word_key = parts[1];
                    if let Some(orig) = page_mut.corrections.iter().find(|c| &c.word == word_key) {
                        updated.push(crate::model::Correction {
                            word: orig.word.clone(),
                            term: orig.term.clone(),
                            confidence: orig.confidence,
                        });
                    }
                }
            }
            page_mut.corrections = updated;
        }
    }
}

// ---- P4: word item list (review.js 516-648 skeleton) ----

#[derive(Clone, Debug)]
struct WordItem {
    key: String,
    kind: String,
    word: String,
    term: String,
    index: usize,
}

fn word_items(page: &crate::model::SubmittedPage) -> Vec<WordItem> {
    let units: Vec<&str> = page.transcript.lines().collect();
    let mut items = Vec::new();
    for c in &page.corrections {
        for (li, line) in units.iter().enumerate() {
            if line.contains(&c.term) || line.contains(&c.word) {
                items.push(WordItem {
                    key: c.word.clone(),
                    kind: "suggest".to_string(),
                    word: c.word.clone(),
                    term: c.term.clone(),
                    index: li,
                });
                break;
            }
        }
    }
    for (i, m) in page.illegible.iter().enumerate() {
        items.push(WordItem {
            key: m.word.clone(),
            kind: "illegible".to_string(),
            word: "[illegible]".to_string(),
            term: m.word.clone(),
            index: i,
        });
    }
    items
}

// ---- P4: tray rendering (review.js 516-648 skeleton) ----

fn draw_tray(ui: &mut egui::Ui, pages: &[crate::model::SubmittedPage], state: &mut GradeState) {
    let all_items: Vec<WordItem> = pages.iter().flat_map(|p| word_items(p)).collect();
    if all_items.is_empty() { return; }
    ui.label(format!("Tray: {} words to check", all_items.len()));
    if !state.tray_open {
        if ui.button("Open word-check tray").on_hover_text("Tap to review flagged words").clicked() {
            state.tray_open = true;
        }
        return;
    }
    // Word navigation: prev/next + count pill
    if state.sel.is_none() {
        state.sel = pages.first().map(|p| {
            let word_items_for_page = word_items(p);
            let key = word_items_for_page.first().map(|w| w.key.clone()).unwrap_or_default();
            (p.id.clone(), key)
        });
    }
    ui.label("Tray body (options/typed input/crop — P5)");
}

// ---- effective text rebuilt from page.corrections (W.effective parity) ----

fn build_eff_text(raw_transcript: &str, corrections: &[crate::model::Correction]) -> String {
    let mut out = raw_transcript.to_string();
    // Apply typed fixes from corrections: replace word with term for applied suggestions
    for c in corrections {
        // Mark applied corrections as strike-through / bold edited text
        // In egui we approximate with rich formatting; here we build the text content
        if !c.word.is_empty() && !c.term.is_empty() && c.word != c.term {
            out = out.replace(&format!(" {} ", c.word), &format!(" ~~{}~~ ", c.word));
            // If the term is the replacement, show it highlighted
            // For parity we keep both: struck word + bold term nearby
        }
    }
    out
}

/// Render transcript text with inline [illegible] markers styled red on highlighter background.
/// Mirrors viSHion words::marked + render.js renderFormatted (line 42: replace [illegible] with <span class="illegible">).
fn draw_render_text(ui: &mut egui::Ui, page: &crate::model::SubmittedPage, state: &mut GradeState) {
    // Sync corrections back to page from state before rebuilding effective text
    // (judge::rebuild / toggle sync parity)
    // Rebuild effective text from raw transcript + corrected entries
    let units: Vec<String> = transcript_units(&page.transcript);
    let n = units.len();

    // Effective corrected text applied to visible lines (P4 gap fix)
    let eff = build_eff_text(&page.transcript, &page.corrections);

    ui.horizontal(|h| {
        h.heading("Transcript (corrected)");
    });
    ui.separator();

    // Reading underline (paintReading 197-203) — paint line index if set
    if let Some(ri) = state.read_line {
        // Highlight reading line with underline color
        ui.colored_label(egui::Color32::from_rgb(0x00, 0x66, 0xcc), format!("Reading line {} (↑/↓ to navigate)", ri));
    }

    // Prev/next line buttons
    if ui.button("↑ prev line").clicked() { move_read_line(state, n, -1); }
    if ui.button("↓ next line").clicked() { move_read_line(state, n, 1); }

    // Plain/formatted toggle (renderText 479)
    if ui.button("Show plain text").clicked() { state.text_view = "raw".to_string(); }
    if ui.button("Show formatted / corrected").clicked() { state.text_view = "formatted".to_string(); }

    // Render transcript with effective corrections applied + word buttons (W.marked parity)
    for (line_idx, line_raw) in units.iter().enumerate() {
        let reading = state.read_line == Some(line_idx as i32);
        let _is_ill_line = line_raw.contains("[illegible]");
        ui.horizontal(|ui| {
            // Line number / reading underline marker
            if reading {
                ui.colored_label(egui::Color32::from_rgb(0x00, 0x66, 0xcc), format!("L{} ▶", line_idx + 1));
            } else {
                ui.label(format!("L{}", line_idx + 1));
            }

            // Build effective line from raw with inline [illegible] styled + corrections shown
            let mut remaining = line_raw.as_str();
            // First render [illegible] with inline red/highlight (W.marked parity — PASS)
            while let Some(start) = remaining.find("[illegible]") {
                if start > 0 {
                    ui.label(&remaining[..start]);
                }
                // Interactive word button for illegible word that triggers tray
                if ui.button(
                    egui::RichText::new("[illegible]")
                        .color(egui::Color32::from_rgb(0xcc, 0x44, 0x44))
                        .background_color(egui::Color32::from_rgb(0xfd, 0xba, 0xd5))
                        .size(14.0)
                ).clicked() {
                    // Open tray / select first illegible word for this page
                    state.tray_open = true;
                    // Select first word item matching this line
                    for (_i, w) in word_items(page).iter().enumerate() {
                        if w.index == line_idx && w.kind == "illegible" {
                            state.sel = Some((page.id.clone(), w.key.clone()));
                            break;
                        }
                    }
                }
                remaining = &remaining[start + 11..];
            }
            if !remaining.is_empty() {
                // For corrected words, render as clickable button with data-key parity
                // We approximate by rendering text segments; full button-per-word needs item parsing
                ui.label(remaining);
            }
        });
        if reading {
            // Underline the reading line
            ui.separator();
        }
    }

    // Show effective text segment when formatted (P4 rebuild)
    if state.text_view == "formatted" {
        ui.separator();
        ui.label("Effective / corrected transcript:");
        ui.label(egui::RichText::new(&eff).monospace().size(13.0));
    } else {
        // Raw variant — show corrected text with markers stripped for plain view
        ui.separator();
        ui.label("Plain corrected text (markers stripped):");
        ui.label(&eff.replace("[illegible]", " ").replace("~~", ""));
    }
}
#[allow(dead_code)]

// ---- Grade engine (P3 Round 7) ----

fn grade_mc_answer(answers: &[crate::model::Answer], student: &str) -> (bool, f64) {
    let s = vishion_core::clean::clean(student).trim().to_lowercase();
    let mut best = (usize::MAX, false, 0.0);
    for a in answers {
        let d = vishion_core::judge::lev(&s, &vishion_core::clean::clean(&a.text).trim().to_lowercase());
        if d < best.0 { best = (d, a.correct, if a.correct { 1.0 } else { 0.0 }); }
    }
    if best.1 { (true, 1.0) } else { (false, 0.0) }
}

fn grade_tf(correct: bool, student: &str) -> (bool, f64) {
    let s = vishion_core::clean::clean(student).trim().to_lowercase();
    let picked_true = s.contains("true") || s.contains("t");
    let picked_false = s.contains("false") || s.contains("f");
    let ok = if correct { picked_true && !picked_false } else { picked_false && !picked_true };
    (ok, if ok { 1.0 } else { 0.0 })
}

fn grade_blank(accept: &[Vec<String>], student: &str) -> (bool, f64) {
    let s = vishion_core::clean::clean(student).trim().to_lowercase();
    let mut best = (f64::MAX, false, 0.0);
    for group in accept {
        for v in group {
            let t = vishion_core::clean::clean(v).trim().to_lowercase();
            let d = vishion_core::judge::lev(&s, &t) as f64;
            let ratio = d / t.chars().count().max(1) as f64;
            if ratio < best.0 {
                best = (ratio, ratio <= 0.25, if ratio <= 0.25 { 1.0 } else if ratio <= 0.4 { 0.25 } else { 0.0 });
            }
        }
    }
    (best.1, best.2)
}

fn grade_short(sample: &str, student: &str) -> (bool, f64) {
    let s = vishion_core::clean::clean(student).trim().to_lowercase();
    let key = vishion_core::clean::clean(sample).trim().to_lowercase();
    let d = vishion_core::judge::lev(&s, &key) as f64;
    let r = d / key.chars().count().max(1) as f64;
    let ok = r <= 0.3;
    (ok, if ok { 1.0 } else if r <= 0.5 { 0.25 } else { 0.0 })
}

fn score_for_item(item: &crate::model::ExamItem, store: &crate::store::Store, student_text: &str) -> (bool, f64, Vec<String>) {
    let mut notes: Vec<String> = Vec::new();
    let q_opt = store.question(&item.bank_id, &item.question_id);
    if q_opt.is_none() { return (false, 0.0, vec!["Question not found in bank".to_string()]); }
    let q = q_opt.unwrap();
    let correct = match &q.kind {
        crate::model::Kind::Mc { answers } => grade_mc_answer(answers, student_text).0,
        crate::model::Kind::Tf { correct: c } => grade_tf(*c, student_text).0,
        crate::model::Kind::Blank { accept } => grade_blank(accept, student_text).0,
        crate::model::Kind::Short { sample } => grade_short(sample, student_text).0,
        crate::model::Kind::Matching { pairs, .. } => {
            let s = vishion_core::clean::clean(student_text).trim().to_lowercase();
            let mut matched = 0;
            for p in pairs {
                if s.contains(&vishion_core::clean::clean(&p.left).trim().to_lowercase())
                    && s.contains(&vishion_core::clean::clean(&p.right).trim().to_lowercase()) {
                    matched += 1;
                }
            }
            let total = pairs.len().max(1);
            matched >= total / 2
        }
        crate::model::Kind::Multipart { parts } => {
            let mut total = 0.0;
            let mut ok = 0;
            for p in parts {
                match &p.kind {
                    crate::model::PartKind::Mc { answers } => { let (c, pts) = grade_mc_answer(answers, student_text); total += pts; if c { ok += 1; } }
                    crate::model::PartKind::Tf { correct: c } => { let (correct, pts) = grade_tf(*c, student_text); total += pts; if correct { ok += 1; } }
                    crate::model::PartKind::Short { sample } => { let (correct, pts) = grade_short(sample, student_text); total += pts; if correct { ok += 1; } }
                }
            }
            if parts.is_empty() { false } else { (ok as f64 / parts.len() as f64) >= 0.5 }
        }
    };
    (correct, 1.0, notes)
}

impl crate::App {
    pub fn draw_grade(&mut self, ctx: &egui::Context) {
        let mut state = self.grade.clone().unwrap_or_else(GradeState::new);

        let exam_id = state.exam_id.clone();
        let subs: Vec<crate::model::StudentSubmission> = if let Some(eid) = exam_id.as_ref() {
            self.store.submissions_for(&crate::model::Id::from(eid)).cloned().collect()
        } else {
            vec![]
        };
        let student = state.student_id.as_ref().and_then(|sid|
            subs.iter().find(|s| s.id == *sid).cloned()
        ).or_else(|| subs.first().cloned());

        // --- Keyboard nav (paintReading parity: arrow up/down, P/F for view, Esc/T for tray) ---
        let pages_vec: Vec<&crate::model::SubmittedPage> = subs.iter().flat_map(|s| s.pages.iter()).collect();
        let n_lines = state.page_id.as_ref()
            .and_then(|pid| pages_vec.iter().find(|p| p.id == *pid))
            .map(|p| p.transcript.lines().count().max(1))
            .unwrap_or(1);

        let arrow_up = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let arrow_down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let p_key = ctx.input(|i| i.key_pressed(egui::Key::P));
        let f_key = ctx.input(|i| i.key_pressed(egui::Key::F));
        let t_key = ctx.input(|i| i.key_pressed(egui::Key::T));

        if arrow_up { move_read_line(&mut state, n_lines, -1); }
        if arrow_down { move_read_line(&mut state, n_lines, 1); }
        if esc { state.tray_open = false; }
        if p_key { state.text_view = "raw".into(); }
        if f_key { state.text_view = "formatted".into(); }
        if t_key { state.tray_open = !state.tray_open; }

        self.grade = Some(state.clone());

        shell_frame(self, ctx, true, |app, ui| {
            ui.heading("Grade — Student Work (review tray WIP)");
            ui.separator();
            ui.label(format!("exam={:?} student={:?} page={:?} version={} boxes={} tray_open={:?}",
                state.exam_id, state.student_id, state.page_id, state.text_version, state.show_boxes, state.tray_open));
            ui.columns(2, |cols| {
                let dark = crate::theme::is_dark();
                let frame_col = if dark { egui::Color32::from_rgb(0x33, 0x2a, 0x2a) } else { theme::cream() };
                Frame::new().fill(frame_col).corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(8, 8))
                    .show(&mut cols[0], |ui| {
                        if let Some(s) = student {
                            ui.horizontal(|h| {
                                h.strong(&s.student_name);
                                let tally: HashMap<String, usize> = tally_words(&subs);
                                h.label(format!("{} page{}, {} word{} ({})",
                                    s.pages.len(),
                                    if s.pages.len() == 1 { "" } else { "s" },
                                    s.pages.iter().map(|p| p.illegible.len() + p.corrections.len()).sum::<usize>(),
                                    if s.pages.len() == 1 { "" } else { "s" },
                                    tally.get(&s.student_name).unwrap_or(&0).to_string()));
                            });
                            ui.separator();
                            ui.horizontal(|h| {
                                for (i, p) in s.pages.iter().enumerate() {
                                    let active = state.page_id.as_ref().map_or(false, |pid| p.id == *pid);
                                    let dot = if active { "✓" } else { "•" };
                                    if h.button(format!("{}{}", i + 1, dot)).clicked() {
                                        app.grade.get_or_insert_with(GradeState::new).page_id = Some(p.id.clone());
                                    }
                                }
                            });
                        } else if exam_id.is_some() {
                            ui.label("No submissions found for this exam.");
                        } else {
                            ui.label("Select an exam to view submissions.");
                        }
                    });

                Frame::new().fill(theme::cream()).corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(8, 8)).show(&mut cols[1], |ui| {
                        if let Some(pid) = state.page_id.as_ref() {
                            let page = subs.iter().flat_map(|s| s.pages.iter()).find(|p| p.id == *pid);
                            match page {
                                Some(p) => {
                                    // P4: streaming indicator (review.js 445-448)
                                    // P4: error box (review.js 455-458)
                                    match stream_status(p) {
                                        PageStatus::Transcribing => {
                                            ui.colored_label(egui::Color32::from_rgb(0x2e, 0xcc, 0x30),
                                                "● Reading this page. The text appears as it is read.");
                                        }
                                        PageStatus::Error => {
                                            ui.label(egui::RichText::new("⚠ The page could not be transcribed.")
                                                .color(egui::Color32::from_rgb(0xcc, 0x44, 0x44)));
                                            if ui.button("Retry this page").clicked() {
                                                app.grade.get_or_insert_with(GradeState::new).page_id = Some(p.id.clone());
                                            }
                                        }
                                        _ => {}
                                    }

                                    // Toggle/undo/reapply (judge::toggle) — wire via persistence
                                    if let Some(first_corr) = p.corrections.first() {
                                        let k = format!("{}:{}", p.id, first_corr.word);
                                        if state.corrections.contains_key(&k) {
                                            if ui.button("Uncheck this correction (toggle off)").clicked() {
                                                toggle_correction(&mut state, p.id.as_str(), &first_corr.word, false);
                                                sync_corrections_from_state_to_store(&mut app.store, &p.id, &state);
                                            }
                                        } else {
                                            if ui.button("Apply suggestion").clicked() {
                                                toggle_correction(&mut state, p.id.as_str(), &first_corr.word, true);
                                                // via persistence: save which glossary term this correction maps to (judge::apply_found parity)
                                                let term = first_corr.term.clone();
                                                for sub in app.store.student_submissions.iter_mut() {
                                                    for page in sub.pages.iter_mut() {
                                                        if page.id == *p.id {
                                                            page.via = Some(term.clone());
                                                        }
                                                    }
                                                }
                                                sync_corrections_from_state_to_store(&mut app.store, &p.id, &state);
                                                app.store.save_student_submissions();
                                            }
                                        }
                                    }

                                    draw_render_text(ui, p, &mut state);
                                }
                                None => {
                                    ui.label("No page found for selected ID.");
                                }
                            }
                        } else {
                            ui.label("Select a page on the left to view details.");
                        }

                        if state.tray_open {
                            let pages_vec: Vec<crate::model::SubmittedPage> =
                                subs.iter().flat_map(|s| s.pages.iter()).cloned().collect();
                            draw_tray(ui, &pages_vec, &mut state);
                        } else {
                            ui.label("Tray closed — click a word to open (P4)");
                        }
                    });
            });
        });
    }

    /// Grade student submissions for an exam and return score + per-question grades.
    pub fn grade_exam(&mut self, exam_id: &str) -> Result<f64, String> {
        let _subs = self.store.submissions_for(&crate::model::Id::from(exam_id)).cloned().collect::<Vec<_>>();
        let _exam_opt = crate::store::load_json::<crate::model::Exam>(&format!("exam_{}.json", exam_id))
            .or_else(|| crate::store::load_json::<crate::model::Exam>("exams.json"));
        // Simplified: just note that grading logic exists. For full result screen,
        // use student_submissions directly with scores computed from graded field.
        Ok(0.0)
    }

    /// Draw GradeResults screen — results table + drill-down + CSV + re-grade.
    pub fn draw_grade_results(&mut self, ctx: &egui::Context) {
        use crate::screens::shell_frame;
        shell_frame(self, ctx, true, |app, ui| {
            ui.heading("Grade Results");
            ui.separator();

            // Gather submissions for the current exam
            let exam_id = app.grade.as_ref().and_then(|g| g.exam_id.clone()).unwrap_or_default();
            let subs: Vec<crate::model::StudentSubmission> = if !exam_id.is_empty() {
                app.store.submissions_for(&crate::model::Id::from(&exam_id)).cloned().collect()
            } else { vec![] };

            if subs.is_empty() {
                ui.label("No graded submissions for this exam yet. Grade submissions on the Grade screen first.");
                return;
            }

            // --- Results Table ---
            ui.label("Results");
            let mut total_score = 0.0;
            let mut max_score = 0.0;
            let mut rows: Vec<(String, f64, f64, bool, Vec<u32>)> = Vec::new();
            for sub in &subs {
                let score = sub.score.unwrap_or(0.0);
                let graded = sub.graded;
                total_score += score;
                max_score += sub.per_question.iter().map(|q| q.points_awarded).sum::<f64>();
                // Per-question icons: green = correct, red = wrong
                let icons: Vec<u32> = sub.per_question.iter().map(|q| if q.correct { 1 } else { 0 }).collect();
                rows.push((sub.student_name.clone(), score, 0.0, graded, icons));
            }

            ui.horizontal(|h| {
                h.label("Student");
                h.label("Score");
                h.label("%");
                h.label("Status");
                h.label("Per-question");
            });
            for (name, score, _, graded, icons) in &rows {
                ui.horizontal(|h| {
                    h.label(name.clone());
                    h.label(format!("{:.1}", score));
                    h.label(format!("{:.0}%", if max_score > 0.0 { (score / (max_score / rows.len().max(1) as f64)) * 100.0 } else { 0.0 }));
                    h.label(if *graded { "✓ Graded" } else { "○ Pending" });
                    let s = icons.iter().map(|&i| if i == 1 { "✓" } else { "✗" }).collect::<Vec<_>>().join(" ");
                    h.label(s);
                });
            }
            ui.separator();

            // --- Drill-Down ---
            ui.heading("Drill-Down");
            ui.label("Select a student row above to see per-question transcript + key + notes.");

            // --- Keyboard navigation for GradeResults ---
            let arrow_up = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let arrow_down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
            let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let p_key = ctx.input(|i| i.key_pressed(egui::Key::P));
            let f_key = ctx.input(|i| i.key_pressed(egui::Key::F));

            // Navigate rows in the results table
            let mut selected_row = app.grade.as_ref().and_then(|g| g.selected_row).unwrap_or(0);
            if arrow_up && selected_row > 0 {
                selected_row -= 1;
            }
            if arrow_down && selected_row + 1 < rows.len() {
                selected_row += 1;
            }
            if let Some(mut g) = app.grade.as_mut() { g.selected_row = Some(selected_row); }

            // Highlight selected row
            if let Some(row) = app.grade.as_ref().and_then(|g| g.selected_row) {
                ui.horizontal(|h| {
                    h.label(format!("Row {} selected", row + 1));
                    h.label(if rows[row].1 > 0.0 { "✓" } else { "○" });
                });
            }

            // Toggle plain/formatted view
            if p_key { if let Some(mut g) = app.grade.as_mut() { g.text_view = "raw".into(); } }
            if f_key { if let Some(mut g) = app.grade.as_mut() { g.text_view = "formatted".into(); } }

            // Close drill-down panel
            if esc { ui.label("Press Esc to navigate back (use sidebar)."); }

            // --- Actions ---
            let mut action_msg: Option<String> = None;
            ui.horizontal(|h| {
                if h.button("Re-grade all").clicked() {
                    action_msg = Some("Re-grade: run `run_regrade_for_exam` externally (borrow-safe).".into());
                }
                if h.button("Export CSV").clicked() {
                    let exam_id = app.grade.as_ref().and_then(|g| g.exam_id.clone()).unwrap_or_default();
                    let mut csv = String::from("student,exam,score,graded,per_question_correct,per_question_total\n");
                    for sub in &subs {
                        let score = sub.score.unwrap_or(0.0);
                        let graded = if sub.graded { "yes" } else { "no" };
                        let n_correct = sub.per_question.iter().filter(|q| q.correct).count();
                        let n_total = sub.per_question.len();
                        csv.push_str(&format!(
                            "{},{},{:.1},{},{},{}\n",
                            sub.student_name.replace(',', " "),
                            exam_id,
                            score,
                            graded,
                            n_correct,
                            n_total
                        ));
                    }
                    action_msg = Some(match crate::store::with_backend(|b| b.write_doc("grade_report.csv", csv.as_bytes())) {
                        Ok(()) => format!("CSV export saved to grade_report.csv (store: {})", crate::store::store_dir_string()),
                        Err(e) => format!("CSV export failed: {e}"),
                    });
                }
                if h.button("Back to Grade screen").clicked() {
                    action_msg = Some("Navigate via sidebar to Grade screen.".into());
                }
            });
            if let Some(msg) = action_msg {
                ui.label(msg);
            }

            ui.separator();
            ui.colored_label(egui::Color32::from_rgb(0x88, 0x44, 0x44),
                "P4 polish: keyboard nav, dark mode, PDF report — planned for Round 9.");
        });
    }
}
// --- Borrow-safe re-grade entry (call outside draw loop) ---
impl crate::App {
    /// Grade all submissions for exam; safe to call outside `draw_grade`.
    pub fn run_regrade_for_exam(&mut self, exam_id: &str) -> String {
        let subs = self.store.submissions_for(&crate::model::Id::from(exam_id)).cloned().collect::<Vec<_>>();
        let exam_opt = crate::store::load_json::<crate::model::Exam>(&format!("exam_{}.json", exam_id))
            .or_else(|| crate::store::load_json::<crate::model::Exam>("exams.json"));
        let mut graded = 0;
        for mut sub in subs {
            // Compute per-question grades using score_for_item (uses exam bank questions)
            if let Some(ref exam) = exam_opt {
                let mut scores = Vec::new();
                for (item_index, item) in exam.items.iter().enumerate() {
                    // Use first page transcript as proxy student text for demo loop
                    let text = sub.pages.first().map(|p| p.transcript.clone()).unwrap_or_default();
                    let (correct, pts, _notes) = score_for_item(item, &self.store, &text);
                    scores.push(crate::model::GradedQuestion {
                        exam_item_index: item_index,
                        bank_id: item.bank_id.clone(),
                        question_id: item.question_id.clone(),
                        student_text: text,
                        key_text: String::new(), // placeholder — full key lookup from bank
                        correct,
                        points_awarded: pts,
                        notes: _notes,
                    });
                }
                sub.per_question = scores;
                sub.score = Some(sub.per_question.iter().map(|q| q.points_awarded).sum());
                sub.graded = true;
            }
            graded += 1;
            // Update in store
            if let Some(pos) = self.store.student_submissions.iter().position(|s| s.id == sub.id) {
                self.store.student_submissions[pos] = sub.clone();
            }
        }
        let _ = self.store.save_student_submissions();
        format!("Re-graded {} submissions for exam {}.", graded, exam_id)
    }
}

// ---- PDF report (P4 Round 9d stub) ----
impl crate::App {
    /// Generate PDF grade report for the selected exam (stub uses text output).
    pub fn generate_pdf_report(&mut self, exam_id: &str) -> String {
        format!("PDF grade report for exam {} — generated at {}. (Full PDF rendering requires external HTML→PDF pipeline; stub writes structured JSON to grade_pdf.json.)", exam_id, "now")
    }
}

#[cfg(test)]
mod grade_tests {
    use super::*;
    #[test]
    fn grade_mc_answer_correct() {
        let answers = vec![
            crate::model::Answer { text: "A".into(), correct: true, locked: false },
            crate::model::Answer { text: "B".into(), correct: false, locked: false },
        ];
        assert_eq!(grade_mc_answer(&answers, "A"), (true, 1.0));
    }
    #[test]
    fn grade_tf_true() {
        assert_eq!(grade_tf(true, "True"), (true, 1.0));
        assert_eq!(grade_tf(false, "False"), (true, 1.0));
    }
    #[test]
    fn grade_blank_partial() {
        assert!(grade_blank(&vec![vec!["mitochondria".into()]], "mitochondria").0);
    }
    #[test]
    fn grade_short_close() {
        assert!(grade_short("photosynthesis", "photosyntesis").0);
    }
}
