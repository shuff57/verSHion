//! Tests for `routetable.rs`: the Worker's table, including the order that keeps `/items/mine` from being an item id and `/circles/join` from a circle id.
use super::*;

const C: &str = "0123456789abcdef0123456789abcdef";
const B: &str = "fedcba9876543210fedcba9876543210";

fn s(x: &str) -> String {
    x.to_string()
}

#[test]
fn static_item_names_win_over_the_item_id() {
    assert_eq!(route_of("/items"), Some(Route::Items));
    assert_eq!(route_of("/items/mine"), Some(Route::Mine));
    assert_eq!(route_of("/items/prefs"), Some(Route::Prefs));
    assert_eq!(route_of("/items/versions"), Some(Route::ItemVersions));
    assert_eq!(route_of("/items/preview"), Some(Route::ItemsPreview));
    assert_eq!(route_of("/items/abc"), Some(Route::Item(s("abc"))));
    assert_eq!(route_of("/items/abc/preview"), Some(Route::ItemPreview(s("abc"))));
    assert_eq!(route_of("/items/abc/counter-notice"), Some(Route::ItemCounterNotice(s("abc"))));
    assert_eq!(route_of("/items/abc/blobs/h1"), Some(Route::ItemBlob(s("abc"), s("h1"))));
    assert_eq!(route_of("/blobs/h1"), Some(Route::BlobPut(s("h1"))));
}

#[test]
fn an_id_is_one_to_eighty_characters_and_never_empty() {
    assert_eq!(route_of("/items/"), None);
    assert!(route_of(&format!("/items/{}", "a".repeat(80))).is_some());
    assert_eq!(route_of(&format!("/items/{}", "a".repeat(81))), None);
    assert_eq!(route_of("/items/a/b/c"), None);
    assert_eq!(route_of("/nope"), None);
    assert_eq!(route_of("items"), None);
}

#[test]
fn circle_ids_are_32_lower_case_hex_so_join_and_heads_are_never_ids() {
    assert_eq!(route_of("/circles"), Some(Route::Circles));
    assert_eq!(route_of("/circles/join"), Some(Route::CircleJoin));
    assert_eq!(route_of("/circles/join/preview"), Some(Route::CircleJoinPreview));
    assert_eq!(route_of("/circles/heads"), Some(Route::CircleHeads));
    assert_eq!(route_of(&format!("/circles/{C}")), Some(Route::Circle(s(C))));
    assert_eq!(route_of(&format!("/circles/{}", C.to_uppercase())), None);
    assert_eq!(route_of(&format!("/circles/{}", &C[1..])), None);
    assert_eq!(route_of(&format!("/circles/{C}/members/role")), Some(Route::CircleMembersRole(s(C))));
    assert_eq!(route_of(&format!("/circles/{C}/transfer/accept")), Some(Route::CircleTransferAccept(s(C))));
    assert_eq!(route_of(&format!("/circles/{C}/invites/email")), Some(Route::CircleInvitesEmail(s(C))));
}

#[test]
fn bank_routes_carry_both_ids_and_versions_are_plain_numbers() {
    assert_eq!(route_of(&format!("/circles/{C}/banks")), Some(Route::Banks(s(C))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}")), Some(Route::Bank(s(C), s(B))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/purge")), Some(Route::BankPurge(s(C), s(B))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/blobs/x")), Some(Route::BankBlob(s(C), s(B), s("x"))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/versions")), Some(Route::BankVersions(s(C), s(B))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/versions/0")), Some(Route::BankVersion(s(C), s(B), s("0"))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/versions/12")), Some(Route::BankVersion(s(C), s(B), s("12"))));
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/versions/012")), None);
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/versions/12345678901")), None);
    assert_eq!(route_of(&format!("/circles/{C}/banks/nothex/trim")), None);
    assert_eq!(route_of(&format!("/circles/{C}/banks/{B}/nope")), None);
}

#[test]
fn methods_follow_the_workers_table() {
    assert_eq!(Route::Items.methods(), ["GET", "POST"]);
    assert_eq!(Route::Item(s("a")).methods(), ["GET", "HEAD", "PUT", "PATCH", "DELETE"]);
    assert_eq!(Route::Bank(s(C), s(B)).methods(), ["GET", "HEAD", "PUT", "PATCH", "DELETE"]);
    assert_eq!(Route::Prefs.methods(), ["GET", "PUT"]);
    assert_eq!(Route::Circle(s(C)).methods(), ["GET", "PATCH", "DELETE"]);
    assert_eq!(Route::CircleInvites(s(C)).methods(), ["GET", "POST"]);
    assert_eq!(Route::BlobPut(s("h")).methods(), ["PUT"]);
    assert_eq!(Route::BankVersion(s(C), s(B), s("1")).methods(), ["GET"]);
    assert_eq!(Route::CircleHeads.methods(), ["POST"]);
}
