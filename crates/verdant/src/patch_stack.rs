//! PatchStack — nested simulation patches centered on the player.
//!
//! Generates a stack of patches at increasing size and decreasing
//! resolution with distance from the player. Inner patches are small
//! and fine-grained (individual plants, daily timestep); outer patches
//! are large and coarse (aggregated biome, yearly timestep).
//!
//! When the player moves, all patches recenter. The stack is configured
//! once at startup with a list of patch specs; the PatchStack generates
//! and positions patches from those specs.

use crate::member::WorldPos;
use crate::patch::{FidelityLevel, PatchBounds, SimulationPatch};
use serde::{Deserialize, Serialize};

/// Configuration for one layer of the patch stack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchSpec {
    /// Patch side length in meters.
    pub size: f32,
    /// Cell size in meters.
    pub dx: f32,
    /// Timestep in years (for succession).
    pub dt: f32,
    /// Fidelity level for this layer.
    pub fidelity: FidelityLevelSerde,
}

/// Serde-compatible wrapper for FidelityLevel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FidelityLevelSerde {
    Individual,
    Density,
    Aggregate,
}

impl From<FidelityLevelSerde> for FidelityLevel {
    fn from(f: FidelityLevelSerde) -> Self {
        match f {
            FidelityLevelSerde::Individual => FidelityLevel::Individual,
            FidelityLevelSerde::Density => FidelityLevel::Density,
            FidelityLevelSerde::Aggregate => FidelityLevel::Aggregate,
        }
    }
}

/// A stack of nested simulation patches centered on the player.
pub struct PatchStack {
    /// Configuration for each layer, innermost first.
    specs: Vec<PatchSpec>,
    /// Generated patches, innermost first.
    pub patches: Vec<SimulationPatch>,
    /// Current player position.
    pub player_pos: WorldPos,
}

impl PatchStack {
    /// Create a new patch stack from specs, centered at the origin.
    pub fn new(specs: Vec<PatchSpec>) -> Self {
        let player_pos = WorldPos::new(0.0, 0.0);
        let patches = Self::generate_patches(&specs, &player_pos);
        Self {
            specs,
            patches,
            player_pos,
        }
    }

    /// Create a new patch stack centered at a specific position.
    pub fn at_position(specs: Vec<PatchSpec>, player_pos: WorldPos) -> Self {
        let patches = Self::generate_patches(&specs, &player_pos);
        Self {
            specs,
            patches,
            player_pos,
        }
    }

    /// Generate patches from specs, centered on a position.
    fn generate_patches(specs: &[PatchSpec], center: &WorldPos) -> Vec<SimulationPatch> {
        specs
            .iter()
            .map(|spec| {
                SimulationPatch::new(
                    PatchBounds::centered(center.x, center.y, spec.size, spec.size),
                    spec.dx,
                    spec.dt,
                    spec.fidelity.into(),
                )
            })
            .collect()
    }

    /// Move the player to a new position. All patches recenter.
    pub fn move_player(&mut self, new_pos: WorldPos) {
        self.player_pos = new_pos;
        self.patches = Self::generate_patches(&self.specs, &new_pos);
    }

    /// Number of patches in the stack.
    pub fn len(&self) -> usize {
        self.patches.len()
    }

    /// Whether the stack has no patches.
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// Get the innermost (highest fidelity) patch.
    pub fn innermost(&self) -> &SimulationPatch {
        &self.patches[0]
    }

    /// Get the innermost patch mutably.
    pub fn innermost_mut(&mut self) -> &mut SimulationPatch {
        &mut self.patches[0]
    }

    /// Get the outermost (lowest fidelity) patch.
    pub fn outermost(&self) -> &SimulationPatch {
        self.patches.last().unwrap()
    }

    /// Get a patch by index (0 = innermost).
    pub fn patch(&self, index: usize) -> &SimulationPatch {
        &self.patches[index]
    }

    /// Get a patch by index, mutably.
    pub fn patch_mut(&mut self, index: usize) -> &mut SimulationPatch {
        &mut self.patches[index]
    }

    /// Verify that all patches are properly nested (each outer contains
    /// all inner patches).
    pub fn is_nested(&self) -> bool {
        for i in 0..self.patches.len() {
            for j in (i + 1)..self.patches.len() {
                if !self.patches[j].contains_patch(&self.patches[i]) {
                    return false;
                }
            }
        }
        true
    }

    /// Total member count across all patches.
    pub fn total_member_count(&self) -> usize {
        self.patches.iter().map(|p| p.member_count()).sum()
    }

    /// Default patch specs for a typical game scenario.
    ///
    /// - 10km zone: 1m cells, daily timestep, individual plants
    /// - 40km zone: 10m cells, monthly timestep, densities
    /// - 160km zone: 100m cells, yearly timestep, densities
    /// - 640km zone: 1km cells, decadal timestep, aggregates
    pub fn default_specs() -> Vec<PatchSpec> {
        vec![
            PatchSpec {
                size: 10_000.0,
                dx: 1.0,
                dt: 1.0 / 365.0,
                fidelity: FidelityLevelSerde::Individual,
            },
            PatchSpec {
                size: 40_000.0,
                dx: 10.0,
                dt: 1.0 / 12.0,
                fidelity: FidelityLevelSerde::Density,
            },
            PatchSpec {
                size: 160_000.0,
                dx: 100.0,
                dt: 1.0,
                fidelity: FidelityLevelSerde::Density,
            },
            PatchSpec {
                size: 640_000.0,
                dx: 1000.0,
                dt: 10.0,
                fidelity: FidelityLevelSerde::Aggregate,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_specs() -> Vec<PatchSpec> {
        vec![
            PatchSpec {
                size: 1000.0,
                dx: 10.0,
                dt: 1.0,
                fidelity: FidelityLevelSerde::Individual,
            },
            PatchSpec {
                size: 4000.0,
                dx: 100.0,
                dt: 10.0,
                fidelity: FidelityLevelSerde::Density,
            },
            PatchSpec {
                size: 16000.0,
                dx: 1000.0,
                dt: 100.0,
                fidelity: FidelityLevelSerde::Aggregate,
            },
        ]
    }

    #[test]
    fn stack_generates_correct_number_of_patches() {
        let stack = PatchStack::new(test_specs());
        assert_eq!(stack.len(), 3);
    }

    #[test]
    fn patches_are_nested() {
        let stack = PatchStack::new(test_specs());
        assert!(stack.is_nested());
    }

    #[test]
    fn innermost_is_smallest() {
        let stack = PatchStack::new(test_specs());
        let inner = stack.innermost();
        assert!(inner.bounds.width() < stack.patch(1).bounds.width());
        assert_eq!(inner.fidelity, FidelityLevel::Individual);
    }

    #[test]
    fn outermost_is_largest() {
        let stack = PatchStack::new(test_specs());
        let outer = stack.outermost();
        assert!(outer.bounds.width() > stack.patch(1).bounds.width());
        assert_eq!(outer.fidelity, FidelityLevel::Aggregate);
    }

    #[test]
    fn move_player_recenters_patches() {
        let mut stack = PatchStack::new(test_specs());
        let old_center = stack.innermost().bounds;
        stack.move_player(WorldPos::new(5000.0, 5000.0));
        let new_center = stack.innermost().bounds;
        assert_ne!(old_center.min_x, new_center.min_x);
        // New center should be around (5000, 5000)
        assert!((new_center.min_x - (5000.0 - 500.0)).abs() < 1e-6);
    }

    #[test]
    fn patches_remain_nested_after_move() {
        let mut stack = PatchStack::new(test_specs());
        stack.move_player(WorldPos::new(-3000.0, 7000.0));
        assert!(stack.is_nested());
    }

    #[test]
    fn player_position_stored() {
        let pos = WorldPos::new(100.0, 200.0);
        let stack = PatchStack::at_position(test_specs(), pos);
        assert_eq!(stack.player_pos, pos);
    }

    #[test]
    fn default_specs_have_four_layers() {
        let specs = PatchStack::default_specs();
        assert_eq!(specs.len(), 4);
        // Innermost is individual
        assert_eq!(specs[0].fidelity, FidelityLevelSerde::Individual);
        // Outermost is aggregate
        assert_eq!(specs[3].fidelity, FidelityLevelSerde::Aggregate);
    }

    #[test]
    fn default_specs_increasing_size() {
        let specs = PatchStack::default_specs();
        for i in 0..specs.len() - 1 {
            assert!(
                specs[i + 1].size > specs[i].size,
                "each layer should be larger than the previous"
            );
            assert!(
                specs[i + 1].dx > specs[i].dx,
                "each layer should have coarser cells"
            );
        }
    }

    #[test]
    fn patch_bounds_centered_on_player() {
        let pos = WorldPos::new(1234.0, 5678.0);
        let stack = PatchStack::at_position(test_specs(), pos);
        let inner = stack.innermost();
        let cx = (inner.bounds.min_x + inner.bounds.max_x) * 0.5;
        let cy = (inner.bounds.min_y + inner.bounds.max_y) * 0.5;
        assert!((cx - 1234.0).abs() < 1e-6);
        assert!((cy - 5678.0).abs() < 1e-6);
    }
}
