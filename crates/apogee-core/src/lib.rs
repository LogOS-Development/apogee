//! Apogee simulation core — headless physics engine.
//!
//! No I/O, no rendering. Depends only on apogee-common and math crates.

pub mod aero;
pub mod components;
pub mod ephemeris;
pub mod frames;
pub mod gravity;
pub mod integrator;
pub mod systems;
