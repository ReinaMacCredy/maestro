//! Age-based staleness signal for proposed features.
//!
//! A `proposed` feature that has not moved in [`STALE_PROPOSED_THRESHOLD_DAYS`]
//! reads as stale: it is sitting in the backlog with no build-or-retire
//! decision. The signal is deliberately age-only -- its sole inputs are the
//! feature's `updated_at` and the current time. maestro never scans git or
//! sibling cards to guess whether the work already shipped under another card;
//! the agent makes that call, prompted by the surfaced reminder.

use crate::domain::feature::schema::FeatureStatus;
use crate::foundation::core::time::timestamp_nanos;

/// A proposed feature idle for at least this many whole days reads as stale.
/// Tunable here at build time; intentionally not a runtime knob.
pub const STALE_PROPOSED_THRESHOLD_DAYS: u64 = 14;

const NANOS_PER_DAY: i128 = 24 * 60 * 60 * 1_000_000_000;

/// Whole days elapsed between `updated_at` and `now`, or `None` when
/// `updated_at` does not parse. A future `updated_at` clamps to `0`.
pub fn age_days(updated_at: &str, now_nanos: i128) -> Option<u64> {
    let then = timestamp_nanos(updated_at)?;
    Some(((now_nanos - then).max(0) / NANOS_PER_DAY) as u64)
}

/// Whether a feature is a stale proposed card: `proposed` and idle for at least
/// [`STALE_PROPOSED_THRESHOLD_DAYS`]. The only inputs are the status,
/// `updated_at`, and `now` -- no git or cross-card scan -- so the signal can
/// never compute a per-feature "looks shipped" verdict.
pub fn is_stale_proposed(status: &FeatureStatus, updated_at: &str, now_nanos: i128) -> bool {
    matches!(status, FeatureStatus::Proposed)
        && age_days(updated_at, now_nanos).is_some_and(|days| days >= STALE_PROPOSED_THRESHOLD_DAYS)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixed reference instant so day-boundary assertions are deterministic.
    const NOW: &str = "2026-06-21T00:00:00.000Z";

    fn now() -> i128 {
        timestamp_nanos(NOW).expect("fixed now parses")
    }

    #[test]
    fn age_days_floors_to_whole_days() {
        // exactly 14 days earlier (2026-06-07 -> 2026-06-21 is 14 days)
        assert_eq!(age_days("2026-06-07T00:00:00.000Z", now()), Some(14));
        // one second short of 14 days -> 13 whole days
        assert_eq!(age_days("2026-06-07T00:00:01.000Z", now()), Some(13));
        // 13 days earlier
        assert_eq!(age_days("2026-06-08T00:00:00.000Z", now()), Some(13));
        // same instant -> 0
        assert_eq!(age_days(NOW, now()), Some(0));
    }

    #[test]
    fn age_days_clamps_future_updates_to_zero() {
        assert_eq!(age_days("2026-07-01T00:00:00.000Z", now()), Some(0));
    }

    #[test]
    fn age_days_is_none_for_unparseable_timestamp() {
        assert_eq!(age_days("not-a-timestamp", now()), None);
    }

    #[test]
    fn proposed_at_or_past_threshold_is_stale() {
        // exactly 14 days idle -> stale (boundary is inclusive)
        assert!(is_stale_proposed(
            &FeatureStatus::Proposed,
            "2026-06-07T00:00:00.000Z",
            now()
        ));
        // 20 days idle -> stale
        assert!(is_stale_proposed(
            &FeatureStatus::Proposed,
            "2026-06-01T00:00:00.000Z",
            now()
        ));
    }

    #[test]
    fn proposed_under_threshold_is_fresh() {
        // 13 days idle -> fresh
        assert!(!is_stale_proposed(
            &FeatureStatus::Proposed,
            "2026-06-08T00:00:00.000Z",
            now()
        ));
        // one second short of 14 days -> fresh
        assert!(!is_stale_proposed(
            &FeatureStatus::Proposed,
            "2026-06-07T00:00:01.000Z",
            now()
        ));
    }

    #[test]
    fn non_proposed_is_never_stale_regardless_of_age() {
        for status in [
            FeatureStatus::Ready,
            FeatureStatus::InProgress,
            FeatureStatus::Closed,
            FeatureStatus::Cancelled,
        ] {
            assert!(!is_stale_proposed(&status, "2026-01-01T00:00:00.000Z", now()));
        }
    }

    #[test]
    fn proposed_with_unparseable_timestamp_is_not_stale() {
        assert!(!is_stale_proposed(&FeatureStatus::Proposed, "garbage", now()));
    }
}
