//! Atmospheric models: NRLMSISE-00, Jacchia-Bowman, HWM winds, drag, SRP, WRF physics.

pub mod atmospheric_drag;
pub mod hwm;
pub mod jacchia_bowman;
pub mod model;
pub mod nrlmsise00;

/// Vendored NRLMSISE-00 implementation (adapted from the Brahe project).
#[path = "../../external/nrlmsise00_brahe/mod.rs"]
pub mod nrlmsise00_brahe;

pub mod solar_radiation_pressure;
pub mod space_weather;

/// WRF physics schemes via FFI (requires `wrf` feature + gfortran).
#[cfg(feature = "wrf")]
pub mod wrf;

pub use atmospheric_drag::*;
pub use hwm::*;
pub use jacchia_bowman::*;
pub use model::*;
pub use nrlmsise00::*;
pub use solar_radiation_pressure::*;
pub use space_weather::*;
