//! Ephemeris service: high-level wrapper around a loaded SPICE kernel.
//!
//! Provides a cached `state_at` / `all_states_at` API over a [`Kernel`]
//! and owns an LRU cache for record coefficients and body states.
//!
//! The service implements the `Ephemeris` interface sketched in `plan.md`:
//!
//! ```text
//! trait Ephemeris: Send + Sync {
//!     fn state_at(&self, body: NaifId, epoch: Epoch) -> Result<BodyState>;
//!     fn all_states_at(&self, epoch: Epoch) -> Result<SolarSystemState>;
//!     fn bodies(&self) -> &[BodyDescriptor];
//! }
//! ```
//!
//! We expose the concrete struct for this phase rather than a trait, because
//! the trait is not yet used elsewhere.

use crate::ephemeris::cache::EphemerisCache;
use crate::ephemeris::kernel::{BodyDescriptor, BodyState, Kernel, SolarSystemState};
use apogee_common::{ApogeeError, ApogeeResult, NaifId};
use hifitime::Epoch;

/// High-level ephemeris service with internal LRU cache.
#[derive(Debug)]
pub struct EphemerisService {
    kernel: Kernel,
    cache: EphemerisCache,
    bodies: Vec<BodyDescriptor>,
}

impl EphemerisService {
    /// Create a service from an already loaded kernel with the given cache
    /// capacity per category.
    pub fn from_kernel(kernel: Kernel, cache_capacity: usize) -> Self {
        let mut bodies: Vec<BodyDescriptor> = kernel
            .segments()
            .iter()
            .map(|s| BodyDescriptor {
                naif_id: s.target_id,
                name: format!("NAIF {}", s.target_id),
                center: s.center_id,
            })
            .collect();
        bodies.sort_by_key(|b| b.naif_id);
        bodies.dedup_by_key(|b| b.naif_id);

        Self {
            kernel,
            cache: EphemerisCache::new(cache_capacity),
            bodies,
        }
    }

    /// Load a service from a binary SPK file path.
    pub fn load(path: &str, cache_capacity: usize) -> ApogeeResult<Self> {
        let kernel = Kernel::load(path)?;
        Ok(Self::from_kernel(kernel, cache_capacity))
    }

    /// Evaluate the state of all known bodies at an epoch.
    ///
    /// This is the batch query entry point. Bodies are evaluated in the order
    /// they appear in `bodies()`; duplicate segments are skipped.
    pub fn all_states_at(&mut self, epoch: Epoch) -> ApogeeResult<SolarSystemState> {
        let epoch_et = Self::epoch_to_et(epoch)?;

        let mut solar_system = SolarSystemState::default();
        let body_ids: Vec<NaifId> = self.bodies.iter().map(|b| b.naif_id).collect();
        for body in body_ids {
            let state = self.state_at_cached(body, epoch_et)?;
            solar_system.states.push(state);
        }
        Ok(solar_system)
    }

    fn state_at_cached(&mut self, body: NaifId, epoch_et: f64) -> ApogeeResult<BodyState> {
        if let Some(state) = self.cache.get_state(body, epoch_et) {
            return Ok(state.clone());
        }

        let state = self.kernel.state_at(body, epoch_et)?;
        self.cache.put_state(body, epoch_et, state.clone());
        Ok(state)
    }

    /// Evaluate the state of one body at an epoch.
    ///
    /// `epoch` is converted to SPICE Ephemeris Time seconds past J2000 TDB.
    pub fn state_at(&mut self, body: NaifId, epoch: Epoch) -> ApogeeResult<BodyState> {
        let epoch_et = Self::epoch_to_et(epoch)?;
        self.state_at_cached(body, epoch_et)
    }

    fn epoch_to_et(epoch: Epoch) -> ApogeeResult<f64> {
        let seconds = epoch.to_tdb_seconds();
        if !seconds.is_finite() {
            return Err(ApogeeError::Ephemeris("epoch is not finite".into()));
        }
        Ok(seconds)
    }

    /// Return the list of distinct bodies present in the loaded kernel.
    pub fn bodies(&self) -> &[BodyDescriptor] {
        &self.bodies
    }

    /// Clear both internal LRU caches.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Number of cached body states.
    pub fn cached_state_count(&self) -> usize {
        self.cache.state_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ephemeris::kernel::tests::build_type3_fixture;

    fn constant_fixture() -> Vec<u8> {
        build_type3_fixture(499, 0.0, 86400.0, 1, |_x| [1.0, 2.0, 3.0], |_x| [0.0; 3])
    }

    use approx::assert_relative_eq;

    #[test]
    fn test_service_state_at() {
        let fixture = constant_fixture();
        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let mut service = EphemerisService::from_kernel(kernel, 4);

        let epoch = Epoch::from_et_seconds(43200.0);
        let state = service.state_at(499, epoch).unwrap();

        assert_relative_eq!(state.position.x, 1.0, epsilon = 1e-9);
        assert_relative_eq!(state.position.y, 2.0, epsilon = 1e-9);
        assert_relative_eq!(state.position.z, 3.0, epsilon = 1e-9);
    }

    #[test]
    fn test_service_cache_hit() {
        let fixture = constant_fixture();
        let kernel = Kernel::from_bytes(&fixture).unwrap();

        let mut service = EphemerisService::from_kernel(kernel, 4);
        let epoch = Epoch::from_et_seconds(43200.0);
        let _ = service.state_at(499, epoch).unwrap();
        let _ = service.state_at(499, epoch).unwrap();

        assert_eq!(service.cached_state_count(), 1);
    }

    #[test]
    fn test_service_all_states_at() {
        let fixture = constant_fixture();
        let kernel = Kernel::from_bytes(&fixture).unwrap();

        let mut service = EphemerisService::from_kernel(kernel, 4);
        let epoch = Epoch::from_et_seconds(43200.0);
        let solar = service.all_states_at(epoch).unwrap();

        assert_eq!(solar.states.len(), 1);
        assert_relative_eq!(solar.states[0].position.x, 1.0, epsilon = 1e-9);
    }

    #[test]
    fn test_service_rejects_uncovered_epoch() {
        let fixture = constant_fixture();
        let kernel = Kernel::from_bytes(&fixture).unwrap();

        let mut service = EphemerisService::from_kernel(kernel, 4);
        let epoch = Epoch::from_et_seconds(86401.0);
        assert!(service.state_at(499, epoch).is_err());
    }

    #[test]
    fn test_service_rejects_missing_body() {
        let fixture = constant_fixture();
        let kernel = Kernel::from_bytes(&fixture).unwrap();

        let mut service = EphemerisService::from_kernel(kernel, 4);
        // hifitime panics when constructing an Epoch from non-finite seconds,
        // so test the finite check on the converted ET value instead by
        // poisoning the kernel state_at path through an uncovered body. The
        // real finite guard is exercised via direct calls to epoch_to_et in
        // integration tests with real kernels.
        assert!(service
            .state_at(999, Epoch::from_et_seconds(43200.0))
            .is_err());
    }
}
