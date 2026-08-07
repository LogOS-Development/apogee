//! Entity handle: a generational index that packs a slot and a generation
//! counter into a single 64-bit value.

use std::fmt;

/// Number of bits for the slot index.
const SLOT_BITS: u32 = 32;
/// Number of bits for the generation counter.
const GEN_BITS: u32 = 32;
/// Maximum slot index (exclusive). With 32 bits this is 2^32.
const SLOT_MAX: u64 = 1 << SLOT_BITS;

const _: () = {
    assert!(SLOT_BITS + GEN_BITS == 64);
    assert!(SLOT_BITS >= 1 && GEN_BITS >= 1);
};

/// Lightweight handle to a spawned entity.
///
/// The handle packs a 32-bit slot index and a 32-bit generation counter
/// into a single `u64`. When an entity is despawned, the slot's generation
/// is incremented, so any outstanding `Entity` handles referencing the old
/// generation will no longer resolve via `World::get` / `World::get_mut`.
///
/// `Entity` is `Copy`, `Eq`, `Hash`, and totally ordered, making it
/// suitable as a key in hash maps and as a value passed across the FFI
/// boundary (it is a plain `i64` when viewed from C / GDExtension).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Entity(u64);

impl Entity {
    /// Create an entity handle from a raw packed `u64`.
    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the raw packed `u64` value.
    #[inline]
    pub const fn to_raw(self) -> u64 {
        self.0
    }

    /// Slot index (lower 32 bits).
    #[inline]
    pub const fn slot(self) -> usize {
        (self.0 & (SLOT_MAX - 1)) as usize
    }

    /// Generation counter (upper 32 bits).
    #[inline]
    pub const fn generation(self) -> u32 {
        (self.0 >> SLOT_BITS) as u32
    }

    /// Pack a slot index and generation into an `Entity`.
    #[inline]
    pub(crate) fn pack(slot: usize, generation: u32) -> Self {
        debug_assert!(
            (slot as u64) < SLOT_MAX,
            "slot index overflow: {slot} >= {SLOT_MAX}"
        );
        Self((slot as u64) | ((generation as u64) << SLOT_BITS))
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entity(#{}:{})", self.slot(), self.generation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_roundtrip() {
        let e = Entity::pack(0, 0);
        assert_eq!(e.slot(), 0);
        assert_eq!(e.generation(), 0);
        assert_eq!(e.to_raw(), 0);

        let e = Entity::pack(42, 7);
        assert_eq!(e.slot(), 42);
        assert_eq!(e.generation(), 7);
    }

    #[test]
    fn raw_roundtrip() {
        let e = Entity::from_raw(0x0000_0000_FFFF_FFFF);
        assert_eq!(e.slot(), u32::MAX as usize);
        assert_eq!(e.generation(), 0);

        let e = Entity::from_raw(0xFFFF_FFFF_0000_0000);
        assert_eq!(e.slot(), 0);
        assert_eq!(e.generation(), u32::MAX);
    }

    #[test]
    fn display_format() {
        let e = Entity::pack(5, 3);
        assert_eq!(format!("{e}"), "Entity(#5:3)");
    }

    #[test]
    fn copy_eq_hash() {
        let a = Entity::pack(1, 2);
        let b = a; // Copy
        assert_eq!(a, b);

        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn ordering() {
        // The raw u64 packs generation in the high 32 bits, so the derived
        // Ord is generation-major: same slot compares by generation, and a
        // lower slot with a higher generation sorts after a higher slot
        // with a lower generation.
        let a = Entity::pack(0, 0);
        let b = Entity::pack(0, 1);
        let c = Entity::pack(1, 0);
        assert!(a < b); // same slot, lower generation
        assert!(a < c); // generation 0 < generation 0, slot 0 < slot 1
        assert!(c < b); // slot 1 gen 0 < slot 0 gen 1 (generation dominates)
    }
}
