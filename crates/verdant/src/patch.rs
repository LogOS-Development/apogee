//! SimulationPatch — a uniform grid of cells holding vegetation members
//! at one fidelity level.
//!
//! Patches are the spatial units of the simulation. Each patch has its
//! own cell size (dx), timestep (dt), and fidelity level. Patches nest
//! around the player: inner patches are small and fine-grained, outer
//! patches are large and coarse. The same code runs at every level —
//! the fidelity level controls how members are represented and how
//! frequently the patch steps.

use crate::member::{Member, WorldPos};
use crate::plant::{Environment, GenericPlant};
use crate::species::SpeciesConfig;

/// Fidelity level controls member representation and timestep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FidelityLevel {
    /// Track individual plants, daily weather, fine spatial detail.
    Individual,
    /// Track per-species densities, monthly updates, medium resolution.
    Density,
    /// Track biome-level aggregates, yearly updates, coarse resolution.
    Aggregate,
}

/// A bounding box in world space (meters).
#[derive(Clone, Copy, Debug)]
pub struct PatchBounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl PatchBounds {
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub fn centered(cx: f32, cy: f32, width: f32, height: f32) -> Self {
        Self::new(
            cx - width * 0.5,
            cy - height * 0.5,
            cx + width * 0.5,
            cy + height * 0.5,
        )
    }

    pub fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    pub fn contains(&self, pos: &WorldPos) -> bool {
        pos.x >= self.min_x && pos.x < self.max_x && pos.y >= self.min_y && pos.y < self.max_y
    }
}

/// One cell in a simulation patch. Holds the members at that location
/// and local environmental state.
#[derive(Clone, Debug, Default)]
pub struct PatchCell {
    /// All members (individual or density) in this cell.
    pub members: Vec<Member>,
    /// Years since last fire.
    pub time_since_fire: u32,
    /// Current soil moisture fraction (0-1).
    pub soil_moisture: f32,
    /// Current temperature (C).
    pub temperature: f32,
}

impl PatchCell {
    /// Total biomass across all members (kg/m^2).
    pub fn total_biomass(&self) -> f32 {
        self.members.iter().map(|m| m.biomass_density()).sum()
    }

    /// Shade fraction from all canopy members (0 = full sun, 1 = full shade).
    pub fn shade(&self) -> f32 {
        self.members
            .iter()
            .map(|m| m.shade_contribution())
            .fold(0.0_f32, |a, b| a.max(b)) // canopy is max, not sum
            .clamp(0.0, 1.0)
    }

    /// Available light fraction (1 - shade).
    pub fn available_light(&self) -> f32 {
        1.0 - self.shade()
    }

    /// Local environment for plant growth.
    pub fn environment(&self) -> Environment {
        Environment {
            available_light: self.available_light(),
            soil_moisture: self.soil_moisture,
            temperature: self.temperature,
        }
    }

    /// Remove dead members.
    pub fn remove_dead(&mut self, species: &[SpeciesConfig]) {
        self.members.retain(|m| !m.is_dead(&species[m.species_id]));
    }
}

/// A uniform grid patch at one fidelity level.
pub struct SimulationPatch {
    /// Spatial extent in world space.
    pub bounds: PatchBounds,
    /// Cell size in meters.
    pub dx: f32,
    /// Timestep in seconds (for fire) or years (for succession).
    pub dt: f32,
    /// Fidelity level controls member representation.
    pub fidelity: FidelityLevel,
    /// Grid cells, row-major: cells[row * ncols + col].
    pub cells: Vec<PatchCell>,
    /// Number of columns.
    pub ncols: usize,
    /// Number of rows.
    pub nrows: usize,
}

impl SimulationPatch {
    /// Create a new patch with empty cells.
    pub fn new(bounds: PatchBounds, dx: f32, dt: f32, fidelity: FidelityLevel) -> Self {
        let ncols = (bounds.width() / dx).ceil() as usize;
        let nrows = (bounds.height() / dx).ceil() as usize;
        Self {
            bounds,
            dx,
            dt,
            fidelity,
            cells: vec![PatchCell::default(); ncols * nrows],
            ncols,
            nrows,
        }
    }

    /// Convert world position to grid indices.
    pub fn world_to_grid(&self, pos: &WorldPos) -> Option<(usize, usize)> {
        if !self.bounds.contains(pos) {
            return None;
        }
        let col = ((pos.x - self.bounds.min_x) / self.dx) as usize;
        let row = ((pos.y - self.bounds.min_y) / self.dx) as usize;
        Some((col.min(self.ncols - 1), row.min(self.nrows - 1)))
    }

    /// Convert grid indices to world position (cell center).
    pub fn grid_to_world(&self, col: usize, row: usize) -> WorldPos {
        let x = self.bounds.min_x + (col as f32 + 0.5) * self.dx;
        let y = self.bounds.min_y + (row as f32 + 0.5) * self.dx;
        WorldPos::new(x, y)
    }

    /// Get a cell by world position.
    pub fn cell_at(&self, pos: &WorldPos) -> Option<&PatchCell> {
        let (col, row) = self.world_to_grid(pos)?;
        Some(&self.cells[row * self.ncols + col])
    }

    /// Get a mutable cell by world position.
    pub fn cell_at_mut(&mut self, pos: &WorldPos) -> Option<&mut PatchCell> {
        let (col, row) = self.world_to_grid(pos)?;
        Some(&mut self.cells[row * self.ncols + col])
    }

    /// Get a cell by grid indices.
    pub fn cell(&self, col: usize, row: usize) -> &PatchCell {
        &self.cells[row * self.ncols + col]
    }

    /// Get a mutable cell by grid indices.
    pub fn cell_mut(&mut self, col: usize, row: usize) -> &mut PatchCell {
        &mut self.cells[row * self.ncols + col]
    }

    /// Add a member to the cell at its position.
    pub fn add_member(&mut self, member: Member) {
        if let Some(cell) = self.cell_at_mut(&member.position) {
            cell.members.push(member);
        }
    }

    /// Total member count across all cells.
    pub fn member_count(&self) -> usize {
        self.cells.iter().map(|c| c.members.len()).sum()
    }

    /// Total biomass across the entire patch (kg/m^2).
    pub fn total_biomass(&self) -> f32 {
        self.cells.iter().map(|c| c.total_biomass()).sum()
    }

    /// Step the succession for this patch by dt years.
    /// Grows all members, removes dead ones.
    pub fn step_succession(&mut self, species: &[SpeciesConfig], dt: f32) {
        for cell in &mut self.cells {
            let env = cell.environment();
            for member in &mut cell.members {
                member.grow(&species[member.species_id], &env, dt);
                member.age_by(dt);
            }
            cell.remove_dead(species);
        }
    }

    /// Check whether this patch's bounds contain another patch's bounds.
    pub fn contains_patch(&self, other: &SimulationPatch) -> bool {
        self.bounds.min_x <= other.bounds.min_x
            && self.bounds.min_y <= other.bounds.min_y
            && self.bounds.max_x >= other.bounds.max_x
            && self.bounds.max_y >= other.bounds.max_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::SpeciesConfig;

    fn test_species() -> SpeciesConfig {
        SpeciesConfig::from_toml_str(
            r#"
name = "test_pine"
longevity = 200
sexual_maturity = 10
shade_tolerance = 2
fire_tolerance = 3
effective_seed_distance = 50.0
max_seed_distance = 500.0
max_biomass = 5.0
"#,
        )
        .unwrap()
    }

    fn make_patch(width: f32, dx: f32) -> SimulationPatch {
        SimulationPatch::new(
            PatchBounds::centered(0.0, 0.0, width, width),
            dx,
            1.0,
            FidelityLevel::Individual,
        )
    }

    #[test]
    fn patch_grid_dimensions() {
        let patch = make_patch(100.0, 10.0);
        assert_eq!(patch.ncols, 10);
        assert_eq!(patch.nrows, 10);
        assert_eq!(patch.cells.len(), 100);
    }

    #[test]
    fn world_to_grid_mapping() {
        let patch = make_patch(100.0, 10.0);
        // Center of patch is (0, 0), bounds are (-50, -50) to (50, 50)
        let (col, row) = patch.world_to_grid(&WorldPos::new(0.0, 0.0)).unwrap();
        assert_eq!(col, 5);
        assert_eq!(row, 5);
    }

    #[test]
    fn world_to_grid_outside_returns_none() {
        let patch = make_patch(100.0, 10.0);
        assert!(patch.world_to_grid(&WorldPos::new(200.0, 200.0)).is_none());
    }

    #[test]
    fn grid_to_world_returns_cell_center() {
        let patch = make_patch(100.0, 10.0);
        let pos = patch.grid_to_world(0, 0);
        // Cell (0,0) center: min_x + 0.5*dx = -50 + 5 = -45
        assert!((pos.x - (-45.0)).abs() < 1e-6);
        assert!((pos.y - (-45.0)).abs() < 1e-6);
    }

    #[test]
    fn add_member_to_correct_cell() {
        let mut patch = make_patch(100.0, 10.0);
        let member = Member::individual(0, WorldPos::new(0.0, 0.0));
        patch.add_member(member);
        let (col, row) = patch.world_to_grid(&WorldPos::new(0.0, 0.0)).unwrap();
        assert_eq!(patch.cell(col, row).members.len(), 1);
        assert_eq!(patch.member_count(), 1);
    }

    #[test]
    fn cell_total_biomass_aggregates_members() {
        let mut patch = make_patch(100.0, 10.0);
        let mut m1 = Member::individual(0, WorldPos::new(0.0, 0.0));
        m1.biomass = 3.0;
        let mut m2 = Member::individual(0, WorldPos::new(1.0, 1.0));
        m2.biomass = 2.0;
        patch.add_member(m1);
        patch.add_member(m2);
        let (col, row) = patch.world_to_grid(&WorldPos::new(0.0, 0.0)).unwrap();
        assert!((patch.cell(col, row).total_biomass() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn cell_shade_from_canopy() {
        let mut cell = PatchCell::default();
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        m.biomass = 8.0;
        cell.members.push(m);
        assert!(cell.shade() > 0.5);
        assert!(cell.available_light() < 0.5);
    }

    #[test]
    fn succession_grows_members() {
        let spp = test_species();
        let species = vec![spp];
        let mut patch = make_patch(100.0, 10.0);
        // Initialize cell environment
        let (col, row) = patch.world_to_grid(&WorldPos::new(0.0, 0.0)).unwrap();
        patch.cell_mut(col, row).soil_moisture = 0.5;
        patch.cell_mut(col, row).temperature = 20.0;
        patch.add_member(Member::individual(0, WorldPos::new(0.0, 0.0)));
        let initial = patch.cell(col, row).members[0].biomass;
        patch.step_succession(&species, 1.0);
        let after = patch.cell(col, row).members[0].biomass;
        assert!(after > initial, "biomass should increase after growth step");
    }

    #[test]
    fn succession_removes_dead() {
        let spp = test_species();
        let species = vec![spp];
        let mut patch = make_patch(100.0, 10.0);
        let mut m = Member::individual(0, WorldPos::new(0.0, 0.0));
        m.age = 250.0; // past longevity (200)
        patch.add_member(m);
        assert_eq!(patch.member_count(), 1);
        patch.step_succession(&species, 1.0);
        assert_eq!(patch.member_count(), 0, "dead member should be removed");
    }

    #[test]
    fn outer_patch_contains_inner() {
        let outer = SimulationPatch::new(
            PatchBounds::centered(0.0, 0.0, 400.0, 400.0),
            100.0,
            10.0,
            FidelityLevel::Aggregate,
        );
        let inner = SimulationPatch::new(
            PatchBounds::centered(0.0, 0.0, 100.0, 100.0),
            10.0,
            1.0,
            FidelityLevel::Individual,
        );
        assert!(outer.contains_patch(&inner));
        assert!(!inner.contains_patch(&outer));
    }

    #[test]
    fn patch_bounds_centered() {
        let bounds = PatchBounds::centered(50.0, 50.0, 100.0, 100.0);
        assert!((bounds.min_x - 0.0).abs() < 1e-6);
        assert!((bounds.min_y - 0.0).abs() < 1e-6);
        assert!((bounds.max_x - 100.0).abs() < 1e-6);
        assert!((bounds.max_y - 100.0).abs() < 1e-6);
    }

    #[test]
    fn bounds_contains_check() {
        let bounds = PatchBounds::new(0.0, 0.0, 100.0, 100.0);
        assert!(bounds.contains(&WorldPos::new(50.0, 50.0)));
        assert!(!bounds.contains(&WorldPos::new(150.0, 50.0)));
    }
}
