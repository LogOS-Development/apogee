//! SPICE binary kernel (.bsp) loader — stub.

use apogee_common::{ApogeeError, ApogeeResult, NaifId};
use hifitime::Epoch;

/// Loaded ephemeris kernel.
#[derive(Debug)]
pub struct Kernel {
    // TODO: segment records, Chebyshev coefficients
}

/// Body state from ephemeris.
#[derive(Debug, Clone)]
pub struct BodyState {
    pub position: apogee_common::Position,
    pub velocity: apogee_common::Velocity,
}

/// Solar system state: all bodies at a single epoch.
#[derive(Debug, Clone, Default)]
pub struct SolarSystemState {
    pub states: Vec<BodyState>,
}

/// Descriptor for a body in the ephemeris.
#[derive(Debug, Clone)]
pub struct BodyDescriptor {
    pub naif_id: NaifId,
    pub name: String,
    pub center: NaifId,
}

/// Core ephemeris trait.
pub trait Ephemeris: Send + Sync {
    fn state_at(&self, body: NaifId, epoch: Epoch) -> ApogeeResult<BodyState>;
    fn all_states_at(&self, epoch: Epoch) -> ApogeeResult<SolarSystemState>;
    fn bodies(&self) -> &[BodyDescriptor];
}

impl Kernel {
    /// Load a binary SPK (.bsp) kernel from file.
    pub fn load(_path: &str) -> ApogeeResult<Self> {
        // TODO: parse DAF/SPK format
        Err(ApogeeError::Ephemeris(
            "kernel loading not yet implemented".into(),
        ))
    }
}
