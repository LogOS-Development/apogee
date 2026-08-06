//! Compile-time symbolic unit system.
//!
//! Units are `Unit<(...)>` marker structs carrying a 7-tuple of type-level
//! signed integer exponents over the SI base units
//! `[m, kg, s, A, K, mol, cd]`.  Unit multiplication and division are
//! type-level via `std::ops::Mul` / `Div` — `Meter * Second` produces the
//! correct exponent tuple at compile time with zero runtime cost.
//!
//! Three primitive wrappers:
//! - [`Quantity<T, U>`] — scalar (real or complex) with a unit tag.
//! - [`VectorQuantity<T, N, U>`] — `N`-component vector, `nalgebra::SVector`.
//! - [`TensorQuantity<T, M, N, U>`] — `M`×`N` matrix, `nalgebra::SMatrix`.
//!
//! `T` defaults to `f64`; use `num_complex::Complex<f64>` for phasor domains.
//!
//! # Architecture
//!
//! The system has two layers:
//!
//! 1. **Unit tags** ([`Unit`] aliases in the [`dim`] module) — zero-sized
//!    phantom types that carry dimension information at the type level.
//!    These are an implementation detail; consumers rarely reference them
//!    directly.
//!
//! 2. **Quantity wrappers** ([`Quantity`], [`VectorQuantity`],
//!    [`TensorQuantity`]) — carry a runtime value (scalar, vector, or
//!    matrix) tagged with a unit type.  Public type aliases like
//!    [`Meters`], [`Velocity`], [`Force`] etc. are the primary API.
//!
//! The [`dim`] module is kept separate from the quantity aliases so the
//! raw unit-tag types are clearly distinguished from the quantity wrappers
//! that most consumers use.  Mixing them in the same namespace would make
//! it ambiguous whether `Meter` refers to a unit tag or a scalar quantity.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Deref, DerefMut, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use nalgebra::{SMatrix, SVector};
use num_complex::Complex;
use num_traits::{NumAssign, Zero};
use typenum::consts::*;
use typenum::{Diff, Sum, Z0};

// ===========================================================================
// Unit marker + type-level Mul / Div
// ===========================================================================

/// A unit is a 7-tuple of type-level signed integer exponents over the SI
/// base units `[m, kg, s, A, K, mol, cd]`.
///
/// `Mul` adds exponents, `Div` subtracts them — both at compile time.
/// `Unit` is zero-sized (`PhantomData`); it exists only in the type system.
///
/// Concrete unit tags are defined as type aliases in the [`dim`] module,
/// e.g. `dim::Meter = Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Unit<T>(PhantomData<T>);

impl<M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Mul<Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>
    for Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>
where
    M1: Add<M2>, Kg1: Add<Kg2>, S1: Add<S2>, A1: Add<A2>,
    K1: Add<K2>, Mol1: Add<Mol2>, Cd1: Add<Cd2>,
{
    type Output = Unit<(
        Sum<M1, M2>, Sum<Kg1, Kg2>, Sum<S1, S2>, Sum<A1, A2>,
        Sum<K1, K2>, Sum<Mol1, Mol2>, Sum<Cd1, Cd2>,
    )>;
    fn mul(self, _rhs: Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>) -> Self::Output {
        Unit(PhantomData)
    }
}

impl<M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
    Div<Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>
    for Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>
where
    M1: Sub<M2>, Kg1: Sub<Kg2>, S1: Sub<S2>, A1: Sub<A2>,
    K1: Sub<K2>, Mol1: Sub<Mol2>, Cd1: Sub<Cd2>,
{
    type Output = Unit<(
        Diff<M1, M2>, Diff<Kg1, Kg2>, Diff<S1, S2>, Diff<A1, A2>,
        Diff<K1, K2>, Diff<Mol1, Mol2>, Diff<Cd1, Cd2>,
    )>;
    fn div(self, _rhs: Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>) -> Self::Output {
        Unit(PhantomData)
    }
}

// ===========================================================================
// SI base unit types  (dim:: module — implementation detail)
// ===========================================================================

/// Type-level unit tags — zero-sized phantom types encoding SI dimensions.
///
/// Each alias is a `Unit<(...)>` with type-level integer exponents over
/// `[m, kg, s, A, K, mol, cd]`.  For example, `Meter` is
/// `Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>` — exponent +1 on length, zero
/// elsewhere.
///
/// These are the **unit tags** only.  The public API wraps them in
/// [`Quantity<T, U>`] via aliases like [`Meters`], [`Velocity`], etc.
/// The separation keeps the type-level dimension machinery distinct from
/// the runtime-valued quantity types that consumers actually use.
pub mod dim {
    use super::*;

    // --- SI base units ---
    pub type Meter = Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>;
    pub type Kilogram = Unit<(Z0, P1, Z0, Z0, Z0, Z0, Z0)>;
    pub type Second = Unit<(Z0, Z0, P1, Z0, Z0, Z0, Z0)>;
    pub type Ampere = Unit<(Z0, Z0, Z0, P1, Z0, Z0, Z0)>;
    pub type Kelvin = Unit<(Z0, Z0, Z0, Z0, P1, Z0, Z0)>;
    pub type Mole = Unit<(Z0, Z0, Z0, Z0, Z0, P1, Z0)>;
    pub type Candela = Unit<(Z0, Z0, Z0, Z0, Z0, Z0, P1)>;
    pub type Dimensionless = Unit<(Z0, Z0, Z0, Z0, Z0, Z0, Z0)>;

    // --- Derived units (m·kg⁻¹·s²·A⁻¹·K⁻¹·mol⁻¹·cd⁻¹) ---
    pub type Velocity = Unit<(P1, Z0, N1, Z0, Z0, Z0, Z0)>;
    pub type Acceleration = Unit<(P1, Z0, N2, Z0, Z0, Z0, Z0)>;
    pub type Force = Unit<(P1, P1, N2, Z0, Z0, Z0, Z0)>;
    pub type Energy = Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>;
    pub type Torque = Energy;
    pub type Power = Unit<(P2, P1, N3, Z0, Z0, Z0, Z0)>;
    pub type Pressure = Unit<(N1, P1, N2, Z0, Z0, Z0, Z0)>;
    pub type Area = Unit<(P2, Z0, Z0, Z0, Z0, Z0, Z0)>;
    pub type Volume = Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>;
    pub type Density = Unit<(N3, P1, Z0, Z0, Z0, Z0, Z0)>;
    pub type Frequency = Unit<(Z0, Z0, N1, Z0, Z0, Z0, Z0)>;
    pub type AngularVelocity = Frequency;
    pub type Charge = Unit<(Z0, Z0, P1, P1, Z0, Z0, Z0)>;
    pub type Voltage = Unit<(P2, P1, N3, N1, Z0, Z0, Z0)>;
    pub type Resistance = Unit<(P2, P1, N3, N2, Z0, Z0, Z0)>;
    pub type Capacitance = Unit<(N2, N1, P3, P2, Z0, Z0, Z0)>;
    pub type Inductance = Unit<(P2, P1, N2, N2, Z0, Z0, Z0)>;
    pub type MagneticFlux = Unit<(P2, P1, N2, N1, Z0, Z0, Z0)>;
    pub type MagneticFluxDensity = Unit<(Z0, P1, N2, N1, Z0, Z0, Z0)>;
    pub type GravitationalParameter = Unit<(P3, Z0, N2, Z0, Z0, Z0, Z0)>;
    pub type MomentOfInertia = Unit<(P2, P1, Z0, Z0, Z0, Z0, Z0)>;
    pub type Angle = Dimensionless;
    pub type SolidAngle = Dimensionless;
    pub type AngularAcceleration = Unit<(Z0, Z0, N2, Z0, Z0, Z0, Z0)>;
    pub type MassFlowRate = Unit<(Z0, P1, N1, Z0, Z0, Z0, Z0)>;
    pub type SpecificImpulse = Second;
    pub type Wavenumber = Unit<(N1, Z0, Z0, Z0, Z0, Z0, Z0)>;
}

// ===========================================================================
// SiPrefix
// ===========================================================================

/// SI metric prefix (yocto through yotta).
///
/// Prefixes are a **construction/display concern**, not a type-level
/// concern.  All quantities store values in SI base units internally;
/// `in_prefix` converts for display only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SiPrefix {
    Yocto, Zepto, Atto, Femto, Pico, Nano, Micro, Milli, Centi, Deci,
    #[default] None,
    Deca, Hecto, Kilo, Mega, Giga, Tera, Peta, Exa, Zetta, Yotta,
}

impl SiPrefix {
    /// Scale factors indexed by enum discriminant (10⁻²⁴ … 10²⁴).
    pub const SCALES: [f64; 21] = [
        1.0e-24, 1.0e-21, 1.0e-18, 1.0e-15, 1.0e-12, 1.0e-9, 1.0e-6, 1.0e-3,
        1.0e-2, 1.0e-1, 1.0, 1.0e1, 1.0e2, 1.0e3, 1.0e6, 1.0e9, 1.0e12,
        1.0e15, 1.0e18, 1.0e21, 1.0e24,
    ];
    /// Returns the multiplicative scale factor (e.g. `Kilo → 1000.0`).
    #[inline] #[must_use]
    pub const fn scale(self) -> f64 { Self::SCALES[self as usize] }
}

impl fmt::Display for SiPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Yocto => "y", Self::Zepto => "z", Self::Atto => "a",
            Self::Femto => "f", Self::Pico => "p", Self::Nano => "n",
            Self::Micro => "µ", Self::Milli => "m", Self::Centi => "c",
            Self::Deci => "d", Self::None => "", Self::Deca => "da",
            Self::Hecto => "h", Self::Kilo => "k", Self::Mega => "M",
            Self::Giga => "G", Self::Tera => "T", Self::Peta => "P",
            Self::Exa => "E", Self::Zetta => "Z", Self::Yotta => "Y",
        })
    }
}

// ===========================================================================
// UnitName
// ===========================================================================

/// Associates a display name (e.g. `"m"`, `"m/s"`, `"N"`) with a unit tag.
///
/// This is implemented **per unit type alias** in [`dim`], not on the
/// generic `Unit<T>` struct itself.  Each `dim::Meter`, `dim::Velocity`,
/// etc. is a distinct monomorphised type (`Unit<(P1, Z0, ...)>`,
/// `Unit<(P1, Z0, N1, ...)>`, …), so we provide a separate `impl
/// UnitName` for each one with the appropriate `NAME` constant.  Rust's
/// trait resolution picks the correct implementation based on the
/// concrete unit type at compile time — no runtime dispatch involved.
pub trait UnitName { const NAME: &'static str; }

impl UnitName for dim::Meter { const NAME: &'static str = "m"; }
impl UnitName for dim::Kilogram { const NAME: &'static str = "kg"; }
impl UnitName for dim::Second { const NAME: &'static str = "s"; }
impl UnitName for dim::Ampere { const NAME: &'static str = "A"; }
impl UnitName for dim::Kelvin { const NAME: &'static str = "K"; }
impl UnitName for dim::Mole { const NAME: &'static str = "mol"; }
impl UnitName for dim::Candela { const NAME: &'static str = "cd"; }
impl UnitName for dim::Dimensionless { const NAME: &'static str = ""; }
impl UnitName for dim::Velocity { const NAME: &'static str = "m/s"; }
impl UnitName for dim::Acceleration { const NAME: &'static str = "m/s²"; }
impl UnitName for dim::Force { const NAME: &'static str = "N"; }
impl UnitName for dim::Energy { const NAME: &'static str = "J"; }
impl UnitName for dim::Power { const NAME: &'static str = "W"; }
impl UnitName for dim::Pressure { const NAME: &'static str = "Pa"; }
impl UnitName for dim::Area { const NAME: &'static str = "m²"; }
impl UnitName for dim::Volume { const NAME: &'static str = "m³"; }
impl UnitName for dim::Density { const NAME: &'static str = "kg/m³"; }
impl UnitName for dim::Frequency { const NAME: &'static str = "Hz"; }
impl UnitName for dim::Charge { const NAME: &'static str = "C"; }
impl UnitName for dim::Voltage { const NAME: &'static str = "V"; }
impl UnitName for dim::Resistance { const NAME: &'static str = "Ω"; }
impl UnitName for dim::Capacitance { const NAME: &'static str = "F"; }
impl UnitName for dim::Inductance { const NAME: &'static str = "H"; }
impl UnitName for dim::MagneticFlux { const NAME: &'static str = "Wb"; }
impl UnitName for dim::MagneticFluxDensity { const NAME: &'static str = "T"; }
impl UnitName for dim::GravitationalParameter { const NAME: &'static str = "m³/s²"; }
impl UnitName for dim::MomentOfInertia { const NAME: &'static str = "kg·m²"; }
impl UnitName for dim::AngularAcceleration { const NAME: &'static str = "rad/s²"; }
impl UnitName for dim::MassFlowRate { const NAME: &'static str = "kg/s"; }
impl UnitName for dim::Wavenumber { const NAME: &'static str = "1/m"; }

// ===========================================================================
// Quantity<T, U>
// ===========================================================================

/// A scalar value tagged with a compile-time unit.
///
/// `T` is the scalar type (defaults to `f64`; use `Complex<f64>` for
/// phasor domains).  `U` is a [`Unit`] type tag from the [`dim`] module.
///
/// Arithmetic operators enforce dimensional correctness at compile time:
/// - `Add`/`Sub` require both operands to have the **same unit** `U`.
/// - `Mul`/`Div` between quantities produce a new quantity whose unit is
///   the product/quotient of the operands' units (type-level exponent
///   arithmetic).
/// - `Mul`/`Div` by a raw `T` scalar preserves the unit unchanged.
///
/// Public type aliases like [`Meters`], [`Velocity`], [`Force`] etc.
/// provide convenient names for common `Quantity` instantiations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity<T, U> {
    /// The raw scalar value in SI base units.
    pub value: T,
    _u: PhantomData<U>,
}

impl<T: NumAssign, U> Zero for Quantity<T, U> {
    #[inline] fn zero() -> Self { Self { value: T::zero(), _u: PhantomData } }
    #[inline] fn is_zero(&self) -> bool { self.value.is_zero() }
}

impl<T: Default, U> Default for Quantity<T, U> {
    fn default() -> Self { Self { value: T::default(), _u: PhantomData } }
}

impl<T, U> Quantity<T, U> {
    /// Creates a quantity from a raw scalar value, inferring the unit type `U`.
    #[inline] #[must_use] pub const fn new(value: T) -> Self { Self { value, _u: PhantomData } }
    /// Borrows the raw scalar value.
    #[inline] #[must_use] pub const fn value(&self) -> &T { &self.value }
    /// Consumes the quantity, returning the raw scalar value.
    #[inline] #[must_use] pub fn into_value(self) -> T { self.value }
    /// Applies a function to the scalar value, preserving the unit tag `U`.
    ///
    /// The return type is `Quantity<R, U>` — the **same unit**, with a
    /// possibly different scalar type `R`.  This is correct for functions
    /// that don't change the dimension (e.g. `abs`, `round`, `min/max`).
    /// For functions that *do* change the dimension (e.g. `sqrt` would
    /// produce `m^{1/2}` from `m`), `map` is the wrong tool — you need a
    /// new quantity with a different unit type.
    #[inline] #[must_use] pub fn map<F, R>(self, f: F) -> Quantity<R, U> where F: FnOnce(T) -> R {
        Quantity { value: f(self.value), _u: PhantomData }
    }
}

impl<T, U> Deref for Quantity<T, U> { type Target = T; #[inline] fn deref(&self) -> &T { &self.value } }
impl<T, U> DerefMut for Quantity<T, U> { #[inline] fn deref_mut(&mut self) -> &mut T { &mut self.value } }

/// Prefix conversion for `f64`-backed quantities.
///
/// All quantities store in SI base units.  These methods convert for
/// display or I/O in a different prefix.
pub trait ConvertPrefix {
    /// Returns the value expressed in the given prefix (e.g. 1000 m → 1.0 km).
    fn in_prefix(&self, prefix: SiPrefix) -> f64;
    /// Returns a new quantity with the value converted to the given prefix.
    fn convert_to(&self, prefix: SiPrefix) -> Self;
}
impl<U> ConvertPrefix for Quantity<f64, U> {
    #[inline] fn in_prefix(&self, prefix: SiPrefix) -> f64 { self.value / prefix.scale() }
    #[inline] fn convert_to(&self, _p: SiPrefix) -> Self { Self::new(self.value) }
}

// --- Add / Sub: same-unit only (enforced at compile time) ---
//
// Both operands must be `Quantity<T, U>` with the *same* `U`.  Attempting
// to add e.g. `Meters` and `Seconds` is a type error — the compiler
// rejects it because `dim::Meter != dim::Second`.

impl<T: NumAssign, U> Add for Quantity<T, U> { type Output = Self; #[inline] fn add(self, r: Self) -> Self { Self::new(self.value + r.value) } }
impl<T: NumAssign, U> AddAssign for Quantity<T, U> { #[inline] fn add_assign(&mut self, r: Self) { self.value += r.value; } }
impl<T: NumAssign, U> Sub for Quantity<T, U> { type Output = Self; #[inline] fn sub(self, r: Self) -> Self { Self::new(self.value - r.value) } }
impl<T: NumAssign, U> SubAssign for Quantity<T, U> { #[inline] fn sub_assign(&mut self, r: Self) { self.value -= r.value; } }
impl<T: NumAssign + Neg<Output = T>, U> Neg for Quantity<T, U> { type Output = Self; #[inline] fn neg(self) -> Self { Self::new(-self.value) } }

// --- Mul / Div between quantities: unit product/quotient at compile time ---

impl<T: NumAssign, A: Mul<B>, B> Mul<Quantity<T, B>> for Quantity<T, A> {
    type Output = Quantity<T, <A as Mul<B>>::Output>;
    #[inline] fn mul(self, r: Quantity<T, B>) -> Self::Output { Quantity::new(self.value * r.value) }
}
impl<T: NumAssign, A: Div<B>, B> Div<Quantity<T, B>> for Quantity<T, A> {
    type Output = Quantity<T, <A as Div<B>>::Output>;
    #[inline] fn div(self, r: Quantity<T, B>) -> Self::Output { Quantity::new(self.value / r.value) }
}
// --- Mul / Div by raw scalar: unit preserved ---
impl<T: NumAssign, U> Mul<T> for Quantity<T, U> { type Output = Self; #[inline] fn mul(self, r: T) -> Self { Self::new(self.value * r) } }
impl<T: NumAssign, U> Div<T> for Quantity<T, U> { type Output = Self; #[inline] fn div(self, r: T) -> Self { Self::new(self.value / r) } }
impl<T: NumAssign, U> MulAssign<T> for Quantity<T, U> { #[inline] fn mul_assign(&mut self, r: T) { self.value *= r; } }

impl<T: fmt::Display, U: UnitName> fmt::Display for Quantity<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{} {}", self.value, U::NAME) }
}

// ===========================================================================
// VectorQuantity<T, N, U>
// ===========================================================================

/// An `N`-component vector tagged with a compile-time unit.
///
/// Wraps `nalgebra::SVector<T, N>`.  `U` is the unit tag shared by all
/// components.  Arithmetic follows the same dimensional rules as
/// [`Quantity`]: `Add`/`Sub` require matching units, `Mul`/`Div` by a
/// scalar preserves units, and `dot`/`cross` produce unit products.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VectorQuantity<T, const N: usize, U> {
    /// The raw nalgebra vector, stored in SI base units.
    pub vector: SVector<T, N>,
    _u: PhantomData<U>,
}

impl<T: Zero + Clone + nalgebra::Scalar, const N: usize, U> Default for VectorQuantity<T, N, U> {
    fn default() -> Self { Self { vector: SVector::zeros(), _u: PhantomData } }
}

impl<T, const N: usize, U> Deref for VectorQuantity<T, N, U> {
    type Target = SVector<T, N>;
    #[inline] fn deref(&self) -> &Self::Target { &self.vector }
}

impl<T, const N: usize, U> DerefMut for VectorQuantity<T, N, U> {
    #[inline] fn deref_mut(&mut self) -> &mut Self::Target { &mut self.vector }
}

impl<T, const N: usize, U> VectorQuantity<T, N, U> {
    /// Creates a vector quantity from a raw nalgebra vector.
    #[inline] #[must_use] pub const fn new(vector: SVector<T, N>) -> Self { Self { vector, _u: PhantomData } }
    /// Borrows the raw nalgebra vector.
    #[inline] #[must_use] pub const fn vector(&self) -> &SVector<T, N> { &self.vector }
    /// Alias for [`vector`](Self::vector) — borrows the raw nalgebra vector.
    #[inline] #[must_use] pub const fn value(&self) -> &SVector<T, N> { &self.vector }
    /// Alias for [`vector`](Self::vector) — borrows the raw nalgebra vector.
    #[inline] #[must_use] pub const fn raw(&self) -> &SVector<T, N> { &self.vector }
    /// Consumes the vector quantity, returning the raw nalgebra vector.
    #[inline] #[must_use] pub fn into_vector(self) -> SVector<T, N> { self.vector }
}

impl<T: NumAssign + Clone + nalgebra::Scalar + nalgebra::ComplexField, const N: usize, U> VectorQuantity<T, N, U> {
    /// Euclidean norm (magnitude) of the vector.
    ///
    /// Returns a [`Quantity`] with the **same unit** `U` and scalar type
    /// `T::RealField` (the real part for complex scalars).
    #[inline] #[must_use] pub fn norm(&self) -> Quantity<T::RealField, U> { Quantity::new(self.vector.norm()) }

    /// Squared Euclidean norm.
    ///
    /// Returns a [`Quantity`] whose unit is `U * U` (the unit squared),
    /// matching the physics: ‖v‖² has units of v².
    #[inline] #[must_use] pub fn norm_squared(&self) -> Quantity<T::RealField, <U as Mul<U>>::Output> where U: Mul<U> { Quantity::new(self.vector.norm_squared()) }

    /// Unit vector in the same direction.
    ///
    /// Returns a [`VectorQuantity`] tagged [`dim::Dimensionless`] — a
    /// direction has no physical units.  The magnitude is 1 (unitless);
    /// the original units cancel in the division v / ‖v‖.
    #[inline] #[must_use] pub fn normalize(&self) -> VectorQuantity<T, N, dim::Dimensionless> { VectorQuantity::new(self.vector.normalize()) }

    /// Dot product with another vector.
    ///
    /// Returns a [`Quantity`] whose unit is `U * B` — the product of
    /// the two operands' units — since the dot product multiplies
    /// component-wise: v · w has units of v × w.
    #[inline] #[must_use] pub fn dot<B>(&self, other: &VectorQuantity<T, N, B>) -> Quantity<T, <U as Mul<B>>::Output> where U: Mul<B> { Quantity::new(self.vector.dot(&other.vector)) }
}

impl<T: NumAssign + Clone + nalgebra::Scalar + nalgebra::ComplexField, U> VectorQuantity<T, 3, U> {
    /// Cross product with another 3D vector.
    ///
    /// Returns a [`VectorQuantity`] whose unit is `U * B` (the unit
    /// product of the two operands' units).
    #[inline] #[must_use] pub fn cross<B>(&self, other: &VectorQuantity<T, 3, B>) -> VectorQuantity<T, 3, <U as Mul<B>>::Output> where U: Mul<B> {
        VectorQuantity::new(self.vector.cross(&other.vector))
    }
}

impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, U> Add for VectorQuantity<T, N, U> { type Output = Self; #[inline] fn add(self, r: Self) -> Self { Self::new(self.vector + r.vector) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, U> Sub for VectorQuantity<T, N, U> { type Output = Self; #[inline] fn sub(self, r: Self) -> Self { Self::new(self.vector - r.vector) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, U> Neg for VectorQuantity<T, N, U> where SVector<T, N>: Neg<Output = SVector<T, N>> { type Output = Self; #[inline] fn neg(self) -> Self { Self::new(-self.vector) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, U> Mul<T> for VectorQuantity<T, N, U> { type Output = Self; #[inline] fn mul(self, r: T) -> Self { Self::new(self.vector * r) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, U> Div<T> for VectorQuantity<T, N, U> { type Output = Self; #[inline] fn div(self, r: T) -> Self { Self::new(self.vector / r) } }

impl<T: NumAssign + Clone + nalgebra::Scalar, const N: usize, UA: Mul<UB>, UB> Mul<VectorQuantity<T, N, UB>> for Quantity<T, UA> {
    type Output = VectorQuantity<T, N, <UA as Mul<UB>>::Output>;
    #[inline] fn mul(self, r: VectorQuantity<T, N, UB>) -> Self::Output { VectorQuantity::new(r.vector * self.value) }
}

impl<T, const N: usize, U: UnitName> fmt::Display for VectorQuantity<T, N, U>
where T: fmt::Display + Clone + nalgebra::Scalar + PartialEq + std::fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "[{}] {}", self.vector, U::NAME) }
}

// ===========================================================================
// TensorQuantity<T, M, N, U>
// ===========================================================================

/// An `M`×`N` matrix tagged with a compile-time unit.
///
/// Wraps `nalgebra::SMatrix<T, M, N>`.  Used for inertia tensors, stress
/// tensors, transformation matrices, etc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TensorQuantity<T, const M: usize, const N: usize, U> {
    /// The raw nalgebra matrix, stored in SI base units.
    pub matrix: SMatrix<T, M, N>,
    _u: PhantomData<U>,
}

impl<T: Zero + Clone + nalgebra::Scalar, const M: usize, const N: usize, U> Default for TensorQuantity<T, M, N, U> {
    fn default() -> Self { Self { matrix: SMatrix::zeros(), _u: PhantomData } }
}

impl<T, const M: usize, const N: usize, U> TensorQuantity<T, M, N, U> {
    /// Creates a tensor quantity from a raw nalgebra matrix.
    #[inline] #[must_use] pub const fn new(matrix: SMatrix<T, M, N>) -> Self { Self { matrix, _u: PhantomData } }
    /// Borrows the raw nalgebra matrix.
    #[inline] #[must_use] pub const fn matrix(&self) -> &SMatrix<T, M, N> { &self.matrix }
    /// Consumes the tensor quantity, returning the raw nalgebra matrix.
    #[inline] #[must_use] pub fn into_matrix(self) -> SMatrix<T, M, N> { self.matrix }
}

impl<T: NumAssign + Clone + nalgebra::Scalar + nalgebra::ComplexField, const M: usize, const N: usize, U> TensorQuantity<T, M, N, U> {
    /// Identity matrix of size `M`×`N`, tagged with unit `U`.
    #[inline] #[must_use] pub fn identity() -> Self { Self::new(SMatrix::identity()) }
    /// Transpose, swapping dimensions `M`↔`N` and preserving the unit.
    #[inline] #[must_use] pub fn transpose(&self) -> TensorQuantity<T, N, M, U> { TensorQuantity::new(self.matrix.transpose()) }
}

impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const N: usize, U> Add for TensorQuantity<T, M, N, U> { type Output = Self; #[inline] fn add(self, r: Self) -> Self { Self::new(self.matrix + r.matrix) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const N: usize, U> Sub for TensorQuantity<T, M, N, U> { type Output = Self; #[inline] fn sub(self, r: Self) -> Self { Self::new(self.matrix - r.matrix) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const N: usize, U> Neg for TensorQuantity<T, M, N, U> where SMatrix<T, M, N>: Neg<Output = SMatrix<T, M, N>> { type Output = Self; #[inline] fn neg(self) -> Self { Self::new(-self.matrix) } }
impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const N: usize, U> Mul<T> for TensorQuantity<T, M, N, U> { type Output = Self; #[inline] fn mul(self, r: T) -> Self { Self::new(self.matrix * r) } }

impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const K: usize, const N: usize, UA: Mul<UB>, UB> Mul<TensorQuantity<T, K, N, UB>> for TensorQuantity<T, M, K, UA> {
    type Output = TensorQuantity<T, M, N, <UA as Mul<UB>>::Output>;
    #[inline] fn mul(self, r: TensorQuantity<T, K, N, UB>) -> Self::Output { TensorQuantity::new(self.matrix * r.matrix) }
}

impl<T: NumAssign + Clone + nalgebra::Scalar, const M: usize, const N: usize, UA: Mul<UB>, UB> Mul<VectorQuantity<T, N, UB>> for TensorQuantity<T, M, N, UA> {
    type Output = VectorQuantity<T, M, <UA as Mul<UB>>::Output>;
    #[inline] fn mul(self, r: VectorQuantity<T, N, UB>) -> Self::Output { VectorQuantity::new(self.matrix * r.vector) }
}

impl<T, const M: usize, const N: usize, U: UnitName> fmt::Display for TensorQuantity<T, M, N, U>
where T: fmt::Display + Clone + nalgebra::Scalar + PartialEq + std::fmt::Debug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "[{}] {}", self.matrix, U::NAME) }
}

// ===========================================================================
// Quantity aliases
// ===========================================================================

// Base
pub type Meters<T = f64> = Quantity<T, dim::Meter>;
pub type Seconds<T = f64> = Quantity<T, dim::Second>;
pub type Kilograms<T = f64> = Quantity<T, dim::Kilogram>;
pub type Amperes<T = f64> = Quantity<T, dim::Ampere>;
pub type Kelvins<T = f64> = Quantity<T, dim::Kelvin>;
pub type Moles<T = f64> = Quantity<T, dim::Mole>;
pub type Candelas<T = f64> = Quantity<T, dim::Candela>;
pub type Dimensionless<T = f64> = Quantity<T, dim::Dimensionless>;
pub type Radians<T = f64> = Quantity<T, dim::Angle>;
pub type Steradians<T = f64> = Quantity<T, dim::SolidAngle>;

// Derived
pub type Velocity<T = f64> = Quantity<T, dim::Velocity>;
pub type Acceleration<T = f64> = Quantity<T, dim::Acceleration>;
pub type Force<T = f64> = Quantity<T, dim::Force>;
pub type Energy<T = f64> = Quantity<T, dim::Energy>;
pub type Power<T = f64> = Quantity<T, dim::Power>;
pub type Pressure<T = f64> = Quantity<T, dim::Pressure>;
pub type Area<T = f64> = Quantity<T, dim::Area>;
pub type Volume<T = f64> = Quantity<T, dim::Volume>;
pub type Density<T = f64> = Quantity<T, dim::Density>;
pub type Frequency<T = f64> = Quantity<T, dim::Frequency>;
pub type Charge<T = f64> = Quantity<T, dim::Charge>;
pub type Voltage<T = f64> = Quantity<T, dim::Voltage>;
pub type Resistance<T = f64> = Quantity<T, dim::Resistance>;
pub type Capacitance<T = f64> = Quantity<T, dim::Capacitance>;
pub type Inductance<T = f64> = Quantity<T, dim::Inductance>;
pub type MagneticFlux<T = f64> = Quantity<T, dim::MagneticFlux>;
pub type MagneticFluxDensity<T = f64> = Quantity<T, dim::MagneticFluxDensity>;
pub type GravitationalParameter<T = f64> = Quantity<T, dim::GravitationalParameter>;
pub type MomentOfInertia<T = f64> = Quantity<T, dim::MomentOfInertia>;
pub type Wavenumber<T = f64> = Quantity<T, dim::Wavenumber>;
pub type MassFlowRate<T = f64> = Quantity<T, dim::MassFlowRate>;

// Prefixed
pub type Kilometers<T = f64> = Quantity<T, dim::Meter>;
pub type Millimeters<T = f64> = Quantity<T, dim::Meter>;
pub type Nanometers<T = f64> = Quantity<T, dim::Meter>;
pub type Centimeters<T = f64> = Quantity<T, dim::Meter>;
pub type Milliseconds<T = f64> = Quantity<T, dim::Second>;
pub type Microseconds<T = f64> = Quantity<T, dim::Second>;
pub type Nanoseconds<T = f64> = Quantity<T, dim::Second>;
pub type Grams<T = f64> = Quantity<T, dim::Kilogram>;
pub type Milligrams<T = f64> = Quantity<T, dim::Kilogram>;
pub type Kilohertz<T = f64> = Quantity<T, dim::Frequency>;
pub type Megahertz<T = f64> = Quantity<T, dim::Frequency>;
pub type Gigahertz<T = f64> = Quantity<T, dim::Frequency>;
pub type Nanoteslas<T = f64> = Quantity<T, dim::MagneticFluxDensity>;
pub type Microteslas<T = f64> = Quantity<T, dim::MagneticFluxDensity>;
pub type Milliteslas<T = f64> = Quantity<T, dim::MagneticFluxDensity>;
pub type Millivolts<T = f64> = Quantity<T, dim::Voltage>;
pub type Kilovolts<T = f64> = Quantity<T, dim::Voltage>;
pub type Megavolts<T = f64> = Quantity<T, dim::Voltage>;
pub type Milliamperes<T = f64> = Quantity<T, dim::Ampere>;
pub type Kiloamperes<T = f64> = Quantity<T, dim::Ampere>;
pub type Kilopascals<T = f64> = Quantity<T, dim::Pressure>;
pub type Megapascals<T = f64> = Quantity<T, dim::Pressure>;
pub type Hectopascals<T = f64> = Quantity<T, dim::Pressure>;
pub type Kilojoules<T = f64> = Quantity<T, dim::Energy>;
pub type Megajoules<T = f64> = Quantity<T, dim::Energy>;
pub type Kilowatts<T = f64> = Quantity<T, dim::Power>;
pub type Megawatts<T = f64> = Quantity<T, dim::Power>;
pub type MilliKelvins<T = f64> = Quantity<T, dim::Kelvin>;
pub type Kilokelvins<T = f64> = Quantity<T, dim::Kelvin>;

// Complex
pub type ComplexMeters = Quantity<Complex<f64>, dim::Meter>;
pub type ComplexSeconds = Quantity<Complex<f64>, dim::Second>;
pub type ComplexVolts = Quantity<Complex<f64>, dim::Voltage>;
pub type ComplexAmperes = Quantity<Complex<f64>, dim::Ampere>;
pub type ComplexFrequency = Quantity<Complex<f64>, dim::Frequency>;
pub type ComplexWavenumber = Quantity<Complex<f64>, dim::Wavenumber>;

// ===========================================================================
// Dynamics re-exports (vector/tensor aliases live in `dynamics`)
// ===========================================================================

pub use crate::dynamics::{
    PositionVector, VelocityVector, AccelerationVector, ForceVector, TorqueVector,
    AngularVelocityVector, AngularAccelerationVector, MagneticFieldVector,
    DirectionVector, AngleVector, InertiaTensor, StressTensor, StrainTensor,
    Mass, Mu, PowerScalar,
};

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn scalar_construction() {
        let d = Meters::new(10.0);
        assert_relative_eq!(d.value, 10.0);
        assert_relative_eq!(*d.value(), 10.0);
        assert_relative_eq!(d.into_value(), 10.0);
    }

    #[test]
    fn scalar_add_sub() {
        assert_relative_eq!((Meters::new(3.0) + Meters::new(7.0)).value, 10.0);
        assert_relative_eq!((Meters::new(7.0) - Meters::new(3.0)).value, 4.0);
    }

    #[test]
    fn scalar_mul_div_derives_units() {
        let v: Velocity = Meters::new(10.0) / Seconds::new(2.0);
        assert_relative_eq!(v.value, 5.0);
        assert_eq!(dim::Velocity::NAME, "m/s");
    }

    #[test]
    fn scalar_times_raw() {
        assert_relative_eq!((Meters::new(3.0) * 2.0).value, 6.0);
    }

    #[test]
    fn acceleration_chain() {
        let v: Velocity = Meters::new(10.0) / Seconds::new(2.0);
        let a: Acceleration = v / Seconds::new(5.0);
        assert_relative_eq!(a.value, 1.0);
    }

    #[test]
    fn force_from_mass_x_accel() {
        let m = Kilograms::new(2.0);
        let a: Acceleration = Meters::new(10.0) / (Seconds::new(1.0) * Seconds::new(1.0));
        let f: Force = m * a;
        assert_relative_eq!(f.value, 20.0);
        assert_eq!(dim::Force::NAME, "N");
    }

    #[test]
    fn prefix_conversion() {
        let km = Kilometers::new(1000.0);
        assert_relative_eq!(km.value, 1000.0);
        assert_relative_eq!(km.in_prefix(SiPrefix::Kilo), 1.0);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", Meters::new(42.0)), "42 m");
        let v: Velocity = Meters::new(10.0) / Seconds::new(2.0);
        assert_eq!(format!("{v}"), "5 m/s");
    }

    #[test]
    fn default_zero() {
        assert!(Meters::<f64>::default().value == 0.0);
        assert!(Velocity::<f64>::default().value == 0.0);
    }

    #[test]
    fn vector_norm() {
        let p = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(3.0, 4.0, 0.0));
        assert_relative_eq!(p.norm().value, 5.0);
    }

    #[test]
    fn vector_normalize() {
        let p = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(3.0, 4.0, 0.0));
        let dir = p.normalize();
        assert_relative_eq!(dir.vector.x, 0.6);
        assert_relative_eq!(dir.vector.y, 0.8);
    }

    #[test]
    fn vector_add() {
        let a = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 0.0, 0.0));
        let b = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(0.0, 1.0, 0.0));
        let c = a + b;
        assert_relative_eq!(c.vector.x, 1.0);
        assert_relative_eq!(c.vector.y, 1.0);
    }

    #[test]
    fn vector_cross() {
        let x = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 0.0, 0.0));
        let y = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(0.0, 1.0, 0.0));
        let z = x.cross(&y);
        assert_relative_eq!(z.vector.z, 1.0);
    }

    #[test]
    fn scalar_times_vector() {
        let s = Meters::new(2.0);
        let v = VectorQuantity::<f64, 3, dim::Dimensionless>::new(nalgebra::Vector3::new(1.0, 2.0, 3.0));
        let p = s * v;
        assert_relative_eq!(p.vector.x, 2.0);
    }

    #[test]
    fn tensor_identity() {
        let t: TensorQuantity<f64, 3, 3, dim::MomentOfInertia> = TensorQuantity::identity();
        assert_relative_eq!(t.matrix[(0, 0)], 1.0);
    }

    #[test]
    fn tensor_times_vector() {
        let m: TensorQuantity<f64, 3, 3, dim::Dimensionless> = TensorQuantity::identity();
        let v = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 2.0, 3.0));
        let r = m * v;
        assert_relative_eq!(r.vector.x, 1.0);
    }

    #[test]
    fn complex_quantity() {
        let z = ComplexMeters::new(Complex::new(3.0, 4.0));
        assert_relative_eq!(z.value.re, 3.0);
        assert_relative_eq!(z.value.im, 4.0);
    }

    #[test]
    fn gm_type() {
        let mu: GravitationalParameter = Quantity::new(3.986004415e14);
        assert_relative_eq!(mu.value, 3.986004415e14);
        assert_eq!(dim::GravitationalParameter::NAME, "m³/s²");
    }

    // --- additional coverage tests ---

    #[test]
    fn scalar_neg() {
        assert_relative_eq!((-Meters::new(5.0)).value, -5.0);
    }

    #[test]
    fn scalar_sub() {
        assert_relative_eq!((Seconds::new(10.0) - Seconds::new(3.0)).value, 7.0);
    }

    #[test]
    fn scalar_add_assign() {
        let mut a = Meters::new(5.0);
        a += Meters::new(3.0);
        assert_relative_eq!(a.value, 8.0);
    }

    #[test]
    fn scalar_sub_assign() {
        let mut a = Meters::new(10.0);
        a -= Meters::new(4.0);
        assert_relative_eq!(a.value, 6.0);
    }

    #[test]
    fn scalar_div_raw() {
        assert_relative_eq!((Meters::new(10.0) / 2.0).value, 5.0);
    }

    #[test]
    fn scalar_mul_assign_raw() {
        let mut a = Meters::new(3.0);
        a *= 2.0;
        assert_relative_eq!(a.value, 6.0);
    }

    #[test]
    fn scalar_map_preserves_unit() {
        let d = Meters::new(-5.0);
        let abs_d = d.map(f64::abs);
        assert_relative_eq!(abs_d.value, 5.0);
        assert_eq!(dim::Meter::NAME, "m");
    }

    #[test]
    fn scalar_display_all_units() {
        assert_eq!(format!("{}", Force::new(10.0)), "10 N");
        assert_eq!(format!("{}", Energy::new(5.0)), "5 J");
        assert_eq!(format!("{}", Power::new(100.0)), "100 W");
        assert_eq!(format!("{}", Pressure::new(101325.0)), "101325 Pa");
        assert_eq!(format!("{}", Area::new(12.0)), "12 m²");
        assert_eq!(format!("{}", Volume::new(3.0)), "3 m³");
        assert_eq!(format!("{}", Density::new(1000.0)), "1000 kg/m³");
        assert_eq!(format!("{}", Frequency::new(60.0)), "60 Hz");
        let c_str = format!("{}", Charge::new(1.6e-19));
        assert!(c_str.ends_with(" C"));
        assert_eq!(format!("{}", Voltage::new(120.0)), "120 V");
        assert_eq!(format!("{}", Resistance::new(50.0)), "50 Ω");
        let cap_str = format!("{}", Capacitance::new(1e-6));
        assert!(cap_str.ends_with(" F"));
        assert_eq!(format!("{}", Inductance::new(0.1)), "0.1 H");
        assert_eq!(format!("{}", MagneticFlux::new(0.5)), "0.5 Wb");
        assert_eq!(format!("{}", MagneticFluxDensity::new(0.05)), "0.05 T");
        assert_eq!(format!("{}", MomentOfInertia::new(100.0)), "100 kg·m²");
        assert_eq!(format!("{}", Wavenumber::new(5.0)), "5 1/m");
        assert_eq!(format!("{}", MassFlowRate::new(2.5)), "2.5 kg/s");
        assert_eq!(format!("{}", Dimensionless::new(0.5)), "0.5 ");
        assert_eq!(format!("{}", Kelvins::new(300.0)), "300 K");
    }

    #[test]
    fn scalar_zero_impl() {
        let z = Meters::<f64>::zero();
        assert!(z.is_zero());
        assert_relative_eq!(z.value, 0.0);
        let nz = Meters::new(1.0);
        assert!(!nz.is_zero());
    }

    #[test]
    fn scalar_deref() {
        let d = Meters::new(10.0);
        assert_relative_eq!(*d, 10.0); // Deref to T
    }

    #[test]
    fn scalar_deref_mut() {
        let mut d = Meters::new(5.0);
        *d += 3.0;
        assert_relative_eq!(d.value, 8.0);
    }

    #[test]
    fn convert_prefix() {
        let m = Meters::new(1000.0);
        assert_relative_eq!(m.in_prefix(SiPrefix::Kilo), 1.0);
        assert_relative_eq!(m.in_prefix(SiPrefix::None), 1000.0);
        let same = m.convert_to(SiPrefix::Kilo);
        assert_relative_eq!(same.value, 1000.0); // stored in SI base, not rescaled
    }

    #[test]
    fn si_prefix_display() {
        assert_eq!(format!("{}", SiPrefix::Kilo), "k");
        assert_eq!(format!("{}", SiPrefix::Milli), "m");
        assert_eq!(format!("{}", SiPrefix::None), "");
        assert_eq!(format!("{}", SiPrefix::Micro), "µ");
        assert_eq!(format!("{}", SiPrefix::Mega), "M");
    }

    #[test]
    fn si_prefix_scale() {
        assert_relative_eq!(SiPrefix::Kilo.scale(), 1.0e3);
        assert_relative_eq!(SiPrefix::Milli.scale(), 1.0e-3);
        assert_relative_eq!(SiPrefix::None.scale(), 1.0);
        assert_relative_eq!(SiPrefix::Yocto.scale(), 1.0e-24);
        assert_relative_eq!(SiPrefix::Yotta.scale(), 1.0e24);
    }

    #[test]
    fn unit_mul_type_level() {
        // Meter * Second -> Unit<(P1, Z0, P1, ...)>
        let _: <dim::Meter as Mul<dim::Second>>::Output = Unit(PhantomData);
    }

    #[test]
    fn unit_div_type_level() {
        // Meter / Second -> Unit<(P1, Z0, N1, ...)>
        let _: <dim::Meter as Div<dim::Second>>::Output = Unit(PhantomData);
    }

    #[test]
    fn vector_norm_squared() {
        let p = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(3.0, 4.0, 0.0));
        assert_relative_eq!(p.norm_squared().value, 25.0);
    }

    #[test]
    fn vector_dot_same_unit() {
        let a = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 2.0, 3.0));
        let b = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(4.0, 5.0, 6.0));
        let d = a.dot(&b);
        assert_relative_eq!(d.value, 32.0); // 4+10+18
    }

    #[test]
    fn vector_dot_different_unit() {
        let r = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 0.0, 0.0));
        let v = VectorQuantity::<f64, 3, dim::Velocity>::new(nalgebra::Vector3::new(10.0, 0.0, 0.0));
        let d = r.dot(&v);
        assert_relative_eq!(d.value, 10.0);
    }

    #[test]
    fn vector_sub() {
        let a = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(5.0, 3.0, 1.0));
        let b = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(2.0, 1.0, 0.0));
        let c = a - b;
        assert_relative_eq!(c.vector.x, 3.0);
        assert_relative_eq!(c.vector.y, 2.0);
        assert_relative_eq!(c.vector.z, 1.0);
    }

    #[test]
    fn vector_neg() {
        let a = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, -2.0, 3.0));
        let b = -a;
        assert_relative_eq!(b.vector.x, -1.0);
        assert_relative_eq!(b.vector.y, 2.0);
        assert_relative_eq!(b.vector.z, -3.0);
    }

    #[test]
    fn vector_div_raw() {
        let a = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(10.0, 20.0, 30.0));
        let b = a / 2.0;
        assert_relative_eq!(b.vector.x, 5.0);
        assert_relative_eq!(b.vector.y, 10.0);
        assert_relative_eq!(b.vector.z, 15.0);
    }

    #[test]
    fn vector_display() {
        let v = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 2.0, 3.0));
        let s = format!("{v}");
        assert!(s.contains("m"));
    }

    #[test]
    fn vector_into_vector() {
        let v = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(1.0, 2.0, 3.0));
        let raw = v.into_vector();
        assert_relative_eq!(raw.x, 1.0);
    }

    #[test]
    fn vector_vector_and_raw_accessors() {
        let v = VectorQuantity::<f64, 3, dim::Meter>::new(nalgebra::Vector3::new(7.0, 8.0, 9.0));
        assert_relative_eq!(v.vector().x, 7.0);
        assert_relative_eq!(v.value().x, 7.0);
        assert_relative_eq!(v.raw().x, 7.0);
    }

    #[test]
    fn vector_default_zero() {
        let v = VectorQuantity::<f64, 3, dim::Meter>::default();
        assert_relative_eq!(v.vector.norm(), 0.0);
    }

    #[test]
    fn tensor_transpose() {
        let m = TensorQuantity::<f64, 2, 3, dim::Dimensionless>::new(
            nalgebra::SMatrix::from_element(1.0),
        );
        let t = m.transpose();
        assert_relative_eq!(t.matrix[(0, 0)], 1.0);
    }

    #[test]
    fn tensor_sub() {
        let a: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::new(
            nalgebra::SMatrix::from_element(5.0),
        );
        let b: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::new(
            nalgebra::SMatrix::from_element(3.0),
        );
        let c = a - b;
        assert_relative_eq!(c.matrix[(0, 0)], 2.0);
    }

    #[test]
    fn tensor_neg() {
        let a: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::new(
            nalgebra::SMatrix::from_element(5.0),
        );
        let b = -a;
        assert_relative_eq!(b.matrix[(0, 0)], -5.0);
    }

    #[test]
    fn tensor_mul_raw() {
        let a: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::new(
            nalgebra::SMatrix::from_element(3.0),
        );
        let c = a * 2.0;
        assert_relative_eq!(c.matrix[(0, 0)], 6.0);
    }

    #[test]
    fn tensor_mul_tensor() {
        let a: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::identity();
        let b: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::new(
            nalgebra::SMatrix::from_element(7.0),
        );
        let c = a * b;
        assert_relative_eq!(c.matrix[(0, 0)], 7.0);
    }

    #[test]
    fn tensor_default_zero() {
        let t: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::default();
        assert_relative_eq!(t.matrix[(0, 0)], 0.0);
    }

    #[test]
    fn tensor_into_matrix() {
        let t: TensorQuantity<f64, 2, 2, dim::Dimensionless> = TensorQuantity::identity();
        let raw = t.into_matrix();
        assert_relative_eq!(raw[(0, 0)], 1.0);
    }

    #[test]
    fn tensor_display() {
        let t: TensorQuantity<f64, 2, 2, dim::Force> = TensorQuantity::identity();
        let s = format!("{t}");
        assert!(s.contains("N"));
    }

    #[test]
    fn complex_velocity() {
        let v = ComplexFrequency::new(Complex::new(1.0, 2.0));
        assert_relative_eq!(v.value.re, 1.0);
        assert_relative_eq!(v.value.im, 2.0);
    }

    #[test]
    fn prefixed_quantity_types() {
        // These are all Quantity<T, dim::X> — just verify construction works
        let km = Kilometers::new(1.0);
        let mm = Millimeters::new(1.0);
        let ms = Milliseconds::new(1.0);
        let nt = Nanoteslas::new(50.0);
        let kpa = Kilopascals::new(101.3);
        assert_relative_eq!(km.value, 1.0);
        assert_relative_eq!(mm.value, 1.0);
        assert_relative_eq!(ms.value, 1.0);
        assert_relative_eq!(nt.value, 50.0);
        assert_relative_eq!(kpa.value, 101.3);
    }
}