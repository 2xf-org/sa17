//! The engine's calendar: the first and the last `DATE`, `TIME` and
//! `TIMESTAMP` that a database can hold.

use chrono::{Days, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta};

const MINUTES_PER_DAY: u32 = 24 * 60;
const MICROSECONDS_PER_MINUTE: u32 = 60 * 1_000_000;

/// The first `DATE`. Dates and timestamps count minutes from its start, in 32
/// bits.
pub const FIRST_DATE: NaiveDate =
    NaiveDate::from_ymd_opt(1600, 2, 29).expect("29 February 1600 is a date");

/// The last `DATE`.
pub const LAST_DATE: NaiveDate =
    NaiveDate::from_ymd_opt(9766, 4, 15).expect("15 April 9766 is a date");

/// The first `TIMESTAMP`.
pub const FIRST_TIMESTAMP: NaiveDateTime = FIRST_DATE.and_time(NaiveTime::MIN);

/// The last `TIMESTAMP`.
pub const LAST_TIMESTAMP: NaiveDateTime = LAST_DATE
    .and_hms_micro_opt(4, 15, 59, 999_999)
    .expect("04:15:59 is a time of day");

/// The first `TIME`.
pub const FIRST_TIME: NaiveTime = NaiveTime::MIN;

/// The last `TIME`.
pub const LAST_TIME: NaiveTime =
    NaiveTime::from_hms_micro_opt(23, 59, 59, 999_999).expect("23:59:59 is a time of day");

/// The day that starts after `minutes`, or `None` when none does.
pub(crate) fn date(minutes: u32) -> Option<NaiveDate> {
    if !minutes.is_multiple_of(MINUTES_PER_DAY) {
        return None;
    }
    FIRST_DATE.checked_add_days(Days::new((minutes / MINUTES_PER_DAY).into()))
}

/// The moment after `minutes` and `microseconds` past the minute, or `None`
/// when the minute has fewer.
pub(crate) fn timestamp(minutes: u32, microseconds: u32) -> Option<NaiveDateTime> {
    if microseconds >= MICROSECONDS_PER_MINUTE {
        return None;
    }
    let elapsed = TimeDelta::minutes(minutes.into()) + TimeDelta::microseconds(microseconds.into());
    FIRST_TIMESTAMP.checked_add_signed(elapsed)
}

/// The time of day after `minutes` and `microseconds` past the minute, or
/// `None` when the day or the minute has fewer.
pub(crate) fn time(minutes: u32, microseconds: u32) -> Option<NaiveTime> {
    if microseconds >= MICROSECONDS_PER_MINUTE {
        return None;
    }
    let seconds = minutes.checked_mul(60)?;
    let seconds = seconds.checked_add(microseconds / 1_000_000)?;
    NaiveTime::from_num_seconds_from_midnight_opt(seconds, microseconds % 1_000_000 * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_timestamps_count_minutes_in_32_bits() {
        assert_eq!(date(0), Some(FIRST_DATE));
        let last_day = u32::MAX / MINUTES_PER_DAY * MINUTES_PER_DAY;
        assert_eq!(date(last_day), Some(LAST_DATE));
        assert_eq!(timestamp(0, 0), Some(FIRST_TIMESTAMP));
        assert_eq!(timestamp(u32::MAX, 59_999_999), Some(LAST_TIMESTAMP));
        assert_eq!(
            timestamp(1, 30_500_000).unwrap().to_string(),
            "1600-02-29 00:01:30.500"
        );
        assert_eq!(time(0, 0), Some(FIRST_TIME));
        assert_eq!(time(1439, 59_999_999), Some(LAST_TIME));
    }

    #[test]
    fn a_count_past_its_unit_is_no_time() {
        assert_eq!(date(1), None);
        assert_eq!(date(1439), None);
        assert_eq!(timestamp(0, 60_000_000), None);
        assert_eq!(timestamp(u32::MAX, u32::MAX), None);
        assert_eq!(time(1440, 0), None);
        assert_eq!(time(u32::MAX, 0), None);
        assert_eq!(time(0, 60_000_000), None);
    }
}
