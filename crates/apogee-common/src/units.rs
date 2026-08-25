//! Compile-time symbolic unit system.
//!
//! This module re-exports the [`metron`](https://github.com/LogOS-Development/metron)
//! crate, which provides `Quantity<T, U>`, `VectorQuantity<T, N, U>`,
//! `TensorQuantity<T, M, N, U>`, the `pow!` macro, and all SI type aliases.
//!
//! Physics-specific vector and tensor aliases (PositionVector, InertiaTensor,
//! etc.) live in [`crate::dynamics`].

pub use metron::*;

// Re-export dynamics aliases through units for backward compatibility.
pub use crate::dynamics::{
    AccelerationVector, AngleVector, AngularAccelerationVector, AngularVelocityVector,
    DirectionVector, ForceVector, InertiaTensor, MagneticFieldVector, Mass, Mu, PositionVector,
    PowerScalar, StrainTensor, StressTensor, TorqueVector, VelocityVector,
};
