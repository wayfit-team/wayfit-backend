use chrono::{DateTime, Datelike, Duration, FixedOffset, Timelike, Utc, Weekday};

/// KST offset: UTC + 9 hours
pub fn kst_offset() -> FixedOffset {
    FixedOffset::east_opt(9 * 3600).expect("valid KST offset")
}

/// Current UTC time
pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

/// Format DateTime<Utc> as ISO-8601 string (e.g., "2026-11-05T03:12:00Z")
pub fn format_rfc3339(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Parse ISO-8601 / RFC3339 string into DateTime<Utc>
pub fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Convert UTC timestamp to KST date string ("YYYY-MM-DD") (NFR-B6)
pub fn to_kst_date_str(dt: DateTime<Utc>) -> String {
    let kst = dt.with_timezone(&kst_offset());
    format!("{:04}-{:02}-{:02}", kst.year(), kst.month(), kst.day())
}

/// Convert UTC timestamp to KST weekday key ("mon".."sun") and minutes since midnight (0..1439)
pub fn to_kst_weekday_and_minutes(dt: DateTime<Utc>) -> (&'static str, u32) {
    let kst = dt.with_timezone(&kst_offset());
    let day_key = match kst.weekday() {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    };
    let minutes = kst.hour() * 60 + kst.minute();
    (day_key, minutes)
}

/// Return the last `days` KST date strings ("YYYY-MM-DD") in ascending chronological order, ending at `now` in KST.
pub fn recent_kst_dates(now: DateTime<Utc>, days: u32) -> Vec<String> {
    let kst_now = now.with_timezone(&kst_offset());
    let mut dates = Vec::with_capacity(days as usize);
    for i in (0..days).rev() {
        let d = kst_now - Duration::days(i as i64);
        dates.push(format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()));
    }
    dates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kst_date_boundary() {
        // 2026-11-04 14:59:59 UTC == 2026-11-04 23:59:59 KST
        let before_midnight = parse_rfc3339("2026-11-04T14:59:59Z").unwrap();
        assert_eq!(to_kst_date_str(before_midnight), "2026-11-04");

        // 2026-11-04 15:00:00 UTC == 2026-11-05 00:00:00 KST
        let after_midnight = parse_rfc3339("2026-11-04T15:00:00Z").unwrap();
        assert_eq!(to_kst_date_str(after_midnight), "2026-11-05");
    }

    #[test]
    fn test_recent_kst_dates() {
        let now = parse_rfc3339("2026-11-05T03:00:00Z").unwrap();
        let dates = recent_kst_dates(now, 3);
        assert_eq!(dates, vec!["2026-11-03", "2026-11-04", "2026-11-05"]);
    }
}
