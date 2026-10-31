//! Tests for `preread.rs`.
use super::*;

fn keys(ip: &str) -> Vec<(String, usize)> {
    address_caps(ip).into_iter().map(|c| (c.key, c.cap)).collect()
}

#[test]
fn an_ipv4_address_is_capped_alone_and_by_its_slash_24() {
    assert_eq!(keys("203.0.113.9"), vec![("v4:203.0.113.9".into(), 4), ("v4:203.0.113/24".into(), 8)]);
    assert_eq!(keys(" 203.0.113.9 "), keys("203.0.113.9"));
    assert_eq!(keys("::ffff:10.1.2.3"), vec![("v4:10.1.2.3".into(), 4), ("v4:10.1.2/24".into(), 8)], "a mapped address is its IPv4");
}

#[test]
fn an_ipv6_address_is_capped_by_four_nested_prefixes() {
    assert_eq!(
        keys("2001:db8::1"),
        vec![
            ("v6:20010db800000000/64".into(), 2),
            ("v6:20010db8000000/56".into(), 4),
            ("v6:20010db80000/48".into(), 8),
            ("v6:20010db8/32".into(), 16)
        ]
    );
    assert_eq!(keys("2001:DB8:1:2:3:4:5:6%eth0")[0].0, "v6:20010db800010002/64", "case and zone are dropped");
    assert_eq!(keys("::1")[0].0, "v6:0000000000000000/64");
    assert_eq!(keys("2001:db8:0:0:0:0:0:1")[0], keys("2001:db8::1")[0], "spelled out or compressed is one address");
    assert_eq!(keys("2001:db8::aaaa")[0], keys("2001:db8::bbbb")[0], "the same /64");
    assert_ne!(keys("2001:db8:0:1::")[0], keys("2001:db8::")[0]);
}

#[test]
fn a_missing_address_shares_one_tight_bucket() {
    assert_eq!(keys(""), vec![("none".into(), 2)]);
    assert_eq!(keys("   "), vec![("none".into(), 2)]);
}

#[test]
fn malformed_groups_read_as_javascripts_parseint_did() {
    // "12zz" reads as 0x12; "zz" and "" as 0.
    assert_eq!(keys("12zz::")[0].0, "v6:0012000000000000/64");
    assert_eq!(keys("zz::1")[0].0, "v6:0000000000000000/64");
}

#[test]
fn routes_belong_to_their_pools_and_pool_routes_read_their_own_bodies() {
    assert_eq!(pool_of("/auth/login"), UnauthPool::Signin);
    assert_eq!(pool_of("/auth/refresh"), UnauthPool::Signin);
    assert_eq!(pool_of("/auth/reauth"), UnauthPool::Signin);
    assert_eq!(pool_of("/auth/signup"), UnauthPool::Signup);
    assert_eq!(pool_of("/auth/forgot"), UnauthPool::General);
    for p in ["/items", "/items/abc", "/blobs/x", "/circles/c/send", "/ai/chat/completions"] {
        assert!(reads_own_body(p), "{p}");
    }
    for p in ["/itemsx", "/auth/login", "/admin/approve", "/me"] {
        assert!(!reads_own_body(p), "{p}");
    }
}

#[test]
fn only_an_admin_on_an_admin_route_gets_the_large_body() {
    assert_eq!(body_max("admin", "/admin/dmca/notices"), ADMIN_BODY_MAX);
    assert_eq!(body_max("admin", "/me"), AUTH_BODY_MAX);
    assert_eq!(body_max("teacher", "/admin/approve"), AUTH_BODY_MAX);
    assert!(TRUSTED.contains(&"teacher") && !TRUSTED.contains(&"pending"));
}
