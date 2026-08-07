//! Slotmap-style generational arena for ECS entity storage.

use super::entity::Entity;

/// A single arena slot.
///
/// When `occupied` is `false` the slot is free and `generation` is the
/// generation the *next* occupant will have (incremented on despawn).
#[derive(Debug, Clone)]
struct Slot<T> {
    /// `Some(bundle)` when the slot is live, `None` when free.
    bundle: Option<T>,
    /// Generation counter. Incremented every time the slot is freed so
    /// stale handles do not alias new occupants.
    generation: u32,
}

/// Generational arena mapping `Entity` handles to component bundles.
///
/// This is a dense slotmap: each slot stores an `Option<T>` plus a
/// generation counter. Despawning increments the generation and clears the
/// slot, so any outstanding `Entity` handle from before the despawn
/// resolves to `None` even after the slot is reused.
///
/// The arena is `Send` when `T: Send`.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    /// Indices of free slots, reused on spawn to avoid unbounded growth.
    free_list: Vec<usize>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
        }
    }
}

impl<T> std::fmt::Debug for Arena<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Arena")
            .field("len", &self.len())
            .field("capacity", &self.slots.len())
            .finish()
    }
}

impl<T> Arena<T> {
    /// Create an empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an arena with the given capacity pre-allocated.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free_list: Vec::new(),
        }
    }

    /// Number of live entities.
    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.bundle.is_some()).count()
    }

    /// Is the arena empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a value, returning the `Entity` handle that references it.
    pub fn insert(&mut self, value: T) -> Entity {
        if let Some(slot_idx) = self.free_list.pop() {
            let slot = &mut self.slots[slot_idx];
            debug_assert!(slot.bundle.is_none());
            slot.bundle = Some(value);
            Entity::pack(slot_idx, slot.generation)
        } else {
            let slot_idx = self.slots.len();
            self.slots.push(Slot {
                bundle: Some(value),
                generation: 0,
            });
            Entity::pack(slot_idx, 0)
        }
    }

    /// Remove the entity, returning its bundle if the handle was valid.
    ///
    /// Invalid or stale handles return `None` and do not modify the arena.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let slot = self.slots.get_mut(entity.slot())?;
        if slot.generation != entity.generation() {
            return None;
        }
        let bundle = slot.bundle.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free_list.push(entity.slot());
        Some(bundle)
    }

    /// Get an immutable reference to the bundle, or `None` if the handle is
    /// stale or the slot is empty.
    pub fn get(&self, entity: Entity) -> Option<&T> {
        let slot = self.slots.get(entity.slot())?;
        if slot.generation != entity.generation() {
            return None;
        }
        slot.bundle.as_ref()
    }

    /// Get a mutable reference to the bundle, or `None` if the handle is
    /// stale or the slot is empty.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let slot = self.slots.get_mut(entity.slot())?;
        if slot.generation != entity.generation() {
            return None;
        }
        slot.bundle.as_mut()
    }

    /// Iterate over all live entity handles.
    ///
    /// The iterator yields handles in slot-index order. It is NOT stable
    /// across insertions and removals — collect the handles if you need a
    /// snapshot.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            slot.bundle.as_ref()?;
            Some(Entity::pack(idx, slot.generation))
        })
    }

    /// Iterate over immutable references to all live bundles (in slot order).
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(idx, slot)| {
            let bundle = slot.bundle.as_ref()?;
            Some((Entity::pack(idx, slot.generation), bundle))
        })
    }

    /// Iterate over mutable references to all live bundles (in slot order).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> + '_ {
        self.slots.iter_mut().enumerate().filter_map(|(idx, slot)| {
            let bundle = slot.bundle.as_mut()?;
            Some((Entity::pack(idx, slot.generation), bundle))
        })
    }

    /// Clear all entities, incrementing every occupied slot's generation.
    pub fn clear(&mut self) {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            if slot.bundle.is_some() {
                slot.generation = slot.generation.wrapping_add(1);
            }
            slot.bundle = None;
            self.free_list.push(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get() {
        let mut arena = Arena::new();
        let e = arena.insert(42u32);
        assert_eq!(arena.get(e), Some(&42));
        assert_eq!(arena.len(), 1);
        assert!(!arena.is_empty());
    }

    #[test]
    fn insert_remove_get() {
        let mut arena = Arena::new();
        let e = arena.insert(42u32);
        assert_eq!(arena.remove(e), Some(42));
        assert_eq!(arena.get(e), None);
        assert_eq!(arena.len(), 0);
        assert!(arena.is_empty());
    }

    #[test]
    fn stale_handle_after_remove_returns_none() {
        let mut arena = Arena::new();
        let e0 = arena.insert(100u32);
        assert_eq!(arena.remove(e0), Some(100));

        // Re-spawn into the same slot — generation should differ.
        let e1 = arena.insert(200u32);
        assert_eq!(arena.get(e0), None); // stale
        assert_eq!(arena.get(e1), Some(&200));
        assert_ne!(e0.generation(), e1.generation());
    }

    #[test]
    fn get_mut() {
        let mut arena = Arena::new();
        let e = arena.insert(10u32);
        *arena.get_mut(e).unwrap() = 20;
        assert_eq!(arena.get(e), Some(&20));
    }

    #[test]
    fn entities_iterator() {
        let mut arena = Arena::new();
        let e0 = arena.insert(1u32);
        let e1 = arena.insert(2u32);
        let e2 = arena.insert(3u32);
        arena.remove(e1);

        let collected: Vec<_> = arena.entities().collect();
        assert_eq!(collected, vec![e0, e2]);
    }

    #[test]
    fn iter_iter_mut() {
        let mut arena = Arena::new();
        let e0 = arena.insert(1u32);
        let e1 = arena.insert(2u32);

        let pairs: Vec<_> = arena.iter().map(|(e, v)| (e, *v)).collect();
        assert_eq!(pairs, vec![(e0, 1), (e1, 2)]);

        for (_, v) in arena.iter_mut() {
            *v *= 10;
        }
        assert_eq!(arena.get(e0), Some(&10));
        assert_eq!(arena.get(e1), Some(&20));
    }

    #[test]
    fn free_slot_reuse() {
        let mut arena = Arena::new();
        let e0 = arena.insert(1u32);
        let _e1 = arena.insert(2u32);
        arena.remove(e0);

        // Reuse e0's slot — generation should have incremented.
        let e2 = arena.insert(3u32);
        assert_eq!(e2.slot(), e0.slot());
        assert_ne!(e2.generation(), e0.generation());
        assert_eq!(arena.get(e0), None);
        assert_eq!(arena.get(e2), Some(&3));
    }

    #[test]
    fn clear() {
        let mut arena = Arena::new();
        let e0 = arena.insert(1u32);
        let e1 = arena.insert(2u32);
        arena.clear();
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.get(e0), None);
        assert_eq!(arena.get(e1), None);
    }

    #[test]
    fn with_capacity() {
        let arena: Arena<u32> = Arena::with_capacity(100);
        assert_eq!(arena.len(), 0);
    }

    #[test]
    fn remove_invalid_handle() {
        let mut arena = Arena::new();
        let _e = arena.insert(1u32);
        let bad = Entity::pack(999, 0);
        assert_eq!(arena.remove(bad), None);
    }
}
