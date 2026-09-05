//! Verdant — community-scale forest resilience planning tool.
//!
//! Built on Apogee's physics engine: 3DEP terrain ingestion,
//! WRF-SFIRE fire spread, and risk prioritization output.
//! The commercial application of Apogee's surface simulation stack.

pub mod terrain;

pub use terrain::{ElevationGrid, GeoTransform};
