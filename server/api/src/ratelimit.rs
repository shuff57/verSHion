//! Sliding-window rate limiter over the `rate_limits` table (port of auth/src/ratelimit.ts, D1RateLimitStore).
//!
//! Each window is split into 12 sub-buckets; the estimate is the sum of the buckets inside the window, with only
//! the oldest, partly elapsed bucket weighted by its overlap. The increment is one UPSERT, and a denied hit is
//! rolled back by the caller (`undo`) only when it should not count, so a lockout ends once counted hits age out.
use rusqlite::{Connection, params};

const SUB_BUCKETS: i64 = 12;

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub limit: i64,
    pub window_seconds: i64,
}

#[derive(Debug, PartialEq)]
pub struct Verdict {
    pub allowed: bool,
    pub retry_after: i64,
}

struct Bucket {
    window_start: i64,
    count: i64,
}

fn bucket_ms(rule: &Rule) -> i64 {
    ((rule.window_seconds * 1000) as f64 / SUB_BUCKETS as f64).round().max(1.0) as i64
}

fn estimate_at(buckets: &[Bucket], rule: &Rule, at: f64) -> f64 {
    let w = (rule.window_seconds * 1000) as f64;
    let b = bucket_ms(rule) as f64;
    let edge = at - w;
    let mut sum = 0.0;
    for x in buckets {
        let start = x.window_start as f64;
        if start + b <= edge {
            continue;
        }
        sum += if start <= edge { x.count as f64 * (start + b - edge) / b } else { x.count as f64 };
    }
    sum
}

fn decide(buckets: &[Bucket], rule: &Rule, now: i64) -> Verdict {
    if estimate_at(buckets, rule, now as f64) <= rule.limit as f64 {
        return Verdict { allowed: true, retry_after: 0 };
    }
    // The estimate only falls as time passes, so a bisection finds the first moment a retry fits.
    let mut lo = now as f64;
    let mut hi = (now + rule.window_seconds * 1000 + bucket_ms(rule)) as f64;
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if estimate_at(buckets, rule, mid) <= rule.limit as f64 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Verdict { allowed: false, retry_after: (((hi - now as f64) / 1000.0).ceil() as i64).max(1) }
}

/// Counts one hit for `key` and says whether it fits the rule.
pub fn hit(conn: &Connection, key: &str, rule: Rule, now: i64) -> rusqlite::Result<Verdict> {
    let w = rule.window_seconds * 1000;
    let b = bucket_ms(&rule);
    let cur = now.div_euclid(b) * b;
    let oldest = (now - w).div_euclid(b) * b;
    conn.execute(
        "INSERT INTO rate_limits (key, window_start, count, expires_at) VALUES (?1, ?2, 1, ?3) ON CONFLICT (key, window_start) DO UPDATE SET count = count + 1",
        params![key, cur, cur + b + w],
    )?;
    let mut stmt = conn.prepare("SELECT window_start, count FROM rate_limits WHERE key = ?1 AND window_start >= ?2 AND window_start <= ?3 ORDER BY window_start")?;
    let rows = stmt.query_map(params![key, oldest, cur], |r| Ok(Bucket { window_start: r.get(0)?, count: r.get(1)? }))?.collect::<Result<Vec<_>, _>>()?;
    Ok(decide(&rows, &rule, now))
}

/// Takes a hit back (a refused request, or a success that should not count).
pub fn undo(conn: &Connection, key: &str, rule: Rule, now: i64) -> rusqlite::Result<()> {
    let b = bucket_ms(&rule);
    let cur = now.div_euclid(b) * b;
    conn.execute("UPDATE rate_limits SET count = count - 1 WHERE key = ?1 AND window_start = ?2 AND count > 0", params![key, cur])?;
    Ok(())
}

/// Drops rows whose window has passed (the hourly cron).
pub fn prune(conn: &Connection, now: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM rate_limits WHERE expires_at < ?1", [now])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE rate_limits (key TEXT NOT NULL, window_start INTEGER NOT NULL, count INTEGER NOT NULL, expires_at INTEGER NOT NULL, PRIMARY KEY (key, window_start)) WITHOUT ROWID;").unwrap();
        c
    }
    const RULE: Rule = Rule { limit: 3, window_seconds: 60 };
    const T: i64 = 1_700_000_000_000;

    #[test]
    fn the_limit_holds_inside_a_window_and_keys_are_independent() {
        let c = db();
        for _ in 0..3 {
            assert!(hit(&c, "a", RULE, T).unwrap().allowed);
        }
        let v = hit(&c, "a", RULE, T).unwrap();
        assert!(!v.allowed && v.retry_after >= 1 && v.retry_after <= 65, "{v:?}");
        assert!(hit(&c, "b", RULE, T).unwrap().allowed);
    }

    #[test]
    fn hits_age_out_and_the_retry_after_is_honest() {
        let c = db();
        for _ in 0..3 {
            hit(&c, "a", RULE, T).unwrap();
        }
        let v = hit(&c, "a", RULE, T).unwrap();
        // The denied hit stays counted unless undone; the caller rolls it back for a lockout that ends on time.
        undo(&c, "a", RULE, T).unwrap();
        assert!(hit(&c, "a", RULE, T + v.retry_after * 1000 + 5_000).unwrap().allowed, "allowed again after retry_after");
        assert!(hit(&c, "a", RULE, T + 2 * 60_000).unwrap().allowed, "a whole window later is clear");
    }

    #[test]
    fn undo_gives_a_hit_back_and_never_goes_below_zero() {
        let c = db();
        for _ in 0..3 {
            hit(&c, "a", RULE, T).unwrap();
        }
        undo(&c, "a", RULE, T).unwrap();
        assert!(hit(&c, "a", RULE, T).unwrap().allowed);
        for _ in 0..10 {
            undo(&c, "z", RULE, T).unwrap();
        }
        let n: i64 = c.query_row("SELECT COALESCE(SUM(count), 0) FROM rate_limits WHERE key = 'z'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn prune_drops_only_expired_rows() {
        let c = db();
        hit(&c, "a", RULE, T).unwrap();
        assert_eq!(prune(&c, T).unwrap(), 0);
        assert_eq!(prune(&c, T + 3 * 60_000).unwrap(), 1);
    }
}
