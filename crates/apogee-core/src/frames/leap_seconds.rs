//! Leap second table parser.
//!
//! Parses IERS leap second announcement files in two formats:
//!
//! 1. IERS `Leap_Second.dat` / `tai-utc.dat` (used by IERS Bulletin C):
//!    ```text
//!    #    MJD        Date        TAI-UTC (s)
//!    #           day month year
//!    #    ---    --------------   ------
//!    #
//!       41317.0    1  1 1972       10
//!       41499.0    1  7 1972       11
//!    ```
//!    Columns: MJD day month year TAI-UTC.
//!
//! 2. NAIF `naif0012.tls` style (used by SPICE):
//!    ```text
//!    + 1972  1  1 41317 10
//!    + 1972  7  1 41499 11
//!    ```
//!    Columns: year month day MJD TAI-UTC.
//!
//! Both formats are supported by the parser.

use apogee_common::{ApogeeError, ApogeeResult};

/// A single leap second entry.
#[derive(Debug, Clone)]
pub struct LeapSecondEntry {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub mjd: u64,
    pub tai_utc: i32,
}

/// Leap second table.
#[derive(Debug, Default)]
pub struct LeapSecondTable {
    entries: Vec<LeapSecondEntry>,
}

impl LeapSecondTable {
    pub fn parse(input: &str) -> ApogeeResult<Self> {
        let mut entries = Vec::new();

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // NAIF/SPICE format: + year month day mjd tai_utc
            if let Some(rest) = trimmed.strip_prefix('+') {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() < 5 {
                    return Err(ApogeeError::Data(format!(
                        "leap second line needs 5 fields, got {}: {line}",
                        parts.len()
                    )));
                }
                let year = parts[0]
                    .parse::<u32>()
                    .map_err(|e| ApogeeError::Data(e.to_string()))?;
                let month = parts[1]
                    .parse::<u32>()
                    .map_err(|e| ApogeeError::Data(e.to_string()))?;
                let day = parts[2]
                    .parse::<u32>()
                    .map_err(|e| ApogeeError::Data(e.to_string()))?;
                let mjd = parts[3]
                    .parse::<u64>()
                    .map_err(|e| ApogeeError::Data(e.to_string()))?;
                let tai_utc = parts[4]
                    .parse::<i32>()
                    .map_err(|e| ApogeeError::Data(e.to_string()))?;
                entries.push(LeapSecondEntry {
                    year,
                    month,
                    day,
                    mjd,
                    tai_utc,
                });
                continue;
            }

            // IERS format: mjd day month year tai_utc
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 5 {
                return Err(ApogeeError::Data(format!(
                    "leap second line needs 5 fields, got {}: {line}",
                    parts.len()
                )));
            }
            let mjd = parts[0]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let day = parts[1]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let month = parts[2]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let year = parts[3]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let tai_utc = parts[4]
                .parse::<i32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;

            entries.push(LeapSecondEntry {
                year,
                month,
                day,
                mjd: mjd.round() as u64,
                tai_utc,
            });
        }

        // Sort by MJD for correct lookup
        entries.sort_by_key(|e| e.mjd);

        Ok(Self { entries })
    }

    /// Load leap second table from a file.
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> ApogeeResult<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ApogeeError::Data(format!("failed to read leap second file: {e}")))?;
        Self::parse(&content)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[LeapSecondEntry] {
        &self.entries
    }

    /// Look up the TAI-UTC offset at a given Modified Julian Date.
    /// Returns the offset from the last entry at or before the given MJD.
    /// Returns 0 if before the first entry.
    pub fn tai_utc_at_mjd(&self, mjd: u64) -> i32 {
        self.entries
            .iter()
            .rev()
            .find(|e| e.mjd <= mjd)
            .map(|e| e.tai_utc)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_iers_leap_second_dat_format() {
        let data = "    41317.0    1  1 1972       10\n\
                      41499.0    1  7 1972       11\n\
                      41683.0    1  1 1973       12";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.len(), 3);
        assert_eq!(table.entries()[0].mjd, 41317);
        assert_eq!(table.entries()[0].tai_utc, 10);
        assert_eq!(table.entries()[1].mjd, 41499);
        assert_eq!(table.entries()[2].tai_utc, 12);
    }

    #[test]
    fn test_parses_naif_format() {
        let data = "+ 1972  1  1 41317 10\n\
                     + 1972  7  1 41499 11";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table.entries()[0].mjd, 41317);
        assert_eq!(table.entries()[0].tai_utc, 10);
    }

    #[test]
    fn test_lookup_tai_utc_at_mjd() {
        let data = "    41317.0    1  1 1972       10\n\
                      41499.0    1  7 1972       11";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.tai_utc_at_mjd(41317), 10);
        assert_eq!(table.tai_utc_at_mjd(41499), 11);
        assert_eq!(table.tai_utc_at_mjd(41400), 10);
    }

    #[test]
    fn test_tai_utc_at_mjd_before_first_is_zero() {
        let data = "+ 1972  1  1 41317 10";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.tai_utc_at_mjd(40000), 0);
    }

    #[test]
    fn test_tai_utc_at_mjd_after_last() {
        let data = "+ 1972  1  1 41317 10\n\
                     + 1972  7  1 41499 11";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.tai_utc_at_mjd(60000), 11);
    }

    #[test]
    fn test_leap_second_offset_matches_iers_history() {
        // Validates the `tai_utc_at_mjd` lookup against the complete IERS
        // leap second history from Bulletin C. The table covers every offset
        // change from 1972 to 2017. We check three boundary conditions:
        //   - before the first leap second (no offset)
        //   - after the last leap second (current 37 s offset)
        //   - mid-table at MJD 53000 (2004-02-01, 32 s offset)
        let data = "    41317.0    1  1 1972       10\n\
                      41499.0    1  7 1972       11\n\
                      41683.0    1  1 1973       12\n\
                      42048.0    1  1 1974       13\n\
                      42413.0    1  1 1975       14\n\
                      42778.0    1  1 1976       15\n\
                      43144.0    1  1 1977       16\n\
                      43509.0    1  1 1978       17\n\
                      43874.0    1  1 1979       18\n\
                      44239.0    1  1 1980       19\n\
                      44786.0    1  7 1981       20\n\
                      45151.0    1  7 1982       21\n\
                      45516.0    1  7 1983       22\n\
                      46247.0    1  7 1985       23\n\
                      47161.0    1  1 1988       24\n\
                      47892.0    1  1 1990       25\n\
                      48257.0    1  1 1991       26\n\
                      48804.0    1  7 1992       27\n\
                      49169.0    1  7 1993       28\n\
                      49534.0    1  7 1994       29\n\
                      50083.0    1  1 1996       30\n\
                      50630.0    1  7 1997       31\n\
                      51179.0    1  1 1999       32\n\
                      53736.0    1  1 2006       33\n\
                      54832.0    1  1 2009       34\n\
                      56109.0    1  7 2012       35\n\
                      57204.0    1  7 2015       36\n\
                      57754.0    1  1 2017       37";
        let table = LeapSecondTable::parse(data).unwrap();
        assert_eq!(table.tai_utc_at_mjd(40000), 0);
        assert_eq!(table.tai_utc_at_mjd(60000), 37);
        assert_eq!(table.tai_utc_at_mjd(53000), 32); // 2004-02-01
    }
}
