//! Earth Orientation Parameters loader and cache.
//!
//! Parses the IERS EOP C04 time series and optionally fetches it from
//! <https://hpiers.obspm.fr/iers/eop/eopc04/eopc04.1962-now>. The data is
//! cached locally so offline/CI runs can fall back to a bundled snapshot.
//!
//! # Format
//!
//! The official IERS EOP C04 file uses fixed-width columns:
//!
//! ```text
//!  YR  MM  DD  HH       MJD        x(")        y(")  UT1-UTC(s)       dX(")       dY(")  xrt("/day)  yrt("/day)      LOD(s)        x Er        y Er  UT1-UTC Er       dX Er       dY Er      xrt Er      yrt Er      LOD Er
//! 1962   1   1   0  37665.00   -0.012700    0.213000   0.0326338    0.000000    0.000000    0.000000    0.000000   0.0017230    0.030000    0.030000   0.0020000    0.004774    0.002000    0.000000    0.000000   0.0014000
//! ```
//!
//! Units: arcseconds for polar motion (x, y) and celestial pole offsets
//! (dX, dY); seconds for UT1-UTC and length-of-day (LOD); arcseconds/day
//! for pole rates.
//!
//! # References
//!
//! - IERS EOP C04: <https://hpiers.obspm.fr/iers/eop/eopc04/>
//! - IERS Conventions (2010), Ch. 5
//!   <https://iers-conventions.obspm.fr/content/tn36.pdf>
//! - IERS EOP 08 C04 format description
//!   <https://hpiers.obspm.fr/eoppc/eop/eopc04/readme>

use apogee_common::{ApogeeError, ApogeeResult};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// EOP entry: polar motion, UT1-UTC, length-of-day, and celestial-pole offsets.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EopEntry {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub mjd: f64,
    /// Polar motion x component in arcseconds.
    pub x_pole: f64,
    /// Polar motion y component in arcseconds.
    pub y_pole: f64,
    /// UT1-UTC offset in seconds.
    pub ut1_utc: f64,
    /// Celestial pole offset dX in arcseconds (IAU 2000/2006).
    pub dx: f64,
    /// Celestial pole offset dY in arcseconds (IAU 2000/2006).
    pub dy: f64,
    /// Polar motion x rate in arcseconds per day.
    pub x_rate: f64,
    /// Polar motion y rate in arcseconds per day.
    pub y_rate: f64,
    /// Length-of-day offset in seconds.
    pub lod: f64,
}

/// In-memory EOP C04 series.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EopData {
    entries: Vec<EopEntry>,
}

impl EopData {
    /// Parse EOP C04 text data.
    ///
    /// The parser is whitespace-delimited and tolerates leading/trailing
    /// header lines, blank lines, and comment lines starting with `#`.
    pub fn parse(input: &str) -> ApogeeResult<Self> {
        let mut entries = Vec::new();

        for line in input.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Skip lines that don't start with a 4-digit year.
            if !trimmed.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 13 {
                continue;
            }

            let year = parts[0]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(format!("invalid year: {e}")))?;
            let month = parts[1]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(format!("invalid month: {e}")))?;
            let day = parts[2]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(format!("invalid day: {e}")))?;
            let hour = parts[3]
                .parse::<u32>()
                .map_err(|e| ApogeeError::Data(format!("invalid hour: {e}")))?;
            let mjd = parts[4]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid MJD: {e}")))?;
            let x_pole = parts[5]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid x_pole: {e}")))?;
            let y_pole = parts[6]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid y_pole: {e}")))?;
            let ut1_utc = parts[7]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid UT1-UTC: {e}")))?;
            let dx = parts[8]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid dX: {e}")))?;
            let dy = parts[9]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid dY: {e}")))?;
            let x_rate = parts[10]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid x_rate: {e}")))?;
            let y_rate = parts[11]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid y_rate: {e}")))?;
            let lod = parts[12]
                .parse::<f64>()
                .map_err(|e| ApogeeError::Data(format!("invalid LOD: {e}")))?;

            entries.push(EopEntry {
                year,
                month,
                day,
                hour,
                mjd,
                x_pole,
                y_pole,
                ut1_utc,
                dx,
                dy,
                x_rate,
                y_rate,
                lod,
            });
        }

        if entries.is_empty() {
            return Err(ApogeeError::Data(
                "no valid EOP entries found in input".to_string(),
            ));
        }

        Ok(Self { entries })
    }

    /// Load from a local file path.
    pub fn load<P: AsRef<Path>>(path: P) -> ApogeeResult<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| ApogeeError::Data(format!("failed to read EOP file: {e}")))?;
        Self::parse(&content)
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

    /// Look up EOP at a given MJD. Linearly interpolates between bracketing
    /// entries for all continuous quantities. Returns `None` if before the
    /// first entry. Clamps to the last entry if after the series end.
    pub fn at_mjd(&self, mjd: f64) -> Option<EopEntry> {
        if self.entries.is_empty() || mjd < self.entries[0].mjd {
            return None;
        }

        let idx = self
            .entries
            .partition_point(|e| e.mjd <= mjd)
            .saturating_sub(1);

        if idx >= self.entries.len() - 1 {
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
            hour: e0.hour,
            mjd,
            x_pole: lerp(e0.x_pole, e1.x_pole, t),
            y_pole: lerp(e0.y_pole, e1.y_pole, t),
            ut1_utc: lerp(e0.ut1_utc, e1.ut1_utc, t),
            dx: lerp(e0.dx, e1.dx, t),
            dy: lerp(e0.dy, e1.dy, t),
            x_rate: lerp(e0.x_rate, e1.x_rate, t),
            y_rate: lerp(e0.y_rate, e1.y_rate, t),
            lod: lerp(e0.lod, e1.lod, t),
        })
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}

/// URL for the official IERS EOP C04 series (ITRF 2020, daily).
pub const IERS_EOP_C04_URL: &str = "https://hpiers.obspm.fr/iers/eop/eopc04/eopc04.1962-now";

/// Local cache directory name under the user's data directory.
const EOP_CACHE_DIR: &str = "apogee/eop";
const EOP_CACHE_FILE: &str = "eopc04.1962-now";

/// Loader that fetches and caches the IERS EOP C04 series.
#[derive(Debug, Default, Clone)]
pub struct EopLoader {
    cache_dir: Option<PathBuf>,
}

impl EopLoader {
    /// Create a loader with the default cache directory (`~/.local/share/apogee/eop`).
    pub fn new() -> Self {
        Self {
            cache_dir: Self::cache_dir(),
        }
    }

    /// Create a loader with a custom cache directory.
    pub fn with_cache_dir<P: AsRef<Path>>(path: P) -> Self {
        Self {
            cache_dir: Some(path.as_ref().to_path_buf()),
        }
    }

    fn cache_dir() -> Option<PathBuf> {
        // Use XDG_DATA_HOME / $HOME/.local/share/apogee/eop.
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })
            .map(|base| base.join(EOP_CACHE_DIR))
    }

    /// Return the path to the cached EOP file, if a cache directory is set.
    pub fn cache_path(&self) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|d| d.join(EOP_CACHE_FILE))
    }

    /// Load EOP data, fetching from the IERS website if the cache is missing
    /// or older than `max_age`. Falls back to `fallback` if the network is
    /// unavailable and no cache exists.
    ///
    /// `max_age` is in seconds. Pass `None` to always use the cache if present.
    pub fn load(
        &self,
        fallback: Option<&str>,
        max_age: Option<std::time::Duration>,
    ) -> ApogeeResult<EopData> {
        if let Some(path) = self.cache_path() {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let use_cache = match (max_age, metadata.modified()) {
                    (Some(max_age), Ok(modified)) => std::time::SystemTime::now()
                        .duration_since(modified)
                        .map(|age| age <= max_age)
                        .unwrap_or(false),
                    (None, _) => true,
                    _ => false,
                };

                if use_cache {
                    info!("loading EOP from cache: {}", path.display());
                    return EopData::load(&path);
                }
            }

            if let Ok(data) = Self::fetch() {
                if let Err(e) = self.save_cache(&data) {
                    warn!("failed to write EOP cache: {e}");
                }
                return EopData::parse(&data);
            }

            if let Ok(data) = EopData::load(&path) {
                warn!("EOP fetch failed; using stale cache");
                return Ok(data);
            }
        }

        if let Some(text) = fallback {
            warn!("EOP fetch failed and no cache; using fallback");
            return EopData::parse(text);
        }

        Err(ApogeeError::Data(
            "EOP data unavailable: fetch failed and no fallback provided".to_string(),
        ))
    }

    /// Synchronous fetch of the IERS EOP C04 file over HTTPS.
    pub fn fetch() -> ApogeeResult<String> {
        info!("fetching EOP from {}", IERS_EOP_C04_URL);
        let response = reqwest::blocking::get(IERS_EOP_C04_URL)
            .map_err(|e| ApogeeError::Network(format!("EOP fetch failed: {e}")))?;
        if !response.status().is_success() {
            return Err(ApogeeError::Network(format!(
                "EOP fetch returned HTTP {}",
                response.status()
            )));
        }
        response
            .text()
            .map_err(|e| ApogeeError::Network(format!("failed to read EOP response: {e}")))
    }

    fn save_cache(&self, text: &str) -> std::io::Result<()> {
        if let Some(dir) = &self.cache_dir {
            std::fs::create_dir_all(dir)?;
            let path = dir.join(EOP_CACHE_FILE);
            std::fs::write(&path, text.as_bytes())?;
            info!("EOP cache saved to {}", path.display());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_eop() -> &'static str {
        "# EARTH ORIENTATION PARAMETER (EOP) PRODUCT CENTER CENTER (PARIS OBSERVATORY)\n\
         # EOP (IERS) 20 C04 TIME SERIES\n\
         1962   1   1   0  37665.00   -0.012700    0.213000   0.0326338    0.000000    0.000000    0.000000    0.000000   0.0017230    0.030000    0.030000   0.0020000    0.004774    0.002000    0.000000    0.000000   0.0014000\n\
         1962   1   2   0  37666.00   -0.015900    0.214100   0.0320547    0.000000    0.000000    0.000000    0.000000   0.0016690    0.030000    0.030000   0.0020000    0.004774    0.002000    0.000000    0.000000   0.0014000"
    }

    #[test]
    fn test_parse_skips_comments_and_blank_lines() {
        let data = EopData::parse(sample_eop()).unwrap();
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_parse_fields() {
        let data = EopData::parse(sample_eop()).unwrap();
        let e = &data.entries()[0];
        assert_eq!(e.year, 1962);
        assert_eq!(e.month, 1);
        assert_eq!(e.day, 1);
        assert_eq!(e.hour, 0);
        assert!((e.mjd - 37665.0).abs() < 1e-9);
        assert!((e.x_pole - -0.0127).abs() < 1e-9);
        assert!((e.y_pole - 0.213).abs() < 1e-9);
        assert!((e.ut1_utc - 0.0326338).abs() < 1e-9);
        assert!((e.dx).abs() < 1e-9);
        assert!((e.dy).abs() < 1e-9);
        assert!((e.x_rate).abs() < 1e-9);
        assert!((e.y_rate).abs() < 1e-9);
        assert!((e.lod - 0.001723).abs() < 1e-9);
    }

    #[test]
    fn test_lookup_interpolates() {
        let data = EopData::parse(sample_eop()).unwrap();
        let e = data.at_mjd(37665.5).unwrap();
        assert!((e.x_pole - (-0.0143)).abs() < 1e-9);
        assert!((e.y_pole - 0.21355).abs() < 1e-9);
    }
}
