//! Re-export of SI-unit vector quantities used by dynamics and force models.
//!
//! These wrap raw `nalgebra::Vector3<f64>` and document the standard SI units
//! used throughout the workspace: meters for position, meters per second for
//! velocity, meters per second squared for acceleration, newtons for force, and
//! newton-meters for torque. Specific output formats (AU, km/s, etc.) are
//! handled at conversion boundaries, not here.

pub use crate::units::{AccelerationVec, ForceVec, PositionVec, TorqueVec, VelocityVec};
