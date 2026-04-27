//! Display-formatting helpers for JMAP Chat clients.

use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};

use crate::jmap::UTCDate;

/// Format a receipt timestamp without exposing sub-minute precision.
///
/// Returns a human-readable relative string:
/// - Same calendar day as now: `"Today"`
/// - Previous calendar day: `"Yesterday"`
/// - 2–6 days ago: `"Mon 14:00"` (weekday abbreviation + HH:MM)
/// - Older: `"Apr 12"` (month abbreviation + day number)
///
/// The full-precision `UTCDate` is preserved internally; this function only
/// affects display output. Never use this for sorting or comparison.
pub fn format_receipt_timestamp(dt: &UTCDate) -> String {
    format_receipt_timestamp_at(dt, Utc::now())
}

/// Like [`format_receipt_timestamp`] but accepts an explicit reference time
/// so that unit tests are deterministic.
pub fn format_receipt_timestamp_at(dt: &UTCDate, now: DateTime<Utc>) -> String {
    let parsed = match dt.as_str().parse::<DateTime<Utc>>() {
        Ok(d) => d,
        Err(_) => return dt.as_str().to_string(),
    };

    let dt_date = parsed.date_naive();
    let now_date = now.date_naive();
    let days_diff = (now_date - dt_date).num_days();

    match days_diff {
        0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        2..=6 => {
            let weekday = match parsed.weekday() {
                Weekday::Mon => "Mon",
                Weekday::Tue => "Tue",
                Weekday::Wed => "Wed",
                Weekday::Thu => "Thu",
                Weekday::Fri => "Fri",
                Weekday::Sat => "Sat",
                Weekday::Sun => "Sun",
            };
            format!("{} {:02}:{:02}", weekday, parsed.hour(), parsed.minute())
        }
        _ => {
            let month = match parsed.month() {
                1 => "Jan",
                2 => "Feb",
                3 => "Mar",
                4 => "Apr",
                5 => "May",
                6 => "Jun",
                7 => "Jul",
                8 => "Aug",
                9 => "Sep",
                10 => "Oct",
                11 => "Nov",
                _ => "Dec",
            };
            format!("{} {}", month, parsed.day())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        // Oracle: fixed reference time — 2024-03-20T15:00:00Z
        Utc.with_ymd_and_hms(2024, 3, 20, 15, 0, 0).unwrap()
    }

    /// Oracle: same-day UTCDate (with seconds precision) formats as "Today".
    #[test]
    fn test_same_day_returns_today() {
        let dt = UTCDate::from_trusted("2024-03-20T14:32:07Z");
        assert_eq!(format_receipt_timestamp_at(&dt, now()), "Today");
    }

    /// Oracle: yesterday's UTCDate formats as "Yesterday" (no sub-minute detail).
    #[test]
    fn test_yesterday_returns_yesterday() {
        let dt = UTCDate::from_trusted("2024-03-19T09:15:45Z");
        assert_eq!(format_receipt_timestamp_at(&dt, now()), "Yesterday");
    }

    /// Oracle: 3 days ago — Wednesday, 2024-03-17 — formats as "Sun 08:03".
    /// Note: 2024-03-17 is a Sunday. No seconds in output.
    #[test]
    fn test_within_week_returns_weekday_and_time() {
        let dt = UTCDate::from_trusted("2024-03-17T08:03:59Z");
        let result = format_receipt_timestamp_at(&dt, now());
        assert_eq!(result, "Sun 08:03");
        assert!(
            !result.contains(':') || result.len() <= "Sun 08:03".len(),
            "output must not contain seconds: {result}"
        );
    }

    /// Oracle: 2024-01-15 is more than 7 days before 2024-03-20 — formats as "Jan 15".
    #[test]
    fn test_old_date_returns_month_and_day() {
        let dt = UTCDate::from_trusted("2024-01-15T09:00:00Z");
        let result = format_receipt_timestamp_at(&dt, now());
        assert_eq!(result, "Jan 15");
    }

    /// Oracle: seconds precision is never exposed in any output format.
    #[test]
    fn test_no_sub_minute_detail_in_any_format() {
        let cases = [
            "2024-03-20T14:32:07Z", // today
            "2024-03-19T09:15:45Z", // yesterday
            "2024-03-17T08:03:59Z", // within week
            "2024-01-15T09:00:30Z", // old
        ];
        for case in cases {
            let dt = UTCDate::from_trusted(case);
            let result = format_receipt_timestamp_at(&dt, now());
            // No seconds component — the format never contains "HH:MM:SS"
            assert!(
                !result.contains(":59")
                    && !result.contains(":45")
                    && !result.contains(":30")
                    && !result.contains(":07"),
                "seconds found in output for {case}: {result}"
            );
        }
    }
}
