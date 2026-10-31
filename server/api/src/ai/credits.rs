//! Credits for the built-in AI (port of worker/src/ai/credits.ts). The server alone prices a service, caps
//! its pages and gives it a budget of model calls: a client names the service and (for page-priced ones) the
//! page count, and nothing else.

pub const HOUR_MS: i64 = 3_600_000;
pub const DAY_MS: i64 = 86_400_000;
pub const WEEK_MS: i64 = 7 * DAY_MS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Import,
    Figure,
    Handwriting,
    Alternatives,
    AnswerKey,
    Grade,
}

impl Service {
    pub const ALL: [Service; 6] =
        [Service::Import, Service::Figure, Service::Handwriting, Service::Alternatives, Service::AnswerKey, Service::Grade];

    /// The wire name; anything else is not a service (`isServiceId`).
    pub fn parse(s: &str) -> Option<Service> {
        Some(match s {
            "import" => Service::Import,
            "figure" => Service::Figure,
            "handwriting" => Service::Handwriting,
            "alternatives" => Service::Alternatives,
            "answer_key" => Service::AnswerKey,
            "grade" => Service::Grade,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Service::Import => "import",
            Service::Figure => "figure",
            Service::Handwriting => "handwriting",
            Service::Alternatives => "alternatives",
            Service::AnswerKey => "answer_key",
            Service::Grade => "grade",
        }
    }

    /// Credits charged for a session of `pages` pages (0 for services that take none).
    pub fn credits(self, pages: i64) -> i64 {
        match self {
            Service::Import | Service::Figure => 2,
            Service::Handwriting => pages,
            Service::Alternatives | Service::AnswerKey => 1,
            Service::Grade => 3,
        }
    }

    pub fn takes_pages(self) -> bool {
        matches!(self, Service::Import | Service::Handwriting | Service::Grade)
    }

    pub fn max_pages(self) -> i64 {
        match self {
            Service::Import | Service::Handwriting => 20,
            Service::Grade => 40,
            _ => 0,
        }
    }

    /// Model calls the session may make. Generous: a page read can retry, redo its picture box and hedge.
    pub fn calls(self, pages: i64) -> i64 {
        match self {
            Service::Import => pages * 40,
            Service::Figure => 40,
            Service::Handwriting => pages * 4,
            Service::Alternatives | Service::AnswerKey => 12,
            Service::Grade => pages * 6 + 12,
        }
    }

    pub fn ttl_ms(self) -> i64 {
        match self {
            Service::Import => 2 * HOUR_MS,
            Service::Figure => 30 * 60_000,
            Service::Handwriting | Service::Grade => HOUR_MS,
            Service::Alternatives | Service::AnswerKey => 15 * 60_000,
        }
    }
}

/// The price list the app may show (`describeServices`).
pub fn describe_services() -> serde_json::Value {
    let mut m = serde_json::Map::new();
    for s in Service::ALL {
        let pages = if s.takes_pages() { 1 } else { 0 };
        m.insert(
            s.id().into(),
            serde_json::json!({ "credits": s.credits(pages), "per_page": s.takes_pages() && s.credits(2) > s.credits(1), "max_pages": s.max_pages() }),
        );
    }
    serde_json::Value::Object(m)
}

/// Start of the UTC day containing `now`.
pub fn utc_day_start(now: i64) -> i64 {
    now - now.rem_euclid(DAY_MS)
}

/// Start of the UTC week (Monday 00:00) containing `now`: credits come back here.
pub fn utc_week_start(now: i64) -> i64 {
    let day = now.div_euclid(DAY_MS);
    // 1970-01-01 was a Thursday, so (day + 3) % 7 is 0 on a Monday.
    (day - (day + 3).rem_euclid(7)) * DAY_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    const MON: i64 = 1_792_972_800_000; // Date.UTC(2026, 9, 26): Monday 2026-10-26

    #[test]
    fn the_week_starts_on_monday_utc() {
        // The same three cases as worker/test/ai-credits.test.ts.
        assert_eq!(utc_week_start(MON + 9 * HOUR_MS), MON);
        assert_eq!(utc_week_start(MON + 6 * DAY_MS + 10 * HOUR_MS), MON, "Sunday evening");
        assert_eq!(utc_week_start(MON + 7 * DAY_MS), 1_793_577_600_000, "Date.UTC(2026, 10, 2)");
        assert_eq!(utc_week_start(MON - 1), MON - WEEK_MS, "one ms before Monday is the previous week");
    }

    #[test]
    fn the_day_starts_at_utc_midnight() {
        assert_eq!(utc_day_start(MON + 13 * HOUR_MS + 5), MON);
        assert_eq!(utc_day_start(MON), MON);
    }

    #[test]
    fn prices_match_the_worker() {
        assert_eq!(Service::Import.credits(7), 2, "a whole test is flat");
        assert_eq!(Service::Handwriting.credits(5), 5, "one credit a page");
        assert_eq!(Service::Grade.credits(10), 3);
        assert_eq!((Service::Import.calls(3), Service::Handwriting.calls(3), Service::Grade.calls(10)), (120, 12, 72));
        assert_eq!((Service::Figure.calls(0), Service::Figure.ttl_ms()), (40, 30 * 60_000));
        assert_eq!((Service::Import.max_pages(), Service::Grade.max_pages(), Service::Figure.max_pages()), (20, 40, 0));
        assert!(Service::Import.takes_pages() && !Service::Figure.takes_pages());
    }

    #[test]
    fn only_the_six_wire_names_parse() {
        for s in Service::ALL {
            assert_eq!(Service::parse(s.id()), Some(s));
        }
        for bad in ["", "Import", "toString", "__proto__", "answer-key"] {
            assert_eq!(Service::parse(bad), None, "{bad}");
        }
    }
}
