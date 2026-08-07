//! Integration and validation tests, organized by module.
//!
//! Each subdirectory mirrors the crate's module structure. Cross-module
//! integration tests live in the subdirectory of the primary module they
//! exercise (e.g. trajectory propagation tests that use the integrator +
//! ephemeris + gravity live under `integrator/` or `ephemeris/`).

pub mod helpers;

mod aero;
mod ephemeris;
mod frames;
mod integrator;
mod systems;
mod tle;
