//! LRU cache for active ephemeris segments — stub.

/// Simple LRU cache for ephemeris segments.
/// TODO: implement with lru crate or custom.
#[derive(Debug, Default)]
pub struct EphemerisCache {
    capacity: usize,
}

impl EphemerisCache {
    pub fn new(capacity: usize) -> Self {
        Self { capacity }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
