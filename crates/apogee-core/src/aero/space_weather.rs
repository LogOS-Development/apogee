//! Space weather data loader: F10.7, Ap/Kp indices.
//!
//! Parses NOAA SWPC CSV format: date,f10.7,f10.7a,ap,kp

use apogee_common::{ApogeeError, ApogeeResult};

/// Space weather data for a given date.
#[derive(Debug, Clone, Default)]
pub struct SpaceWeather {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub f107: f64,  // solar flux units (10^-22 W/m^2/Hz)
    pub f107a: f64, // 81-day smoothed
    pub ap: f64,    // geomagnetic index
    pub kp: f64,    // planetary K-index
}

/// Space weather database.
#[derive(Debug, Default)]
pub struct SpaceWeatherData {
    entries: Vec<SpaceWeather>,
}

impl SpaceWeatherData {
    /// Parse CSV data: date,f10.7,f10.7a,ap,kp (one per line).
    /// Lines with alphabetic first field (headers) are skipped.
    /// Lines that look like data but fail to parse return an error.
    pub fn parse(input: &str) -> ApogeeResult<Self> {
        let mut entries = Vec::new();

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parts: Vec<&str> = trimmed.split(',').collect();
            if parts.len() < 5 {
                return Err(ApogeeError::Data(format!(
                    "space weather line needs 5 fields, got {}: {line}",
                    parts.len()
                )));
            }

            // Skip header lines where first field is a known header keyword
            let first_field = parts[0].trim().to_lowercase();
            if first_field == "date" || first_field == "yyyy-mm-dd" || first_field == "yyyymmdd" {
                continue;
            }

            // Parse date: YYYY-MM-DD
            let date_parts: Vec<&str> = parts[0].split('-').collect();
            if date_parts.len() != 3 {
                return Err(ApogeeError::Data(format!("bad date format: {}", parts[0])));
            }
            let year = date_parts[0]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let month = date_parts[1]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let day = date_parts[2]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;

            let f107 = parts[1]
                .trim()
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let f107a = parts[2]
                .trim()
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let ap = parts[3]
                .trim()
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;
            let kp = parts[4]
                .trim()
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(e.to_string()))?;

            entries.push(SpaceWeather {
                year,
                month,
                day,
                f107,
                f107a,
                ap,
                kp,
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

    pub fn entries(&self) -> &[SpaceWeather] {
        &self.entries
    }

    /// Look up space weather by date.
    pub fn at_date(&self, year: u32, month: u32, day: u32) -> Option<&SpaceWeather> {
        self.entries
            .iter()
            .find(|e| e.year == year && e.month == month && e.day == day)
    }
}
