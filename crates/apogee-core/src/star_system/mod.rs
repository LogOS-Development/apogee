//! Star-system construction and simulation.
//!
//! A star system is a self-contained gravitational system: a star, its
//! planets, and minor bodies (asteroids, comets, dwarf planets).
//!
//! Three sources of truth build one:
//!
//! - [`definition::SystemDefinition`]: config-driven descriptions —
//!   presets, JSON files, seeded random generation.
//! - [`EphemerisService`]: JPL SPICE kernels (SPK segments) for major-body
//!   states.
//! - [`StarSystem`]: the live simulation manager binding a definition, an
//!   optional ephemeris service, and the ECS [`World`] together.
//!
//! ## Body classes
//!
//! - **Star / Planet / Moon** — kinematic when an ephemeris is attached:
//!   `step` queries their states from SPICE and writes them into the
//!   `World`. Without an ephemeris they stay at their configured states.
//! - **Minor / Asteroid / AsteroidCluster** — dynamic: integrated by
//!   `step_world` under full N-body gravity from all `GravitySource`s,
//!   with self-gravity excluded per body.
//! - **AsteroidCluster** — one entity carrying the aggregate GM of the
//!   whole cluster (the cluster's gravitational influence on everything
//!   else) plus a member table for high-fidelity promotion of individual
//!   rocks. See [`AsteroidCluster`].
//!
//! All state is SI: positions in meters, velocities in m/s, GM in m³/s².

pub mod definition;

pub use definition::{
    presets, AsteroidClusterSpec, BodyDefinition, BodyRole, ClusterMemberSpec, GravityConfig,
    SystemDefinition,
};

use crate::ephemeris::EphemerisService;
use crate::world::World;
use apogee_common::units::{GravitationalParameter, Kilograms, Seconds};
use apogee_common::ApogeeResult;
use hifitime::Epoch;

/// A member of an [`AsteroidCluster`]: an individual rock with its own
/// state, recorded for later promotion to a full N-body entity.
#[derive(Debug, Clone, PartialEq)]
pub struct ClusterMember {
    /// Human-readable identifier, e.g. "belt-1-rock-17".
    pub name: String,
    /// Position relative to the cluster barycenter (meters).
    pub offset: nalgebra::Vector3<f64>,
    /// Velocity relative to the cluster barycenter (m/s).
    pub velocity_offset: nalgebra::Vector3<f64>,
    /// GM of the individual rock (m³/s²).
    pub gm: GravitationalParameter<f64>,
}

/// ECS component: asteroid cluster membership and aggregate data.
///
/// A cluster is ONE entity in the ECS with the aggregate GM of all its
/// members — that is the gravitational field everything else feels. The
/// member table records individual rocks so a member can later be promoted
/// to its own full N-body entity (the cluster's aggregate GM is reduced by
/// the promoted member's GM).
#[derive(Debug, Clone, PartialEq)]
pub struct AsteroidCluster {
    /// Individual rocks, positions relative to the cluster entity.
    pub members: Vec<ClusterMember>,
}

impl AsteroidCluster {
    /// Total GM of all members (m³/s²).
    pub fn aggregate_gm(&self) -> GravitationalParameter<f64> {
        let total: f64 = self.members.iter().map(|m| m.gm.into_value()).sum();
        GravitationalParameter::new(total)
    }

    /// Total mass of all members (kg), derived via G.
    pub fn aggregate_mass(&self) -> Kilograms<f64> {
        Kilograms::new(self.aggregate_gm().into_value() / apogee_common::constants::G.into_value())
    }
}

/// A live star system: definition + ephemeris + ECS world.
///
/// This is the top-level object for Phase 1 simulation. `step` advances
/// the whole system one interval:
///
/// 1. Kinematic bodies (star, planets, moons) are updated from the
///    ephemeris service at the new epoch, if attached.
/// 2. `step_world` integrates dynamic bodies (asteroids, clusters,
///    spacecraft) under N-body gravity and steps all spacecraft states.
///
/// Without an ephemeris, kinematic bodies stay at their configured states
/// and only dynamic bodies move.
///
/// # Example
///
/// ```ignore
/// use apogee_core::star_system::{presets, StarSystem};
///
/// // Real solar system from a JPL kernel:
/// let system = StarSystem::builder(presets::inner_solar_system())
///     .with_ephemeris(EphemerisService::load("de440s.bsp", 32)?)
///     .build()?;
/// system.step(Seconds::new(60.0))?;
///
/// // Fictional system, no ephemeris — planets stay at configured states:
/// let system = StarSystem::builder(SystemDefinition::random(42, 5)).build()?;
/// ```
pub struct StarSystem {
    /// The immutable system description (bodies, gravity models, clusters).
    pub definition: SystemDefinition,
    /// The live ECS world.
    pub world: World,
    /// Simulation epoch, advanced by `step`.
    pub epoch: Epoch,
}

impl StarSystem {
    /// Start building a star system from a definition.
    pub fn builder(definition: SystemDefinition) -> StarSystemBuilder {
        StarSystemBuilder {
            definition,
            ephemeris: None,
        }
    }

    /// Construct from parts (builder is preferred).
    pub fn new(
        definition: SystemDefinition,
        ephemeris: Option<EphemerisService>,
        epoch: Epoch,
    ) -> ApogeeResult<Self> {
        let mut world = World::new();
        if let Some(eph) = ephemeris {
            world = world.with_ephemeris(eph);
        }
        // `add_system` handles ephemeris-driven vs dynamic roles.
        world.add_system(&definition)?;
        let mut system = Self {
            definition,
            world,
            epoch,
        };
        system.sync_kinematic_from_ephemeris()?;
        Ok(system)
    }

    /// Advance the whole system by `dt`.
    ///
    /// 1. Advance the epoch.
    /// 2. Update kinematic bodies from the ephemeris at the new epoch.
    /// 3. Integrate dynamic bodies and spacecraft via `step_world`.
    pub fn step(&mut self, dt: Seconds<f64>) -> ApogeeResult<()> {
        self.epoch += dt.into_value() * hifitime::Unit::Second;
        self.sync_kinematic_from_ephemeris()?;
        crate::systems::step::step_world(&mut self.world, dt);
        Ok(())
    }

    /// Update all ephemeris-driven kinematic bodies from the ephemeris at
    /// the current epoch. No-op without an ephemeris service.
    fn sync_kinematic_from_ephemeris(&mut self) -> ApogeeResult<()> {
        let Some(ephemeris) = self.world.ephemeris.as_mut() else {
            return Ok(());
        };
        let epoch = self.epoch;

        // Collect states first to avoid holding the borrow across the
        // world mutation.
        let mut updates = Vec::new();
        for body in &self.definition.bodies {
            let Some(naif_id) = body.naif_id else {
                continue;
            };
            if !body.role.is_kinematic() {
                continue;
            }
            let state = ephemeris.state_at(naif_id, epoch)?;
            updates.push((naif_id, state.position, state.velocity));
        }

        for (naif_id, position, velocity) in updates {
            self.world
                .update_kinematic_celestial(naif_id, position, velocity);
        }
        Ok(())
    }
}

/// Builder for [`StarSystem`].
pub struct StarSystemBuilder {
    definition: SystemDefinition,
    ephemeris: Option<EphemerisService>,
}

impl StarSystemBuilder {
    /// Attach a JPL ephemeris service for major-body states.
    pub fn with_ephemeris(mut self, ephemeris: EphemerisService) -> Self {
        self.ephemeris = Some(ephemeris);
        self
    }

    /// Build the live star system at the given epoch.
    pub fn build(self) -> ApogeeResult<StarSystem> {
        StarSystem::new(
            self.definition,
            self.ephemeris,
            Epoch::from_gregorian_utc_at_midnight(2000, 1, 1),
        )
    }

    /// Build at a specific epoch.
    pub fn build_at(self, epoch: Epoch) -> ApogeeResult<StarSystem> {
        StarSystem::new(self.definition, self.ephemeris, epoch)
    }
}
