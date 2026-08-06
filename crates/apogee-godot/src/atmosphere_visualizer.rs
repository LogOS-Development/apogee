//! Godot node that samples atmosphere models and wind, and exposes the data as
//! arrays for 3D visualization.
//!
//! This node does not perform any rendering itself; it provides data that a
//! Godot `MeshInstance3D` or `MultiMeshInstance3D` can consume.

use std::f64::consts::PI;

use apogee_common::units::Meters;
use apogee_core::aero::{
    jacchia_bowman::JacchiaBowman, model::AtmosphereInput, nrlmsise00::Nrlmsise00,
};
use godot::classes::Node3D;
use godot::prelude::*;

/// Available atmosphere models for side-by-side visualization.
///
/// Stored as a `u32` Godot property; 0 = NRLMSISE-00, 1 = Jacchia-Bowman.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum AtmosphereModelKind {
    #[default]
    Nrlmsise00 = 0,
    JacchiaBowman = 1,
}

impl From<u32> for AtmosphereModelKind {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::JacchiaBowman,
            _ => Self::Nrlmsise00,
        }
    }
}

impl From<AtmosphereModelKind> for u32 {
    fn from(value: AtmosphereModelKind) -> u32 {
        value as u32
    }
}

/// A single sampled point in the atmosphere/wind field.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AtmosphereSample {
    pub altitude_km: f64,
    pub latitude_rad: f64,
    pub longitude_rad: f64,
    pub density_kg_m3: f64,
    pub temperature_k: f64,
    pub east_mps: f64,
    pub north_mps: f64,
    pub up_mps: f64,
}

/// Godot node that samples density, temperature, and wind on a regular
/// latitude/longitude/altitude grid.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct AtmosphereGridSampler {
    base: Base<Node3D>,

    /// Number of latitude divisions (inclusive).
    #[var]
    lat_steps: i32,
    /// Number of longitude divisions (inclusive).
    #[var]
    lon_steps: i32,
    /// Number of altitude divisions (inclusive).
    #[var]
    alt_steps: i32,

    /// Minimum altitude in kilometres.
    #[var]
    altitude_min_km: f64,
    /// Maximum altitude in kilometres.
    #[var]
    altitude_max_km: f64,

    /// Day of year (1..=366).
    #[var]
    day_of_year: i32,
    /// UTC seconds since midnight.
    #[var]
    seconds_utc: f64,
    /// Daily F10.7 solar flux.
    #[var]
    f107: f64,
    /// 81-day centred F10.7 (used by NRLMSISE-00).
    #[var]
    f107a: f64,
    /// Daily Ap geomagnetic index.
    #[var]
    ap: f64,

    /// Active atmosphere model: 0 = NRLMSISE-00, 1 = Jacchia-Bowman.
    #[var]
    model_kind: u32,

    /// Latest sampled grid, flat in lat-major order.
    samples: Vec<AtmosphereSample>,
}

#[godot_api]
impl INode3D for AtmosphereGridSampler {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            lat_steps: 9,
            lon_steps: 18,
            alt_steps: 10,
            altitude_min_km: 100.0,
            altitude_max_km: 500.0,
            day_of_year: 80,
            seconds_utc: 12.0 * 3600.0,
            f107: 150.0,
            f107a: 150.0,
            ap: 4.0,
            model_kind: 0,
            samples: Vec::new(),
        }
    }
}

#[godot_api]
impl AtmosphereGridSampler {
    /// Resample the atmosphere/wind grid with the current parameters.
    /// Call this from GDScript after changing parameters or on a timer to animate.
    #[func]
    fn resample(&mut self) {
        let kind = AtmosphereModelKind::from(self.model_kind);
        let lat_min = -PI / 2.0;
        let lat_max = PI / 2.0;
        let lon_min = -PI;
        let lon_max = PI;

        let capacity =
            (self.lat_steps as usize) * (self.lon_steps as usize) * (self.alt_steps as usize);
        let mut samples = Vec::with_capacity(capacity);

        for i_lat in 0..self.lat_steps {
            let t_lat = if self.lat_steps > 1 {
                i_lat as f64 / (self.lat_steps - 1) as f64
            } else {
                0.5
            };
            let lat_rad = lat_min + t_lat * (lat_max - lat_min);

            for i_lon in 0..self.lon_steps {
                let t_lon = if self.lon_steps > 1 {
                    i_lon as f64 / (self.lon_steps - 1) as f64
                } else {
                    0.5
                };
                let lon_rad = lon_min + t_lon * (lon_max - lon_min);

                for i_alt in 0..self.alt_steps {
                    let t_alt = if self.alt_steps > 1 {
                        i_alt as f64 / (self.alt_steps - 1) as f64
                    } else {
                        0.5
                    };
                    let alt_km = self.altitude_min_km
                        + t_alt * (self.altitude_max_km - self.altitude_min_km);

                    let sample = sample_point(
                        alt_km,
                        lat_rad,
                        lon_rad,
                        self.day_of_year,
                        self.seconds_utc,
                        self.f107,
                        self.f107a,
                        self.ap,
                        kind,
                    );
                    samples.push(sample);
                }
            }
        }

        self.samples = samples;
    }

    /// Return the sampled grid as a PackedFloat64Array in flat lat-major order.
    /// Each sample occupies 8 floats: alt, lat, lon, rho, T, wind_e, wind_n, wind_u.
    #[func]
    fn get_samples(&self) -> PackedFloat64Array {
        let mut out = PackedFloat64Array::new();
        out.resize(self.samples.len() * 8);
        let slice = out.as_mut_slice();
        for (i, sample) in self.samples.iter().enumerate() {
            let base = i * 8;
            slice[base] = sample.altitude_km;
            slice[base + 1] = sample.latitude_rad;
            slice[base + 2] = sample.longitude_rad;
            slice[base + 3] = sample.density_kg_m3;
            slice[base + 4] = sample.temperature_k;
            slice[base + 5] = sample.east_mps;
            slice[base + 6] = sample.north_mps;
            slice[base + 7] = sample.up_mps;
        }
        out
    }

    /// Convenience helper: get the maximum density in the current grid.
    #[func]
    fn max_density(&self) -> f64 {
        self.samples
            .iter()
            .map(|s| s.density_kg_m3)
            .fold(0.0, f64::max)
    }

    /// Convenience helper: get the maximum wind speed in the current grid.
    #[func]
    fn max_wind_speed(&self) -> f64 {
        self.samples
            .iter()
            .map(|s| {
                (s.east_mps * s.east_mps + s.north_mps * s.north_mps + s.up_mps * s.up_mps).sqrt()
            })
            .fold(0.0, f64::max)
    }
}

#[allow(clippy::too_many_arguments)]
fn sample_point(
    alt_km: f64,
    lat_rad: f64,
    lon_rad: f64,
    day_of_year: i32,
    seconds_utc: f64,
    f107: f64,
    f107a: f64,
    ap: f64,
    model_kind: AtmosphereModelKind,
) -> AtmosphereSample {
    let altitude_m = alt_km * 1000.0;
    let doy = day_of_year.clamp(1, 366) as u16;

    let input = AtmosphereInput {
        altitude_m: Meters::new(altitude_m),
        latitude_rad: lat_rad,
        longitude_rad: lon_rad,
        day_of_year: doy,
        seconds_utc,
        f107,
        f107a,
        ap,
    };

    let (density, temperature) = match model_kind {
        AtmosphereModelKind::Nrlmsise00 => {
            let out = Nrlmsise00::evaluate_simple(&input);
            (out.density.into_value(), out.temperature.into_value())
        }
        AtmosphereModelKind::JacchiaBowman => {
            let out = JacchiaBowman::evaluate_approx(&input);
            (out.density.into_value(), out.temperature.into_value())
        }
    };

    // Wind: real HWM14 when the `hwm14` feature is enabled, otherwise the
    // placeholder zero-wind model. The default GDExtension build deliberately
    // does not depend on gfortran; enable the feature for real wind vectors.
    #[cfg(feature = "hwm14")]
    let wind = {
        use apogee_core::aero::hwm::Hwm14;
        let wind_input = apogee_core::aero::WindInput {
            altitude_m,
            latitude_rad: lat_rad,
            longitude_rad: lon_rad,
            local_solar_time_hours: (seconds_utc / 3600.0 + lon_rad.to_degrees() / 15.0)
                .rem_euclid(24.0),
            day_of_year: doy,
            f107,
            ap,
        };
        Hwm14::evaluate(&wind_input)
    };

    #[cfg(not(feature = "hwm14"))]
    let wind = apogee_core::aero::WindOutput::default();

    AtmosphereSample {
        altitude_km: alt_km,
        latitude_rad: lat_rad,
        longitude_rad: lon_rad,
        density_kg_m3: density,
        temperature_k: temperature,
        east_mps: wind.east_mps,
        north_mps: wind.north_mps,
        up_mps: wind.up_mps,
    }
}
