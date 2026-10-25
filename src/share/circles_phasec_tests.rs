//! Circles phase C against a scripted session (no network, no egui window): email invitations, the
//! join link with `#code=`, hiding a bank's answer keys from viewers (the decisions flow, every answer
//! and refusal), a viewer's stripped bank and its keyless copy, the partial purge, the activity's new
//! kinds and cursor, null-tolerant answers, plain words for every new code, one change at a time, a
//! sign-in that ends in the middle, and smoke frames of every new state wide and narrow.

use std::sync::Arc;

use serde_json::{Value, json};

use super::api::{ApiError, ErrorDetail};
use super::browse::{BrowseModel, ImportLog};
use super::circles::*;
use super::circles_api::*;
use super::circles_tests::*;
use super::model::Runner;
use super::pool::*;
use super::secret::Secret;
use super::transport::{Method, Response};
use crate::model::QuestionBank;

const EMAIL_PATH: &str = "/invites/email";

fn problem_of_panel(m: &CirclesModel) -> String {
    match &m.panel {
        Panel::Email(f) => f.problem.as_ref().map(|p| p.message.clone()).unwrap_or_default(),
        _ => String::new(),
    }
}

fn email_form(m: &mut CirclesModel) -> &mut EmailForm {
    match &mut m.panel {
        Panel::Email(f) => f,
        other => panic!("not the email form: {other:?}"),
    }
}

fn posts(s: &Arc<Script>, suffix: &str) -> Vec<super::circles_tests::Sent> {
    s.seen().into_iter().filter(|x| x.path.ends_with(suffix)).collect()
}

/// The owner's page with the email form open and an address typed.
fn owner_email(address: &str, role: Role) -> (CirclesModel, Arc<Script>) {
    let (mut m, s) = open_circle("owner", json!({}));
    m.begin_email();
    let f = email_form(&mut m);
    f.address = address.into();
    f.role = role;
    (m, s)
}

fn sent_answer(extra: Value) -> Value {
    let mut v = json!({"ok": true, "invite_id": "9".repeat(32), "kind": "email", "role": "editor", "max_uses": 1, "expires_at": NOW + 7 * DAY, "emailed": true, "address_label": "j***@school.org"});
    merge(&mut v, extra);
    v
}

/// A refusal's sentence, as the person would read it.
fn said(status: u16, body: Value) -> String {
    let d = ErrorDetail::from_response(&Response::new(status, &body.to_string()));
    let r = Refusal { kind: RefusalKind::classify(&d), detail: d };
    r.message()
}

// ---- email invitations ------------------------------------------------------------------------------------------

#[test]
fn an_email_invitation_is_sent_once_and_the_address_is_cleared() {
    let (mut m, s) = owner_email("  Jo.Smith@School.org ", Role::Viewer);
    assert!(m.email_blocker().is_none());
    let invites_before = s.seen().iter().filter(|x| x.path.ends_with("/invites") && x.method == Method::Get).count();
    s.act("Post", EMAIL_PATH, 202, sent_answer(json!({"role": "viewer"})));
    m.send_email_invite();
    // a second press while it runs sends nothing
    m.send_email_invite();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let p = posts(&s, EMAIL_PATH);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].json(), json!({"email": "Jo.Smith@School.org", "role": "viewer"}));
    let f = email_form(&mut m);
    assert!(f.address.is_empty(), "the address is not kept once sent");
    assert!(!f.working && f.problem.is_none());
    let sent = f.sent.clone().expect("the result is shown");
    assert_eq!(sent.address_label, "j***@school.org");
    assert_eq!(sent.role(), Role::Viewer);
    assert!(!sent.resent);
    let invites_after = s.seen().iter().filter(|x| x.path.ends_with("/invites") && x.method == Method::Get).count();
    assert!(invites_after > invites_before, "the list of invitations is read again");
    // "Invite someone else" keeps the role, clears the result
    m.another_email();
    let f = email_form(&mut m);
    assert!(f.sent.is_none() && f.address.is_empty() && f.role == Role::Viewer);
}

#[test]
fn a_second_invitation_to_the_same_address_is_said_to_replace_the_first() {
    let (mut m, s) = owner_email("jo@school.org", Role::Editor);
    s.act("Post", EMAIL_PATH, 202, sent_answer(json!({"resent": true, "message": "RAWSERVER words"})));
    m.send_email_invite();
    m.frame(now(), &ready(&s));
    let f = email_form(&mut m);
    assert!(f.sent.as_ref().is_some_and(|x| x.resent));
    assert!(super::circles_text::EMAIL_RESENT.contains("earlier one no longer works"));
}

#[test]
fn an_address_that_is_not_one_is_never_sent() {
    for bad in ["", "   ", "jo", "jo@school", "@school.org", "jo smith@school.org", "a@b.org, c@d.org", "jo@.org", "jo@school.org."] {
        let (mut m, s) = owner_email(bad, Role::Editor);
        assert!(m.email_blocker().is_some(), "{bad:?}");
        let n = s.seen().len();
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        assert_eq!(s.seen().len(), n, "{bad:?} was sent");
    }
    assert!(email_problem(&format!("{}@school.org", "a".repeat(260))).is_some());
}

#[test]
fn only_the_owner_gets_the_email_form() {
    for role in ["editor", "viewer"] {
        let (mut m, _) = open_circle(role, json!({}));
        m.begin_email();
        assert!(matches!(m.panel, Panel::None), "{role}");
    }
    let (m, s) = open_circle("viewer", json!({}));
    let _ = m;
    let sess: Arc<Script> = s.clone();
    let owner_role = super::transport::block_on(async { Pool::new(sess.as_ref()).email_invite(CID, &Secret::new("jo@school.org"), Role::Owner).await });
    assert!(matches!(owner_role, Err(PoolError::Local(_))), "an owner invitation is refused before anything is sent");
}

#[test]
fn every_email_refusal_has_its_own_plain_words_and_keeps_the_address() {
    let cases: Vec<(u16, Value, &str)> = vec![
        (422, json!({"error": "invite_not_sent", "emailed": false, "reason": "address_limit", "message": "RAWSERVER"}), "today"),
        (422, json!({"error": "invite_not_sent", "emailed": false, "reason": "not_deliverable", "message": "RAWSERVER"}), "can not send mail"),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_invites_hourly", "message": "RAWSERVER"}), "this hour"),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_inviter_domain_hourly", "limit": 10, "message": "RAWSERVER"}), "(10)"),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_domain_hourly", "message": "RAWSERVER"}), "from everyone"),
        (429, json!({"error": "daily_limit", "metric": "circle_email_invites"}), "today"),
        (409, json!({"error": "circle_full", "limit": 50, "message": "RAWSERVER"}), "50 members"),
        (422, json!({"error": "circles_limit", "metric": "circle_open_email_invites", "used": 40, "limit": 40}), "40"),
        (409, json!({"error": "conflict", "message": "RAWSERVER"}), "invite the address again"),
        (503, json!({"error": "busy", "message": "RAWSERVER"}), "Try again later"),
        (400, json!({"error": "invalid_request", "field": "email", "message": "RAWSERVER"}), "typo"),
        (403, json!({"error": "circle_role_required", "needed": "owner", "role": "editor"}), "owner"),
        (404, json!({"error": "not_found"}), "not available"),
    ];
    for (status, body, want) in cases {
        let (mut m, s) = owner_email("jo@school.org", Role::Editor);
        s.act("Post", EMAIL_PATH, status, body.clone());
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        let p = problem_of_panel(&m);
        let code = body["error"].as_str().unwrap();
        assert!(p.contains(want), "{code}: {p}");
        assert!(!(code.contains('_') && p.contains(code)) && !p.contains("RAWSERVER") && !p.contains('_'), "{code}: raw words reached the screen: {p}");
        let f = email_form(&mut m);
        assert!(!f.working, "{code}: the button stays stuck");
        assert_eq!(f.address, "jo@school.org", "{code}: the address is kept to fix a typo");
        assert_eq!(m.screen, Screen::Circle, "{code}");
    }
}

#[test]
fn a_waiting_limit_offers_try_again_after_the_wait() {
    let (mut m, s) = owner_email("jo@school.org", Role::Editor);
    s.act("Post", EMAIL_PATH, 429, json!({"error": "hourly_limit", "metric": "circle_email_inviter_domain_hourly", "limit": 10}));
    m.send_email_invite();
    m.frame(now(), &ready(&s));
    let Panel::Email(f) = &m.panel else { panic!() };
    assert!(f.problem.as_ref().unwrap().retry, "the hour's limit can be tried again");
    // "Try again" on the circle page never repeats an emailed invitation (its address is not kept there)
    let n = s.seen().len();
    m.retry_act();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), n);
}

#[test]
fn a_mail_domains_hourly_limit_holds_back_only_that_domain() {
    // (critic 57 I4: the 11th colleague at one school greyed the button for every address, for up to an hour)
    let email = format!("/circles/{CID}{EMAIL_PATH}");
    for metric in ["circle_email_inviter_domain_hourly", "circle_email_domain_hourly"] {
        let (mut m, s) = owner_email("a@pvhs.org", Role::Editor);
        s.on_wait("Post", &email, 429, json!({"error": "hourly_limit", "metric": metric, "limit": 10}), 3600);
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        assert!(email_form(&mut m).problem.as_ref().is_some_and(|p| p.retry_at.is_some()), "{metric}");
        let n = posts(&s, EMAIL_PATH).len();
        // the same domain waits, in the model too (not only a grey button), and the page says the way on
        email_form(&mut m).address = "b@PVHS.org".into();
        let why = m.email_blocker().unwrap_or_else(|| panic!("{metric}: the same domain waits"));
        assert!(why.contains("another") && !why.contains('_'), "{metric}: {why}");
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        assert_eq!(posts(&s, EMAIL_PATH).len(), n, "{metric}");
        // another domain goes now
        email_form(&mut m).address = "c@gmail.com".into();
        assert_eq!(m.email_blocker(), None, "{metric}");
        s.act("Post", EMAIL_PATH, 202, sent_answer(json!({})));
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        assert_eq!(posts(&s, EMAIL_PATH).len(), n + 1, "{metric}");
        assert!(email_form(&mut m).sent.is_some(), "{metric}");
    }
    // the inviter's own hourly limit holds for every address
    let (mut m, s) = owner_email("a@pvhs.org", Role::Editor);
    s.on_wait("Post", &email, 429, json!({"error": "hourly_limit", "metric": "circle_email_invites_hourly"}), 3600);
    m.send_email_invite();
    m.frame(now(), &ready(&s));
    email_form(&mut m).address = "c@gmail.com".into();
    assert!(m.email_blocker().is_some());
}

#[test]
fn a_sign_in_that_ends_while_inviting_frees_the_button() {
    let (mut m, s) = owner_email("jo@school.org", Role::Editor);
    s.fail(ApiError::SignedOut);
    m.send_email_invite();
    m.frame(now(), &ready(&s));
    let f = email_form(&mut m);
    assert!(!f.working);
    assert_eq!(f.address, "jo@school.org");
    assert_eq!(m.gate, super::publish::Gate::SignedOut);
}

#[test]
fn no_address_or_code_reaches_debug() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let email_id = egui::Id::new("circle_email_address");
    let (mut m, _) = owner_email("", Role::Editor);
    typed_frame(&ctx, &mut m, email_id, 0.0);
    email_form(&mut m).address = "secret.person@school.org".into();
    for t in [0.1, 0.2, 1.5, 2.0] {
        typed_frame(&ctx, &mut m, email_id, t);
    }
    let dbg = format!("{:?}", m.panel);
    assert!(!dbg.contains("secret.person"), "{dbg}");
    m.start_join();
    m.join.text = "https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01".into();
    let dbg = format!("{:?}", m.join);
    assert!(!dbg.contains("ABCD") && !dbg.contains("example.org"), "{dbg}");
    // closing the model wipes both, egui's memory of the fields too (critic 57 I3)
    m.close();
    assert!(m.join.text.is_empty());
    assert!(matches!(m.panel, Panel::None));
    typed_frame(&ctx, &mut m, email_id, 3.0);
    assert!(egui::text_edit::TextEditState::load(&ctx, email_id).is_none(), "the typed address stays in egui's memory");
}

/// One frame of the Circles tab with `id` focused, at time `t` (the text edit's undo history settles over time).
fn typed_frame(ctx: &egui::Context, m: &mut CirclesModel, id: egui::Id, t: f64) {
    ctx.memory_mut(|mm| mm.request_focus(id));
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 4000.0))), time: Some(t), ..Default::default() };
    let out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            super::circles_ui::body_of(ui, m);
        });
    });
    out.drop_without_applying_deltas();
}

/// What Ctrl+Z in an empty field would bring back, if egui still holds the field's history.
fn undo_gives(ctx: &egui::Context, id: egui::Id) -> Option<String> {
    let st = egui::text_edit::TextEditState::load(ctx, id)?;
    let mut u = st.undoer();
    let cur = (egui::text::CCursorRange::one(egui::text::CCursor::new(0)), String::new());
    u.undo(&cur).map(|x| x.1.clone())
}

#[test]
fn no_address_or_code_stays_in_the_fields_undo_history() {
    // (critic 57 I3: Ctrl+Z brought the sent address back after "cleared from this form")
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let (mut m, s) = owner_email("", Role::Editor);
    let id = egui::Id::new("circle_email_address");
    typed_frame(&ctx, &mut m, id, 0.0);
    typed_frame(&ctx, &mut m, id, 0.1);
    email_form(&mut m).address = "secret.person@school.org".into();
    for t in [0.2, 0.3, 1.5, 2.0] {
        typed_frame(&ctx, &mut m, id, t);
    }
    assert!(egui::text_edit::TextEditState::load(&ctx, id).is_some(), "the fixture types into the field");
    s.act("Post", EMAIL_PATH, 202, sent_answer(json!({})));
    m.send_email_invite();
    m.frame(now(), &ready(&s));
    assert!(email_form(&mut m).sent.is_some());
    m.another_email();
    typed_frame(&ctx, &mut m, id, 5.0);
    assert!(email_form(&mut m).address.is_empty());
    assert_eq!(undo_gives(&ctx, id), None, "Ctrl+Z gives the address back");
    // the same for a pasted invitation link once the box is empty again
    let id = egui::Id::new("circle_code_text");
    m.start_join();
    typed_frame(&ctx, &mut m, id, 6.0);
    m.join.text = "https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01".into();
    for t in [6.1, 6.2, 7.5, 8.0] {
        typed_frame(&ctx, &mut m, id, t);
    }
    m.start_join();
    typed_frame(&ctx, &mut m, id, 9.0);
    assert_eq!(undo_gives(&ctx, id), None, "Ctrl+Z gives the code back");
}

#[test]
fn the_owners_list_shows_kind_masked_address_role_and_time() {
    let (mut m, s) = open_circle("owner", json!({}));
    s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [
        {"invite_id": "a".repeat(32), "kind": "email", "address_label": "j***@school.org #2", "role": "editor", "created_at": NOW - 2 * 3_600_000, "created_by": "u-owner", "created_by_name": "Ms Frizzle", "max_uses": 1, "uses": 0, "expires_at": NOW + 7 * DAY, "revoked_at": null, "note": "", "state": "active"},
        {"invite_id": "b".repeat(32), "kind": "code", "address_label": "", "role": "viewer", "created_at": NOW - DAY, "created_by": "u-owner", "created_by_name": "Ms Frizzle", "max_uses": 3, "uses": 1, "expires_at": NOW + DAY, "revoked_at": null, "note": "", "state": "active"},
        {"invite_id": "c".repeat(32), "kind": null, "address_label": null, "role": "viewer", "created_at": null, "max_uses": 1, "uses": 0, "expires_at": NOW - DAY, "note": null, "state": "expired"}
    ], "next_cursor": null}));
    m.circle.as_mut().unwrap().invites = None;
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let l = m.circle.as_ref().unwrap().invites.clone().expect("a null kind or label is read as empty");
    assert_eq!(l.len(), 3);
    assert!(l[0].emailed() && !l[1].emailed() && !l[2].emailed());
    let line = super::circles_ui::invite_line(&l[0], NOW);
    assert!(line.contains("emailed to j***@school.org #2") && line.contains("can add banks") && line.contains("2 hours ago"), "{line}");
    let line = super::circles_ui::invite_line(&l[1], NOW);
    assert!(line.starts_with("code") && line.contains("1 of 3 used"), "{line}");
    // an emailed invitation is withdrawn like a code
    s.act("Post", "/invites/revoke", 200, json!({"ok": true, "invite_id": "a".repeat(32)}));
    m.withdraw_code(&"a".repeat(32));
    m.frame(now(), &ready(&s));
    assert_eq!(posts(&s, "/invites/revoke")[0].json()["invite_id"], "a".repeat(32));
}

#[test]
fn an_email_answer_with_nulls_is_read() {
    let s = Script::new();
    s.act("Post", EMAIL_PATH, 202, json!({"ok": true, "invite_id": null, "kind": "email", "role": null, "expires_at": null, "emailed": null, "address_label": null, "resent": null}));
    let got = super::transport::block_on(async { Pool::new(s.as_ref()).email_invite(CID, &Secret::new("jo@school.org"), Role::Editor).await }).expect("nulls are read as empty");
    assert!(got.address_label.is_empty() && got.expires_at.is_none() && !got.resent);
}

// ---- the join link -------------------------------------------------------------------------------------------------

#[test]
fn a_pasted_invitation_link_gives_its_code() {
    let code = Some("C-ABCDEFGHJKMNPQRSTVWXYZ01".to_string());
    assert_eq!(code_in("https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01"), code);
    assert_eq!(code_in("Or open this link:\nhttps://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01\n\nThe code works once"), code);
    assert_eq!(code_in("https://app.example.org/circles/join#CODE=c%2Dabcd%2Defgh%2Djkmn%2Dpqrs%2Dtvwx%2Dyz01"), code);
    // the fragment wins over anything in the host or path that looks like a code
    assert_eq!(code_in("https://c-zzzz-zzzz-zzzz-zzzz-zzzz-zzz9.example/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01"), code);
    assert!(code_in("https://app.example.org/circles/join#code=").is_none());
    // the join box takes it, and only the code is sent
    let s = Script::new();
    s.keep("Get", "/circles", 200, json!({"circles": []}));
    let mut m = model();
    m.open_list();
    m.frame(now(), &ready(&s));
    m.start_join();
    m.join.text = "https://app.example.org/circles/join#code=C-ABCD-EFGH-JKMN-PQRS-TVWX-YZ01".into();
    s.on("Post", "/circles/join/preview", 200, json!({"circle": {"id": CID, "name": "Biology team", "owner_name": "Ms Frizzle", "members": 3}, "role": "viewer", "already_member": false}));
    m.look_up_code();
    m.frame(now(), &ready(&s));
    assert_eq!(s.last().json(), json!({"code": "C-ABCDEFGHJKMNPQRSTVWXYZ01"}));
    assert!(matches!(m.join.stage, JoinStage::Preview(..)));
}

// ---- hiding answer keys from viewers ------------------------------------------------------------------------------

fn bank_row(is_author: bool, hide_key: Value) -> Value {
    json!({"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": is_author, "head_version": 2, "question_count": 3, "created_at": NOW - DAY, "updated_at": NOW - DAY, "hide_key": hide_key})
}

/// A bank head as served, with `extra` merged into its `bank`.
fn head_body(extra: Value, stems: &[&str], keyless: bool) -> Vec<u8> {
    let qs: Vec<Value> = stems
        .iter()
        .map(|s| {
            if keyless {
                json!({"id": format!("src-{s}"), "stem": s, "kind": {"Mc": {"answers": [{"text": "A", "correct": false}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": true})
            } else {
                json!({"id": format!("src-{s}"), "stem": s, "kind": {"Mc": {"answers": [{"text": "A", "correct": true}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6, "keyless": false})
            }
        })
        .collect();
    let payload = json!({"schema_version": 2, "bank": {"id": "b-src", "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    let mut bank = json!({"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": 2, "version_id": "v2", "question_count": stems.len(), "has_images": false, "committed_at": NOW - DAY});
    merge(&mut bank, extra);
    let h = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": bank, "origins": null}).to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], payload).into_bytes()
}

/// The bank page for `role`, the bank read with `extra` in its head.
fn open_bank_as(role: &str, is_author: bool, extra: Value, keyless: bool) -> (CirclesModel, Arc<Script>) {
    let hide = extra.get("hide_key").cloned().unwrap_or(json!(false));
    let (mut m, s) = open_circle(role, json!({"banks": [bank_row(is_author, hide)]}));
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), head_body(extra, &["First", "Second", "Third"], keyless));
    // (the head is read again right before a change of the setting: still version 2 unless a test says otherwise)
    s.keep("Post", "/circles/heads", 200, heads_reply(2));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    (m, s)
}

fn hide(m: &CirclesModel) -> &HideStage {
    &m.bank.as_ref().unwrap().hide
}

fn patches(s: &Arc<Script>) -> Vec<Value> {
    s.seen().into_iter().filter(|x| x.method == Method::Patch).map(|x| x.json()).collect()
}

fn patch(s: &Arc<Script>, status: u16, body: Value) {
    s.act("Patch", &format!("/banks/{BID}"), status, body);
}

/// `POST /circles/heads` with the bank at `version`.
fn heads_reply(version: u64) -> Value {
    json!({"banks": [{"bank_id": BID, "status": "current", "version": version, "version_id": format!("v{version}"), "license": "CC-BY-4.0", "at": NOW - DAY, "by": "Mr Keesler"}]})
}

fn heads_reads(s: &Arc<Script>) -> usize {
    s.seen().iter().filter(|x| x.method == Method::Post && x.path == "/circles/heads").count()
}

#[test]
fn who_gets_the_switch() {
    let (m, _) = open_bank_as("owner", false, json!({}), false);
    assert!(m.hide_allowed(), "the circle's owner");
    let (m, _) = open_bank_as("editor", true, json!({"is_author": true}), false);
    assert!(m.hide_allowed(), "the bank's author while an editor");
    let (m, _) = open_bank_as("editor", false, json!({}), false);
    assert!(!m.hide_allowed(), "another editor");
    let (m, _) = open_bank_as("viewer", true, json!({"is_author": true, "hide_key": true, "keys_hidden": true}), true);
    assert!(!m.hide_allowed(), "a viewer (a demoted author too)");
    let b = BankEntry { id: BID.into(), is_author: true, removed: Some(BankRemoved::default()), ..Default::default() };
    assert!(!bank_can(Role::Owner, false, &b, BankAction::HideKeys), "not on a removed bank");
    assert!(!bank_can(Role::Owner, true, &BankEntry::default(), BankAction::HideKeys), "not in a closed circle");
}

#[test]
fn hiding_asks_first_then_follows_the_pools_steps_with_only_the_listed_decisions() {
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    assert!(matches!(hide(&m), HideStage::Confirm { on: true }));
    assert!(patches(&s).is_empty(), "nothing is sent before Yes");
    // step 1: letter labels
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_need_decision", "path": "bank.questions[1]"}], "label_findings": [{"question": 1, "order_revealing": true, "strip_allowed": true, "choices": [{"before": "A) Mitosis", "after": "Mitosis"}]}]}));
    m.confirm_hide();
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s), vec![json!({"hide_key": true})], "one request, no decisions yet");
    let HideStage::Decide(d) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(!d.ready());
    // nothing goes while a listed decision is open
    m.send_hide_decisions();
    assert_eq!(patches(&s).len(), 1);
    // a decision for a question the pool did not list is ignored
    m.hide_decide_label(Pos::question(0), LabelChoice::Keep);
    m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
    // step 2: an order to confirm
    patch(&s, 422, json!({"error": "order_ack_required", "errors": [{"code": "order_ack_required", "path": "bank.questions[2]"}], "needs_order_ack": [{"question": 2}], "order_reasons": [{"question": 2, "reason": "sibling_refs"}]}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s)[1], json!({"hide_key": true, "labels": [{"question": 1, "choice": "strip"}]}));
    let HideStage::Decide(d) = hide(&m) else { panic!() };
    assert_eq!(d.todo().orders, 1);
    m.hide_set_order(Pos::question(2), true);
    // step 3: done
    patch(&s, 200, json!({"ok": true, "hide_key": true, "held": false, "withheld": false, "labels_removed": [{"question": 1}], "images_kept": [], "images_removed": [0], "figures_removed": [], "order_reasons": []}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s)[2], json!({"hide_key": true, "labels": [{"question": 1, "choice": "strip"}], "order_acks": [{"question": 2}]}), "the earlier step's decisions go again");
    let HideStage::Done(r) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(r.hide_key && r.labels_removed == 1 && r.images_removed == 1);
    let lines = super::circles_text::hide_result_lines(r, false).join(" ");
    assert!(lines.contains("now hidden from viewers") && lines.contains("1 picture was left out"), "{lines}");
    // the bank is read again for the new setting
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Ready(_)));
}

#[test]
fn a_formatting_finding_can_be_acknowledged_and_a_hard_one_blocks() {
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "invalid_payload", "errors": [{"code": "key_in_formatting", "path": "bank.questions[0].kind.Mc.answers[1].text", "reason": "capitals", "ack_allowed": true, "author_only": true, "hint": "Check it."}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert_eq!(d.ackable(), vec![0]);
    m.hide_set_formatting(0, true);
    patch(&s, 200, json!({"ok": true, "hide_key": true}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s)[1], json!({"hide_key": true, "ack_formatting": [0]}));
    // a star mark can not be acknowledged: nothing is sent, the page says to change the question
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "key_in_formatting", "path": "bank.questions[2].kind.Mc.answers[0].text", "reason": "star", "ack_allowed": false, "author_only": true}, {"code": "labels_need_decision", "path": "bank.questions[1]"}], "label_findings": [{"question": 1, "strip_allowed": true, "choices": []}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!() };
    assert_eq!(d.hard().len(), 1);
    m.hide_decide_label(Pos::question(1), LabelChoice::Keep);
    m.send_hide_decisions();
    assert_eq!(patches(&s).len(), 1, "a finding that must be changed in the text blocks the send");
}

#[test]
fn a_decision_for_a_question_without_labels_any_more_starts_the_labels_again() {
    // (critic 57 B1: every earlier decision is dropped on a restart, not only the labels, and the pool's echo of a
    // choice sent for the old head is not shown as made)
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [
        {"code": "key_in_formatting", "path": "bank.questions[0].kind.Mc.answers[1].text", "reason": "capitals", "ack_allowed": true, "author_only": true},
        {"code": "labels_need_decision", "path": "bank.questions[1]"}, {"code": "labels_need_decision", "path": "bank.questions[2]"}],
        "label_findings": [{"question": 1, "strip_allowed": true}, {"question": 2, "strip_allowed": true}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
    m.hide_decide_label(Pos::question(2), LabelChoice::Keep);
    m.hide_set_formatting(0, true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_unused", "path": "labels"}], "label_findings": [{"question": 2, "strip_allowed": true, "decision": "keep"}, {"question": 3, "strip_allowed": true}]}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!() };
    assert!(d.restarted);
    assert_eq!(d.decisions(), StripDecisions::default(), "no stale decision is sent again (labels, checks or orders)");
    assert!(!d.formatting_acked(0), "the check was ticked for the old question 1");
    assert_eq!(d.label(Pos::question(2)), None, "the pool's echo of the old choice is not shown as made");
    assert_eq!(d.todo().labels, 2);
    m.hide_decide_label(Pos::question(2), LabelChoice::Keep);
    assert_eq!(d_labels(&m), vec![(2, LabelChoice::Keep)]);
}

#[test]
fn a_restart_sends_only_what_was_decided_for_the_new_head() {
    // (critic 57 B1 repro, turned around: the "I checked" and the order made for the old question 1 went with the
    // inserted question nobody saw)
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [
        {"code": "key_in_formatting", "path": "bank.questions[0].kind.Mc.answers[1].text", "reason": "capitals", "ack_allowed": true, "author_only": true},
        {"code": "labels_need_decision", "path": "labels[question=1]"}, {"code": "order_ack_required", "path": "bank.questions[2]"}],
        "label_findings": [{"question": 1, "strip_allowed": true}], "needs_order_ack": [{"question": 2}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Keep);
    m.hide_set_formatting(0, true);
    m.hide_set_order(Pos::question(2), true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_unused", "path": "labels[question=1]"}, {"code": "labels_need_decision", "path": "labels[question=2]"}], "label_findings": [{"question": 2, "strip_allowed": true}]}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s)[1], json!({"hide_key": true, "labels": [{"question": 1, "choice": "keep"}], "ack_formatting": [0], "order_acks": [{"question": 2}]}));
    let paths = s.paths();
    assert!(paths[paths.len() - 2] == "Post /circles/heads" && paths[paths.len() - 1].starts_with("Patch "), "the head is read right before the change: {paths:?}");
    let HideStage::Decide(d) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(d.restarted);
    m.hide_decide_label(Pos::question(2), LabelChoice::Strip);
    patch(&s, 200, json!({"ok": true, "hide_key": true}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s)[2], json!({"hide_key": true, "labels": [{"question": 2, "choice": "strip"}]}), "no check or order made for the old head goes");
    assert!(matches!(hide(&m), HideStage::Done(_)));
}

#[test]
fn a_choice_the_pool_echoes_is_never_taken_as_made() {
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true}, {"question": 2, "strip_allowed": true}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Keep);
    m.hide_decide_label(Pos::question(2), LabelChoice::Strip);
    // a new head: question 1 still has labels (the pool echoes the choice sent), 2 has none, 0 is new
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "labels_unused", "path": "labels[question=2]"}, {"code": "labels_need_decision", "path": "labels[question=0]"}], "label_findings": [{"question": 0, "strip_allowed": true}, {"question": 1, "strip_allowed": true, "decision": "keep"}]}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!() };
    assert!(d.restarted, "the page says the decisions were dropped");
    assert_eq!(d.label(Pos::question(1)), None, "so none is shown as made");
    assert_eq!(d.todo().labels, 2);
    assert!(!d.ready());
    // a first step's echo of a choice this page never made is not taken either
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true, "decision": "keep"}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!() };
    assert_eq!(d.label(Pos::question(1)), None);
    m.send_hide_decisions();
    assert_eq!(patches(&s).len(), 1, "nothing goes until the person decides");
}

#[test]
fn a_new_head_before_the_change_goes_stops_it_and_drops_the_decisions() {
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
    // an editor sends version 3 before the button is pressed
    s.on("Post", "/circles/heads", 200, heads_reply(3));
    let reads = heads_reads(&s);
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert_eq!(heads_reads(&s), reads + 1, "the head is read again right before");
    assert_eq!(patches(&s).len(), 1, "nothing goes for a version the decisions were not made for");
    let HideStage::Failed { problem, .. } = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(problem.message.contains("new version") && problem.message.contains("dropped") && !problem.message.contains('_'), "{}", problem.message);
    assert!(!problem.retry, "the switch is pressed again for the new version");
    // the bank is read again; starting again sends none of the old decisions
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Ready(_)));
    m.ask_hide(true);
    patch(&s, 200, json!({"ok": true, "hide_key": true}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s).last(), Some(&json!({"hide_key": true})));
    // the head moved before the first step too: nothing is sent
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    s.on("Post", "/circles/heads", 200, heads_reply(3));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert!(patches(&s).is_empty());
    let HideStage::Failed { problem, .. } = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(problem.message.contains("new version") && !problem.message.contains("dropped"), "{}", problem.message);
}

fn d_labels(m: &CirclesModel) -> Vec<(usize, LabelChoice)> {
    match hide(m) {
        HideStage::Decide(d) => d.decisions().labels.iter().map(|l| (l.question, l.choice)).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn turning_it_off_sends_no_decisions_and_says_what_came_back() {
    let (mut m, s) = open_bank_as("owner", false, json!({"hide_key": true}), false);
    assert!(m.hide_key_on());
    m.ask_hide(false);
    patch(&s, 200, json!({"ok": true, "hide_key": false, "restored": [3, 5]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s), vec![json!({"hide_key": false})]);
    let HideStage::Done(r) = hide(&m) else { panic!() };
    let lines = super::circles_text::hide_result_lines(r, false).join(" ");
    assert!(lines.contains("see the answer keys again") && lines.contains("Versions 3 and 5"), "{lines}");
}

#[test]
fn the_same_setting_again_is_said_as_unchanged() {
    let (mut m, s) = open_bank_as("owner", false, json!({"hide_key": true}), false);
    m.ask_hide(true);
    patch(&s, 200, json!({"ok": true, "hide_key": true, "unchanged": true}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    let HideStage::Done(r) = hide(&m) else { panic!() };
    assert!(r.unchanged);
    assert!(super::circles_text::hide_result_lines(r, false)[0].starts_with("Nothing changed"));
}

#[test]
fn held_or_withheld_after_hiding_is_said_without_naming_why() {
    let r = HideKeyResult { hide_key: true, held: true, withheld: true, ..Default::default() };
    let lines = super::circles_text::hide_result_lines(&r, false).join(" ");
    assert!(lines.contains("held back") && lines.contains("administrator") && !lines.contains("student") && !lines.contains("copyright"), "{lines}");
}

#[test]
fn every_hide_refusal_is_plain_and_frees_the_switch() {
    let cases: Vec<(u16, Value, &str, bool)> = vec![
        (429, json!({"error": "daily_limit", "metric": "circle_hide_attempts", "message": "RAWSERVER"}), "tried to change answer-key settings", true),
        (429, json!({"error": "daily_limit", "metric": "circle_hide_changes", "message": "RAWSERVER"}), "changed answer-key settings", true),
        (409, json!({"error": "conflict", "head_version": 3, "message": "RAWSERVER"}), "new version", false),
        (400, json!({"error": "invalid_request", "field": "labels", "message": "This bank's current version has 2 questions."}), "changed meanwhile", false),
        (403, json!({"error": "circle_role_required", "needed": "owner_or_author", "role": "editor"}), "bank's author", false),
        (503, json!({"error": "busy"}), "busy", true),
        (409, json!({"error": "bank_removed", "reason": "owner_removed"}), "removed", false),
    ];
    for (status, body, want, retry) in cases {
        let (mut m, s) = open_bank_as("owner", false, json!({}), false);
        m.ask_hide(true);
        patch(&s, status, body.clone());
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        let code = body["error"].as_str().unwrap();
        let HideStage::Failed { on, problem, .. } = hide(&m) else { panic!("{code}: {:?}", hide(&m)) };
        assert!(*on);
        assert!(problem.message.contains(want), "{code}: {}", problem.message);
        assert!(!(code.contains('_') && problem.message.contains(code)) && !problem.message.contains("RAWSERVER") && !problem.message.contains('_'), "{code}: {}", problem.message);
        assert_eq!(problem.retry, retry, "{code}");
        // "Try again" asks first again
        m.retry_hide();
        assert!(matches!(hide(&m), HideStage::Confirm { on: true }) || !m.hide_allowed(), "{code}");
    }
}

#[test]
fn a_sign_in_that_ends_while_hiding_keeps_the_decisions() {
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
    s.fail(ApiError::SignedOut);
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    let HideStage::Decide(d) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert_eq!(d.label(Pos::question(1)), Some(LabelChoice::Strip), "the decision made is still there");
    // (critic 57 I2) any other failure keeps it too, and Try again sends it
    s.on("Post", "/circles/heads", 200, heads_reply(2));
    s.fail(ApiError::Offline("The request timed out.".into()));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert!(matches!(hide(&m), HideStage::Failed { back: Some(_), .. }), "{:?}", hide(&m));
    let n = patches(&s).len();
    patch(&s, 200, json!({"ok": true, "hide_key": true}));
    m.retry_hide();
    m.frame(now(), &ready(&s));
    let p = patches(&s);
    assert_eq!(p.len(), n + 1, "Try again sends");
    assert_eq!(p[n], json!({"hide_key": true, "labels": [{"question": 1, "choice": "strip"}]}));
    assert!(matches!(hide(&m), HideStage::Done(_)));
    // the confirmation alone goes back to the switch
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    s.fail(ApiError::SignedOut);
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert!(matches!(hide(&m), HideStage::Idle));
}

#[test]
fn any_other_failure_keeps_the_decisions_and_try_again_sends_them() {
    // (critic 57 I2: a timeout on school Wi-Fi lost every decision, and "Try again" went with none)
    for case in ["timeout", "busy"] {
        let (mut m, s) = open_bank_as("owner", false, json!({}), false);
        m.ask_hide(true);
        patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true}]}));
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
        if case == "timeout" {
            s.on("Post", "/circles/heads", 200, heads_reply(2));
            s.fail(ApiError::Offline("The request timed out.".into()));
        } else {
            patch(&s, 503, json!({"error": "busy"}));
        }
        m.send_hide_decisions();
        m.frame(now(), &ready(&s));
        let HideStage::Failed { problem, .. } = hide(&m) else { panic!("{case}: {:?}", hide(&m)) };
        assert!(problem.retry, "{case}");
        if case == "timeout" {
            assert!(!problem.message.contains("Nothing changed"), "a lost answer can not know that: {}", problem.message);
        }
        let n = patches(&s).len();
        patch(&s, 200, json!({"ok": true, "hide_key": true}));
        m.retry_hide();
        m.frame(now(), &ready(&s));
        let p = patches(&s);
        assert_eq!(p.len(), n + 1, "{case}: one more try, not two");
        assert_eq!(p[n], json!({"hide_key": true, "labels": [{"question": 1, "choice": "strip"}]}), "{case}: Try again goes with the decisions");
        assert!(matches!(hide(&m), HideStage::Done(_)), "{case}");
    }
    // a new version meanwhile drops them: Try again asks first, with nothing decided
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 422, json!({"error": "labels_need_decision", "errors": [], "label_findings": [{"question": 1, "strip_allowed": true}]}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    m.hide_decide_label(Pos::question(1), LabelChoice::Strip);
    patch(&s, 409, json!({"error": "conflict", "head_version": 3}));
    m.send_hide_decisions();
    m.frame(now(), &ready(&s));
    assert!(matches!(hide(&m), HideStage::Failed { .. }));
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    m.ask_hide(true);
    patch(&s, 200, json!({"ok": true, "hide_key": true}));
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s).last(), Some(&json!({"hide_key": true})));
}

/// Every text the Circles tab draws for `m`, one frame on a tall screen.
fn page_text(m: &mut CirclesModel) -> String {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 6000.0))), time: Some(0.0), ..Default::default() };
    let out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            super::circles_ui::body_of(ui, m);
        });
    });
    let mut all = String::new();
    for s in &out.shapes {
        if let egui::epaint::Shape::Text(t) = &s.shape {
            all.push_str(t.galley.text());
            all.push('\n');
        }
    }
    out.drop_without_applying_deltas();
    all
}

#[test]
fn a_hide_that_holds_the_newest_version_back_still_offers_turning_it_off() {
    // (critic 58 B2: the head read failed once hiding withheld it, the page lost the switch, and said only an
    // administrator can release it; turning hiding off is what releases it)
    let (mut m, s) = open_bank_as("owner", false, json!({}), false);
    m.ask_hide(true);
    patch(&s, 200, json!({"ok": true, "hide_key": true, "held": true, "withheld": true}));
    let mut held = bank_row(false, json!(true));
    held["held"] = json!(true);
    s.keep("Get", &format!("/circles/{CID}"), 200, super::circles_tests::detail("owner", json!({"banks": [held]})));
    s.act("Get", &format!("/banks/{BID}"), 404, json!({"error": "version_withheld", "hide_off_releases": true, "message": "RAWSERVER"}));
    m.confirm_hide();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)), "the newest version is withheld now");
    let t = page_text(&mut m);
    assert!(t.contains("Hide answer keys from viewers") && t.contains("Turn hiding off"), "the switch and the way out stay: {t}");
    assert!(t.contains("turning hiding off makes it readable again"), "{t}");
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    for width in [1280.0, 760.0, 420.0] {
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, &mut m);
        });
    }
    assert!(!t.contains("This version is hidden while it is reviewed") && !t.contains("Sending a clean version") && !t.contains("RAWSERVER"), "{t}");
    // turning it off goes (with no head check: nothing positional is sent) and the bank reads again
    m.ask_hide(false);
    assert!(matches!(hide(&m), HideStage::Confirm { on: false }), "{:?}", hide(&m));
    patch(&s, 200, json!({"ok": true, "hide_key": false, "restored": [2]}));
    let reads = heads_reads(&s);
    m.confirm_hide();
    m.frame(now(), &ready(&s));
    assert_eq!(patches(&s).last(), Some(&json!({"hide_key": false})));
    assert_eq!(heads_reads(&s), reads);
    let HideStage::Done(r) = hide(&m) else { panic!("{:?}", hide(&m)) };
    assert!(!r.hide_key && r.restored == vec![2]);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Ready(_)));
    // nobody else gets it: a viewer, another editor, or a withheld head of a bank that does not hide its keys
    for (role, hides) in [("viewer", true), ("editor", true), ("owner", false)] {
        let mut row = bank_row(false, json!(hides));
        row["held"] = json!(true);
        let (mut m, s) = open_circle(role, json!({"banks": [row]}));
        s.act("Get", &format!("/banks/{BID}"), 404, json!({"error": "version_withheld"}));
        m.open_bank(BID);
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)), "{role}");
        let n = patches(&s).len();
        m.ask_hide(false);
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        assert_eq!(patches(&s).len(), n, "{role} {hides}");
        assert!(!page_text(&mut m).contains("Turn hiding off"), "{role} {hides}");
    }
}

#[test]
fn a_withheld_head_that_turning_hiding_off_would_not_release_keeps_the_switch_both_ways() {
    // (critic 59 B1: a send to a bank that hides its keys was withheld; "Turn hiding off" was offered, said to help,
    // released nothing, and then the switch to turn hiding on again was gone)
    for said in [json!({"error": "version_withheld", "hide_off_releases": false, "message": "RAWSERVER"}), json!({"error": "version_withheld"})] {
        let mut row = bank_row(false, json!(true));
        row["held"] = json!(true);
        let (mut m, s) = open_circle("owner", json!({"banks": [row.clone()]}));
        s.keep("Get", &format!("/circles/{CID}/banks/{BID}"), 404, said.clone());
        m.open_bank(BID);
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)), "{said}");
        assert!(!m.hide_off_offered(), "{said}");
        let t = page_text(&mut m);
        assert!(!t.contains("Turn hiding off") && !t.contains("turning hiding off makes it readable again") && !t.contains("RAWSERVER"), "{t}");
        assert!(t.contains("Only an administrator can release it") && t.contains("Hide answer keys from viewers"), "the plain words, and the switch: {t}");
        // turned off with the switch anyway: it goes, nothing is restored, and the head is still withheld
        m.ask_hide(false);
        assert!(matches!(hide(&m), HideStage::Confirm { on: false }), "{:?}", hide(&m));
        patch(&s, 200, json!({"ok": true, "hide_key": false}));
        // (the circle page, read again after the change, says hiding is off)
        let mut off = row.clone();
        off["hide_key"] = json!(false);
        s.keep("Get", &format!("/circles/{CID}"), 200, super::circles_tests::detail("owner", json!({"banks": [off]})));
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        assert_eq!(patches(&s).last(), Some(&json!({"hide_key": false})));
        for _ in 0..4 {
            m.frame(now(), &ready(&s));
        }
        assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)));
        m.cancel_hide();
        assert!(!m.hide_key_on() && !m.hide_off_offered());
        let t = page_text(&mut m);
        assert!(t.contains("Hide answer keys from viewers") && !t.contains("Turn hiding off"), "the switch stays: {t}");
        // nothing to send for the setting it already has
        let n = patches(&s).len();
        m.ask_hide(false);
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        assert_eq!(patches(&s).len(), n);
        // hiding goes back on from the withheld head (no head check: there is no version read to check against)
        m.ask_hide(true);
        assert!(matches!(hide(&m), HideStage::Confirm { on: true }), "{:?}", hide(&m));
        patch(&s, 200, json!({"ok": true, "hide_key": true}));
        let reads = heads_reads(&s);
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        assert_eq!(patches(&s).last(), Some(&json!({"hide_key": true})));
        assert_eq!(heads_reads(&s), reads);
        assert!(matches!(hide(&m), HideStage::Done(r) if r.hide_key), "{:?}", hide(&m));
    }
}

// ---- what viewers get --------------------------------------------------------------------------------------------

#[test]
fn a_viewer_reads_the_stripped_bank_and_gets_a_keyless_plain_copy() {
    let (mut m, s) = open_bank_as("viewer", false, json!({"hide_key": true, "keys_hidden": true}), true);
    assert!(m.keys_hidden_here() && m.hides_keys_here());
    let BankLoad::Ready(l) = &m.bank.as_ref().unwrap().load else { panic!() };
    assert!(l.view.as_ref().is_ok_and(|v| v.warnings.is_empty()), "the viewers' form reads as one with no keys");
    assert!(!m.can_make_working_copy());
    m.begin_copy();
    m.get_copy();
    m.frame(now(), &ready(&s));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    assert!(m.sync(&mut banks, &mut browse));
    assert_eq!(banks.len(), 1);
    assert!(banks[0].questions.iter().all(crate::model::is_keyless), "every question is marked as having no key");
    let local = banks[0].id.clone();
    assert!(m.edit.link_for_local(&local).is_none(), "never a working copy");
    // (critic 57) the head's `keys_hidden` alone marks the copy: here no question carries the flag itself
    let (mut m, s) = open_bank_as("viewer", false, json!({"hide_key": true, "keys_hidden": true}), false);
    s.keep_raw("Get", &format!("/circles/{CID}/banks/{BID}"), stripped_without_flags());
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    assert!(m.keys_hidden_here());
    m.begin_copy();
    m.get_copy();
    m.frame(now(), &ready(&s));
    let mut banks: Vec<QuestionBank> = Vec::new();
    let mut browse = BrowseModel::new(Runner::Inline, Arc::new(Mem::default()), ImportLog::memory());
    assert!(m.sync(&mut banks, &mut browse));
    assert_eq!(banks.len(), 1);
    assert!(!banks[0].questions.is_empty());
    assert!(banks[0].questions.iter().all(crate::model::is_keyless), "a stripped read is marked as having no keys, flag or not");
}

/// The viewers' form of a bank whose questions do not carry `keyless` themselves (no key marked either).
fn stripped_without_flags() -> Vec<u8> {
    let qs: Vec<Value> = ["First", "Second"]
        .iter()
        .map(|s| json!({"id": format!("src-{s}"), "stem": s, "kind": {"Mc": {"answers": [{"text": "A", "correct": false}, {"text": "B", "correct": false}]}}, "points": 1.0, "difficulty": null, "created": 5, "modified": 6}))
        .collect();
    let payload = json!({"schema_version": 2, "bank": {"id": "b-src", "name": "Cells", "questions": qs, "created": 1, "modified": 2}}).to_string();
    let bank = json!({"id": BID, "title": "Cells", "subject": "Biology", "license": "CC-BY-4.0", "version_license": "CC-BY-4.0", "author_name": "Mr Keesler", "is_author": false, "version": 2, "version_id": "v2", "question_count": 2, "has_images": false, "committed_at": NOW - DAY, "hide_key": true, "keys_hidden": true});
    let h = json!({"circle": {"id": CID, "name": "Biology team"}, "bank": bank, "origins": null}).to_string();
    format!("{},\"payload\":{}}}", &h[..h.len() - 1], payload).into_bytes()
}

#[test]
fn an_editor_never_makes_a_working_copy_from_a_stripped_read() {
    // (a viewer promoted while the page was open still holds the viewers' form)
    let (mut m, s) = open_bank_as("editor", false, json!({"hide_key": true, "keys_hidden": true}), true);
    assert!(!m.can_make_working_copy());
    m.begin_copy();
    let n = s.seen().len();
    m.make_working_copy();
    m.frame(now(), &ready(&s));
    assert_eq!(s.seen().len(), n, "nothing is fetched for a working copy");
    // an editor's full read of a hiding bank is a normal working copy
    let (m, _) = open_bank_as("editor", false, json!({"hide_key": true}), false);
    assert!(!m.keys_hidden_here() && m.hides_keys_here());
    assert!(m.can_make_working_copy());
}

#[test]
fn a_viewers_hidden_read_refusals_are_plain() {
    for (status, body, want) in [
        (409, json!({"error": "keys_hidden_unavailable", "message": "RAWSERVER"}), "can not be shown without them"),
        (429, json!({"error": "daily_limit", "metric": "circle_hidden_reads", "message": "RAWSERVER"}), "answer keys are hidden"),
        (503, json!({"error": "busy"}), "busy"),
    ] {
        let (mut m, s) = open_circle("viewer", json!({"banks": [bank_row(false, json!(true))]}));
        s.act("Get", &format!("/banks/{BID}"), status, body.clone());
        m.open_bank(BID);
        m.frame(now(), &ready(&s));
        m.frame(now(), &ready(&s));
        let BankLoad::Failed(p) = &m.bank.as_ref().unwrap().load else { panic!("{body}") };
        let code = body["error"].as_str().unwrap();
        assert!(p.message.contains(want), "{code}: {}", p.message);
        assert!(!(code.contains('_') && p.message.contains(code)) && !p.message.contains("RAWSERVER"), "{code}: {}", p.message);
    }
}

#[test]
fn heads_and_rows_with_null_flags_are_read() {
    let mut row = bank_row(false, Value::Null);
    row["held"] = Value::Null;
    let (m, _) = open_circle("viewer", json!({"banks": [row]}));
    let d = m.circle.as_ref().unwrap().detail.as_ref().expect("a null hide_key is read as false");
    assert!(!d.banks[0].hide_key);
    let (m, _) = open_bank_as("viewer", false, json!({"hide_key": null, "keys_hidden": null}), false);
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Ready(_)));
    assert!(!m.keys_hidden_here());
}

// ---- purge, trim, evidence ---------------------------------------------------------------------------------------

#[test]
fn a_partial_purge_is_said_honestly_and_offers_no_undo() {
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/purge"), 200, json!({"ok": true, "purged_partial": true, "pending_review": true, "evidence_kept": 2, "message": "RAWSERVER"}));
    m.purge_bank(BID);
    m.frame(now(), &ready(&s));
    let Some(super::model::Notice::Info(n)) = &m.notice else { panic!("{:?}", m.notice) };
    assert!(n.contains("Mostly deleted") && n.contains("2 versions are kept") && n.contains("Undo is no longer possible") && !n.contains("RAWSERVER"), "{n}");
    assert_eq!(m.purge_claimed.get(BID), Some(&2));
    // the page is read again; the pool still lists the bank as removed with time left: no Undo, no second purge
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let k = s.seen().len();
    m.restore_bank(BID);
    m.purge_bank(BID);
    assert_eq!(s.seen().len(), k);
    let line = super::circles_text::purge_pending_line(2);
    assert!(line.contains("can not be brought back") && line.contains("2 versions"), "{line}");
    // a whole purge is just that
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/purge"), 200, json!({"ok": true}));
    m.purge_bank(BID);
    m.frame(now(), &ready(&s));
    assert!(matches!(&m.notice, Some(super::model::Notice::Info(n)) if n.contains("deleted for good")));
    assert!(m.purge_claimed.is_empty());
    // an answer with nothing readable in it is a plain purge
    let s2 = Script::new();
    s2.act("Post", &format!("/banks/{BID}/purge"), 200, json!(null));
    let p = super::transport::block_on(async { Pool::new(s2.as_ref()).purge_circle_bank(CID, BID).await }).unwrap();
    assert!(!p.partial);
}

#[test]
fn an_undo_after_the_purge_started_is_explained() {
    let (mut m, s) = open_circle("owner", json!({"banks": [removed_bank("owner_removed", false, json!(NOW + DAY))]}));
    s.act("Post", &format!("/banks/{BID}/undelete"), 409, json!({"error": "bank_removed", "reason": "owner_removed", "message": "This bank can no longer be restored."}));
    m.restore_bank(BID);
    m.frame(now(), &ready(&s));
    let p = m.circle.as_ref().unwrap().act_problem.as_ref().map(|p| p.message.clone()).unwrap_or_default();
    assert!(p.contains("can no longer be brought back"), "{p}");
}

#[test]
fn the_storage_full_text_points_to_what_frees_space() {
    let t = said(413, json!({"error": "quota_exceeded", "scope": "circle", "used": 90966, "needed": 30511, "limit": 106127, "message": "RAWSERVER"}));
    assert!(t.contains("Trim") && t.contains("deleting it for good") && t.contains("only an administrator can free that space"), "{t}");
    assert!(!t.contains("RAWSERVER"));
}

#[test]
fn a_pruned_review_copy_and_pending_review_say_nothing_about_why() {
    let t = said(404, json!({"error": "version_not_retained", "withheld": true, "review_copy": "hashes_only", "oldest_kept": 3, "pinned_versions": [], "message": "RAWSERVER"}));
    assert!(t.contains("administrator") && t.contains("no longer kept") && !t.contains("student"), "{t}");
    let plain = said(404, json!({"error": "version_not_retained", "oldest_kept": 3}));
    assert!(plain.contains("last 20 versions"), "{plain}");
    for t in [said(409, json!({"error": "pending_review", "message": "RAWSERVER"})), said(404, json!({"error": "version_withheld"}))] {
        assert!(!t.contains("student") && !t.contains("copyright") && t.contains("administrator"), "{t}");
    }
}

// ---- activity ----------------------------------------------------------------------------------------------------

#[test]
fn the_new_activity_kinds_read_as_sentences() {
    use super::circles_text::activity_line;
    let e = |what: &str, extra: Value| {
        let mut v = json!({"at": NOW, "what": what, "by": "Ms Frizzle", "member": null});
        merge(&mut v, extra);
        serde_json::from_value::<ActivityEntry>(v).unwrap()
    };
    let l = activity_line(&e("email_invited", json!({"role": "editor"})));
    assert_eq!(l, "Ms Frizzle emailed an invitation to join as editor.");
    let l = activity_line(&e("hide_key_changed", json!({"hide_key": true, "bank_id": BID, "bank_title": "Cells"})));
    assert_eq!(l, "Ms Frizzle hid the answer keys of \"Cells\" from viewers.");
    let l = activity_line(&e("hide_key_changed", json!({"hide_key": false, "bank_id": BID, "bank_title": null})));
    assert_eq!(l, "Ms Frizzle showed the answer keys of a bank to viewers again.");
    let l = activity_line(&e("rolled_back", json!({"bank_id": BID, "bank_title": "Cells", "rollback_of": 2})));
    assert!(l.contains("restored an older version") && l.contains("version 2"), "{l}");
    let l = activity_line(&e("versions_trimmed", json!({"bank_id": BID, "bank_title": "Cells"})));
    assert!(l.contains("trimmed"), "{l}");
    for what in ["email_invited", "hide_key_changed", "rolled_back", "versions_trimmed"] {
        let l = activity_line(&e(what, json!({"role": null, "hide_key": null})));
        assert!(!l.contains(what) && !l.contains("does not know"), "{what}: {l}");
    }
}

#[test]
fn the_activity_cursor_is_passed_back_as_given_and_a_refused_one_starts_the_list_again() {
    let (mut m, s) = open_circle("owner", json!({}));
    let cursor = "1760000000000:0123456789abcdef0123456789abcdef";
    s.act("Get", "/activity", 200, json!({"activity": [{"at": NOW, "what": "email_invited", "by": "Ms Frizzle", "role": "viewer"}], "next_before": cursor}));
    m.circle.as_mut().unwrap().activity_loaded = false;
    m.frame(now(), &ready(&s));
    m.frame(now(), &ready(&s));
    s.act("Get", "/activity", 400, json!({"error": "invalid_request", "field": "before", "message": "Use the next_before of the previous page."}));
    s.act("Get", "/activity", 200, json!({"activity": [{"at": NOW, "what": "joined", "member": "Ms Viewer", "role": "viewer"}], "next_before": null}));
    m.more_activity();
    for _ in 0..3 {
        m.frame(now(), &ready(&s));
    }
    let paths = s.paths();
    assert!(paths.iter().any(|p| p.ends_with(&format!("/activity?before={cursor}"))), "{paths:?}");
    assert!(paths.last().is_some_and(|p| p.ends_with("/activity")), "the list started again: {paths:?}");
    let c = m.circle.as_ref().unwrap();
    assert!(c.activity_problem.is_none() && c.activity_loaded && c.activity.len() == 1);
    // an odd cursor is encoded, not refused
    let s2 = Script::new();
    s2.act("Get", "/activity", 200, json!({"activity": [], "next_before": null}));
    super::transport::block_on(async { Pool::new(s2.as_ref()).circle_activity(CID, Some("12:a b+c/d")).await }).unwrap();
    assert!(s2.paths()[0].ends_with("?before=12:a%20b%2Bc%2Fd"), "{:?}", s2.paths());
}

// ---- no raw codes ------------------------------------------------------------------------------------------------

#[test]
fn every_new_code_maps_to_plain_words() {
    let cases = [
        (422, json!({"error": "invite_not_sent", "reason": "address_limit"})),
        (422, json!({"error": "invite_not_sent", "reason": "not_deliverable"})),
        (422, json!({"error": "invite_not_sent"})),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_invites_hourly"})),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_inviter_domain_hourly", "limit": 10})),
        (429, json!({"error": "hourly_limit", "metric": "circle_email_domain_hourly"})),
        (429, json!({"error": "hourly_limit"})),
        (409, json!({"error": "keys_hidden_unavailable"})),
        (409, json!({"error": "hide_key_changed", "hide_key": true, "head_version": 4})),
        (409, json!({"error": "hide_key_changed", "hide_key": false, "head_version": 4})),
        (422, json!({"error": "keyless_question", "positions": [0, 1, 3], "kinds": ["Short", "Mc", "Tf"]})),
        (429, json!({"error": "daily_limit", "metric": "circle_hide_attempts"})),
        (429, json!({"error": "daily_limit", "metric": "circle_hide_changes"})),
        (429, json!({"error": "daily_limit", "metric": "circle_hidden_reads"})),
        (429, json!({"error": "daily_limit", "metric": "circle_email_invites"})),
        (422, json!({"error": "circles_limit", "metric": "circle_open_email_invites", "used": 40, "limit": 40})),
        (404, json!({"error": "version_not_retained", "withheld": true, "review_copy": "hashes_only"})),
        (413, json!({"error": "quota_exceeded", "scope": "circle", "used": 1, "limit": 2})),
        (409, json!({"error": "pending_review"})),
        (400, json!({"error": "invalid_request", "field": "email"})),
        (400, json!({"error": "invalid_request", "field": "before"})),
    ];
    for (status, body) in cases {
        let t = said(status, body.clone());
        let code = body["error"].as_str().unwrap();
        assert!(!t.is_empty(), "{body}");
        assert!(!t.contains(code) && !t.contains('_') && !t.contains("refused the request"), "{body}: {t}");
    }
    // the keyless words fit the kinds
    let t = said(422, json!({"error": "keyless_question", "positions": [4], "kinds": ["Short"]}));
    assert!(t.contains("question 5") && t.contains("sample answer"), "{t}");
    let t = said(422, json!({"error": "keyless_question", "positions": [0, 2], "kinds": ["Mc", "Tf"]}));
    assert!(t.contains("questions 1 and 3") && t.contains("mark the correct answer") && t.contains("editor"), "{t}");
    assert_eq!(question_numbers(&(0..12).collect::<Vec<_>>()), "questions 1, 2, 3, 4, 5, 6 ... and 6 more");
    // label codes inside a payload refusal have plain words too
    for code in ["labels_unused", "labels_out_of_range", "labels_duplicate", "labels_strip_refused", "labels_need_decision", "order_ack_required"] {
        let t = super::publish_text::generic_code_text(code, None);
        assert!(!t.contains(code) && !t.contains("refused it"), "{code}: {t}");
    }
}

// ---- smoke frames ------------------------------------------------------------------------------------------------

#[test]
fn every_new_state_draws_wide_and_narrow() {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    let draw = |m: &mut CirclesModel, width: f32| {
        frame(&ctx, width, &mut |ui| {
            super::circles_ui::body_of(ui, m);
        });
    };
    for width in [1280.0, 760.0, 420.0] {
        // the email form: empty, typed, refused, sent, resent
        let (mut m, s) = owner_email("", Role::Editor);
        draw(&mut m, width);
        email_form(&mut m).address = "jo@school".into();
        draw(&mut m, width);
        email_form(&mut m).address = "jo@school.org".into();
        s.act("Post", EMAIL_PATH, 429, json!({"error": "hourly_limit", "metric": "circle_email_inviter_domain_hourly", "limit": 10}));
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
        s.act("Post", EMAIL_PATH, 202, sent_answer(json!({"resent": true})));
        m.send_email_invite();
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
        // the bank page: viewer's stripped bank, its copy panel
        let (mut m, _) = open_bank_as("viewer", false, json!({"hide_key": true, "keys_hidden": true}), true);
        draw(&mut m, width);
        m.begin_copy();
        draw(&mut m, width);
        // the owner's switch in every stage
        let (mut m, s) = open_bank_as("owner", false, json!({}), false);
        draw(&mut m, width);
        m.ask_hide(true);
        draw(&mut m, width);
        patch(&s, 422, json!({"error": "labels_need_decision", "errors": [{"code": "key_in_formatting", "path": "bank.questions[0].kind.Mc.answers[1].text", "reason": "capitals", "ack_allowed": true, "hint": "Check the capitals."}, {"code": "key_in_formatting", "path": "bank.questions[2].kind.Mc.answers[0].text", "reason": "star", "ack_allowed": false}],
            "label_findings": [{"question": 1, "order_revealing": true, "strip_allowed": true, "choices": [{"before": "A) Mitosis is the division of a cell nucleus into two identical nuclei", "after": "Mitosis is the division of a cell nucleus into two identical nuclei"}]}],
            "needs_order_ack": [{"question": 2}, {"question": 0, "part": 1}], "order_reasons": [{"question": 2, "reason": "sibling_refs"}],
            "formatting_findings": [{"question": 0, "code": "key_in_formatting", "path": "x", "ack_allowed": true, "served": "B) MITOSIS"}]}));
        m.confirm_hide();
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
        patch(&s, 200, json!({"ok": true, "hide_key": true, "held": true, "withheld": true, "labels_removed": [{"question": 1}], "images_removed": [0, 2], "figures_removed": [1]}));
        if let Some(b) = m.bank.as_mut() {
            b.hide = HideStage::Decide(Box::new(super::circle_hide::Decisions::new(PayloadRefusal::default(), StripDecisions::default())));
        }
        m.send_hide_decisions();
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
        if let Some(b) = m.bank.as_mut() {
            b.hide = HideStage::Failed { on: true, problem: super::browse::Problem { message: said(429, json!({"error": "daily_limit", "metric": "circle_hide_attempts"})), retry: true, retry_at: None, account: false }, back: None };
        }
        draw(&mut m, width);
        // the circle page: a hiding bank, a partly purged bank, emailed invitations
        let (mut m, s) = open_circle("owner", json!({"banks": [bank_row(true, json!(true)), removed_bank("owner_removed", false, json!(NOW + DAY))]}));
        m.purge_claimed.insert(BID.into(), 3);
        s.keep("Get", &format!("/circles/{CID}/invites"), 200, json!({"invites": [{"invite_id": "a".repeat(32), "kind": "email", "address_label": "j***@pleasant-valley-high-school.example.org #2", "role": "editor", "created_at": NOW - 3_600_000, "created_by_name": "Ms Frizzle", "max_uses": 1, "uses": 0, "expires_at": NOW + 7 * DAY, "state": "active"}], "next_cursor": null}));
        m.circle.as_mut().unwrap().invites = None;
        m.frame(now(), &ready(&s));
        draw(&mut m, width);
    }
}

// ---- round 50: critic 60 B1, B2 (the client's words) and I1 (another account) -------------------------------------

/// `ready`, signed in as `who` (the pool's address and the account's email, as the Account window gives it).
fn ready_as(s: &Arc<Script>, who: &str) -> super::publish::AccountSnapshot {
    super::publish::AccountSnapshot { account: Some(format!("https://pool.test\n{who}@school.org")), ..ready(s) }
}

#[test]
fn c60_b1_turning_hiding_on_over_a_withheld_head_never_says_turning_it_off_helps() {
    // (critic 60 B1: a send's match withheld the head, the pool says hide_off_releases: false; turning hiding on from
    // the switch said "Turning hiding off makes it readable again" beside "Only an administrator can release it")
    // The answer of an older pool (the bank's state) and of this one (what the PATCH did): neither promises it.
    for answer in [json!({"ok": true, "hide_key": true, "held": true, "withheld": true}), json!({"ok": true, "hide_key": true, "held": false, "withheld": false})] {
        let mut row = bank_row(false, json!(false));
        row["held"] = json!(true);
        let (mut m, s) = open_circle("owner", json!({"banks": [row.clone()]}));
        s.keep("Get", &format!("/circles/{CID}/banks/{BID}"), 404, json!({"error": "version_withheld", "hide_off_releases": false, "message": "RAWSERVER"}));
        m.open_bank(BID);
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert!(m.hide_switch_on_held_head() && !m.hide_off_offered());
        m.ask_hide(true);
        assert!(matches!(hide(&m), HideStage::Confirm { on: true }), "{:?}", hide(&m));
        patch(&s, 200, answer.clone());
        let mut on = row.clone();
        on["hide_key"] = json!(true);
        s.keep("Get", &format!("/circles/{CID}"), 200, super::circles_tests::detail("owner", json!({"banks": [on]})));
        m.confirm_hide();
        for _ in 0..5 {
            m.frame(now(), &ready(&s));
        }
        assert!(matches!(hide(&m), HideStage::Done(r) if r.hide_key), "{:?}", hide(&m));
        assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)) && !m.hide_off_offered());
        let t = page_text(&mut m);
        assert!(!t.to_lowercase().contains("turning hiding off makes it readable again") && !t.contains("Turn hiding off"), "{answer}: {t}");
        assert!(t.contains("Only an administrator can release it") && !t.contains("RAWSERVER"), "{answer}: {t}");
    }
}

#[test]
fn c60_b2_a_turn_off_that_released_nothing_says_the_newest_version_is_still_held_back_and_keeps_the_switch() {
    // (critic 59 B1 fix item 4, critic 60 B2 step 4: "Viewers see the answer keys again" was the only line, and
    // nothing said the newest version was still held back.) Whether the pool had said it would release it or not.
    for releases in [true, false] {
        let mut row = bank_row(false, json!(true));
        row["held"] = json!(true);
        let (mut m, s) = open_circle("owner", json!({"banks": [row.clone()]}));
        let head = format!("/circles/{CID}/banks/{BID}");
        s.keep("Get", &head, 404, json!({"error": "version_withheld", "hide_off_releases": releases, "message": "RAWSERVER"}));
        m.open_bank(BID);
        for _ in 0..3 {
            m.frame(now(), &ready(&s));
        }
        assert_eq!(m.hide_off_offered(), releases);
        m.ask_hide(false);
        assert!(matches!(hide(&m), HideStage::Confirm { on: false }), "{:?}", hide(&m));
        patch(&s, 200, json!({"ok": true, "hide_key": false}));
        let mut off = row.clone();
        off["hide_key"] = json!(false);
        s.keep("Get", &format!("/circles/{CID}"), 200, super::circles_tests::detail("owner", json!({"banks": [off]})));
        s.keep("Get", &head, 404, json!({"error": "version_withheld", "hide_off_releases": false, "message": "RAWSERVER"}));
        m.confirm_hide();
        for _ in 0..5 {
            m.frame(now(), &ready(&s));
        }
        assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Failed(_)), "{releases}");
        assert!(matches!(hide(&m), HideStage::Done(r) if !r.hide_key && r.restored.is_empty()), "{:?}", hide(&m));
        let t = page_text(&mut m);
        // (critic 61 M3: while the bank is held viewers see nothing of it, so "Viewers see the answer keys again" is not said)
        assert!(!t.contains("see the answer keys again") && t.contains("did not release the newest version") && t.contains("still held back"), "{releases}: {t}");
        assert!(!t.contains("Turn hiding off") && !t.contains("RAWSERVER"), "{releases}: {t}");
        // the switch to turn hiding on again stays
        assert!(m.hide_switch_on_held_head() && !m.hide_key_on() && t.contains("Hide answer keys from viewers"), "{releases}: {t}");
        m.ask_hide(true);
        assert!(matches!(hide(&m), HideStage::Confirm { on: true }), "{:?}", hide(&m));
    }
}

#[test]
fn c60_b2_a_turn_off_that_released_the_head_does_not_say_it_is_still_held() {
    let (mut m, s) = open_bank_as("owner", false, json!({"hide_key": true}), false);
    m.ask_hide(false);
    patch(&s, 200, json!({"ok": true, "hide_key": false, "restored": [2]}));
    m.confirm_hide();
    for _ in 0..4 {
        m.frame(now(), &ready(&s));
    }
    assert!(matches!(m.bank.as_ref().unwrap().load, BankLoad::Ready(_)));
    let t = page_text(&mut m);
    assert!(t.contains("see the answer keys again") && t.contains("Version 2 had been held back") && !t.contains("still held back"), "{t}");
}

/// The owner on a bank that hides its keys, its newest version withheld by an answer-key change ("Turn hiding off").
fn owner_on_released_head() -> (CirclesModel, Arc<Script>) {
    let mut held = bank_row(false, json!(true));
    held["held"] = json!(true);
    let (mut m, s) = open_circle("owner", json!({"banks": [held]}));
    s.keep("Get", &format!("/circles/{CID}/banks/{BID}"), 404, json!({"error": "version_withheld", "hide_off_releases": true, "message": "RAWSERVER"}));
    m.open_bank(BID);
    for _ in 0..3 {
        m.frame(now(), &ready_as(&s, "olga"));
    }
    assert!(m.hide_off_offered() && page_text(&mut m).contains("Turn hiding off"));
    (m, s)
}

/// A viewer of the same circle, on its own session: the pool tells a viewer only `not_found` for a held bank.
fn viewer_session() -> Arc<Script> {
    let s2 = Script::new();
    s2.keep("Get", "/circles", 200, json!({"circles": [super::circles_tests::summary("viewer", json!({}))]}));
    s2.keep("Get", &format!("/circles/{CID}"), 200, super::circles_tests::detail("viewer", json!({"banks": []})));
    s2.keep("Get", &format!("/circles/{CID}/activity"), 200, json!({"activity": [], "next_before": null}));
    s2.keep("Get", &format!("/circles/{CID}/banks/{BID}"), 404, json!({"error": "not_found"}));
    s2
}

fn nothing_of_the_owner_is_left(m: &mut CirclesModel, s2: &Arc<Script>, how: &str) {
    assert!(!m.hide_off_offered() && !m.hide_switch_on_held_head(), "{how}");
    assert!(m.bank.is_none() && m.circle.is_none(), "{how}: the bank page and the circle page are gone");
    assert!(matches!(m.edit.view, super::circle_edit::EditView::Closed), "{how}");
    assert_eq!(m.screen, Screen::List, "{how}");
    let t = page_text(m);
    assert!(!t.contains("Turn hiding off") && !t.contains(super::circles_text::HIDE_HELD_HEAD) && !t.contains("Hide answer keys from viewers"), "{how}: {t}");
    assert!(s2.paths().iter().all(|p| !p.contains("/banks/")), "{how}: the viewer's session asked for nothing of the owner's: {:?}", s2.paths());
    assert!(s2.paths().contains(&"Get /circles".to_string()), "{how}: the list is read with the new session: {:?}", s2.paths());
}

#[test]
fn c60_i1_another_account_signing_in_never_sees_the_first_accounts_bank_page() {
    // (critic 60 I1) After a sign-out: the Shared page shows the sign-in card, not the tabs, so the model is told of
    // the account each frame (SharedUi) but not framed.
    let (mut m, s) = owner_on_released_head();
    m.note_account(&super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut));
    assert!(!m.hide_off_offered(), "nothing read for the owner stays once signed out");
    assert!(m.bank.as_ref().is_none_or(|b| matches!(b.load, BankLoad::Loading)) && m.circle.as_ref().is_none_or(|c| c.detail.is_none()));
    let before = s.seen().len();
    let s2 = viewer_session();
    for _ in 0..3 {
        m.note_account(&ready_as(&s2, "vic"));
        m.frame(now(), &ready_as(&s2, "vic"));
    }
    nothing_of_the_owner_is_left(&mut m, &s2, "after a sign-out");
    assert_eq!(s.seen().len(), before, "the owner's session is not used again");
    // Without a sign-out in between (another account's session, straight away): the same.
    let (mut m, _s) = owner_on_released_head();
    let s2 = viewer_session();
    for _ in 0..3 {
        m.frame(now(), &ready_as(&s2, "vic"));
    }
    nothing_of_the_owner_is_left(&mut m, &s2, "straight away");
}

#[test]
fn c60_i1_the_same_account_signing_in_again_is_back_where_it_was_with_the_bank_read_again() {
    let (mut m, s) = owner_on_released_head();
    let reads = |s: &Arc<Script>| s.paths().iter().filter(|p| *p == &format!("Get /circles/{CID}/banks/{BID}")).count();
    let n = reads(&s);
    m.note_account(&super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut));
    assert!(!m.hide_off_offered());
    for _ in 0..4 {
        m.note_account(&ready_as(&s, "olga"));
        m.frame(now(), &ready_as(&s, "olga"));
    }
    assert_eq!(m.screen, Screen::Bank, "where the person was is kept");
    assert_eq!(reads(&s), n + 1, "the bank is read again, not taken from before the sign-out");
    assert!(m.hide_off_offered(), "what the pool says again");
}

#[test]
fn c60_i1_another_account_never_gets_the_first_accounts_refused_versions() {
    // A viewer's history: version 2 has no viewers' copy (keys_hidden_unavailable), kept per version.
    let (mut m, s) = open_circle("viewer", json!({}));
    let path = format!("/circles/{CID}/banks/{BID}");
    let lines: Vec<Value> = [3u64, 2, 1]
        .iter()
        .map(|v| json!({"version": v, "version_id": format!("{v:032x}"), "by": "Mr Keesler", "at": NOW - DAY * (*v as i64), "message": format!("note {v}"), "added": 0, "changed": 1, "removed": 0, "question_count": 2, "license": "CC-BY-4.0", "kept": true, "withheld": false}))
        .collect();
    s.keep("Get", &format!("{path}/versions"), 200, json!({"bank_id": BID, "head_version": 3, "oldest_kept": 1, "pinned_versions": [], "versions": lines, "next_before": null}));
    s.on("Get", &format!("{path}/versions/2"), 409, json!({"error": "keys_hidden_unavailable"}));
    m.edit.open_history(CID, BID, "Cells", super::circle_edit::Perm { role: Role::Viewer, is_author: false, closed: false });
    for _ in 0..2 {
        m.frame(now(), &ready_as(&s, "vic"));
    }
    m.edit.view_version(2);
    for _ in 0..2 {
        m.frame(now(), &ready_as(&s, "vic"));
    }
    m.edit.back();
    let super::circle_edit::EditView::History(h) = &m.edit.view else { panic!("{:?}", std::mem::discriminant(&m.edit.view)) };
    assert!(h.keys_closed.contains(&2), "the refusal is kept for this viewer");
    // The owner signs in on this computer.
    let s2 = Script::new();
    s2.keep("Get", "/circles", 200, json!({"circles": [super::circles_tests::summary("owner", json!({}))]}));
    m.note_account(&super::publish::AccountSnapshot::of_gate(super::publish::Gate::SignedOut));
    for _ in 0..2 {
        m.note_account(&ready_as(&s2, "olga"));
        m.frame(now(), &ready_as(&s2, "olga"));
    }
    assert!(matches!(m.edit.view, super::circle_edit::EditView::Closed), "the viewer's history and its refusals are gone");
    assert!(m.circle.is_none() && m.screen == Screen::List);
}

#[test]
fn c61_m3_a_turn_off_that_left_the_bank_held_does_not_say_viewers_see_the_keys_again() {
    let r = HideKeyResult { hide_key: false, restored: vec![2], ..HideKeyResult::default() };
    let held = super::circles_text::hide_result_lines(&r, true);
    assert_eq!(held[0], super::circles_text::HIDE_OFF_STILL_HELD, "the headline says the newest version is still held back");
    assert!(held.iter().all(|l| !l.contains("see the answer keys again")), "{held:?}");
    assert!(held.iter().any(|l| l.starts_with("Version 2 had been held back")), "{held:?}");
    let free = super::circles_text::hide_result_lines(&r, false);
    assert!(free[0].starts_with("Viewers see the answer keys again"), "{free:?}");
    assert!(free.iter().all(|l| l != super::circles_text::HIDE_OFF_STILL_HELD));
}
