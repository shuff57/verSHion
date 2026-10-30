//! The AI loop end to end: one generated page per question kind -> the relay -> Ditto package ->
//! editable questions -> figure reconstruction, saved as a bank.
//!
//! Opt-in, because it needs a running relay and spends model calls:
//!
//! ```text
//! VERSHION_RELAY_BASE=http://127.0.0.1:18795/ai \
//!   cargo test --test ai_loop -- --ignored --nocapture
//! ```
//!
//! The pages come from `tests/ai_loop/make_pages.py` (committed fixtures). The run uses its own
//! store, so the real bank list is never touched, and the relay must be reachable at the base URL
//! (the `workers` provider is pointed at it through a temporary `settings.json`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ai_loop").join(name)
}

/// A fresh store whose settings point the `workers` provider at `base`.
fn temp_store(tag: &str, base: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vershion-ai-loop-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let settings = serde_json::json!({ "ai_provider": "workers", "ai_base": base, "ai_model": "" });
    std::fs::write(dir.join("settings.json"), settings.to_string()).unwrap();
    dir
}

/// Runs the binary's `VERSHION_IMPORT_PDF` hook; returns (stdout, stderr).
fn run_import(store: &Path, pdf: &Path) -> (String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vershion"))
        .env("VERSHION_STORE_DIR", store)
        .env("VERSHION_IMPORT_PDF", pdf)
        .env("VERSHION_BANK_NAME", "AI loop test")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn vershion");
    let deadline = Instant::now() + Duration::from_secs(1500);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("VERSHION_IMPORT_PDF did not finish within 1500 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "import failed:\nstdout:\n{stdout}\nstderr:\n{stderr}");
    (stdout, stderr)
}

/// Question kinds saved into the new bank.
fn kinds_in(store: &Path) -> Vec<String> {
    let bytes = std::fs::read(store.join("banks.json")).expect("banks.json written");
    let banks: serde_json::Value = serde_json::from_slice(&bytes).expect("banks.json is JSON");
    let mut kinds = Vec::new();
    for bank in banks.as_array().into_iter().flatten() {
        for q in bank["questions"].as_array().into_iter().flatten() {
            let kind = q["kind"].as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
            kinds.push(kind);
        }
    }
    kinds
}

/// The figure half on its own: a cropped diagram goes through `fit_with_retries` (3 attempts) and
/// must come back as an editable `FigureSpec` that renders, or as a clean "no generator fits".
/// Its prompts are short, so unlike the whole-page import it survives a slow dev tunnel.
#[test]
#[ignore = "needs a running relay; set VERSHION_RELAY_BASE and run with --ignored"]
fn ai_loop_fit_retries_turn_a_diagram_into_an_editable_figure() {
    let Ok(base) = std::env::var("VERSHION_RELAY_BASE") else {
        eprintln!("skipped: set VERSHION_RELAY_BASE (e.g. http://127.0.0.1:18795/ai)");
        return;
    };
    let settings = vershion::Settings {
        ai_provider: "workers".into(),
        ai_base: base.trim().trim_end_matches('/').to_string(),
        ..Default::default()
    };
    let cfg = vershion::ai::AiCfg::from(&settings);
    // The diagram page is a labelled right triangle; crop it out of the committed fixture's PNG.
    let crop = std::fs::read(fixture("triangle_crop.png")).expect("tests/ai_loop/triangle_crop.png");
    let started = Instant::now();
    let fitted = vershion::figfit::fit_with_retries(&crop, "What is the length of the hypotenuse x?", &cfg, 3);
    eprintln!("fit_with_retries took {:?}", started.elapsed());
    match fitted {
        Ok(Some(f)) => {
            eprintln!("fit: {} {:?}", f.spec.id, f.spec.params);
            assert!(f.png.starts_with(b"\x89PNG"), "the redraw is a real PNG");
            assert!(vershion::figures::registry().iter().any(|d| d.id == f.spec.id), "a registered, editable generator");
        }
        // An honest "nothing fits" after three attempts is a valid outcome: the caller keeps the crop.
        Ok(None) => eprintln!("no generator fits (kept the cropped image)"),
        Err(e) => panic!("the fitting loop errored instead of answering: {e}"),
    }
}

/// Figures the registry can draw, rendered with known parameters and handed to the fitter as if they
/// were a scanned diagram: the expected outcome is an editable figure of the same kind. Prints one
/// line per case; fails only when the loop errors, because "no fit" is an honest answer and the
/// printed table is the measurement.
#[test]
#[ignore = "needs a running relay; set VERSHION_RELAY_BASE and run with --ignored"]
fn ai_loop_fit_round_trips_registered_figures() {
    let Ok(base) = std::env::var("VERSHION_RELAY_BASE") else {
        eprintln!("skipped: set VERSHION_RELAY_BASE (e.g. http://127.0.0.1:18795/ai)");
        return;
    };
    let settings = vershion::Settings { ai_provider: "workers".into(), ai_base: base.trim().trim_end_matches('/').to_string(), ..Default::default() };
    let cfg = vershion::ai::AiCfg::from(&settings);
    // (generator, parameters it is drawn with, the question text printed beside it).
    let cases: [(&str, serde_json::Value, &str); 6] = [
        ("rectangle", serde_json::json!({"w": 5.0, "h": 3.0, "unit": "cm"}), "Find the perimeter of the rectangle."),
        ("number-line", serde_json::json!({}), "What number does point A show on the number line?"),
        ("scatterplot", serde_json::json!({}), "Describe the association shown in the scatterplot."),
        ("lewis-structures", serde_json::json!({"formula": "H2O"}), "How many lone pairs are on the central atom?"),
        ("circuit-diagram", serde_json::json!({}), "What is the total resistance of the circuit?"),
        ("histogram", serde_json::json!({}), "Which interval holds the most values?"),
    ];
    let only = std::env::var("FIT_ONLY").ok();
    let mut table = Vec::new();
    for (id, params, context) in cases {
        if only.as_deref().is_some_and(|o| o != id) {
            continue;
        }
        let def = vershion::figures::registry().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no generator {id}"));
        let p = vershion::figures::params_from_json(&def, &params);
        let png = vershion::figures::to_png(&(def.render)(&p), 1.5).expect("render");
        let png = vershion::figures::spec::flatten_white(&png).expect("flatten");
        let started = Instant::now();
        let outcome = match vershion::figfit::fit_with_retries(&png, context, &cfg, 3) {
            Ok(Some(f)) if f.spec.id == id => format!("FOUND {}", f.spec.id),
            Ok(Some(f)) => format!("FOUND other generator: {}", f.spec.id),
            Ok(None) => "no fit (crop kept)".to_string(),
            Err(e) => panic!("the fitting loop errored on {id}: {e}"),
        };
        let line = format!("{id:<18} {outcome:<40} {:>4.0}s", started.elapsed().as_secs_f32());
        eprintln!("{line}");
        table.push(line);
    }
    eprintln!("---- round trip ----\n{}", table.join("\n"));
}

#[test]
#[ignore = "needs a running relay; set VERSHION_RELAY_BASE and run with --ignored"]
fn ai_loop_reads_every_question_type_and_rebuilds_a_figure() {
    let Ok(base) = std::env::var("VERSHION_RELAY_BASE") else {
        eprintln!("skipped: set VERSHION_RELAY_BASE (e.g. http://127.0.0.1:18795/ai)");
        return;
    };
    let base = base.trim().trim_end_matches('/').to_string();
    let store = temp_store("all-types", &base);
    let (stdout, stderr) = run_import(&store, &fixture("quiz_all_types.pdf"));

    // The seven pages, in order: mc, tf, blank, short, matching, multipart, figure.
    let page_lines: Vec<&str> = stderr.lines().filter(|l| l.starts_with("page ")).collect();
    assert_eq!(page_lines.len(), 7, "one line per PDF page:\n{stderr}");
    for (i, line) in page_lines.iter().enumerate() {
        eprintln!("{}", line);
        let questions: usize = line
            .split_whitespace()
            .nth(2)
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        assert!(questions >= 1, "page {} produced no questions: {line}", i + 1);
    }
    // The diagram page (7) must crop a picture; when it does, the figure loop must make it editable.
    let figure_page = page_lines[6];
    let numbers: Vec<usize> = figure_page
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    // "page 7: 1 questions, 1 cropped, 0 redrawn, notes [...]" -> [page, questions, cropped, redrawn].
    let [_, _, cropped, redrawn, ..] = numbers.as_slice() else { panic!("unreadable page line: {figure_page}") };
    eprintln!("figure page: {cropped} cropped, {redrawn} redrawn");
    assert!(*cropped >= 1, "the diagram must be cropped out of the page (a bad box drops it): {figure_page}");
    // Fitting a generator can honestly find nothing; then the crop is kept, with a note saying why.
    assert!(*redrawn >= 1 || figure_page.contains("kept the cropped image"), "a crop must become an editable figure or say why not: {figure_page}");

    let kinds = kinds_in(&store);
    eprintln!("saved kinds: {kinds:?}");
    for want in ["Mc", "Tf", "Blank", "Short", "Matching", "Multipart"] {
        assert!(kinds.iter().any(|k| k == want), "missing {want} in {kinds:?}\n{stderr}");
    }
    assert!(stdout.contains("saved bank"), "the bank is saved:\n{stdout}");

    let _ = std::fs::remove_dir_all(&store);
}
