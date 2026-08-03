//! Magnetosphere models: geomagnetic field.
//!
//! Provides a pluggable set of magnetic field models for celestial bodies.
//! The primary implementation is the International Geomagnetic Reference Field
//! (IGRF-13) for Earth, evaluated via spherical harmonics with time-varying
//! secular variation. Other bodies can be added later with the same trait
//! interface.

pub(crate) mod data;
pub(crate) mod disturbance;
pub mod igrf;
pub(crate) mod legendre;

pub use igrf::Igrf;

use apogee_common::units::Nanoteslas;
use apogee_common::NaifId;
use hifitime::Epoch;
use nalgebra::Vector3;

/// A magnetic flux-density vector in nanotesla (nT).
///
/// The inner storage is a raw `Vector3<f64>` so it interoperates with nalgebra;
/// unit-aware accessors return `Nanoteslas<f64>` per component.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MagneticFieldVector(Vector3<f64>);

impl MagneticFieldVector {
    /// Wrap a raw nT vector.
    #[must_use]
    #[allow(non_snake_case)]
    pub fn from_nT(raw: Vector3<f64>) -> Self {
        Self(raw)
    }

    /// Borrow the raw vector in nT.
    #[must_use]
    pub const fn raw(&self) -> &Vector3<f64> {
        &self.0
    }

    /// Dot product with a raw direction/measurement vector.
    #[must_use]
    pub fn dot(&self, other: &Vector3<f64>) -> f64 {
        self.0.dot(other)
    }

    /// X component in nT.
    #[must_use]
    #[allow(non_snake_case)]
    pub fn x_nT(&self) -> Nanoteslas<f64> {
        Nanoteslas::new(self.0.x)
    }

    /// Y component in nT.
    #[must_use]
    #[allow(non_snake_case)]
    pub fn y_nT(&self) -> Nanoteslas<f64> {
        Nanoteslas::new(self.0.y)
    }

    /// Z component in nT.
    #[must_use]
    #[allow(non_snake_case)]
    pub fn z_nT(&self) -> Nanoteslas<f64> {
        Nanoteslas::new(self.0.z)
    }
}

impl From<Vector3<f64>> for MagneticFieldVector {
    fn from(raw: Vector3<f64>) -> Self {
        Self(raw)
    }
}

impl From<MagneticFieldVector> for Vector3<f64> {
    fn from(v: MagneticFieldVector) -> Self {
        v.0
    }
}

/// Fidelity selector for magnetic field models.
///
/// Higher-fidelity modes include more spherical-harmonic terms and run slower.
/// Models that do not support a requested fidelity should fall back to the
/// closest supported level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MagneticFieldFidelity {
    /// Axial dipole only. Fastest; useful for coarse long-range propagation.
    Dipole,
    /// Low-degree model (n ≈ 4–5). Good for most navigation and attitude work.
    LowDegree,
    /// Medium-degree model (n ≈ 8). Balances accuracy and cost.
    MediumDegree,
    /// Full model degree. For high-precision work near the body.
    Full,
}

impl MagneticFieldFidelity {
    /// Maximum spherical-harmonic degree for IGRF-based models.
    #[must_use]
    pub const fn max_degree_igrf(self) -> usize {
        match self {
            Self::Dipole => 1,
            Self::LowDegree => 5,
            Self::MediumDegree => 8,
            Self::Full => 13,
        }
    }
}

/// Trait for geomagnetic field models.
///
/// Implementations must be deterministic and thread-safe, and are associated
/// with a single celestial body via [`MagneticFieldModel::body_id`].
pub trait MagneticFieldModel: Send + Sync {
    /// NAIF ID of the body whose magnetic field this model evaluates.
    fn body_id(&self) -> NaifId;

    /// Magnetic flux density at a body-fixed position and epoch.
    ///
    /// The position is expressed in the body-fixed frame appropriate to the
    /// model (e.g. ECEF for Earth) and is in meters. The returned vector is in
    /// nanotesla.
    fn field(&self, position_m: &Vector3<f64>, epoch: Epoch) -> MagneticFieldVector;
}

impl MagneticFieldModel for Igrf {
    fn body_id(&self) -> NaifId {
        399 // Earth
    }

    fn field(&self, position_m: &Vector3<f64>, epoch: Epoch) -> MagneticFieldVector {
        self.field(position_m, epoch)
    }
}

/// Catalog of magnetic field models indexed by body.
///
/// A system can register one model per body. Bodies without a registered model
/// return a zero field, which is safe for spacecraft that never encounter them.
#[derive(Default)]
pub struct MagneticFieldCatalog {
    models: Vec<Box<dyn MagneticFieldModel>>,
}

impl std::fmt::Debug for MagneticFieldCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MagneticFieldCatalog")
            .field("bodies", &self.bodies())
            .finish()
    }
}

impl MagneticFieldCatalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a model for its associated body, replacing any existing model.
    pub fn register(&mut self, model: Box<dyn MagneticFieldModel>) {
        let id = model.body_id();
        if let Some(existing) = self.models.iter_mut().find(|m| m.body_id() == id) {
            *existing = model;
        } else {
            self.models.push(model);
        }
    }

    /// Evaluate the field for the given body at a body-fixed position and epoch.
    pub fn field(
        &self,
        body_id: NaifId,
        position_m: &Vector3<f64>,
        epoch: Epoch,
    ) -> MagneticFieldVector {
        self.models
            .iter()
            .find(|m| m.body_id() == body_id)
            .map(|m| m.field(position_m, epoch))
            .unwrap_or_else(|| MagneticFieldVector::from_nT(Vector3::zeros()))
    }

    /// Return the registered body IDs.
    pub fn bodies(&self) -> Vec<NaifId> {
        self.models.iter().map(|m| m.body_id()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_returns_zero_for_unknown_body() {
        let catalog = MagneticFieldCatalog::new();
        let pos = Vector3::new(1.0, 0.0, 0.0);
        let epoch = Epoch::from_gregorian_utc(2024, 1, 1, 0, 0, 0, 0);
        let b = catalog.field(499, &pos, epoch);
        assert_eq!(b.raw(), &Vector3::zeros());
    }

    #[test]
    fn catalog_registers_and_looks_up_earth_model() {
        let mut catalog = MagneticFieldCatalog::new();
        catalog.register(Box::new(Igrf::new()));
        assert!(catalog.bodies().contains(&399));
    }

    #[test]
    fn fidelity_maps_to_sensible_max_degree() {
        assert_eq!(MagneticFieldFidelity::Dipole.max_degree_igrf(), 1);
        assert_eq!(MagneticFieldFidelity::LowDegree.max_degree_igrf(), 5);
        assert_eq!(MagneticFieldFidelity::MediumDegree.max_degree_igrf(), 8);
        assert_eq!(MagneticFieldFidelity::Full.max_degree_igrf(), 13);
    }
}
