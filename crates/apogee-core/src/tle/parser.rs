//! TLE (Two-Line Element) parser.
//!
//! Parses NORAD Two-Line Element sets. Reference format:
//! https://celestrak.org/NORAD/documentation/tle-fmt.php

use apogee_common::{ApogeeError, ApogeeResult};

/// Parsed TLE data.
#[derive(Debug, Clone)]
pub struct Tle {
    /// Name line (optional).
    pub name: Option<String>,
    /// Satellite catalog number (line 1, cols 3–7).
    pub satellite_number: u64,
    /// Classification: U=Unclassified, C=Classified, S=Secret (col 8).
    pub classification: char,
    /// International designator (cols 10–17).
    pub international_designator: String,
    /// Epoch year (4-digit, e.g. 2024).
    pub epoch_year: u32,
    /// Epoch day of year + fractional day (cols 21–32).
    pub epoch_day: f64,
    /// First derivative of mean motion (cols 34–43).
    pub mean_motion_dot: f64,
    /// Second derivative of mean motion (cols 45–52).
    pub mean_motion_ddot: f64,
    /// BSTAR drag term (cols 54–61).
    pub bstar: f64,
    /// Ephemeris type (col 63).
    pub ephemeris_type: u8,
    /// Element set number (cols 65–68).
    pub element_set_number: u32,
    /// Line 1 checksum (col 69).
    pub line1_checksum: u8,
    // Line 2 fields
    /// Inclination in degrees (cols 9–16).
    pub inclination: f64,
    /// Right ascension of ascending node in degrees (cols 18–25).
    pub raan: f64,
    /// Eccentricity (cols 27–33, implied decimal point).
    pub eccentricity: f64,
    /// Argument of perigee in degrees (cols 35–42).
    pub arg_perigee: f64,
    /// Mean anomaly in degrees (cols 44–51).
    pub mean_anomaly: f64,
    /// Mean motion in revs/day (cols 53–63).
    pub mean_motion: f64,
    /// Revolution number at epoch (cols 64–68).
    pub revolution_number: u64,
    /// Line 2 checksum (col 69).
    pub line2_checksum: u8,
    // Raw lines for checksum verification.
    raw_line1: String,
    raw_line2: String,
}

impl Tle {
    /// Parse a TLE from a string containing optional name line + two lines.
    pub fn parse(input: &str) -> ApogeeResult<Self> {
        let trimmed = input.trim();
        let all_lines: Vec<&str> = trimmed.lines().collect();

        let (name, line1, line2) = match all_lines.len() {
            3 => (
                Some(all_lines[0].trim().to_string()),
                all_lines[1],
                all_lines[2],
            ),
            2 => (None, all_lines[0], all_lines[1]),
            _ => return Err(ApogeeError::Data("TLE must have 2 or 3 lines".into())),
        };

        if !line1.starts_with('1') {
            return Err(ApogeeError::Data("Line 1 must start with '1'".into()));
        }
        if !line2.starts_with('2') {
            return Err(ApogeeError::Data("Line 2 must start with '2'".into()));
        }
        if line1.len() < 24 {
            return Err(ApogeeError::Data("Line 1 too short".into()));
        }

        let satellite_number = parse_u64(line1, 2, 7)?;
        let classification = line1.chars().nth(7).unwrap();
        let international_designator = line1[9..17].trim().to_string();
        let epoch_year = parse_u32(line1, 18, 20)? + 2000;
        let epoch_day = parse_f64(line1, 20, 32)?;
        let mean_motion_dot = parse_f64(line1, 33, 43)?;
        let mean_motion_ddot = parse_scientific(line1, 44, 52)?;
        let bstar = parse_scientific(line1, 53, 61)?;
        let ephemeris_type = parse_u8(line1, 62, 63)?;
        let element_set_number = parse_u32(line1, 64, 68)?;
        let line1_checksum = parse_u8(line1, 68, 69)?;

        let inclination = parse_f64(line2, 8, 16)?;
        let raan = parse_f64(line2, 17, 25)?;
        let eccentricity = format!("0.{}", line2[26..33].trim())
            .parse::<f64>()
            .map_err(|e| ApogeeError::Data(e.to_string()))?;
        let arg_perigee = parse_f64(line2, 34, 42)?;
        let mean_anomaly = parse_f64(line2, 43, 51)?;
        let mean_motion = parse_f64(line2, 52, 63)?;
        let revolution_number = parse_u64(line2, 63, 68)?;
        let line2_checksum = parse_u8(line2, 68, 69)?;

        Ok(Self {
            name,
            satellite_number,
            classification,
            international_designator,
            epoch_year,
            epoch_day,
            mean_motion_dot,
            mean_motion_ddot,
            bstar,
            ephemeris_type,
            element_set_number,
            line1_checksum,
            inclination,
            raan,
            eccentricity,
            arg_perigee,
            mean_anomaly,
            mean_motion,
            revolution_number,
            line2_checksum,
            raw_line1: line1.to_string(),
            raw_line2: line2.to_string(),
        })
    }

    /// Verify line 1 checksum (mod-10 sum of digits, minus signs = 1, other chars = 0).
    pub fn verify_line1_checksum(&self) -> bool {
        compute_checksum(&self.raw_line1) == self.line1_checksum
    }

    /// Verify line 2 checksum.
    pub fn verify_line2_checksum(&self) -> bool {
        compute_checksum(&self.raw_line2) == self.line2_checksum
    }
}

/// Compute TLE checksum: sum all digits, minus signs count as 1,
/// other characters count as 0, take mod 10.
/// Computed on all characters except the last (the checksum column itself).
fn compute_checksum(line: &str) -> u8 {
    let mut sum: u64 = 0;
    for c in line.chars().take(line.len().saturating_sub(1)) {
        match c {
            '0'..='9' => sum += c.to_digit(10).unwrap() as u64,
            '-' => sum += 1,
            _ => {}
        }
    }
    (sum % 10) as u8
}

fn parse_u64(line: &str, start: usize, end: usize) -> ApogeeResult<u64> {
    line.get(start..end)
        .ok_or_else(|| ApogeeError::Data(format!("column {}..{} out of range", start, end)))?
        .trim()
        .parse::<u64>()
        .map_err(|e| ApogeeError::Data(e.to_string()))
}

fn parse_u32(line: &str, start: usize, end: usize) -> ApogeeResult<u32> {
    line.get(start..end)
        .ok_or_else(|| ApogeeError::Data(format!("column {}..{} out of range", start, end)))?
        .trim()
        .parse::<u32>()
        .map_err(|e| ApogeeError::Data(e.to_string()))
}

fn parse_u8(line: &str, start: usize, end: usize) -> ApogeeResult<u8> {
    line.get(start..end)
        .ok_or_else(|| ApogeeError::Data(format!("column {}..{} out of range", start, end)))?
        .trim()
        .parse::<u8>()
        .map_err(|e| ApogeeError::Data(e.to_string()))
}

fn parse_f64(line: &str, start: usize, end: usize) -> ApogeeResult<f64> {
    line.get(start..end)
        .ok_or_else(|| ApogeeError::Data(format!("column {}..{} out of range", start, end)))?
        .trim()
        .parse::<f64>()
        .map_err(|e| ApogeeError::Data(e.to_string()))
}

/// Parse TLE scientific notation: " 10270-3" means 0.10270 × 10^-3 = 0.00010270.
/// Format: 5-digit mantissa (implied decimal after first digit) + exponent sign + 1-digit exponent.
fn parse_scientific(line: &str, start: usize, end: usize) -> ApogeeResult<f64> {
    let raw = line
        .get(start..end)
        .ok_or_else(|| ApogeeError::Data(format!("column {}..{} out of range", start, end)))?;
    let trimmed = raw.trim();

    if trimmed.is_empty() || trimmed.len() < 3 {
        return Ok(0.0);
    }

    let exp_part = &trimmed[trimmed.len() - 2..];
    let mantissa_part = &trimmed[..trimmed.len() - 2];

    let exp_sign = exp_part.chars().next().unwrap();
    let exp_digit = exp_part[1..]
        .parse::<i32>()
        .map_err(|e| ApogeeError::Data(e.to_string()))?;
    let exponent = if exp_sign == '-' {
        -exp_digit
    } else {
        exp_digit
    };

    // Mantissa has implied decimal point: "10270" → 0.10270
    let mantissa: f64 = if mantissa_part.trim().is_empty() {
        0.0
    } else {
        let digits = mantissa_part.trim();
        let val: f64 = digits
            .parse::<f64>()
            .map_err(|e| ApogeeError::Data(e.to_string()))?;
        val / 10f64.powi(digits.len() as i32)
    };

    Ok(mantissa * 10f64.powi(exponent))
}
