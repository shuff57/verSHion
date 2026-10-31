//! US federal business days, for the DMCA put-back window (17 U.S.C. 512(g)(2)(C): no less than 10, no more than 14
//! business days after a counter-notice). Weekends and the federal holidays of 5 U.S.C. 6103 (with the
//! Saturday->Friday, Sunday->Monday observance rule) are skipped. Port of worker/src/pool/bizdays.ts.
use std::collections::HashSet;

const DAY: i64 = 86_400_000;

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// (year, month 1-12, day) of a day number since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 0 = Sunday .. 6 = Saturday.
fn weekday(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

/// The day of month of the `n`th `wd` of `month` (0-based month, as the Worker's callers wrote it).
fn nth_weekday(year: i64, month: i64, wd: i64, n: i64) -> i64 {
    let first = weekday(days_from_civil(year, month + 1, 1));
    1 + (wd - first + 7) % 7 + 7 * (n - 1)
}

fn last_weekday(year: i64, month: i64, wd: i64) -> i64 {
    let first_next = if month == 11 { days_from_civil(year + 1, 1, 1) } else { days_from_civil(year, month + 2, 1) };
    let last_day = first_next - 1;
    let last_wd = weekday(last_day);
    let days = last_day - days_from_civil(year, month + 1, 1) + 1;
    days - (last_wd - wd + 7) % 7
}

/// Observed federal holidays of `year`, as day numbers.
pub fn federal_holidays(year: i64) -> HashSet<i64> {
    let mut out = HashSet::new();
    let mut add = |m: i64, d: i64| {
        out.insert(days_from_civil(year, m + 1, d));
    };
    let fixed = |m: i64, d: i64| {
        let wd = weekday(days_from_civil(year, m + 1, d));
        let obs = match wd {
            6 => d - 1,
            0 => d + 1,
            _ => d,
        };
        days_from_civil(year, m + 1, obs)
    };
    let mut fixeds = Vec::new();
    for (m, d) in [(0, 1), (5, 19), (6, 4), (10, 11), (11, 25)] {
        fixeds.push(fixed(m, d));
    }
    add(0, nth_weekday(year, 0, 1, 3)); // Birthday of Martin Luther King, Jr.
    add(1, nth_weekday(year, 1, 1, 3)); // Washington's Birthday
    add(4, last_weekday(year, 4, 1)); // Memorial Day
    add(8, nth_weekday(year, 8, 1, 1)); // Labor Day
    add(9, nth_weekday(year, 9, 1, 2)); // Columbus Day
    add(10, nth_weekday(year, 10, 4, 4)); // Thanksgiving Day
    // New Year's Day of the next year observed on Dec 31 when it falls on a Saturday.
    let next_new_year_saturday = weekday(days_from_civil(year + 1, 1, 1)) == 6;
    if next_new_year_saturday {
        add(11, 31);
    }
    out.extend(fixeds);
    out
}

pub fn is_business_day(ms: i64) -> bool {
    let day = ms.div_euclid(DAY);
    let wd = weekday(day);
    if wd == 0 || wd == 6 {
        return false;
    }
    let (year, _, _) = civil_from_days(day);
    !federal_holidays(year).contains(&day)
}

/// The end (23:59:59.999 UTC) of the `n`th business day after the day of `from_ms`.
pub fn add_business_days(from_ms: i64, n: i64) -> i64 {
    let mut day = from_ms.div_euclid(DAY) * DAY;
    let mut left = n;
    while left > 0 {
        day += DAY;
        if is_business_day(day) {
            left -= 1;
        }
    }
    day + DAY - 1
}

/// The start (00:00 UTC) of the `n`th business day after the day of `from_ms`.
pub fn start_of_business_day(from_ms: i64, n: i64) -> i64 {
    add_business_days(from_ms, n) - DAY + 1
}

#[cfg(test)]
#[path = "bizdays_tests.rs"]
mod tests;
