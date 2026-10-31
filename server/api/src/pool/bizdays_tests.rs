//! Tests for `bizdays.rs`, replaying worker/test/pool/moderation.test.ts "business days skip weekends and observed federal holidays".
use super::*;

fn d(y: i64, m: i64, day: i64) -> i64 {
    days_from_civil(y, m, day)
}
fn ms(y: i64, m: i64, day: i64, h: i64) -> i64 {
    d(y, m, day) * DAY + h * 3_600_000
}
fn ymd(ms: i64) -> (i64, i64, i64) {
    civil_from_days(ms.div_euclid(DAY))
}

#[test]
fn the_calendar_helpers_agree_with_known_dates() {
    assert_eq!(d(1970, 1, 1), 0);
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(weekday(0), 4, "1970-01-01 was a Thursday");
    assert_eq!(weekday(d(2026, 11, 20)), 5, "Friday");
    for z in [-1000, 0, 11_000, 20_000, 25_000, 40_000] {
        let (y, m, day) = civil_from_days(z);
        assert_eq!(days_from_civil(y, m, day), z);
    }
}

#[test]
fn the_federal_holidays_are_observed_on_the_weekday() {
    let h = federal_holidays(2026);
    for (m, day) in [(1, 1), (1, 19), (2, 16), (5, 25), (6, 19), (7, 3), (9, 7), (10, 12), (11, 11), (11, 26), (12, 25)] {
        assert!(h.contains(&d(2026, m, day)), "2026-{m}-{day}");
    }
    assert_eq!(h.len(), 11);
    assert!(!h.contains(&d(2026, 7, 4)), "a Saturday July 4 is observed on Friday");
    assert!(federal_holidays(2027).contains(&d(2027, 12, 31)), "New Year 2028 is a Saturday: observed Fri Dec 31, 2027");
    assert!(federal_holidays(2027).contains(&d(2027, 12, 24)), "Christmas 2027 is a Saturday: observed Fri Dec 24");
    assert!(federal_holidays(2028).contains(&d(2028, 7, 4)) && federal_holidays(2028).contains(&d(2028, 1, 17)));
}

#[test]
fn a_business_day_is_not_a_weekend_or_a_holiday() {
    assert!(is_business_day(ms(2026, 11, 20, 15)));
    assert!(!is_business_day(ms(2026, 11, 21, 0)), "Saturday");
    assert!(!is_business_day(ms(2026, 11, 22, 23)), "Sunday");
    assert!(!is_business_day(ms(2026, 11, 26, 12)), "Thanksgiving");
}

#[test]
fn ten_business_days_after_a_counter_notice_skip_thanksgiving() {
    // A counter-notice on Fri 2026-11-20: Thanksgiving is skipped, the 10th business day is Mon Dec 7.
    let end = add_business_days(ms(2026, 11, 20, 15), 10);
    assert_eq!(ymd(end), (2026, 12, 7));
    assert_eq!(end.rem_euclid(DAY), DAY - 1, "the end of that day");
    assert_eq!(start_of_business_day(ms(2026, 11, 20, 15), 10), d(2026, 12, 7) * DAY);
    // July 2026: Jul 3 is the observed holiday.
    assert_eq!(ymd(add_business_days(ms(2026, 7, 1, 12), 3)), (2026, 7, 7));
    assert_eq!(add_business_days(ms(2026, 7, 1, 12), 0), ms(2026, 7, 1, 0) + DAY - 1, "zero days is the end of today");
}
