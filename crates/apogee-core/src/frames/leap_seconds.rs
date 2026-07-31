//! Leap second table parser.
//!
//! Parses IERS leap second announcement files (e.g. naif0012.txt, tai-utc.dat).
//! Format: lines starting with '+' contain: year month day MJD TAI-UTC

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
            if !trimmed.starts_with('+') {
                continue;
            }

            let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
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
        }

        // Sort by MJD for correct lookup
        entries.sort_by_key(|e| e.mjd);

        Ok(Self { entries })
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
