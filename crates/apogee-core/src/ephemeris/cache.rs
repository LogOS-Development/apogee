//! LRU cache for active ephemeris data.
//!
//! Caches two kinds of hot data:
//!
//! 1. **Segment records**: the Chebyshev coefficients for a specific
//!    `(segment.first_data_record, record_index)` tuple. Record coefficients
//!    are the largest repeated allocation when evaluating many bodies at the
//!    same epoch.
//! 2. **Body states**: the full `BodyState` for a `(body, epoch_et)` query.
//!    This avoids recomputing the same body for callers that request state
//!    one body at a time.
//!
//! The cache uses a simple LRU eviction policy with fixed capacity. It is
//! intentionally not thread-safe; the `EphemerisService` (or a higher-level
//! synchronizer) serializes access.

use std::collections::HashMap;

use crate::ephemeris::kernel::BodyState;
use apogee_common::NaifId;

/// Key for a cached set of Chebyshev record coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RecordKey {
    first_data_record: i32,
    record_index: i32,
}

/// Key for a cached body state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StateKey {
    body: NaifId,
    epoch_et_ns: i64,
}

/// Fixed-capacity LRU cache for ephemeris record coefficients and states.
#[derive(Debug)]
pub struct EphemerisCache {
    capacity: usize,
    order: Vec<RecordKey>,
    records: HashMap<RecordKey, [Vec<f64>; 3]>,
    state_order: Vec<StateKey>,
    states: HashMap<StateKey, BodyState>,
}

impl EphemerisCache {
    /// Create a new cache with the given maximum number of cached items of
    /// each type (record coefficients and body states).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: Vec::with_capacity(capacity),
            records: HashMap::with_capacity(capacity),
            state_order: Vec::with_capacity(capacity),
            states: HashMap::with_capacity(capacity),
        }
    }

    /// Maximum number of cached items per category.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Look up a cached record coefficient set.
    pub fn get_record(
        &mut self,
        first_data_record: i32,
        record_index: i32,
    ) -> Option<&[Vec<f64>; 3]> {
        let key = RecordKey {
            first_data_record,
            record_index,
        };
        if self.records.contains_key(&key) {
            self.touch_record(key);
            self.records.get(&key)
        } else {
            None
        }
    }

    /// Insert a record coefficient set into the cache.
    pub fn put_record(
        &mut self,
        first_data_record: i32,
        record_index: i32,
        coefficients: [Vec<f64>; 3],
    ) {
        let key = RecordKey {
            first_data_record,
            record_index,
        };
        if self.records.contains_key(&key) {
            self.records.insert(key, coefficients);
            self.touch_record(key);
        } else {
            if self.records.len() >= self.capacity {
                self.evict_oldest_record();
            }
            self.records.insert(key, coefficients);
            self.order.push(key);
        }
    }

    /// Look up a cached body state.
    pub fn get_state(&mut self, body: NaifId, epoch_et: f64) -> Option<&BodyState> {
        let key = Self::state_key(body, epoch_et);
        if self.states.contains_key(&key) {
            self.touch_state(key);
            self.states.get(&key)
        } else {
            None
        }
    }

    /// Insert a body state into the cache.
    pub fn put_state(&mut self, body: NaifId, epoch_et: f64, state: BodyState) {
        let key = Self::state_key(body, epoch_et);
        if self.states.contains_key(&key) {
            self.states.insert(key, state);
            self.touch_state(key);
        } else {
            if self.states.len() >= self.capacity {
                self.evict_oldest_state();
            }
            self.states.insert(key, state);
            self.state_order.push(key);
        }
    }

    /// Remove all cached entries.
    pub fn clear(&mut self) {
        self.order.clear();
        self.records.clear();
        self.state_order.clear();
        self.states.clear();
    }

    /// Number of cached record coefficient sets.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Number of cached body states.
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    fn state_key(body: NaifId, epoch_et: f64) -> StateKey {
        StateKey {
            body,
            epoch_et_ns: (epoch_et * 1_000_000_000.0).round() as i64,
        }
    }

    fn touch_record(&mut self, key: RecordKey) {
        if let Some(pos) = self.order.iter().position(|k| *k == key) {
            self.order.remove(pos);
            self.order.push(key);
        }
    }

    fn touch_state(&mut self, key: StateKey) {
        if let Some(pos) = self.state_order.iter().position(|k| *k == key) {
            self.state_order.remove(pos);
            self.state_order.push(key);
        }
    }

    fn evict_oldest_record(&mut self) {
        if let Some(key) = self.order.first().copied() {
            self.order.remove(0);
            self.records.remove(&key);
        }
    }

    fn evict_oldest_state(&mut self) {
        if let Some(key) = self.state_order.first().copied() {
            self.state_order.remove(0);
            self.states.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn make_state(naif_id: NaifId, x: f64, y: f64, z: f64) -> BodyState {
        BodyState {
            naif_id,
            position: nalgebra::Vector3::new(x, y, z),
            velocity: nalgebra::Vector3::new(0.0, 0.0, 0.0),
        }
    }

    fn make_coeffs(value: f64, len: usize) -> [Vec<f64>; 3] {
        [vec![value; len], vec![value; len], vec![value; len]]
    }

    #[test]
    fn test_record_cache_basic() {
        let mut cache = EphemerisCache::new(2);
        assert!(cache.get_record(10, 0).is_none());

        cache.put_record(10, 0, make_coeffs(1.0, 4));
        assert_eq!(cache.record_count(), 1);

        let rec = cache.get_record(10, 0).unwrap();
        assert_relative_eq!(rec[0][0], 1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_record_cache_eviction() {
        let mut cache = EphemerisCache::new(2);
        cache.put_record(10, 0, make_coeffs(1.0, 4));
        cache.put_record(10, 1, make_coeffs(2.0, 4));
        // Access the first entry to make it most recently used.
        let _ = cache.get_record(10, 0);
        // Insert a third entry; the least recently used (10, 1) is evicted.
        cache.put_record(10, 2, make_coeffs(3.0, 4));

        assert!(cache.get_record(10, 0).is_some());
        assert!(cache.get_record(10, 1).is_none());
        assert!(cache.get_record(10, 2).is_some());
    }

    #[test]
    fn test_state_cache_basic() {
        let mut cache = EphemerisCache::new(2);
        assert!(cache.get_state(499, 0.0).is_none());

        cache.put_state(499, 0.0, make_state(499, 1.0, 2.0, 3.0));
        let state = cache.get_state(499, 0.0).unwrap();
        assert_relative_eq!(state.position.x, 1.0, epsilon = 1e-12);
    }

    #[test]
    fn test_state_cache_eviction() {
        let mut cache = EphemerisCache::new(2);
        cache.put_state(499, 0.0, make_state(499, 1.0, 0.0, 0.0));
        cache.put_state(500, 0.0, make_state(500, 2.0, 0.0, 0.0));
        cache.put_state(501, 0.0, make_state(501, 3.0, 0.0, 0.0));

        assert!(cache.get_state(499, 0.0).is_none());
        assert!(cache.get_state(500, 0.0).is_some());
        assert!(cache.get_state(501, 0.0).is_some());
    }

    #[test]
    fn test_state_cache_nanoseconds_rounding() {
        let mut cache = EphemerisCache::new(2);
        cache.put_state(499, 0.0, make_state(499, 7.0, 0.0, 0.0));
        let state = cache.get_state(499, 1e-10).unwrap();
        assert_relative_eq!(state.position.x, 7.0, epsilon = 1e-12);
    }
}
