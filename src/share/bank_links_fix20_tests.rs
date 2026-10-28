//! Phase D1, round 20: the critic 100 leftovers 3, 4, 6, 8 and 11. Each test is written against the words or the geometry it
//! pins, so a change to either shows.

use super::bank_links::*;
use super::bank_links_fix11_tests::rig;
use super::bank_links_fix18_tests::many_news;
use super::bank_links_fix4_tests::*;
use super::bank_links_fix8_tests::*;
use super::bank_links_fix9_tests::*;
use super::bank_links_fix_tests::*;
use super::bank_links_tests::*;
use super::circles_text as t;
use super::circle_links::{DifficultyNews, NewsItem, NewsState};
use super::circles::CirclesModel;
use super::circles_tests::*;
use super::publish::{AccountSnapshot, Gate};
use crate::model::{Difficulty, QuestionBank};

fn signed_out() -> AccountSnapshot {
    AccountSnapshot { gate: Gate::SignedOut, session: None, account_open: true, account: None, name: None }
}

fn click(ctx: &egui::Context, m: &mut CirclesModel, banks: &mut [QuestionBank], acct: &mut FakeAccount, who: &AccountSnapshot, label: &str, nth: usize) -> bool {
    let mut b = browse();
    let mut save = false;
    let hit = super::circles_phasec_tests::click_text(ctx, 1280.0, label, nth, &mut |ui| {
        save |= super::bank_links_ui::strip_with(ui, m, banks, &mut b, acct, who, now(), NOW, LOCAL).save;
    });
    assert!(hit, "no {label:?} drawn");
    save
}

fn drawn(width: f32, draw: &mut dyn FnMut(&mut egui::Ui)) -> Vec<(String, egui::Rect, egui::Color32)> {
    let ctx = ctx();
    let mut all = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 6000.0))), ..Default::default() };
        let out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| draw(ui));
        });
        all = out
            .shapes
            .iter()
            .filter_map(|s| if let egui::epaint::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()), t.galley.job.sections.first().map_or(t.fallback_color, |x| x.format.color))) } else { None })
            .collect();
        out.drop_without_applying_deltas();
    }
    all
}

/// A news list of short rows (two questions, "One" and "Two"), both offering "Put mine back".
fn short_news() -> (CirclesModel, Vec<QuestionBank>) {
    let (mut bank, mut l) = linked(&[(QA, "One"), (QB, "Two")]);
    for q in &mut bank.questions {
        q.difficulty = Some(Difficulty::High);
    }
    for b in &mut l.base {
        b.snap.difficulty = None;
    }
    l.difficulty_read = Some(l.base_version);
    // the news names questions by their id in the bank (not the circle's id), so it is taken from the bank itself
    let ids: Vec<String> = bank.questions.iter().map(|q| q.id.clone()).collect();
    l.difficulty_news = Some(DifficultyNews {
        version: l.base_version,
        questions: ids.iter().map(|id| NewsItem { id: id.clone(), before: Some(Difficulty::Low), after: Some(Difficulty::High), ..Default::default() }).collect(),
    });
    l.mark_bank(&bank);
    let mut m = model();
    m.edit.add_link(l).unwrap();
    (m, vec![bank])
}

// ---- minor 3: a row with no button says why, and what she can still do by hand ------------------------------------------------

#[test]
fn c100_m3_a_row_with_no_button_says_why_and_how_to_keep_hers() {
    let kept = t::news_row(Some(1), "One", Some(Difficulty::Low), Some(Difficulty::High), NewsState::Kept);
    assert!(kept.contains("This copy has moved on, so it can't be put back"), "{kept}");
    assert!(kept.contains("set it by hand if you want yours"), "{kept}");
    let changed = t::news_row(Some(1), "One", Some(Difficulty::Low), Some(Difficulty::High), NewsState::Changed(Some(Difficulty::Medium)));
    assert!(changed.contains("changed since (now Medium)"), "{changed}");
    assert!(changed.contains("set it by hand if you want yours"), "{changed}");
    // the buttonless rows still name the pair, unchanged
    assert!(kept.contains("Easy\u{a0}->\u{a0}Hard"), "{kept}");
}

// ---- minor 4: what a put-back row says, and "Put all" asking first -----------------------------------------------------------

#[test]
fn c100_m4_a_put_back_row_names_the_circles_value() {
    let back = t::news_row(Some(2), "Two", Some(Difficulty::Low), Some(Difficulty::High), NewsState::Back);
    assert!(back.contains("yours (Easy) is back, not sent yet; the circle's is Hard"), "{back}");
    // and the real strip says it after a put back
    let (mut m, s, mut banks, ctx, mut acct) = many_news(3);
    let who = olga_named(&s);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 0);
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(strip.contains("the circle's is Hard"), "{strip}");
}

#[test]
fn c100_m4_put_all_asks_first_and_keep_them_changes_nothing() {
    let (mut m, s, mut banks, ctx, mut acct) = many_news(3);
    let who = olga_named(&s);
    assert!(banks[0].questions.iter().all(|q| q.difficulty == Some(Difficulty::High)), "the circle's, before anything");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put all of mine back", 0);
    let asking = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(asking.contains("Put yours back on 3 questions?"), "{asking}");
    assert!(asking.contains("Yes, put all back") && asking.contains("Keep them"), "{asking}");
    assert!(!asking.contains("Put all of mine back"), "the button makes room for the question: {asking}");
    assert!(banks[0].questions.iter().all(|q| q.difficulty == Some(Difficulty::High)), "nothing changes before the yes");
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Keep them", 0);
    assert!(banks[0].questions.iter().all(|q| q.difficulty == Some(Difficulty::High)), "Keep them changes nothing");
    let strip = frames(&mut m, &mut banks, &ctx, &mut acct, &who, 1);
    assert!(strip.contains("Put all of mine back") && !strip.contains("Yes, put all back"), "{strip}");
}

// ---- minor 6: the news list's geometry -----------------------------------------------------------------------------------------

#[test]
fn c100_m6_a_row_with_no_button_keeps_the_whole_width() {
    let (mut m, s, mut banks, ctx, mut acct) = many_news(3);
    let who = olga_named(&s);
    click(&ctx, &mut m, &mut banks, &mut acct, &who, "Put mine back", 0);
    let d = drawn(1280.0, &mut |ui| {
        let mut b = browse();
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &who, now(), NOW, LOCAL);
    });
    let rows: Vec<_> = d.iter().filter(|x| x.0.starts_with("Question ")).collect();
    let back = rows.iter().find(|r| r.0.contains("is back")).expect("the put-back row");
    let with_button = rows.iter().find(|r| !r.0.contains("is back")).expect("a row that still offers it");
    assert!(back.1.width() > with_button.1.width() + 60.0, "no empty button column: back {} vs button row {}", back.1.width(), with_button.1.width());
}

#[test]
fn c100_m6_a_short_row_keeps_its_button_beside_it() {
    let (mut m, mut banks) = short_news();
    let d = drawn(1280.0, &mut |ui| {
        let mut b = browse();
        let mut acct = FakeAccount::default();
        super::bank_links_ui::strip_with(ui, &mut m, &mut banks, &mut b, &mut acct, &signed_out(), now(), NOW, LOCAL);
    });
    let texts: Vec<&str> = d.iter().map(|x| x.0.as_str()).collect();
    let row = d.iter().find(|x| x.0.starts_with("Question 1, \"One\"")).unwrap_or_else(|| panic!("the first row: {texts:?}"));
    let button = d.iter().find(|x| x.0 == "Put mine back").unwrap_or_else(|| panic!("its button: {texts:?}"));
    let gap = button.1.left() - row.1.right();
    assert!((0.0..40.0).contains(&gap), "the button sits beside its row, not across the page: {gap} px");
}

// ---- minor 8: one bank name on the circle's pages ------------------------------------------------------------------------------

#[test]
fn c100_m8_the_as_is_and_get_latest_pages_name_the_same_bank() {
    let (mut m, s, _banks, _ctx, _acct) = rig();
    // its difficulties are already read: the page's own read is not asked for
    m.edit.update_link(LOCAL, |l| {
        l.difficulty_read = Some(l.base_version);
        true
    });
    s.on("Post", &format!("/circles/{CID}/banks/{BID}/merge"), 200, merge_start(1, 2));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), served(2, &[(QA, "One"), (QB, "Two")], false));
    assert!(m.edit.open_merge(LOCAL, owner()));
    let latest = drawn(1280.0, &mut |ui| super::circle_edit_ui::draw_page(ui, &mut m));
    assert!(latest.iter().any(|x| x.0 == "Get latest: \"Cells\""), "{latest:?}");
    assert!(!latest.iter().any(|x| x.0.contains("Unit 2 quiz")), "not her bank's own name: {latest:?}");
    // the as-is page draws the same title, from the same name: the circle's, never her bank's own (status.bank_name)
    assert_eq!(super::circle_edit_ui::merge_title(true, "Cells"), "Use my bank as it is: \"Cells\"");
    assert_ne!(super::circle_edit_ui::merge_title(true, "Cells"), super::circle_edit_ui::merge_title(false, "Cells"));
}

// ---- minor 11: the badge's short forms, "removed", and the menu's words -----------------------------------------------------------

#[test]
fn c100_m11_the_news_part_is_short_where_the_room_is_small() {
    assert_eq!(short_note(super::circles_text::NEWS_BADGE), "difficulty");
    assert_eq!(short_note(WAITING_NOTE), "waiting");
    assert_eq!(short_note("newer version · difficulty updated · 1 unsent changes"), "newer · difficulty · 1 unsent");
}

#[test]
fn c100_m11_removed_is_said_as_removed_never_as_left() {
    let (bank, mut l) = linked(&[(QA, "One")]);
    l.seen.removed = true;
    l.role_seen = Some(super::circle_links::ROLE_NONE.into());
    assert!(l.gone(), "a removed copy is gone too: the order of the two is what this pins");
    assert_eq!(badge_of(&l, None).note.as_deref(), Some("removed from the circle"));
    assert_eq!(short_note("removed from the circle"), "removed");
    let _ = bank;
}

#[test]
fn c100_m11_the_more_hint_uses_the_menus_own_words() {
    let v = StripView { line: String::new(), buttons: Vec::new(), busy: false, warn: false, menu_history: true, menu_circle_page: true, menu_check: true };
    let hint = super::bank_links_ui::more_hint(&v);
    assert!(hint.contains("circle page"), "{hint}");
    assert!(!hint.contains("circle's page"), "the menu says Circle page: {hint}");
}

/// (minor 6) A long stem is named by less of it where the room is small: the row stays a few lines, not four.
#[test]
fn c100_m6_a_narrow_row_names_less_of_the_question() {
    let stem = "Question number 1 about solving linear equations with fractions and decimals";
    let (mut bank, mut l) = linked(&[(QA, stem)]);
    for q in &mut bank.questions {
        q.difficulty = Some(Difficulty::High);
    }
    for b in &mut l.base {
        b.snap.difficulty = None;
    }
    l.difficulty_read = Some(l.base_version);
    let ids: Vec<String> = bank.questions.iter().map(|q| q.id.clone()).collect();
    l.difficulty_news = Some(DifficultyNews {
        version: l.base_version,
        questions: ids.iter().map(|id| NewsItem { id: id.clone(), before: Some(Difficulty::Low), after: Some(Difficulty::High), ..Default::default() }).collect(),
    });
    l.mark_bank(&bank);
    let mut m = model();
    m.edit.add_link(l).unwrap();
    let mut banks = vec![bank];
    let row_at = |m: &mut CirclesModel, banks: &mut Vec<QuestionBank>, w: f32| -> String {
        let d = drawn(w, &mut |ui| {
            let mut b = browse();
            let mut acct = FakeAccount::default();
            super::bank_links_ui::strip_with(ui, m, banks, &mut b, &mut acct, &signed_out(), now(), NOW, LOCAL);
        });
        d.iter().find(|x| x.0.starts_with("Question 1,")).map(|x| x.0.clone()).unwrap_or_default()
    };
    let wide = row_at(&mut m, &mut banks, 1280.0);
    let narrow = row_at(&mut m, &mut banks, 420.0);
    assert!(wide.contains("fracti"), "the wide row names more of it: {wide}");
    assert!(narrow.starts_with("Question 1,"), "{narrow}");
    assert!(narrow.chars().count() + 20 < wide.chars().count(), "the narrow row names less: {narrow} vs {wide}");
}

// ---- minor 12: walls of text -------------------------------------------------------------------------------------------------

/// (minor 12) The review-plus-not-kept line is three sentences: the reason, that waiting is enough, and Check now. No second
/// telling of the same advice, and no repeating "use your bank as it is" that the button already says.
#[test]
fn c100_m12_the_review_line_is_three_sentences() {
    use super::circle_edit::Unread;
    for why in [Unread::Withheld, Unread::NotKept, Unread::Other] {
        let line = t::review_not_kept_line("Algebra team", 3, why);
        let sentences = line.matches(". ").count() + usize::from(line.ends_with('.'));
        assert!(sentences <= 3, "{why:?}: {sentences} sentences: {line}");
        assert!(!line.contains("Once the review is done"), "no second telling of the same advice: {line}");
        assert!(!line.to_lowercase().contains("use your bank as it is"), "the button says that: {line}");
        assert!(line.contains("Check now"), "{line}");
        assert!(line.ends_with("Check now asks the circle again."), "{line}");
    }
}

/// (minor 12) The held strip names verSHion three times at most (it was four): twice in the saved-banks reason, once in the
/// advice, and never again in the "tries again" that follows.
#[test]
fn c100_m12_the_held_line_names_vershion_once() {
    let why = format!("{} {}", t::save_problem(""), t::SAVE_ADVICE);
    // the words the strip puts before it, as the app composes them
    let outer = "Shared with Algebra team. verSHion couldn't save your banks (verSHion isn't allowed to write to its data folder), so the circle's difficulty isn't in this bank yet, and your own edits aren't saved either.";
    for tries in [0, 1, 2, 9] {
        let line = format!("{outer} {}", t::held_line(&why, tries));
        let n = line.matches("verSHion").count();
        assert!(n <= 3, "{n} times in: {line}");
    }
    assert!(t::held_line(&why, 1).contains("tries again every minute"));
    assert!(t::held_line(&why, 3).contains("keeps trying every minute"));
}

/// (minor 12) The Account window says the date, not the time zone's name.
#[test]
fn c100_m12_the_account_window_has_no_utc_jargon() {
    assert!(!include_str!("ui.rs").contains("(UTC)"), "the Account window says the date");
}
