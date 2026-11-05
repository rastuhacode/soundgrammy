//! Persisted timestamps and export labels use UTC; deadlines use monotonic clocks.
use std::time::{SystemTime, UNIX_EPOCH};
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
pub fn iso_utc(milliseconds: i64) -> String {
    chrono::DateTime::from_timestamp_millis(milliseconds)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}
pub fn date_utc(milliseconds: i64) -> String {
    chrono::DateTime::from_timestamp_millis(milliseconds)
        .unwrap_or_default()
        .format("%Y-%m-%d")
        .to_string()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utc_labels_handle_leap_days_and_day_boundaries() {
        let value = chrono::DateTime::parse_from_rfc3339("2024-02-29T23:59:59Z")
            .unwrap()
            .timestamp_millis();
        assert_eq!(iso_utc(value), "2024-02-29T23:59:59Z");
        assert_eq!(date_utc(value + 1000), "2024-03-01");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    }
}
