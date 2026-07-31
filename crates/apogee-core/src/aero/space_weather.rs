//! Space weather data loader: F10.7, Ap/Kp indices — stub.

/// Space weather data for a given date.
#[derive(Debug, Clone, Default)]
pub struct SpaceWeather {
    pub f107: f64,  // solar flux units (10^-22 W/m^2/Hz)
    pub f107a: f64, // 81-day smoothed
    pub ap: f64,    // geomagnetic index
    pub kp: f64,    // planetary K-index
}

/// Space weather database.
#[derive(Debug, Default)]
pub struct SpaceWeatherData {
    // TODO: date-indexed entries
}

impl SpaceWeatherData {
    pub fn load(_path: &str) -> Result<Self, std::io::Error> {
        // TODO: parse CSV/JSON
        Ok(Self::default())
    }

    pub fn at(&self, _date: hifitime::Epoch) -> SpaceWeather {
        SpaceWeather::default()
    }
}
