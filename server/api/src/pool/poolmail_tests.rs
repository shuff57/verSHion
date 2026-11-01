//! Tests for `poolmail.rs`: who each notice goes to, that the texts hold ids only, and the invitation's fixed wording.
use super::*;
use crate::pool::ctx::PoolNotice;

const CODE: &str = "C-ABCD-EFGH-JKMN-PQRS-TVWX-0123";

#[test]
fn admin_notices_name_ids_and_go_to_the_admins() {
    let m = render(&PoolNotice::StudentDataReport { item_id: "i1".into(), report_id: "r1".into() }, "").unwrap();
    assert_eq!(m.to, To::Admins);
    assert!(m.text.contains("Item i1") && m.text.contains("report r1"));
    let m = render(&PoolNotice::Cap { user_id: "u".into(), metric: "publishes".into() }, "").unwrap();
    assert_eq!(m.subject, "verSHion: an account reached its daily publishes limit");
    let m = render(&PoolNotice::RepeatInfringer { user_id: "u".into(), strikes: 3 }, "").unwrap();
    assert_eq!(m.to, To::Everyone);
    let m = render(&PoolNotice::CircleReport { circle_id: "c".into(), bank_id: "b".into(), report_id: "r".into(), reason: "student_data".into(), held: true }, "").unwrap();
    assert_eq!(m.subject, "verSHion: student data reported in a circle (bank hidden)");
    let m = render(&PoolNotice::PrivacyTextMatch { item_id: "b".into(), held: false, circle_id: Some("c".into()), version: Some(4) }, "").unwrap();
    assert!(m.text.contains("Version 4 of bank b in circle c") && m.text.contains("&basis=privacy"));
    let m = render(&PoolNotice::PrivacyTextMatch { item_id: "i".into(), held: true, circle_id: None, version: None }, "").unwrap();
    assert!(m.text.contains("held for review"));
}

#[test]
fn authors_get_their_own_mail_and_a_counter_notice_goes_to_the_complainant_only_if_it_is_an_address() {
    let m = render(&PoolNotice::TakedownAuthor { item_id: "i".into(), author_id: "u".into(), action_id: "a".into() }, "").unwrap();
    assert_eq!(m.to, To::Author("u".into()));
    let m = render(&PoolNotice::CopyHeld { item_id: "i".into(), author_id: "u".into(), cause: "student_data".into() }, "").unwrap();
    assert!(m.text.contains("contains student data"));
    let at = 1_800_000_000_000;
    let m = render(&PoolNotice::CounterNotice { action_id: "a".into(), complainant_contact: " owner@example.org ".into(), restore_not_before: at }, "").unwrap();
    assert_eq!(m.to, To::Address("owner@example.org".into()));
    assert!(m.text.contains("2027-01-15"), "{}", m.text);
    let m = render(&PoolNotice::CounterNotice { action_id: "a".into(), complainant_contact: "555 Main St".into(), restore_not_before: at }, "").unwrap();
    assert_eq!(m.to, To::Admins);
    assert!(m.text.contains("forward it to them yourself"));
}

#[test]
fn the_invitation_is_fixed_text_with_the_code_a_link_and_the_expiry_and_a_bad_one_is_never_mailed() {
    let n = |to: &str, code: &str| PoolNotice::CircleEmailInvite { to: to.into(), code: code.into(), role: "editor".into(), expires_at: 1_800_000_000_000 };
    let m = render(&n("t@school.org", CODE), "https://app.example/").unwrap();
    assert_eq!((m.to, m.subject.as_str()), (To::Address("t@school.org".into()), CIRCLE_INVITE_SUBJECT));
    assert!(m.text.contains("as an editor") && m.text.contains(CODE));
    assert!(m.text.contains(&format!("https://app.example/circles/join#code={CODE}")));
    assert!(m.text.contains("expires on 2027-01-15 08:00 UTC"), "{}", m.text);
    assert!(render(&n("t@school.org", "C-bad"), "").is_none());
    assert!(render(&n("not an address", CODE), "").is_none());
    let m = render(&n("t@school.org", CODE), "").unwrap();
    assert!(!m.text.contains("Or open this link"));
}
