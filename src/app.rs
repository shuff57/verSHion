//! Test Parrot clone — app entry and shared shell state. Screen bodies live in the screen
//! modules (`screens`, `bankui`/`qedit`, `examui`). Persistence is `store`; AI is `ai`.
//!
//! Shell-owned extra fields (added by the shell worker, per NOTES contract extension —
//! not serialized anywhere, so no `#[serde]` applies to `App`):
//! `toast_times` (toasts vec parallel timestamps), `help_open`, `confirm_restore`,
//! `ai_busy`, `import_file`, `ai_hint`.

use egui;
use crate::{ai, figuresui, model, qedit, screens, store, theme};
use crate::model::{ExamItem, Id};
use crate::store::Store;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ai_provider: String, // "ollama" | "openrouter" | "custom"
    #[serde(default)]
    pub ai_model: String,
    #[serde(default)]
    pub ai_base: String, // custom OpenAI-compatible base URL
    /// A self-hosted Ollama's URL (empty: this machine, 127.0.0.1:11434).
    #[serde(default)]
    pub ollama_base: String,
    /// Models "Find models" kept for `ai_models_for` ("provider|base"): the ones this account can use
    /// to read a picture. Stored so the picker keeps them across restarts.
    #[serde(default)]
    pub ai_models_ok: Vec<String>,
    #[serde(default)]
    pub ai_models_for: String,
    #[serde(default)]
    pub dark: bool, // dark palette; applied via theme::set_dark
    /// Home screen view modes (false = Cards, true = List).
    #[serde(default)]
    pub home_exams_list: bool,
    #[serde(default)]
    pub home_banks_list: bool,
    /// Banks list / Bank editor view modes (false = Cards, true = List), bank worker.
    #[serde(default)]
    pub banks_list_view: bool,
    #[serde(default)]
    pub bank_edit_list_view: bool,
}

#[derive(Clone)]
pub enum Screen {
    Welcome,
    Home,
    Banks,
    Exams,
    Imports,
    Figures,
    Settings,
    Shared,
    /// Sign in, create an account, confirm, reset, status: the account behind sharing and hosted AI.
    Account,
    /// Uploaded student work, its transcript, the word-check tray, and the marks.
    Grade,
    BankEdit(Id),
    ExamEdit(Id),
}

/// What Ctrl/Cmd+N creates on the current screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    NewBank,
    NewExam,
}

/// Ctrl/Cmd+N starts a new exam on Home/Exams and a new bank on Banks. Editors and an open
/// question draft keep the key for themselves.
pub fn shortcut_action(screen: &Screen, has_draft: bool) -> Option<Shortcut> {
    if has_draft {
        return None;
    }
    match screen {
        Screen::Home | Screen::Exams => Some(Shortcut::NewExam),
        Screen::Banks => Some(Shortcut::NewBank),
        _ => None,
    }
}

pub struct App {
    pub store: Store,
    pub screen: Screen,
    pub bank: Option<Id>,                // open bank editor
    pub exam: Option<Id>,                // open exam editor
    pub qedit: Option<qedit::Draft>,     // open question editor modal
    pub paste: String,
    pub ai: ai::Ai,
    pub storage_info: bool, // topbar ⓘ: "where your work is stored" popup
    /// Whether secondary viewports (the bank pop-over) are allowed. False when embedded in a
    /// host that owns the window.
    pub allow_viewports: bool,
    pub panel_bank: Option<Id>,          // bank tab open in the exam editor side panel
    pub export_open: bool,
    pub history_open: bool,
    pub toasts: Vec<(String, bool)>,     // (text, is_error)
    pub settings: Settings,
    pub textures: std::collections::HashMap<String, egui::TextureHandle>, // media cache, key = model::ImageRef.file
    pub undo: Vec<Vec<ExamItem>>,        // exam editor snapshots (push before each change)
    pub redo: Vec<Vec<ExamItem>>,
    // Shell-owned extensions:
    /// Per-toast wall-clock timestamps (ms), parallel to `toasts` (5s expiry).
    pub toast_times: Vec<i64>,
    /// "How it works" overlay open.
    pub help_open: bool,
    /// Backup path awaiting the replace-everything confirmation modal.
    pub confirm_restore: Option<std::path::PathBuf>,
    /// An AI conversion job is running (import dialog spinner).
    pub ai_busy: bool,
    /// Staged import file chosen in the dialog (name, mime, bytes).
    pub import_file: Option<screens::ImportFile>,
    /// Optional hint sent with the AI conversion.
    pub ai_hint: String,
    /// Name of the file currently staged for AI conversion (shown while busy).
    pub import_name: String,
    /// Figures screen + insert-figure modal state.
    pub figs: figuresui::FigUi,
    /// Shared-pool Account window (`share`). Closed until opened; until then it does no I/O.
    pub account: crate::share::AccountUi,
    /// Shared-pool "Share to pool…" window. Closed until a bank or exam asks for it (`share::publish_ui`).
    pub publish: crate::share::PublishUi,
    /// Shared-pool browser, import and update checks. Closed until asked for (`share::browse_ui`).
    pub browse: crate::share::BrowseUi,
    /// Shared-pool "My shared items" window. Closed until asked for (`share::mine_ui`).
    pub mine: crate::share::MineUi,
    /// Shared-pool "Shared" page (a left-nav screen hosting the browser, my items and the account).
    pub shared: crate::share::SharedUi,
    /// The Grade screen's own state (selected submission, tray, corrections), kept while the app runs.
    pub grade: Option<crate::gradeui::GradeState>,
    /// QA hook: scripted input (VERSHION_SCRIPT), see `raw_input_hook`.
    pub qa_steps: Vec<String>,
    /// QA hook: VERSHION_SHOT=<file.png> saves a screenshot of this window's own contents and exits.
    pub qa_shot: Option<(String, u32)>,
    pub qa_frame: u64,
}

/// Shared constructor so the window path and the headless selftest path build the
/// same App. `ctx` is the egui context (theme/fonts setup happens here too).
impl App {
    /// Flip the palette and remember it (the sidebar switch calls this).
    pub fn set_dark_mode(&mut self, dark: bool, ctx: &egui::Context) {
        self.settings.dark = dark;
        theme::set_dark(dark, Some(ctx));
        let _ = store::save_json("settings.json", &self.settings);
    }

    /// Re-installs the app's fonts and palette after a host tab switch in a shared context.
    pub fn context_changed(&mut self, ctx: &egui::Context) {
        theme::setup(ctx);
        theme::set_dark(self.settings.dark, Some(ctx));
        ctx.request_repaint();
    }

    /// Opens a file from a path (native host: officecraft, command line).
    pub fn open_path(&mut self, path: &std::path::Path) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        match crate::platform::read(path) {
            Ok(bytes) => self.open_import_bytes(&name, bytes),
            Err(e) => self.toast_err(format!("Couldn't read {name}: {e}")),
        }
    }

    /// Opens a file already in memory (browser drop/picker, officecraft `deliver`).
    pub fn open_import_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        crate::importui::open_external(self, name, bytes);
    }

    pub fn new_from(ctx: &egui::Context) -> Self {
        theme::setup(ctx);
        let mut settings: Settings = store::load_json("settings.json").unwrap_or_default();
        if settings.ai_provider.trim().is_empty() {
            settings.ai_provider = "cloud".to_string();
        }
        if settings.ai_model.trim().is_empty() {
            settings.ai_model = "gemma4:31b".to_string();
        }
        theme::set_dark(settings.dark, Some(ctx));
        let app = App {
            store: Store::load(),
            screen: Screen::Welcome,
            bank: None,
            exam: None,
            qedit: None,
            paste: String::new(),
            ai: ai::Ai::default(),
            panel_bank: None,
            export_open: false,
            history_open: false,
            toasts: Vec::new(),
            settings,
            textures: Default::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            toast_times: Vec::new(),
            help_open: false,
            confirm_restore: None,
            ai_busy: false,
            storage_info: false,
            allow_viewports: true,
            import_file: None,
            ai_hint: String::new(),
            import_name: String::new(),
            figs: Default::default(),
            account: crate::share::AccountUi::in_page(),
            shared: Default::default(),
            publish: Default::default(),
            browse: Default::default(),
            mine: Default::default(),
            grade: None,
            qa_shot: crate::env_var("VERSHION_SHOT").ok().map(|p| (p, 0)),
            qa_steps: crate::env_var("VERSHION_SCRIPT")
                .map(|v| v.split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
                .unwrap_or_default(),
            qa_frame: 0,
        };
        let mut app = app; // non-mut unless the capture hook below needs it
        // Opened from a mailed link (/account/verify#token=..., reset, enrol): straight to it.
        #[cfg(target_arch = "wasm32")]
        if let Some(link) = crate::share::take_web_deep_link() {
            app.screen = Screen::Account;
            app.account.open_link(link);
        }
        // Screen-capture/test hook: VERSHION_SCREEN opens a given screen on startup.
        if let Ok(s) = crate::env_var("VERSHION_SCREEN") {
            app.screen = match s.trim() {
                "home" => Screen::Home,
                "banks" => Screen::Banks,
                "exams" => Screen::Exams,
                "imports" => Screen::Imports,
                "figures" => Screen::Figures,
                "grade" => Screen::Grade,
                // The Grade screen with work to review: "grade:N" picks the exam at index N, "grade:N:S"
                // opens the first page of its S-th submission, "grade:N:S:P" its page P; a last ":tray"
                // also opens the word tray.
                s if s.starts_with("grade:") => {
                    let mut parts = s["grade:".len()..].split(':');
                    let mut st = crate::gradeui::GradeState::new();
                    if let Some(e) = parts.next().and_then(|n| n.parse::<usize>().ok()).and_then(|i| app.store.exams.get(i)) {
                        st.exam_id = Some(e.id.clone());
                        let rest: Vec<&str> = parts.collect();
                        st.tray_open = rest.last() == Some(&"tray");
                        let num = |k: usize| rest.get(k).and_then(|n| n.parse::<usize>().ok());
                        if let Some(sub) = num(0).and_then(|i| app.store.submissions_for(&e.id).nth(i)) {
                            st.student_id = Some(sub.id.clone());
                            st.page_id = sub.pages.get(num(1).unwrap_or(0)).map(|p| p.id.clone());
                        }
                    }
                    app.grade = Some(st);
                    Screen::Grade
                }
                "settings" => Screen::Settings,
                // The Shared page, optionally on a tab: "shared", "shared:browse|mine|imported|reports|settings",
                // "shared:choose" (the "Share to pool…" chooser open). Circles: "shared:circles",
                // "shared:circles:join", "shared:circles:create", "shared:circle:<circle id>[:<bank id>]".
                // The approval queue, drawn as a signed-in admin's with no server: "shared:admin",
                // "shared:admin:approve|refuse" (the decision form for the first application),
                // "shared:admin:confirm" (an approval waiting for the password). The queue is read from
                // VERSHION_ADMIN_FIXTURE (a GET /admin/queue answer), else admin-queue.json in the store.
                s if s == "shared" || s.starts_with("shared:") => {
                    use crate::share::shared::Tab;
                    match s.trim_start_matches("shared").trim_start_matches(':') {
                        // The growth report, read from VERSHION_GROWTH_FIXTURE (a GET /admin/growth answer).
                        "admin:growth" => {
                            let growth = crate::env_var("VERSHION_GROWTH_FIXTURE")
                                .ok()
                                .and_then(|p| std::fs::read_to_string(&p).ok())
                                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                                .and_then(|v| crate::share::admin::parse_growth(&v));
                            match growth {
                                Some(g) => {
                                    app.shared.qa_admin(crate::share::admin::Queue::default(), "");
                                    app.shared.admin.qa_show_growth(g);
                                }
                                None => eprintln!("VERSHION_SCREEN=shared:admin:growth: no growth fixture could be read"),
                            }
                        }
                        a if a == "admin" || a.starts_with("admin:") => {
                            let queue = match crate::env_var("VERSHION_ADMIN_FIXTURE") {
                                Ok(p) => std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()),
                                Err(_) => crate::store::load_json::<serde_json::Value>("admin-queue.json"),
                            }
                            .and_then(|v| crate::share::admin::parse_queue(&v));
                            match queue {
                                Some(q) => app.shared.qa_admin(q, a.trim_start_matches("admin").trim_start_matches(':')),
                                None => eprintln!("VERSHION_SCREEN=shared:admin: no admin queue fixture could be read"),
                            }
                        }
                        "mine" => app.shared.model.goto(Tab::Mine),
                        "imported" => app.shared.model.goto(Tab::Imported),
                        "reports" => app.shared.model.goto(Tab::Reports),
                        "settings" => app.shared.model.goto(Tab::Settings),
                        "choose" => app.shared.model.choosing = true,
                        "circles" => app.shared.model.goto(Tab::Circles),
                        "circles:join" => {
                            app.shared.model.goto(Tab::Circles);
                            app.shared.circles.open_join("");
                        }
                        "circles:create" => {
                            app.shared.model.goto(Tab::Circles);
                            app.shared.circles.open_create();
                        }
                        c if c.starts_with("circle:") => {
                            app.shared.model.goto(Tab::Circles);
                            match c["circle:".len()..].split_once(':') {
                                Some((cid, bid)) => app.shared.circles.open_bank_at(cid, bid),
                                None => app.shared.circles.open_circle_at(&c["circle:".len()..]),
                            }
                        }
                        _ => app.shared.model.goto(Tab::Browse),
                    }
                    Screen::Shared
                }
                // The "Share to pool..." window for the first bank with "One of my circles" chosen
                // ("publish:circle"), or a given circle preselected ("publish:circle:<circle id>").
                s if s == "publish:circle" || s.starts_with("publish:circle:") => {
                    if let Some(b) = app.store.banks.first()
                        && let Ok(src) = crate::share::publish::ShareSource::from_bank(b)
                    {
                        crate::share::publish_ui::request_for_circle(ctx, src, s.trim_start_matches("publish:circle").trim_start_matches(':'));
                    }
                    Screen::Banks
                }
                // The "Share to pool..." window for the first bank, over the Banks page.
                "publish" => {
                    if let Some(b) = app.store.banks.first() {
                        let _ = crate::share::publish_ui::share_bank(ctx, b);
                    }
                    Screen::Banks
                }
                // The Account screen (QA entry).
                "account" => {
                    app.account.open();
                    Screen::Account
                }
                // The same, drawn as a signed-in teacher's with no server (the initials on the rail).
                "account:signed-in" => {
                    app.account.open();
                    app.account.model.me = Some(crate::share::api::Me {
                        email: "jordan.lee@school.org".into(),
                        display_name: "Jordan Lee".into(),
                        school: "Chico High".into(),
                        status: crate::share::api::Status::Approved,
                        trust: "teacher".into(),
                    });
                    app.account.model.page = crate::share::model::Page::Status;
                    Screen::Account
                }
                "bank" => app
                    .store
                    .banks
                    .first()
                    .map(|b| Screen::BankEdit(b.id.clone()))
                    .unwrap_or(app.screen),
                "exam" => app
                    .store
                    .exams
                    .first()
                    .map(|e| Screen::ExamEdit(e.id.clone()))
                    .unwrap_or(app.screen),
                // "exam:N" opens the exam at index N of the store.
                s if s.starts_with("exam:") => s[5..]
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| app.store.exams.get(i))
                    .map(|e| Screen::ExamEdit(e.id.clone()))
                    .unwrap_or(app.screen),
                _ => app.screen,
            };
        }
        app
    }

}

impl App {
    /// Test hook: VERSHION_SCRIPT="click:x,y;text:abc;key:Enter;wait" injects one input step
    /// every 10 frames after a 60 frame warm-up. Coordinates are egui points.
    pub fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some((path, _)) = &self.qa_shot {
            for ev in &raw.events {
                if let egui::Event::Screenshot { image, .. } = ev {
                    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Some(img) = image::RgbaImage::from_raw(image.width() as u32, image.height() as u32, bytes) {
                        let _ = img.save(path);
                    }
                    std::process::exit(0);
                }
            }
        }
        if self.qa_steps.is_empty() {
            return;
        }
        ctx.request_repaint();
        self.qa_frame += 1;
        if self.qa_frame < 60 || self.qa_frame % 10 != 0 {
            return;
        }
        let step = self.qa_steps.remove(0);
        eprintln!("qa step {} at frame {}", step, self.qa_frame);
        if step == "dump" {
            eprintln!("qa draft stem = {:?}", self.qedit.as_ref().map(|d| d.stem.clone()));
            if let Some(model::Kind::Blank { accept }) = self.qedit.as_ref().map(|d| &d.kind) {
                eprintln!("qa draft blanks = {accept:?}");
            }
            if let Some(model::Kind::Mc { answers }) = self.qedit.as_ref().map(|d| &d.kind) {
                eprintln!("qa draft answers = {:?}", answers.iter().map(|a| a.text.clone()).collect::<Vec<_>>());
            }
        }
        let (cmd, arg) = step.split_once(':').unwrap_or((step.as_str(), ""));
        let mods = egui::Modifiers::NONE;
        let pt = |a: &str| {
            let (x, y) = a.split_once(',').unwrap_or(("0", "0"));
            egui::pos2(x.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0))
        };
        match cmd {
            "click" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
            }
            "move" => raw.events.push(egui::Event::PointerMoved(pt(arg))),
            // A real pointer click: hover first, press 10 frames later, release 10 after that
            // (a one-frame "click" teleports the pointer, which no mouse does).
            "tap" => {
                raw.events.push(egui::Event::PointerMoved(pt(arg)));
                self.qa_steps.insert(0, format!("down:{arg}"));
                self.qa_steps.insert(1, format!("up:{arg}"));
            }
            // drag:x1,y1,x2,y2 = hover, press, move in 4 steps, release (one step per 10 frames).
            "drag" => {
                let v: Vec<f32> = arg.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                if let [x1, y1, x2, y2] = v[..] {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x1, y1)));
                    let mut steps = vec![format!("down:{x1},{y1}")];
                    steps.extend((1..=4).map(|k| {
                        let t = k as f32 / 4.0;
                        format!("move:{},{}", x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
                    }));
                    steps.push(format!("up:{x2},{y2}"));
                    for (i, s) in steps.into_iter().enumerate() {
                        self.qa_steps.insert(i, s);
                    }
                }
            }
            // Drag support: down:x,y then move:x,y ... then up:x,y (one step per 10 frames).
            "down" | "up" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: cmd == "down", modifiers: mods });
            }
            "dclick" => {
                let p = pt(arg);
                raw.events.push(egui::Event::PointerMoved(p));
                for _ in 0..2 {
                    raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
                    raw.events.push(egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
                }
            }
            "text" => raw.events.push(egui::Event::Text(arg.to_string())),
            // key:Enter, key:Ctrl+B, key:Shift+ArrowLeft, key:Ctrl+Shift+Num8 ...
            "key" => {
                let mut kmods = egui::Modifiers::NONE;
                let mut name = arg;
                loop {
                    if let Some(r) = name.strip_prefix("Ctrl+") {
                        kmods.ctrl = true;
                        kmods.command = true;
                        name = r;
                    } else if let Some(r) = name.strip_prefix("Shift+") {
                        kmods.shift = true;
                        name = r;
                    } else if let Some(r) = name.strip_prefix("Alt+") {
                        kmods.alt = true;
                        name = r;
                    } else {
                        break;
                    }
                }
                let key = match name {
                    "Num7" => Some(egui::Key::Num7),
                    "Num8" => Some(egui::Key::Num8),
                    "Num9" => Some(egui::Key::Num9),
                    "Comma" => Some(egui::Key::Comma),
                    "Period" => Some(egui::Key::Period),
                    n => egui::Key::from_name(n),
                };
                if let Some(key) = key {
                    for pressed in [true, false] {
                        raw.events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: kmods });
                    }
                }
            }
            "paste" => raw.events.push(egui::Event::Paste(arg.replace("\\n", "\n"))),
            // wheel:x,y,dy = hover at x,y and turn the mouse wheel by dy points (negative scrolls down)
            "wheel" => {
                let v: Vec<f32> = arg.split(',').filter_map(|t| t.trim().parse().ok()).collect();
                if let [x, y, dy] = v[..] {
                    raw.events.push(egui::Event::PointerMoved(egui::pos2(x, y)));
                    raw.events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, dy), phase: egui::TouchPhase::Move, modifiers: mods });
                }
            }
            _ => {}
        }
    }

    /// Ctrl/Cmd+N: start the item that matches the current screen (see [`shortcut_action`]).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let Some(action) = shortcut_action(&self.screen, self.qedit.is_some()) else {
            return;
        };
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N))
        });
        if !pressed {
            return;
        }
        match action {
            Shortcut::NewBank => {
                let id = self.new_bank("Untitled Question Bank");
                self.open_bank(&id);
            }
            Shortcut::NewExam => {
                let id = self.new_exam("Untitled Exam");
                self.open_exam(&id);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        theme::begin_frame(self.settings.dark);
        theme::set_area(ui.max_rect());
        let ctx = ui.ctx().clone();
        #[cfg(target_arch = "wasm32")]
        crate::platform::set_repaint(ctx.clone());
        if !self.qa_steps.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16)); // keep the VERSHION_SCRIPT clock advancing
        }
        if let Some((_, frames)) = &mut self.qa_shot {
            *frames += 1;
            // VERSHION_SHOT_FRAME=<n> delays the capture (lets a VERSHION_SCRIPT finish first).
            let at = crate::env_var("VERSHION_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(60u32);
            if *frames == at {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        self.handle_shortcuts(&ctx);
        // Sidebar + active screen + shell overlays (import dialog, help, toasts).
        self.draw_screen(&ctx);
        // The "Share to pool…" window (looks the sign-in up through the Account window), then the
        // Account window, over whatever screen is open; both return at once while closed.
        self.publish.show(&ctx, &mut self.account);
        if self.browse.show(&ctx, &mut self.account, &mut self.store.banks) {
            self.store.save_banks();
        }
        self.mine.show(&ctx, &mut self.account, &self.store.banks);
        self.account.show(&ctx);
        // The hosted AI is charged to the signed-in account: hand it the server and a way to get a fresh token. Choosing it
        // restores a saved sign-in in the background, so an Import started before anyone opened Account still finds it.
        if self.settings.ai_provider == "account" {
            self.account.open_quiet();
        }
        {
            let signed_in = self.account.model.me.is_some();
            match (signed_in, self.account.model.client().cloned()) {
                (true, Some(c)) if !self.account.model.base_url().is_empty() => {
                    let base = self.account.model.base_url().to_string();
                    #[cfg(not(target_arch = "wasm32"))]
                    crate::ai::set_account_ai(Some((base, std::sync::Arc::new(move || c.access_token_blocking()))));
                    // The page cannot wait on a refresh: it keeps a refresh going before the token ends, and hands the
                    // AI layer the token as it stands this frame.
                    #[cfg(target_arch = "wasm32")]
                    {
                        if c.claim_warming(std::time::Duration::from_secs(180)) {
                            let warm = c.clone();
                            wasm_bindgen_futures::spawn_local(async move { warm.warm_access().await });
                        }
                        let token = c.access_token_now();
                        crate::ai::set_account_ai(Some((base, std::sync::Arc::new(move || token.clone()))));
                    }
                }
                _ => crate::ai::set_account_ai(None),
            }
        }
        // "Open account..." anywhere in the app lands on the Account screen, not on a Shared tab.
        if self.account.take_wants_page() {
            self.screen = Screen::Account;
        }
        if crate::share::shared_ui::take_goto(&ctx, &mut self.shared, &mut self.account) {
            self.screen = Screen::Shared;
        }
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::{Screen, Shortcut, shortcut_action};

    #[test]
    fn ctrl_n_creates_the_right_item_for_the_screen() {
        assert_eq!(shortcut_action(&Screen::Home, false), Some(Shortcut::NewExam));
        assert_eq!(shortcut_action(&Screen::Exams, false), Some(Shortcut::NewExam));
        assert_eq!(shortcut_action(&Screen::Banks, false), Some(Shortcut::NewBank));
    }

    #[test]
    fn ctrl_n_does_nothing_elsewhere_or_with_a_draft_open() {
        for s in [
            Screen::Welcome,
            Screen::Imports,
            Screen::Figures,
            Screen::Settings,
            Screen::BankEdit("b".into()),
            Screen::ExamEdit("e".into()),
        ] {
            assert_eq!(shortcut_action(&s, false), None);
        }
        assert_eq!(shortcut_action(&Screen::Banks, true), None);
    }
}
