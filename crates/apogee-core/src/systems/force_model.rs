//! `ForceModel` trait: discrete physics force contributions as composable
//! ECS systems.
//!
//! Each force model (gravity, atmospheric drag, SRP) implements
//! [`ForceModel`] and contributes an acceleration (and optionally a torque)
//! to a body. The [`aggregate_forces`] function collects all registered
//! models into an [`AggregatedForces`] struct.
//!
//! This trait is the extension point for adding new physics effects to the
//! simulation: implement `ForceModel`, register the instance with the force
//! pipeline, and the integrator picks it up automatically. Future models
//! (J2, spherical harmonics, third-body perturbation, thrust) slot in
//! without modifying existing code.

use apogee_common::units::{AccelerationVector, TorqueVector};

use crate::components::kinematics::Kinematics;
use crate::components::rigid_body::{RigidBody, SimulationConfig, SpacecraftConfig};
use crate::ephemeris::kernel::SolarSystemState;

/// Shared context passed to every [`ForceModel`] during a single evaluation.
///
/// Contains everything a force model might need: the body's kinematic state,
/// its physical properties, spacecraft config, the celestial ephemeris, the
/// space-weather environment, and the current simulation epoch. Individual
/// force models read only the fields they need.
pub struct ForceContext<'a> {
    /// Translational + rotational state of the body being evaluated.
    pub kinematics: &'a Kinematics,
    /// Mass and inertia of the body.
    pub rigid_body: &'a RigidBody,
    /// Spacecraft-specific configuration (drag coefficients, SRP area, etc.).
    pub config: &'a SpacecraftConfig,
    /// Space-weather / environment configuration.
    pub sim_config: &'a SimulationConfig,
    /// Celestial ephemeris state (positions/velocities of all massive bodies).
    pub celestial: &'a SolarSystemState,
    /// Current simulation epoch.
    pub epoch: hifitime::Epoch,
}

/// A discrete physics force model contributing acceleration and/or torque.
///
/// Implementors are typically stateless structs (e.g. `PointMassGravity`,
/// `AtmosphericDrag`, `SolarRadiationPressure`). The trait is the
/// composition point: the force aggregator iterates all registered models
/// and sums their contributions.
pub trait ForceModel: Send + Sync {
    /// Human-readable name for diagnostics (e.g. `"point-mass gravity"`).
    fn name(&self) -> &str;

    /// Compute the acceleration contribution (m/s²) for the given context.
    fn acceleration(&self, ctx: &ForceContext) -> AccelerationVector;

    /// Compute the torque contribution (N·m). Default: zero (most force
    /// models produce no torque directly; gravity-gradient torque is a
    /// separate model).
    fn torque(&self, _ctx: &ForceContext) -> TorqueVector {
        TorqueVector::new(nalgebra::Vector3::zeros())
    }
}
