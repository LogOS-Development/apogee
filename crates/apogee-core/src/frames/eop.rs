//! Earth Orientation Parameters loader.
//!
//! Parses IERS EOP C04 series. Fixed-width column format:
//! Year Month Day MJD x-pole y-pole UT1-UTC LOD dPsi dEps dX dY
//! Units: arcseconds for x/y pole, seconds for UT1-UTC and LOD.

use apogee_common::{ApogeeError, ApogeeResult};

/// EOP entry: polar motion, UT1-UTC, and length-of-day offset.
#[derive(Debug, Default, Clone)]
pub struct EopEntry {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub mjd: f64,
    pub x_pole: f64,  // arcseconds
    pub y_pole: f64,  // arcseconds
    pub ut1_utc: f64, // seconds
    pub lod: f64,     // seconds
}

/// EOP C04 series.
#[derive(Debug, Default)]
pub struct EopData {
    entries: Vec<EopEntry>,
}

impl EopData {
    /// Parse EOP C04 text data.
    pub fn parse(input: &str) -> ApogeeResult<Self> {
        let mut entries = Vec::new();

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip header lines that don't start with a 4-digit year
            if !trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            // Skip lines that don't have enough fields
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 8 {
                continue;
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
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let x_pole = parts[4]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let y_pole = parts[5]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let ut1_utc = parts[6]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let lod = parts[7]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;

            entries.push(EopEntry {
                year,
                month,
                day,
                mjd,
                x_pole,
                y_pole,
                ut1_utc,
                lod,
            });
        }

        Ok(Self { entries })
    }

    /// Load from file path.
    pub fn load(path: &str) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[EopEntry] {
        &self.entries
    }

    /// Look up EOP at a given MJD. Linearly interpolates between entries.
    /// Returns None if before the first entry. Clamps to last entry if after.
    pub fn at_mjd(&self, mjd: f64) -> Option<EopEntry> {
        if self.entries.is_empty() || mjd < self.entries[0].mjd {
            return None;
        }

        // Find bracketing entries
        let idx = self
            .entries
            .partition_point(|e| e.mjd <= mjd)
            .saturating_sub(1);

        if idx >= self.entries.len() - 1 {
            // At or after last entry — return last
            return Some(self.entries.last().unwrap().clone());
        }

        let e0 = &self.entries[idx];
        let e1 = &self.entries[idx + 1];
        let dt = e1.mjd - e0.mjd;
        if dt == 0.0 {
            return Some(e0.clone());
        }
        let t = (mjd - e0.mjd) / dt;

        Some(EopEntry {
            year: e0.year,
            month: e0.month,
            day: e0.day,
            mjd,
            x_pole: e0.x_pole + t * (e1.x_pole - e0.x_pole),
            y_pole: e0.y_pole + t * (e1.y_pole - e0.y_pole),
            ut1_utc: e0.ut1_utc + t * (e1.ut1_utc - e0.ut1_utc),
            lod: e0.lod + t * (e1.lod - e0.lod),
        })
    }
}
