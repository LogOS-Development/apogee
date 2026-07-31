//! Earth Orientation Parameters loader — stub.

/// Polar motion and UT1-UTC offset.
#[derive(Debug, Default, Clone)]
pub struct EopEntry {
    pub x_pole: f64,  // arcseconds
    pub y_pole: f64,  // arcseconds
    pub ut1_utc: f64, // seconds
}

/// EOP C04 series.
#[derive(Debug, Default)]
pub struct EopData {
    entries: Vec<EopEntry>,
}

impl EopData {
    pub fn load(_path: &str) -> Result<Self, std::io::Error> {
        // TODO: parse IERS EOP C04
        Ok(Self::default())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
