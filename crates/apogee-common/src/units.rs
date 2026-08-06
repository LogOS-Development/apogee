//! Compile-time symbolic unit system.
//!
//! Quantities are tagged by a seven-tuple of type-level signed integers
//! (typenum) representing exponents of the SI base units
//! (meter, kilogram, second, ampere, kelvin, mole, candela). All unit
//! derivation is type-level: `*` adds exponents, `/` subtracts them, `sqrt`
//! halves them. Distinct expressions with the same dimensions collapse to
//! the same Rust type, so assigning `Meters / Seconds` to `Velocity` works
//! without explicit conversion.
//!
//! # SI prefix handling
//!
//! [`SiPrefix`] is a runtime enum that maps each SI decimal prefix
//! (Yocto..Yotta, including the identity `None`) to its multiplicative
//! scale factor. A const [`SiPrefix::SCALES`] table provides the factors
//! for programmatic conversion; users can:
//!
//! 1. Construct a quantity at a specific scale with the prefixed-type
//!    aliases (e.g. `Kilometers::new(1.0)`) and have it normalize to the
//!    underlying SI base on construction.
//! 2. Convert between any two prefixed representations of the same
//!    dimension via [`ConvertPrefix::convert_to`], which uses the
//!    `SiPrefix` table to compute the multiplicative factor at runtime.
//! 3. Read the scale factor directly with [`SiPrefix::scale`] for
//!    ad-hoc arithmetic.
//!
//! # Design cost
//!
//! * Type-checking cost per operation: **O(1)** — each unit is a fixed 7-tuple.
//! * Monomorphization cost: **O(number of distinct unit types used)** — bounded
//!   by the combinations actually referenced.
//! * Runtime cost: identical to the wrapped scalar; the wrapper is a single-field
//!   struct with no runtime unit table.
//! * Memory cost: identical to the wrapped scalar.
//!
//! # Example
//! ```
//! use apogee_common::units::*;
//!
//! let x = Meters::new(10.0);
//! let t = Seconds::new(2.0);
//! let v: Velocity<f64> = x / t;
//! let a: Acceleration<f64> = v / t;
//! ```

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut, Div, Mul, Neg, Sub};

use nalgebra::Vector3;
use typenum::consts::*;
use typenum::{Diff, Sum, Z0};

/// SI decimal prefixes in increasing order, with the identity
/// (`SiPrefix::None`, no scaling) at index 10. Indexing [`SCALES`] by
/// `variant as usize` gives the multiplicative factor relative to the
/// unprefixed SI base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SiPrefix {
    Yocto,
    Zepto,
    Atto,
    Femto,
    Pico,
    Nano,
    Micro,
    Milli,
    Centi,
    Deci,
    /// Identity: 1.0 (no scaling).
    #[default]
    None,
    Deca,
    Hecto,
    Kilo,
    Mega,
    Giga,
    Tera,
    Peta,
    Exa,
    Zetta,
    Yotta,
}

impl SiPrefix {
    /// Multiplicative scale factors indexed by `variant as usize`:
    /// 10^index_minus_10 for the 21 prefixes (Yocto at index 0 → 10^-24,
    /// Yotta at index 20 → 10^24). The identity prefix (`None`, index 10)
    /// is 1.0.
    pub const SCALES: [f64; 21] = [
        1.0e-24, // Yocto
        1.0e-21, // Zepto
        1.0e-18, // Atto
        1.0e-15, // Femto
        1.0e-12, // Pico
        1.0e-9,  // Nano
        1.0e-6,  // Micro
        1.0e-3,  // Milli
        1.0e-2,  // Centi
        1.0e-1,  // Deci
        1.0,     // None
        1.0e1,   // Deca
        1.0e2,   // Hecto
        1.0e3,   // Kilo
        1.0e6,   // Mega
        1.0e9,   // Giga
        1.0e12,  // Tera
        1.0e15,  // Peta
        1.0e18,  // Exa
        1.0e21,  // Zetta
        1.0e24,  // Yotta
    ];

    /// Return the multiplicative scale for this prefix. Equivalent to
    /// `Self::SCALES[self as usize]`.
    #[inline]
    #[must_use]
    pub const fn scale(self) -> f64 {
        Self::SCALES[self as usize]
    }
}

impl fmt::Display for SiPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Yocto => "y",
            Self::Zepto => "z",
            Self::Atto => "a",
            Self::Femto => "f",
            Self::Pico => "p",
            Self::Nano => "n",
            Self::Micro => "µ",
            Self::Milli => "m",
            Self::Centi => "c",
            Self::Deci => "d",
            Self::None => "",
            Self::Deca => "da",
            Self::Hecto => "h",
            Self::Kilo => "k",
            Self::Mega => "M",
            Self::Giga => "G",
            Self::Tera => "T",
            Self::Peta => "P",
            Self::Exa => "E",
            Self::Zetta => "Z",
            Self::Yotta => "Y",
        };
        f.write_str(name)
    }
}

/// A unit is a 7-tuple of type-level signed integer exponents over the SI
/// base units, in order: `[m, kg, s, A, K, mol, cd]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Unit<T>(PhantomData<T>);

/// Convenience: base units. These are the unprefixed SI base dimensions;
/// use [`SiPrefix`] (or the prefixed aliases below) for scaled variants.
pub type Meter = Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>;
pub type Kilogram = Unit<(Z0, P1, Z0, Z0, Z0, Z0, Z0)>;
pub type Second = Unit<(Z0, Z0, P1, Z0, Z0, Z0, Z0)>;
pub type Ampere = Unit<(Z0, Z0, Z0, P1, Z0, Z0, Z0)>;
pub type Kelvin = Unit<(Z0, Z0, Z0, Z0, P1, Z0, Z0)>;
pub type Mole = Unit<(Z0, Z0, Z0, Z0, Z0, P1, Z0)>;
pub type Candela = Unit<(Z0, Z0, Z0, Z0, Z0, Z0, P1)>;
pub type Dimensionless = Unit<(Z0, Z0, Z0, Z0, Z0, Z0, Z0)>;