//! Load-time spacecraft definition: a serializable blueprint consumed at
//! entity-construction time to build the entity's component tree.
//!
//! [`SpacecraftDefinition`] is NOT a runtime ECS component. It is a
//! serializable data structure ingested on load (from a save file,
//! scenario definition, or network message) that drives the construction
//! of an entity's components. The builder function
//! [`SpacecraftDefinition::build`] takes a definition and produces the
//! tuple of runtime components to spawn into the ECS world. The definition
//! is consumed and discarded — the entity carries only runtime components.
//!
//! See issue #150 for the design rationale.

use serde::{Deserialize, Serialize};

use apogee_common::units::{Area, Kilograms};

use crate::components::drag_surfaces::{DragSurface, DragSurfaces};
use crate::components::rigid_body::RigidBody;
use crate::components::srp_surfaces::{SrpSurface, SrpSurfaces};

/// Specification for a single drag surface in a spacecraft definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragSurfaceSpec {
    /// Physical surface area (m^2).
    pub area: Area<f64>,
    /// Drag coefficient (dimensionless).
    pub cd: f64,
    /// Body-frame outward normal direction. `[0, 0, 0]` = cannonball
    /// (isotropic). Non-zero = flat plate.
    pub normal_dir: [f64; 3],
    /// Reference point in the body frame (m), relative to the center of
    /// mass. Used for drag torque computation (future).
    pub reference_point: [f64; 3],
}

impl From<DragSurfaceSpec> for DragSurface {
    fn from(spec: DragSurfaceSpec) -> Self {
        if spec.normal_dir == [0.0, 0.0, 0.0] {
            DragSurface::new(spec.area, spec.cd)
        } else {
            DragSurface::flat_plate(
                spec.area,
                spec.cd,
                nalgebra::Vector3::new(spec.normal_dir[0], spec.normal_dir[1], spec.normal_dir[2]),
                nalgebra::Vector3::new(
                    spec.reference_point[0],
                    spec.reference_point[1],
                    spec.reference_point[2],
                ),
            )
        }
    }
}

/// Specification for a single SRP surface in a spacecraft definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrpSurfaceSpec {
    /// Physical surface area (m^2).
    pub area: Area<f64>,
    /// Reflectivity coefficient (0.0 = fully absorbing, 1.0 = perfectly
    /// reflecting).
    pub reflectivity: f64,
    /// Body-frame outward normal direction. `[0, 0, 0]` = cannonball
    /// (isotropic). Non-zero = flat plate.
    pub normal_dir: [f64; 3],
    /// Reference point in the body frame (m), relative to the center of
    /// mass. Used for SRP torque computation (future).
    pub reference_point: [f64; 3],
}

impl From<SrpSurfaceSpec> for SrpSurface {
    fn from(spec: SrpSurfaceSpec) -> Self {
        if spec.normal_dir == [0.0, 0.0, 0.0] {
            SrpSurface::new(spec.area, spec.reflectivity)
        } else {
            SrpSurface::flat_plate(
                spec.area,
                spec.reflectivity,
                nalgebra::Vector3::new(spec.normal_dir[0], spec.normal_dir[1], spec.normal_dir[2]),
                nalgebra::Vector3::new(
                    spec.reference_point[0],
                    spec.reference_point[1],
                    spec.reference_point[2],
                ),
            )
        }
    }
}

/// A serializable load-time blueprint for a spacecraft entity.
///
/// Consumed by [`SpacecraftDefinition::build`] to produce the runtime
/// component tuple. The definition itself is not stored as an ECS
/// component — it is a load-time description only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacecraftDefinition {
    /// Total spacecraft mass (kg).
    pub mass: Kilograms<f64>,
    /// Inertia tensor in the body frame (kg m^2), row-major 3x3.
    pub inertia: [[f64; 3]; 3],
    /// Center-of-mass offset from the reference point (m).
    pub cg_offset: [f64; 3],
    /// Drag surfaces (main body, solar panels, antenna, etc.).
    pub drag_surfaces: Vec<DragSurfaceSpec>,
    /// SRP-exposed surfaces (main body, solar panels, etc.).
    pub srp_surfaces: Vec<SrpSurfaceSpec>,
}

impl SpacecraftDefinition {
    /// Build the runtime components from this definition.
    ///
    /// Returns a `(RigidBody, Option<DragSurfaces>, Option<SrpSurfaces>)`
    /// tuple. The `Option` components are `None` when the definition has no
    /// surfaces of that type, so the force aggregator skips them
    /// automatically.
    pub fn build(&self) -> (RigidBody, Option<DragSurfaces>, Option<SrpSurfaces>) {
        let rigid_body = RigidBody {
            mass: self.mass,
            inertia: nalgebra::Matrix3::new(
                self.inertia[0][0],
                self.inertia[0][1],
                self.inertia[0][2],
                self.inertia[1][0],
                self.inertia[1][1],
                self.inertia[1][2],
                self.inertia[2][0],
                self.inertia[2][1],
                self.inertia[2][2],
            ),
            cg_offset: nalgebra::Vector3::new(
                self.cg_offset[0],
                self.cg_offset[1],
                self.cg_offset[2],
            ),
        };

        let drag_surfaces = if self.drag_surfaces.is_empty() {
            None
        } else {
            Some(DragSurfaces::from_surfaces(
                self.drag_surfaces
                    .iter()
                    .map(|s| s.clone().into())
                    .collect(),
            ))
        };

        let srp_surfaces = if self.srp_surfaces.is_empty() {
            None
        } else {
            Some(SrpSurfaces::from_surfaces(
                self.srp_surfaces.iter().map(|s| s.clone().into()).collect(),
            ))
        };

        (rigid_body, drag_surfaces, srp_surfaces)
    }
}

impl Default for SpacecraftDefinition {
    fn default() -> Self {
        Self {
            mass: Kilograms::new(1000.0),
            inertia: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            cg_offset: [0.0, 0.0, 0.0],
            drag_surfaces: Vec::new(),
            srp_surfaces: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn build_produces_rigid_body_and_surfaces() {
        let def = SpacecraftDefinition {
            mass: Kilograms::new(500.0),
            inertia: [[10.0, 0.0, 0.0], [0.0, 20.0, 0.0], [0.0, 0.0, 30.0]],
            cg_offset: [0.1, 0.0, 0.0],
            drag_surfaces: vec![DragSurfaceSpec {
                area: Area::new(5.0),
                cd: 2.2,
                normal_dir: [0.0, 0.0, 0.0],
                reference_point: [0.0, 0.0, 0.0],
            }],
            srp_surfaces: vec![
                SrpSurfaceSpec {
                    area: Area::new(2.0),
                    reflectivity: 0.3,
                    normal_dir: [0.0, 0.0, 0.0],
                    reference_point: [0.0, 0.0, 0.0],
                },
                SrpSurfaceSpec {
                    area: Area::new(8.0),
                    reflectivity: 0.5,
                    normal_dir: [0.0, 0.0, 0.0],
                    reference_point: [0.0, 0.0, 0.0],
                },
            ],
        };

        let (rb, drag, srp) = def.build();

        assert_relative_eq!(rb.mass.value, 500.0);
        assert_relative_eq!(rb.inertia[(0, 0)], 10.0);
        assert_relative_eq!(rb.inertia[(1, 1)], 20.0);
        assert_relative_eq!(rb.inertia[(2, 2)], 30.0);
        assert_relative_eq!(rb.cg_offset.x, 0.1);

        let drag = drag.unwrap();
        assert_eq!(drag.len(), 1);
        assert_relative_eq!(drag.surfaces[0].area.value, 5.0);
        assert_relative_eq!(drag.surfaces[0].cd, 2.2);

        let srp = srp.unwrap();
        assert_eq!(srp.len(), 2);
        assert_relative_eq!(srp.surfaces[0].area.value, 2.0);
        assert_relative_eq!(srp.surfaces[1].area.value, 8.0);
    }

    #[test]
    fn build_with_no_surfaces_produces_none() {
        let def = SpacecraftDefinition::default();
        let (rb, drag, srp) = def.build();
        assert_relative_eq!(rb.mass.value, 1000.0);
        assert!(drag.is_none());
        assert!(srp.is_none());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let def = SpacecraftDefinition {
            mass: Kilograms::new(420_000.0),
            inertia: [[1e7, 0.0, 0.0], [0.0, 1e7, 0.0], [0.0, 0.0, 1e7]],
            cg_offset: [0.0, 0.0, 0.0],
            drag_surfaces: vec![DragSurfaceSpec {
                area: Area::new(2500.0),
                cd: 2.2,
                normal_dir: [0.0, 0.0, 0.0],
                reference_point: [0.0, 0.0, 0.0],
            }],
            srp_surfaces: vec![SrpSurfaceSpec {
                area: Area::new(2500.0),
                reflectivity: 1.2,
                normal_dir: [0.0, 0.0, 0.0],
                reference_point: [0.0, 0.0, 0.0],
            }],
        };

        let json = serde_json::to_string(&def).unwrap();
        let def2: SpacecraftDefinition = serde_json::from_str(&json).unwrap();
        assert_relative_eq!(def2.mass.value, def.mass.value);
        assert_eq!(def2.drag_surfaces.len(), 1);
        assert_relative_eq!(def2.drag_surfaces[0].area.value, 2500.0);
        assert_eq!(def2.srp_surfaces.len(), 1);
        assert_relative_eq!(def2.srp_surfaces[0].reflectivity, 1.2);
    }
}
