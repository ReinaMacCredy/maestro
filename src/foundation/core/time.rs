use std::time::{SystemTime, UNIX_EPOCH};

/// Parsed UTC timestamp used for event ordering.
#[derive(Debug)]
pub struct ParsedTimestamp {
    /// Original timestamp string.
    pub raw: String,
    /// Nanoseconds since the Unix epoch.
    pub nanos_since_epoch: i128,
}

/// Return the current UTC timestamp with millisecond precision.
pub fn utc_now_millis_timestamp() -> String {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "1970-01-01T00:00:00.000Z".to_string();
    };
    format_unix_timestamp_millis(duration.as_secs(), duration.subsec_millis())
}

/// Return a filesystem-safe current UTC timestamp with millisecond precision.
pub fn utc_now_filesystem_millis_timestamp() -> String {
    utc_now_millis_timestamp().replace(':', "-")
}

/// Deprecated name retained for call-site stability while the persisted format
/// has moved from nanosecond epochs to RFC3339 UTC milliseconds.
pub fn nanos_since_epoch_string() -> String {
    utc_now_millis_timestamp()
}

/// Return the current time as a UTC event timestamp.
pub fn utc_now_timestamp() -> String {
    utc_now_millis_timestamp()
}

/// Parse an event timestamp emitted by `utc_now_timestamp`.
pub fn parse_utc_timestamp(value: &str) -> Option<ParsedTimestamp> {
    let value = value.strip_suffix('Z')?;
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let seconds = time_parts.next()?;
    if time_parts.next().is_some() {
        return None;
    }

    let (second, nanos) = parse_seconds(seconds)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let days = days_from_civil(year, month, day)?;
    let seconds_since_epoch = days * 86_400 + i128::from(hour * 3_600 + minute * 60 + second);
    Some(ParsedTimestamp {
        raw: format!("{value}Z"),
        nanos_since_epoch: seconds_since_epoch * 1_000_000_000 + i128::from(nanos),
    })
}

/// Format Unix epoch seconds as an RFC3339 UTC timestamp with millisecond precision
/// (e.g. `2026-05-26T05:16:16.000Z`). Seconds-resolution input always renders `.000`.
pub fn format_utc_seconds_rfc3339_millis(seconds: u64) -> String {
    format_utc_millis_rfc3339(seconds, 0)
}

/// Format Unix epoch seconds + milliseconds as an RFC3339 UTC timestamp.
pub fn format_utc_millis_rfc3339(seconds: u64, millis: u32) -> String {
    format_unix_timestamp_millis(seconds, millis)
}

fn format_unix_timestamp_millis(seconds: u64, millis: u32) -> String {
    let (year, month, day, hour, minute, second) = civil_parts(seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Render a persisted nanos-since-epoch string as a human RFC3339 UTC timestamp.
/// Falls back to the raw value when it does not parse as nanoseconds, so a
/// hand-edited or legacy field is shown verbatim rather than mangled.
pub fn render_timestamp(value: &str) -> String {
    match value.trim().parse::<u64>() {
        Ok(timestamp) => render_numeric_timestamp(timestamp, value.trim().len()),
        Err(_) => value.to_string(),
    }
}

fn render_numeric_timestamp(timestamp: u64, digits: usize) -> String {
    match digits {
        0..=10 => format_utc_seconds_rfc3339_millis(timestamp),
        11..=13 => format_utc_millis_rfc3339(timestamp / 1_000, (timestamp % 1_000) as u32),
        14..=16 => {
            format_utc_millis_rfc3339(timestamp / 1_000_000, ((timestamp / 1_000) % 1_000) as u32)
        }
        _ => format_utc_millis_rfc3339(
            timestamp / 1_000_000_000,
            ((timestamp / 1_000_000) % 1_000) as u32,
        ),
    }
}

/// Decompose Unix epoch seconds into UTC civil parts `(year, month, day, hour, minute, second)`.
fn civil_parts(seconds: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    (year, month, day, hour, minute, second)
}

/// Human relative age (e.g. `5d ago`) for a Unix-epoch-seconds instant in the past.
/// Returns None only when the system clock is unavailable.
pub fn relative_age_from_unix_seconds(then_seconds: i64) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let elapsed = now.saturating_sub(then_seconds);
    Some(if elapsed < 60 {
        "less than 1m ago".to_string()
    } else if elapsed < 3_600 {
        format!("{}m ago", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{}h ago", elapsed / 3_600)
    } else {
        format!("{}d ago", elapsed / 86_400)
    })
}

fn parse_seconds(value: &str) -> Option<(u32, u32)> {
    let (seconds, fraction) = match value.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (value, None),
    };
    let seconds = seconds.parse::<u32>().ok()?;
    let nanos = match fraction {
        Some(fraction) if fraction.is_empty() || fraction.len() > 9 => return None,
        Some(fraction) => {
            if !fraction.chars().all(|character| character.is_ascii_digit()) {
                return None;
            }
            let padded = format!("{fraction:0<9}");
            padded.parse::<u32>().ok()?
        }
        None => 0,
    };
    Some((seconds, nanos))
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let adjusted_year = year + i64::from(month <= 2);
    (adjusted_year, month as u32, day as u32)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i128> {
    if day > days_in_month(year, month)? {
        return None;
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(i128::from(era * 146_097 + day_of_era - 719_468))
}

fn days_in_month(year: i64, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
