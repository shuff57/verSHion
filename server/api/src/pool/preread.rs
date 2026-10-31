//! The pool's pre-read of auth request bodies (worker/src/pool/preread.ts): the pure parts. Body limits, which accounts
//! are read under their own budget, which pool an unauthenticated route belongs to, and the nested per-address caps.
//! The reader itself (Content-Length required and exact, deadlines, the isolate budget) belongs to the router.

/// The auth package reads (account and admin) bodies of at most 8 KB ...
pub const AUTH_BODY_MAX: usize = 8 * 1024;
/// ... except an admin's, on an admin route: 256 KB (hash lists, long notices).
pub const ADMIN_BODY_MAX: usize = 256 * 1024;
/// Trust levels whose bearer requests are read under their account.
pub const TRUSTED: [&str; 2] = ["teacher", "admin"];

/// Unauthenticated bodies at once per address prefix.
pub struct PrefixCaps {
    pub v6_64: usize,
    pub v6_56: usize,
    pub v6_48: usize,
    pub v6_32: usize,
    pub v4: usize,
    pub v4_24: usize,
}
pub const PREFIX_CAPS: PrefixCaps = PrefixCaps { v6_64: 2, v6_56: 4, v6_48: 8, v6_32: 16, v4: 4, v4_24: 8 };

#[derive(Debug, PartialEq, Eq)]
pub struct AddrCap {
    pub key: String,
    pub cap: usize,
}

fn cap(key: String, cap: usize) -> AddrCap {
    AddrCap { key, cap }
}

/// JavaScript's `parseInt(x, 16) || 0`, as the Worker wrote it: the leading hex digits (after an optional sign), else 0.
fn parse_group(x: &str) -> i64 {
    let t = x.trim_start();
    let (neg, rest) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let rest = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")).unwrap_or(rest);
    let digits: String = rest.chars().take_while(char::is_ascii_hexdigit).take(15).collect();
    let n = i64::from_str_radix(&digits, 16).unwrap_or(0);
    if neg { -n } else { n }
}

fn pad4(n: i64) -> String {
    let s = if n < 0 { format!("-{:x}", -n) } else { format!("{n:x}") };
    format!("{s:0>4}")
}

fn v4(x: &str) -> Vec<AddrCap> {
    let net: Vec<&str> = x.split('.').take(3).collect();
    vec![cap(format!("v4:{x}"), PREFIX_CAPS.v4), cap(format!("v4:{}/24", net.join(".")), PREFIX_CAPS.v4_24)]
}

/// `^::ffff:(\d+\.\d+\.\d+\.\d+)$`
fn mapped_v4(a: &str) -> Option<&str> {
    let rest = a.strip_prefix("::ffff:")?;
    let parts: Vec<&str> = rest.split('.').collect();
    (parts.len() == 4 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))).then_some(rest)
}

/// The per-address cap keys of an address: an IPv4 address and its /24, or an IPv6 address's /64, /56, /48 and /32.
pub fn address_caps(ip: &str) -> Vec<AddrCap> {
    let lower = ip.trim().to_lowercase();
    let a = lower.split('%').next().unwrap_or("");
    if a.is_empty() {
        return vec![cap("none".into(), PREFIX_CAPS.v6_64)];
    }
    if !a.contains(':') {
        return v4(a);
    }
    if let Some(m) = mapped_v4(a) {
        return v4(m);
    }
    let mut halves = a.split("::");
    let head = halves.next().unwrap_or("");
    let tail = halves.next();
    let h: Vec<&str> = if head.is_empty() { vec![] } else { head.split(':').collect() };
    let t: Vec<&str> = match tail {
        Some(t) if !t.is_empty() => t.split(':').collect(),
        _ => vec![],
    };
    let fill = if a.contains("::") { 8usize.saturating_sub(h.len() + t.len()) } else { 0 };
    let mut groups: Vec<String> = h.iter().map(|x| pad4(parse_group(x))).collect();
    groups.extend(std::iter::repeat_n("0000".to_string(), fill).map(|g| pad4(parse_group(&g))));
    groups.extend(t.iter().map(|x| pad4(parse_group(x))));
    let hex = groups.concat();
    let cut = |n: usize| hex.chars().take(n).collect::<String>();
    vec![
        cap(format!("v6:{}/64", cut(16)), PREFIX_CAPS.v6_64),
        cap(format!("v6:{}/56", cut(14)), PREFIX_CAPS.v6_56),
        cap(format!("v6:{}/48", cut(12)), PREFIX_CAPS.v6_48),
        cap(format!("v6:{}/32", cut(8)), PREFIX_CAPS.v6_32),
    ]
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnauthPool {
    Signin,
    Signup,
    General,
}

/// Separate pools for sign-in and sign-up so other unauthenticated traffic cannot starve them.
pub fn pool_of(path: &str) -> UnauthPool {
    if matches!(path, "/auth/login" | "/auth/refresh" | "/auth/reauth") {
        return UnauthPool::Signin;
    }
    if path == "/auth/signup" {
        return UnauthPool::Signup;
    }
    UnauthPool::General
}

/// Pool routes (/items, /blobs, /circles, /ai) read their own bodies after their own authentication.
pub fn reads_own_body(path: &str) -> bool {
    ["/items", "/blobs", "/circles", "/ai"].iter().any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

/// The most a body may be for this request, once its sender is known.
pub fn body_max(trust: &str, path: &str) -> usize {
    if trust == "admin" && path.starts_with("/admin/") { ADMIN_BODY_MAX } else { AUTH_BODY_MAX }
}

#[cfg(test)]
#[path = "preread_tests.rs"]
mod tests;
