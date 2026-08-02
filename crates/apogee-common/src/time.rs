//! Time utilities shared across Apogee crates.
//!
//! Wrappers around [`hifitime`] for the common time representations needed in
//! physical models (decimal years, secular variation, etc.).

use hifitime::{Epoch, TimeScale, Unit};

/// Convert an epoch to a decimal year in UTC.
///
/// Decimal years are used by empirical models such as IGRF for secular
/// variation extrapolation. The conversion accounts for the actual number of
/// days in the civil year (365 or 366), so 1 July is always ~0.5 regardless of
/// leap years.
///
/// # Sources
/// * IGRF technical note: coefficients are referenced to epoch `YYYY.0` and
///   linearly extrapolated using annual secular-variation rates.
#[must_use]
pub fn decimal_year(epoch: Epoch) -> f64 {
    let utc = epoch.to_time_scale(TimeScale::UTC);
    let year = utc.year();
    let start_of_year = Epoch::from_gregorian_utc(year, 1, 1, 0, 0, 0, 0);
    let start_of_next = Epoch::from_gregorian_utc(year + 1, 1, 1, 0, 0, 0, 0);
    let days_in_year = (start_of_next - start_of_year).to_unit(Unit::Day);
    let day_of_year = utc.day_of_year(); // 1-based, fractional
    f64::from(year) + (day_of_year - 1.0) / days_in_year
}

/// Parse an ISO-like date string `YYYY-MM-DD` into a `(year, month, day)` tuple.
///
/// This helper is used by fixture-driven tests that read date columns from CSV
/// files. It does not perform validation; the caller (typically `hifitime`)
/// validates the resulting date.
#[must_use]
pub fn parse_iso_date(s: &str) -> (i32, u8, u8) {
    let parts: Vec<&str> = s.split('-').collect();
    (
        parts[0].parse().expect("year"),
        parts[1].parse().expect("month"),
        parts[2].parse().expect("day"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn decimal_year_start_of_year() {
        let epoch = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0);
        assert_relative_eq!(decimal_year(epoch), 2024.0, epsilon = 1e-6);
    }

    #[test]
    fn decimal_year_mid_year() {
        let epoch = Epoch::from_gregorian_utc(2023, 7, 2, 0, 0, 0, 0);
        // 2023-07-02 is day 183 of a 365-day year -> 2023 + 182/365.
        assert_relative_eq!(decimal_year(epoch), 2_023.498_630_136_986_4, epsilon = 1e-9);
    }

    #[test]
    fn decimal_year_leap_year() {
        let epoch = Epoch::from_gregorian_utc(2024, 7, 2, 0, 0, 0, 0);
        // 184 days into 366-day year -> 2024 + 183/366.
        assert_relative_eq!(decimal_year(epoch), 2024.5, epsilon = 1e-9);
    }

    #[test]
    fn parse_iso_date_splits_components() {
        assert_eq!(parse_iso_date("2024-03-15"), (2024, 3, 15));
    }
}
