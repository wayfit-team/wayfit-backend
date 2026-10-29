use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::time::to_kst_weekday_and_minutes;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DayHours {
    /// Open time in "HH:MM" (24-hour KST), e.g. "08:00"
    #[schema(example = "08:00")]
    pub open: String,
    /// Close time in "HH:MM" (24-hour KST), e.g. "22:00"
    #[schema(example = "22:00")]
    pub close: String,
}

pub type OpenHoursMap = HashMap<String, Option<DayHours>>;

fn parse_hhmm(s: &str) -> Option<u32> {
    let mut parts = s.split(':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    if h <= 24 && m < 60 {
        Some(h * 60 + m)
    } else {
        None
    }
}

/// Check if a store is open at `now_utc` (evaluated in KST).
/// Supports per-day keys ("mon".."sun") or a fallback "all" key.
pub fn is_store_open(open_hours_json: &str, now_utc: DateTime<Utc>) -> bool {
    let map: OpenHoursMap = match serde_json::from_str(open_hours_json) {
        Ok(m) => m,
        Err(_) => return true, // Fallback open if malformed in seed
    };

    let (day_key, minutes) = to_kst_weekday_and_minutes(now_utc);
    let hours_opt = map
        .get(day_key)
        .or_else(|| map.get("all"))
        .and_then(|v| v.as_ref());

    let Some(hours) = hours_opt else {
        return false;
    };

    let (Some(open_min), Some(close_min)) = (parse_hhmm(&hours.open), parse_hhmm(&hours.close))
    else {
        return false;
    };

    if open_min <= close_min {
        minutes >= open_min && minutes <= close_min
    } else {
        // Overnight hours (e.g. 18:00 ~ 02:00)
        minutes >= open_min || minutes <= close_min
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::parse_rfc3339;

    #[test]
    fn test_is_store_open_kst() {
        let json = r#"{
            "mon": {"open": "09:00", "close": "21:00"},
            "tue": {"open": "09:00", "close": "21:00"},
            "wed": {"open": "09:00", "close": "21:00"},
            "thu": {"open": "09:00", "close": "21:00"},
            "fri": {"open": "09:00", "close": "21:00"},
            "sat": {"open": "10:00", "close": "20:00"},
            "sun": null
        }"#;

        // 2026-11-05 is Thursday. 01:00 UTC = 10:00 KST (Open)
        let thu_10am_kst = parse_rfc3339("2026-11-05T01:00:00Z").unwrap();
        assert!(is_store_open(json, thu_10am_kst));

        // Thursday 13:00 UTC = 22:00 KST (Closed)
        let thu_10pm_kst = parse_rfc3339("2026-11-05T13:00:00Z").unwrap();
        assert!(!is_store_open(json, thu_10pm_kst));

        // 2026-11-08 is Sunday. 03:00 UTC = 12:00 KST (Closed on Sunday)
        let sun_noon_kst = parse_rfc3339("2026-11-08T03:00:00Z").unwrap();
        assert!(!is_store_open(json, sun_noon_kst));
    }
}
