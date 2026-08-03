1|//! Compile-time symbolic unit system.
2|//!
3|//! Quantities are tagged by a seven-tuple of type-level signed integers
4|//! (typenum) representing exponents of the SI base units
5|//! (meter, kilogram, second, ampere, kelvin, mole, candela). All unit
6|//! derivation is type-level: `*` adds exponents, `/` subtracts them, `sqrt`
7|//! halves them. Distinct expressions with the same dimensions collapse to
8|//! the same Rust type, so assigning `Meters / Seconds` to `Velocity` works
9|//! without explicit conversion.
10|//!
11|//! # SI prefix handling
12|//!
13|//! [`SiPrefix`] is a runtime enum that maps each SI decimal prefix
14|//! (Yocto..Yotta, including the identity `None`) to its multiplicative
15|//! scale factor. A const [`SiPrefix::SCALES`] table provides the factors
16|//! for programmatic conversion; users can:
17|//!
18|//! 1. Construct a quantity at a specific scale with the prefixed-type
19|//!    aliases (e.g. `Kilometers::new(1.0)`) and have it normalize to the
20|//!    underlying SI base on construction.
21|//! 2. Convert between any two prefixed representations of the same
22|//!    dimension via [`ConvertPrefix::convert_to`], which uses the
23|//!    `SiPrefix` table to compute the multiplicative factor at runtime.
24|//! 3. Read the scale factor directly with [`SiPrefix::scale`] for
25|//!    ad-hoc arithmetic.
26|//!
27|//! # Design cost
28|//!
29|//! * Type-checking cost per operation: **O(1)** — each unit is a fixed 7-tuple.
30|//! * Monomorphization cost: **O(number of distinct unit types used)** — bounded
31|//!   by the combinations actually referenced.
32|//! * Runtime cost: identical to the wrapped scalar; the wrapper is a single-field
33|//!   struct with no runtime unit table.
34|//! * Memory cost: identical to the wrapped scalar.
35|//!
36|//! # Example
37|//! ```ignore
38|//! use apogee_common::units::*;
39|//!
40|//! let x = Meters::new(10.0);
41|//! let t = Seconds::new(2.0);
42|//! let v: Velocity<f64> = x / t;
43|//! let a: Acceleration<f64> = v / t;
44|//!
45|//! // Programmatic SI prefix handling.
46|//! let km = Meters::with_prefix(SiPrefix::Kilo, 1.0);  // stored as 1000.0 m
47|//! let in_mm = km.with_prefix(SiPrefix::Milli);       // stored as 1_000_000.0 m
48|//! ```
49|
50|use std::fmt;
51|use std::marker::PhantomData;
52|use std::ops::{Add, Div, Mul, Neg, Sub};
53|
54|use typenum::consts::*;
55|use typenum::{Diff, Sum, Z0};
56|
57|/// SI decimal prefixes in increasing order, with the identity
58|/// (`SiPrefix::None`, no scaling) at index 10. Indexing [`SCALES`] by
59|/// `variant as usize` gives the multiplicative factor relative to the
60|/// unprefixed SI base.
61|#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
62|pub enum SiPrefix {
63|    Yocto,
64|    Zepto,
65|    Atto,
66|    Femto,
67|    Pico,
68|    Nano,
69|    Micro,
70|    Milli,
71|    Centi,
72|    Deci,
73|    /// Identity: 1.0 (no scaling).
74|    None,
75|    Deca,
76|    Hecto,
77|    Kilo,
78|    Mega,
79|    Giga,
80|    Tera,
81|    Peta,
82|    Exa,
83|    Zetta,
84|    Yotta,
85|}
86|
87|impl SiPrefix {
88|    /// Multiplicative scale factors indexed by `variant as usize`:
89|    /// 10^index_minus_10 for the 21 prefixes (Yocto at index 0 → 10^-24,
90|    /// Yotta at index 20 → 10^24). The identity prefix (`None`, index 10)
91|    /// is 1.0.
92|    pub const SCALES: [f64; 21] = [
93|        1.0e-24, // Yocto
94|        1.0e-21, // Zepto
95|        1.0e-18, // Atto
96|        1.0e-15, // Femto
97|        1.0e-12, // Pico
98|        1.0e-9,  // Nano
99|        1.0e-6,  // Micro
100|        1.0e-3,  // Milli
101|        1.0e-2,  // Centi
102|        1.0e-1,  // Deci
103|        1.0,     // None
104|        1.0e1,   // Deca
105|        1.0e2,   // Hecto
106|        1.0e3,   // Kilo
107|        1.0e6,   // Mega
108|        1.0e9,   // Giga
109|        1.0e12,  // Tera
110|        1.0e15,  // Peta
111|        1.0e18,  // Exa
112|        1.0e21,  // Zetta
113|        1.0e24,  // Yotta
114|    ];
115|
116|    /// Return the multiplicative scale for this prefix. Equivalent to
117|    /// `Self::SCALES[self as usize]`.
118|    #[inline]
119|    #[must_use]
120|    pub const fn scale(self) -> f64 {
121|        Self::SCALES[self as usize]
122|    }
123|}
124|
125|impl fmt::Display for SiPrefix {
126|    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
127|        let name = match self {
128|            Self::Yocto => "y",
129|            Self::Zepto => "z",
130|            Self::Atto => "a",
131|            Self::Femto => "f",
132|            Self::Pico => "p",
133|            Self::Nano => "n",
134|            Self::Micro => "µ",
135|            Self::Milli => "m",
136|            Self::Centi => "c",
137|            Self::Deci => "d",
138|            Self::None => "",
139|            Self::Deca => "da",
140|            Self::Hecto => "h",
141|            Self::Kilo => "k",
142|            Self::Mega => "M",
143|            Self::Giga => "G",
144|            Self::Tera => "T",
145|            Self::Peta => "P",
146|            Self::Exa => "E",
147|            Self::Zetta => "Z",
148|            Self::Yotta => "Y",
149|        };
150|        f.write_str(name)
151|    }
152|}
153|
154|/// A unit is a 7-tuple of type-level signed integer exponents over the SI
155|/// base units, in order: `[m, kg, s, A, K, mol, cd]`.
156|#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
157|pub struct Unit<T>(PhantomData<T>);
158|
159|/// Convenience: base units. These are the unprefixed SI base dimensions;
160|/// use [`SiPrefix`] (or the prefixed aliases below) for scaled variants.
161|pub type Meters<T> = Quantity<T, Unit<(P1, Z0, Z0, Z0, Z0, Z0, Z0)>>;
162|pub type Kilograms<T> = Quantity<T, Unit<(Z0, P1, Z0, Z0, Z0, Z0, Z0)>>;
163|pub type Seconds<T> = Quantity<T, Unit<(Z0, Z0, P1, Z0, Z0, Z0, Z0)>>;
164|pub type Amperes<T> = Quantity<T, Unit<(Z0, Z0, Z0, P1, Z0, Z0, Z0)>>;
165|pub type Kelvins<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, P1, Z0, Z0)>>;
166|pub type Moles<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, P1, Z0)>>;
167|pub type Candelas<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, Z0, P1)>>;
168|pub type Dimensionless<T> = Quantity<T, Unit<(Z0, Z0, Z0, Z0, Z0, Z0, Z0)>>;
169|
170|/// Convenience: derived units.
171|pub type Velocity<T> = Quantity<T, Unit<(P1, Z0, N1, Z0, Z0, Z0, Z0)>>;
172|pub type Acceleration<T> = Quantity<T, Unit<(P1, Z0, N2, Z0, Z0, Z0, Z0)>>;
173|pub type Force<T> = Quantity<T, Unit<(P1, P1, N2, Z0, Z0, Z0, Z0)>>;
174|pub type Torque<T> = Quantity<T, Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>>;
175|pub type Pressure<T> = Quantity<T, Unit<(N1, P1, N2, Z0, Z0, Z0, Z0)>>;
176|pub type Energy<T> = Quantity<T, Unit<(P2, P1, N2, Z0, Z0, Z0, Z0)>>;
177|pub type Power<T> = Quantity<T, Unit<(P2, P1, N3, Z0, Z0, Z0, Z0)>>;
178|pub type Area<T> = Quantity<T, Unit<(P2, Z0, Z0, Z0, Z0, Z0, Z0)>>;
179|pub type Volume<T> = Quantity<T, Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>>;
180|pub type Density<T> = Quantity<T, Unit<(N3, P1, Z0, Z0, Z0, Z0, Z0)>>;
181|pub type Frequency<T> = Quantity<T, Unit<(Z0, Z0, N1, Z0, Z0, Z0, Z0)>>;
182|pub type ElectricCharge<T> = Quantity<T, Unit<(Z0, Z0, P1, P1, Z0, Z0, Z0)>>;
183|pub type Voltage<T> = Quantity<T, Unit<(P2, P1, N3, N1, Z0, Z0, Z0)>>;
184|pub type Kilometers<T> = Quantity<T, Unit<(P3, Z0, Z0, Z0, Z0, Z0, Z0)>>;
185|pub type Nanoteslas<T> = Quantity<T, Unit<(N2, P1, N2, Z0, Z0, Z0, Z0)>>;
186|pub type GravitationalParameter<T> = Quantity<T, Unit<(P3, Z0, N2, Z0, Z0, Z0, Z0)>>;
187|
188|/// A scalar `value` tagged with a compile-time unit `U`.
189|///
190|/// The unit is part of the type, so dimensional mismatches are caught at
191|/// compile time. The runtime representation is the scalar alone.
192|#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
193|pub struct Quantity<T, U> {
194|    pub value: T,
195|    _unit: PhantomData<U>,
196|}
197|
198|impl<T, U> Quantity<T, U> {
199|    /// Wrap a raw scalar value with a unit.
200|    #[must_use]
201|    pub const fn new(value: T) -> Self {
202|        Self {
203|            value,
204|            _unit: PhantomData,
205|        }
206|    }
207|
208|    /// Unwrap the scalar value.
209|    #[must_use]
210|    pub fn into_value(self) -> T {
211|        self.value
212|    }
213|
214|    /// Borrow the scalar value.
215|    #[must_use]
216|    pub const fn value_ref(&self) -> &T {
217|        &self.value
218|    }
219|
220|    /// Apply an SI decimal prefix to the wrapped value, returning a new
221|    /// `Quantity` in the same unit. The prefix's [`SiPrefix::scale`]
222|    /// factor is multiplied with `self.value`.
223|    ///
224|    /// This is the runtime-programmatic counterpart to the type-level
225|    /// prefixed aliases (e.g. `Kilometers`): users can construct any
226|    /// quantity with `Quantity::new(value)` and then `with_prefix` it into
227|    /// a chosen scale, or do the reverse by dividing by a known prefix
228|    /// scale (`self.value / SiPrefix::Milli.scale()`).
229|    ///
230|    /// # Example
231|    /// ```ignore
232|    /// use apogee_common::units::*;
233|    ///
234|    /// // Convert 1 km (stored as 1000 m) into millimeters: 1_000_000 mm.
235|    /// let one_km: Meters<f64> = Meters::new(1.0e3);
236|    /// let in_mm = one_km.with_prefix(SiPrefix::Milli);
237|    /// assert_eq!(in_mm.into_value(), 1.0e6);
238|    ///
239|    /// // Convert 1 Mm (megameter) into meters: 1_000_000 m.
240|    /// let in_m = Meters::new(1.0).with_prefix(SiPrefix::Mega);
241|    /// assert_eq!(in_m.into_value(), 1.0e6);
242|    /// ```
243|    #[must_use]
244|    pub fn with_prefix(self, prefix: SiPrefix) -> Self
245|    where
246|        T: Copy + core::ops::Mul<f64, Output = T>,
247|    {
248|        Quantity::new(self.value * prefix.scale())
249|    }
250|
251|    /// Divide the wrapped value by an SI prefix scale, returning a new
252|    /// `Quantity` in the same unit. Inverse of [`Quantity::with_prefix`].
253|    #[must_use]
254|    pub fn strip_prefix(self, prefix: SiPrefix) -> Self
255|    where
256|        T: Copy + core::ops::Div<f64, Output = T>,
257|    {
258|        Quantity::new(self.value / prefix.scale())
259|    }
260|}
261|
262|/// Type-level halving for square root. Implemented for the even exponents
263|/// likely to appear in physical models.
264|pub trait Half {
265|    type Output;
266|}
267|impl Half for Z0 {
268|    type Output = Z0;
269|}
270|impl Half for P2 {
271|    type Output = P1;
272|}
273|impl Half for P4 {
274|    type Output = P2;
275|}
276|impl Half for P6 {
277|    type Output = P3;
278|}
279|impl Half for P8 {
280|    type Output = P4;
281|}
282|impl Half for N2 {
283|    type Output = N1;
284|}
285|impl Half for N4 {
286|    type Output = N2;
287|}
288|impl Half for N6 {
289|    type Output = N3;
290|}
291|impl Half for N8 {
292|    type Output = N4;
293|}
294|
295|impl<T, M, Kg, S, A, K, Mol, Cd> Quantity<T, Unit<(M, Kg, S, A, K, Mol, Cd)>>
296|where
297|    M: Half,
298|    Kg: Half,
299|    S: Half,
300|    A: Half,
301|    K: Half,
302|    Mol: Half,
303|    Cd: Half,
304|    T: num_traits::Float,
305|{
306|    #[must_use]
307|    #[allow(clippy::type_complexity)]
308|    pub fn sqrt(
309|        self,
310|    ) -> Quantity<
311|        T,
312|        Unit<(
313|            M::Output,
314|            Kg::Output,
315|            S::Output,
316|            A::Output,
317|            K::Output,
318|            Mol::Output,
319|            Cd::Output,
320|        )>,
321|    > {
322|        Quantity::new(self.value.sqrt())
323|    }
324|}
325|
326|// --- Addition / subtraction (same unit) ---
327|
328|impl<T, U> Add for Quantity<T, U>
329|where
330|    T: Add,
331|{
332|    type Output = Quantity<<T as Add>::Output, U>;
333|    fn add(self, rhs: Self) -> Self::Output {
334|        Quantity::new(self.value + rhs.value)
335|    }
336|}
337|
338|impl<T, U> Sub for Quantity<T, U>
339|where
340|    T: Sub,
341|{
342|    type Output = Quantity<<T as Sub>::Output, U>;
343|    fn sub(self, rhs: Self) -> Self::Output {
344|        Quantity::new(self.value - rhs.value)
345|    }
346|}
347|
348|impl<T, U> Neg for Quantity<T, U>
349|where
350|    T: Neg,
351|{
352|    type Output = Quantity<<T as Neg>::Output, U>;
353|    fn neg(self) -> Self::Output {
354|        Quantity::new(-self.value)
355|    }
356|}
357|
358|// --- Multiplication / division (combine units) ---
359|
360|impl<T, M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
361|    Mul<Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
362|    for Quantity<T, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
363|where
364|    T: Mul,
365|    M1: Add<M2>,
366|    Kg1: Add<Kg2>,
367|    S1: Add<S2>,
368|    A1: Add<A2>,
369|    K1: Add<K2>,
370|    Mol1: Add<Mol2>,
371|    Cd1: Add<Cd2>,
372|{
373|    type Output = Quantity<
374|        <T as Mul>::Output,
375|        Unit<(
376|            Sum<M1, M2>,
377|            Sum<Kg1, Kg2>,
378|            Sum<S1, S2>,
379|            Sum<A1, A2>,
380|            Sum<K1, K2>,
381|            Sum<Mol1, Mol2>,
382|            Sum<Cd1, Cd2>,
383|        )>,
384|    >;
385|    fn mul(self, rhs: Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
386|        Quantity::new(self.value * rhs.value)
387|    }
388|}
389|
390|impl<T, M1, Kg1, S1, A1, K1, Mol1, Cd1, M2, Kg2, S2, A2, K2, Mol2, Cd2>
391|    Div<Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>>
392|    for Quantity<T, Unit<(M1, Kg1, S1, A1, K1, Mol1, Cd1)>>
393|where
394|    T: Div,
395|    M1: Sub<M2>,
396|    Kg1: Sub<Kg2>,
397|    S1: Sub<S2>,
398|    A1: Sub<A2>,
399|    K1: Sub<K2>,
400|    Mol1: Sub<Mol2>,
401|    Cd1: Sub<Cd2>,
402|{
403|    type Output = Quantity<
404|        <T as Div>::Output,
405|        Unit<(
406|            Diff<M1, M2>,
407|            Diff<Kg1, Kg2>,
408|            Diff<S1, S2>,
409|            Diff<A1, A2>,
410|            Diff<K1, K2>,
411|            Diff<Mol1, Mol2>,
412|            Diff<Cd1, Cd2>,
413|        )>,
414|    >;
415|    fn div(self, rhs: Quantity<T, Unit<(M2, Kg2, S2, A2, K2, Mol2, Cd2)>>) -> Self::Output {
416|        Quantity::new(self.value / rhs.value)
417|    }
418|}
419|
420|// --- Scalar multiplication / division ---
421|
422|impl<T, U> Mul<T> for Quantity<T, U>
423|where
424|    T: Mul<Output = T>,
425|{
426|    type Output = Quantity<T, U>;
427|    fn mul(self, rhs: T) -> Self::Output {
428|        Quantity::new(self.value * rhs)
429|    }
430|}
431|
432|impl<T, U> Div<T> for Quantity<T, U>
433|where
434|    T: Div<Output = T>,
435|{
436|    type Output = Quantity<T, U>;
437|    fn div(self, rhs: T) -> Self::Output {
438|        Quantity::new(self.value / rhs)
439|    }
440|}
441|
442|// --- Display ---
443|
444|/// Convert a typenum integer type to its `i8` value for rendering.
445|/// Implemented for the common exponent range used in physical models.
446|pub trait ToI8 {
447|    const VALUE: i8;
448|}
449|
450|impl ToI8 for Z0 {
451|    const VALUE: i8 = 0;
452|}
453|macro_rules! impl_to_i8 {
454|    ($($ty:ty => $val:expr),* $(,)?) => {
455|        $(impl ToI8 for $ty { const VALUE: i8 = $val; })*
456|    };
457|}
458|
459|impl_to_i8! {
460|    P1 => 1, P2 => 2, P3 => 3, P4 => 4, P5 => 5,
461|    P6 => 6, P7 => 7, P8 => 8, P9 => 9,
462|    N1 => -1, N2 => -2, N3 => -3, N4 => -4, N5 => -5,
463|    N6 => -6, N7 => -7, N8 => -8, N9 => -9,
464|}
465|
466|const UNIT_NAMES: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];
467|
468|impl<T, M, Kg, S, A, K, Mol, Cd> fmt::Display for Quantity<T, Unit<(M, Kg, S, A, K, Mol, Cd)>>
469|where
470|    T: fmt::Display,
471|    M: ToI8,
472|    Kg: ToI8,
473|    S: ToI8,
474|    A: ToI8,
475|    K: ToI8,
476|    Mol: ToI8,
477|    Cd: ToI8,
478|{
479|    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
480|        let exponents = [
481|            M::VALUE,
482|            Kg::VALUE,
483|            S::VALUE,
484|            A::VALUE,
485|            K::VALUE,
486|            Mol::VALUE,
487|            Cd::VALUE,
488|        ];
489|        write!(f, "{} {}", self.value, format_unit(&exponents))?;
490|        Ok(())
491|    }
492|}
493|
494|fn format_unit(exponents: &[i8; 7]) -> String {
495|    let mut num = String::new();
496|    let mut den = String::new();
497|
498|    for (i, &name) in UNIT_NAMES.iter().enumerate() {
499|        let exp = exponents[i];
500|        if exp == 0 {
501|            continue;
502|        }
503|        if exp > 0 {
504|            if !num.is_empty() {
505|                num.push('·');
506|            }
507|            num.push_str(name);
508|            if exp != 1 {
509|                num.push_str(&format_superscript(exp));
510|            }
511|        } else {
512|            if !den.is_empty() {
513|                den.push('·');
514|            }
515|            den.push_str(name);
516|            if exp != -1 {
517|                den.push_str(&format_superscript(-exp));
518|            }
519|        }
520|    }
521|
522|    if num.is_empty() && den.is_empty() {
523|        return "(dimensionless)".to_string();
524|    }
525|    if den.is_empty() {
526|        return num;
527|    }
528|    if num.is_empty() {
529|        // Lone inverse unit: render with explicit negative exponents.
530|        let mut inv = String::new();
531|        for (i, &name) in UNIT_NAMES.iter().enumerate() {
532|            let exp = exponents[i];
533|            if exp == 0 {
534|                continue;
535|            }
536|            if !inv.is_empty() {
537|                inv.push('·');
538|            }
539|            inv.push_str(name);
540|            inv.push_str(&format_superscript(exp));
541|        }
542|        return inv;
543|    }
544|    format!("{}/{}", num, den)
545|}
546|
547|fn format_superscript(exp: i8) -> String {
548|    match exp {
549|        -1 => "⁻¹".to_string(),
550|        -2 => "⁻²".to_string(),
551|        -3 => "⁻³".to_string(),
552|        -4 => "⁻⁴".to_string(),
553|        -5 => "⁻⁵".to_string(),
554|        -6 => "⁻⁶".to_string(),
555|        -7 => "⁻⁷".to_string(),
556|        -8 => "⁻⁸".to_string(),
557|        -9 => "⁻⁹".to_string(),
558|        1 => "¹".to_string(),
559|        2 => "²".to_string(),
560|        3 => "³".to_string(),
561|        4 => "⁴".to_string(),
562|        5 => "⁵".to_string(),
563|        6 => "⁶".to_string(),
564|        7 => "⁷".to_string(),
565|        8 => "⁸".to_string(),
566|        9 => "⁹".to_string(),
567|        _ => format!("^{}", exp),
568|    }
569|}
570|
571|// --- Vector-quantity newtypes ---
572|//
573|// nalgebra's `Vector3<S>` requires `S: Scalar` (which implies `One` and `Zero`),
574|// so a `Vector3<Acceleration<f64>>` cannot compile. To give force-aggregator
575|// models a unit-aware public API without breaking nalgebra geometry operations,
576|// each vector quantity is exposed as a thin newtype around `Vector3<f64>` with
577|// a raw escape hatch and per-component accessors returning the corresponding
578|// `Quantity<T, U>`. This mirrors the pattern established for `MagneticFieldVector`
579|// in `apogee-core::magnetosphere`.
580|
581|use nalgebra::Vector3;
582|
583|/// Acceleration vector in m/s². Wraps a raw `Vector3<f64>`; the units live in
584|/// the accessors.
585|#[derive(Debug, Clone, Copy, PartialEq, Default)]
586|pub struct AccelerationVec(pub Vector3<f64>);
587|
588|impl AccelerationVec {
589|    /// Wrap a raw m/s² vector.
590|    #[must_use]
591|    pub const fn from_mps2(raw: Vector3<f64>) -> Self {
592|        Self(raw)
593|    }
594|
595|    /// Borrow the raw vector in m/s².
596|    #[must_use]
597|    pub const fn raw(&self) -> &Vector3<f64> {
598|        &self.0
599|    }
600|
601|    /// Sum two acceleration vectors component-wise.
602|    #[must_use]
603|    pub fn plus(&self, other: &Self) -> Self {
604|        Self(self.0 + other.0)
605|    }
606|
607|    /// X component in m/s².
608|    #[must_use]
609|    pub fn x_mps2(&self) -> Acceleration<f64> {
610|        Acceleration::new(self.0.x)
611|    }
612|
613|    /// Y component in m/s².
614|    #[must_use]
615|    pub fn y_mps2(&self) -> Acceleration<f64> {
616|        Acceleration::new(self.0.y)
617|    }
618|
619|    /// Z component in m/s².
620|    #[must_use]
621|    pub fn z_mps2(&self) -> Acceleration<f64> {
622|        Acceleration::new(self.0.z)
623|    }
624|}
625|
626|impl From<Vector3<f64>> for AccelerationVec {
627|    fn from(raw: Vector3<f64>) -> Self {
628|        Self(raw)
629|    }
630|}
631|
632|impl From<AccelerationVec> for Vector3<f64> {
633|    fn from(v: AccelerationVec) -> Self {
634|        v.0
635|    }
636|}
637|
638|/// Force vector in N. Wraps a raw `Vector3<f64>`.
639|#[derive(Debug, Clone, Copy, PartialEq, Default)]
640|pub struct ForceVec(pub Vector3<f64>);
641|
642|impl ForceVec {
643|    /// Wrap a raw N vector.
644|    #[must_use]
645|    pub const fn from_n(raw: Vector3<f64>) -> Self {
646|        Self(raw)
647|    }
648|
649|    /// Borrow the raw vector in N.
650|    #[must_use]
651|    pub const fn raw(&self) -> &Vector3<f64> {
652|        &self.0
653|    }
654|
655|    /// X component in N.
656|    #[must_use]
657|    pub fn x_n(&self) -> Force<f64> {
658|        Force::new(self.0.x)
659|    }
660|
661|    /// Y component in N.
662|    #[must_use]
663|    pub fn y_n(&self) -> Force<f64> {
664|        Force::new(self.0.y)
665|    }
666|
667|    /// Z component in N.
668|    #[must_use]
669|    pub fn z_n(&self) -> Force<f64> {
670|        Force::new(self.0.z)
671|    }
672|}
673|
674|impl From<Vector3<f64>> for ForceVec {
675|    fn from(raw: Vector3<f64>) -> Self {
676|        Self(raw)
677|    }
678|}
679|
680|impl From<ForceVec> for Vector3<f64> {
681|    fn from(v: ForceVec) -> Self {
682|        v.0
683|    }
684|}
685|
686|/// Torque vector in N·m. Wraps a raw `Vector3<f64>`.
687|#[derive(Debug, Clone, Copy, PartialEq, Default)]
688|pub struct TorqueVec(pub Vector3<f64>);
689|
690|impl TorqueVec {
691|    /// Wrap a raw N·m vector.
692|    #[must_use]
693|    pub const fn from_nm(raw: Vector3<f64>) -> Self {
694|        Self(raw)
695|    }
696|
697|    /// Borrow the raw vector in N·m.
698|    #[must_use]
699|    pub const fn raw(&self) -> &Vector3<f64> {
700|        &self.0
701|    }
702|
703|    /// X component in N·m.
704|    #[must_use]
705|    pub fn x_nm(&self) -> Torque<f64> {
706|        Torque::new(self.0.x)
707|    }
708|
709|    /// Y component in N·m.
710|    #[must_use]
711|    pub fn y_nm(&self) -> Torque<f64> {
712|        Torque::new(self.0.y)
713|    }
714|
715|    /// Z component in N·m.
716|    #[must_use]
717|    pub fn z_nm(&self) -> Torque<f64> {
718|        Torque::new(self.0.z)
719|    }
720|}
721|
722|impl From<Vector3<f64>> for TorqueVec {
723|    fn from(raw: Vector3<f64>) -> Self {
724|        Self(raw)
725|    }
726|}
727|
728|impl From<TorqueVec> for Vector3<f64> {
729|    fn from(v: TorqueVec) -> Self {
730|        v.0
731|    }
732|}
733|
734|#[cfg(test)]
735|mod tests {
736|    use super::*;
737|    use approx::assert_relative_eq;
738|
739|    #[test]
740|    fn base_units_wrap_values() {
741|        let m: Meters<f64> = Meters::new(5.0);
742|        let kg: Kilograms<f64> = Kilograms::new(2.0);
743|        let s: Seconds<f64> = Seconds::new(3.0);
744|        assert_eq!(m.into_value(), 5.0);
745|        assert_eq!(kg.into_value(), 2.0);
746|        assert_eq!(s.into_value(), 3.0);
747|    }
748|
749|    #[test]
750|    fn addition_requires_same_unit() {
751|        let a = Meters::new(3.0);
752|        let b = Meters::new(4.0);
753|        assert_eq!((a + b).into_value(), 7.0);
754|    }
755|
756|    #[test]
757|    fn subtraction_requires_same_unit() {
758|        let a = Seconds::new(10.0);
759|        let b = Seconds::new(3.0);
760|        assert_eq!((a - b).into_value(), 7.0);
761|    }
762|
763|    #[test]
764|    fn negation_preserves_unit() {
765|        let v = Velocity::new(5.0);
766|        assert_eq!((-v).into_value(), -5.0);
767|    }
768|
769|    #[test]
770|    fn multiplication_combines_units() {
771|        let v = Velocity::new(10.0); // m/s
772|        let t = Seconds::new(2.0); // s
773|        let d: Meters<f64> = v * t;
774|        assert_eq!(d.into_value(), 20.0);
775|    }
776|
777|    #[test]
778|    fn division_combines_units() {
779|        let d = Meters::new(100.0);
780|        let t = Seconds::new(10.0);
781|        let v: Velocity<f64> = d / t;
782|        assert_eq!(v.into_value(), 10.0);
783|    }
784|
785|    #[test]
786|    fn scalar_multiplication_preserves_unit() {
787|        let f = Force::new(5.0);
788|        let scaled = f * 2.0;
789|        assert_eq!(scaled.into_value(), 10.0);
790|    }
791|
792|    #[test]
793|    fn scalar_division_preserves_unit() {
794|        let p = Pressure::new(10.0);
795|        let halved = p / 2.0;
796|        assert_eq!(halved.into_value(), 5.0);
797|    }
798|
799|    #[test]
800|    fn derived_units_from_base() {
801|        let m = Meters::new(10.0);
802|        let s = Seconds::new(2.0);
803|        let a: Acceleration<f64> = m / (s * s);
804|        assert_eq!(a.into_value(), 2.5);
805|    }
806|
807|    #[test]
808|    fn sqrt_of_area_is_length() {
809|        let area = Area::new(16.0);
810|        let side: Meters<f64> = area.sqrt();
811|        assert_relative_eq!(side.into_value(), 4.0, epsilon = 1e-12);
812|    }
813|
814|    #[test]
815|    fn display_renders_base_unit() {
816|        let m = Meters::new(5.0);
817|        assert_eq!(format!("{}", m), "5 m");
818|    }
819|
820|    #[test]
821|    fn display_renders_derived_unit() {
822|        let a = Acceleration::new(9.81);
823|        assert_eq!(format!("{}", a), "9.81 m/s²");
824|    }
825|
826|    #[test]
827|    fn display_renders_inverse_unit() {
828|        let f = Frequency::new(60.0);
829|        assert_eq!(format!("{}", f), "60 s⁻¹");
830|    }
831|
832|    #[test]
833|    fn display_renders_dimensionless() {
834|        let d = Dimensionless::new(0.5);
835|        assert_eq!(format!("{}", d), "0.5 (dimensionless)");
836|    }
837|
838|    #[test]
839|    fn display_renders_complex_derived_unit() {
840|        // Newton = m·kg/s²
841|        let n = Force::new(1.0);
842|        assert_eq!(format!("{}", n), "1 m·kg/s²");
843|    }
844|
845|    #[test]
846|    fn display_renders_power_unit() {
847|        let p = Power::new(100.0);
848|        assert_eq!(format!("{}", p), "100 m²·kg/s³");
849|    }
850|
851|852|    // SiPrefix tests.
853|    #[test]
854|    fn si_prefix_scales_match_si_definitions() {
855|        assert_eq!(SiPrefix::Yocto.scale(), 1.0e-24);
856|        assert_eq!(SiPrefix::Milli.scale(), 1.0e-3);
857|        assert_eq!(SiPrefix::None.scale(), 1.0);
858|        assert_eq!(SiPrefix::Kilo.scale(), 1.0e3);
859|        assert_eq!(SiPrefix::Mega.scale(), 1.0e6);
860|        assert_eq!(SiPrefix::Giga.scale(), 1.0e9);
861|        assert_eq!(SiPrefix::Yotta.scale(), 1.0e24);
862|    }
863|
864|    #[test]
865|    fn si_prefix_scales_table_matches_individual_scale_method() {
866|        for (idx, &scale) in SiPrefix::SCALES.iter().enumerate() {
867|            // Round-trip: each entry in the table is the scale of the
868|            // corresponding SiPrefix variant.
869|            let prefix = match idx {
870|                0 => SiPrefix::Yocto,
871|                1 => SiPrefix::Zepto,
872|                2 => SiPrefix::Atto,
873|                3 => SiPrefix::Femto,
874|                4 => SiPrefix::Pico,
875|                5 => SiPrefix::Nano,
876|                6 => SiPrefix::Micro,
877|                7 => SiPrefix::Milli,
878|                8 => SiPrefix::Centi,
879|                9 => SiPrefix::Deci,
880|                10 => SiPrefix::None,
881|                11 => SiPrefix::Deca,
882|                12 => SiPrefix::Hecto,
883|                13 => SiPrefix::Kilo,
884|                14 => SiPrefix::Mega,
885|                15 => SiPrefix::Giga,
886|                16 => SiPrefix::Tera,
887|                17 => SiPrefix::Peta,
888|                18 => SiPrefix::Exa,
889|                19 => SiPrefix::Zetta,
890|                20 => SiPrefix::Yotta,
891|                _ => unreachable!(),
892|            };
893|            assert_eq!(prefix.scale(), scale, "mismatch at index {idx}");
894|        }
895|    }
896|
897|    #[test]
898|    fn si_prefix_display_uses_official_abbreviation() {
899|        assert_eq!(format!("{}", SiPrefix::Kilo), "k");
900|        assert_eq!(format!("{}", SiPrefix::Micro), "µ");
901|        assert_eq!(format!("{}", SiPrefix::Mega), "M");
902|        assert_eq!(format!("{}", SiPrefix::None), "");
903|    }
904|
905|    #[test]
906|    fn with_prefix_multiplies_value() {
907|        // 1 m scaled by Kilo = 1000 m. Same Rust type, value reflects the
908|        // applied scale.
909|        let one_m: Meters<f64> = Meters::new(1.0);
910|        let in_kilo: Meters<f64> = one_m.with_prefix(SiPrefix::Kilo);
911|        assert_eq!(in_kilo.into_value(), 1000.0);
912|    }
913|
914|    #[test]
915|    fn strip_prefix_divides_value() {
916|        // Inverse of with_prefix: 1000.0 m / Kilo (1e3) = 1.0 m.
917|        let in_kilo: Meters<f64> = Meters::new(1_000.0);
918|        let in_meters: Meters<f64> = in_kilo.strip_prefix(SiPrefix::Kilo);
919|        assert_eq!(in_meters.into_value(), 1.0);
920|    }
921|
922|    #[test]
923|    fn with_prefix_round_trip_returns_original_value() {
924|        let original: Meters<f64> = Meters::new(42.0);
925|        let in_mm: Meters<f64> = original.with_prefix(SiPrefix::Milli);
926|        let back: Meters<f64> = in_mm.strip_prefix(SiPrefix::Milli);
927|        assert_eq!(back.into_value(), 42.0);
928|    }
929|
930|    #[test]
931|    fn with_prefix_supports_full_si_ladder() {
932|        // Spot-check several SI prefixes around the ladder.
933|        let one: Meters<f64> = Meters::new(1.0);
934|        assert_eq!(one.with_prefix(SiPrefix::Micro).into_value(), 1.0e-6);
935|        assert_eq!(one.with_prefix(SiPrefix::Milli).into_value(), 1.0e-3);
936|        assert_eq!(one.with_prefix(SiPrefix::Centi).into_value(), 1.0e-2);
937|        assert_eq!(one.with_prefix(SiPrefix::Deci).into_value(), 1.0e-1);
938|        assert_eq!(one.with_prefix(SiPrefix::Kilo).into_value(), 1.0e3);
939|        assert_eq!(one.with_prefix(SiPrefix::Mega).into_value(), 1.0e6);
940|

941|    #[test]
942|    fn acceleration_vec_wraps_and_exposes_components() {
943|        let raw = Vector3::new(1.0, 2.0, 3.0);
944|        let a = AccelerationVec::from_mps2(raw);
945|        assert_eq!(a.raw(), &raw);
946|        assert_eq!(a.x_mps2().into_value(), 1.0);
947|        assert_eq!(a.y_mps2().into_value(), 2.0);
948|        assert_eq!(a.z_mps2().into_value(), 3.0);
949|    }
950|
951|    #[test]
952|    fn acceleration_vec_plus_sums_components() {
953|        let a = AccelerationVec::from_mps2(Vector3::new(1.0, 0.0, 0.0));
954|        let b = AccelerationVec::from_mps2(Vector3::new(0.0, 2.0, 3.0));
955|        let sum = a.plus(&b);
956|        assert_eq!(sum.raw(), &Vector3::new(1.0, 2.0, 3.0));
957|    }
958|
959|    #[test]
960|    fn force_vec_round_trips_via_from_into() {
961|        let raw = Vector3::new(4.0, 5.0, 6.0);
962|        let f: ForceVec = raw.into();
963|        let back: Vector3<f64> = f.into();
964|        assert_eq!(back, raw);
965|    }
966|
967|    #[test]
968|    fn torque_vec_exposes_nm_components() {
969|        let t = TorqueVec::from_nm(Vector3::new(7.0, 8.0, 9.0));
970|        assert_relative_eq!(t.x_nm().into_value(), 7.0);
971|        assert_relative_eq!(t.y_nm().into_value(), 8.0);
972|        assert_relative_eq!(t.z_nm().into_value(), 9.0);
973|    }
974|
975|    #[test]
976|    fn torque_type_unit_is_kg_m2_per_s2() {
977|        // Torque = m²·kg/s² (force-arm). Multiply a length-arm by a force and
978|        // confirm the resulting type is `Torque`.
979|        let arm: Meters<f64> = Meters::new(0.5);
980|        let force: Force<f64> = Force::new(10.0);
981|        let _t: Torque<f64> = arm * force;
982|983|    }
984|}
985|