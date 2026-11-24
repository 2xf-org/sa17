//! Days, as the engine counts them.

/// Days before the first of each month in a year that starts with March.
const BEFORE_MONTH: [u32; 12] = [0, 31, 61, 92, 122, 153, 184, 214, 245, 275, 306, 337];

/// The days from 29 February 1600 to a day written as `2024-01-31`.
///
/// # Panics
///
/// Panics when `text` is not such a day.
pub(crate) fn days(text: &str) -> u32 {
    let parts = text.split('-').map(str::parse);
    let parts: Result<Vec<u32>, _> = parts.collect();
    let Ok([year, month, day]) = parts.as_deref() else {
        panic!("{text} is not a day");
    };
    assert!(
        (1601..9999).contains(year) && (1..=12).contains(month) && (1..=31).contains(day),
        "{text} is not a day"
    );
    // A year that starts with March ends with the leap day.
    let march_year = if *month < 3 { year - 1 } else { *year } - 1600;
    let leap_days = march_year / 4 - march_year / 100 + march_year / 400;
    let month = (month + 9) % 12;
    march_year * 365 + leap_days + BEFORE_MONTH[month as usize] + day
}
