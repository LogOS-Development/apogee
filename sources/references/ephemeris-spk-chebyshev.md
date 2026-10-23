# SPICE SPK ephemeris kernel reader — primary sources

DAF-based SPK binary kernel reader: file record + summary records + segment
descriptors, Chebyshev evaluation (Clenshaw), types 2 (position-only),
3 (position + velocity), and 13 (Hermite, unequal steps), km -> m conversion.

**Apogee code**

- `crates/apogee-core/src/ephemeris/chebyshev.rs` — Clenshaw evaluation +
  derivative-coefficient recurrence.
- `crates/apogee-core/src/ephemeris/kernel.rs` — DAF parsing + type 2/3/13
  state reconstruction + `state_at` dispatch.
- `crates/apogee-core/src/ephemeris/service.rs`, `cache.rs` —
  `EphemerisService`, `epoch_to_et` (TDB seconds past J2000).

## Primary sources

- **[SPK]** NAIF, "SPK Required Reading", SPICE Toolkit (C edition) —
  https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html
  (HTML edition has **no numbered equations/figures** — layouts are
  unnumbered text diagrams inside named sections).
- **[DAF]** NAIF, "DAF Required Reading" —
  https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/daf.html
- **[SPKE02]/[SPKE03]** NAIF SPICELIB routine headers —
  https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/spicelib/spke02.html
  and .../spke03.html
- **[CHBINT]/[CHBVAL]** NAIF SPICELIB routine headers + source —
  https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/spicelib/chbint.html
  and .../chbval.html
- **[SPKR02]/[SPKR03]/[SPKW03]** NAIF SPICELIB source (f2c translation,
  fetched from the arturania/cspice GitHub mirror of CSPICE; content agrees
  with the official NAIF doc pages).
- **[TUT]** NAIF, "SPK" tutorial (18_spk.pdf), slide 40 —
  https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/Tutorials/pdf/individual_docs/18_spk.pdf
- **[ACT96]** Acton, C.H. (1996), "Ancillary data services of NASA's
  Navigation and Ancillary Information Facility", Planet. Space Sci.
  44(1), 65-67, doi:10.1016/0032-0633(95)00107-7 (freely retrievable via DOI).
- **[OLI77]** Oliver, J. (1977), "An error analysis of the modified Clenshaw
  method for evaluating Chebyshev and Fourier series", IMA J. Appl. Math.
  20(3), 379-392, doi:10.1093/imamat/20.3.379 — restates the Clenshaw 1955
  recurrence (retrievable via DOI).
- Not fetchable: Clenshaw (1955) Math. Tables Aids Comput. 9(51) (AMS free
  archive returns an HTML interstitial to automated clients); Fox & Parker
  (1968) "Chebyshev Polynomials in Numerical Analysis" Ch. 4 (print book —
  the derivative-coefficient recurrence cited from it is `UNV`); Newhall
  (1989) JPL IOM 89-032 (internal memo, not online).

## Equations and layouts

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| kernel.rs:639-772 | DAF file record (IDWORD, ND, NI, LOCIFN, FWARD, BWARD, FREE) | DAF | "File Records" items 1-7 | "LOCIDW ( 8 characters, 8 bytes): An identification word ('DAF/xxxx') ... 5. FWARD: The record number of the initial summary record ... 7. FREE: The first free address [84]" | OK |
| :70-96 `detect_endianness` | LOCFMT binary-format word | DAF | "File Records" item 8 [Address 88] | "LOCFMT ... The string has value either 'LTL-IEEE' or 'BIG-IEEE.'" | OK |
| :773-884 `parse_segments` | summary-record control words (read as doubles) | DAF | "Summary Records" items 1-3 | "The first three words of each summary record are reserved for: 1. The record number of the next summary record ... 3. The number of summaries stored in this record." | OK |
| :98-130 descriptor | SPK segment descriptor (2 doubles + 6 ints) | SPK | "Segments--The Fundamental SPK Building Blocks" | "The summary ... has two double precision components: 1. The initial epoch ... 2. The final epoch ... six integer components: 1. The NAIF integer code for the target ... 4. The integer code for the representation (type of ephemeris data) ... 6. The final address of the array." | OK |
| :407-453 `state_at_type2` | type-2 record layout (MID, RADIUS, X/Y/Z blocks) | SPK | "Type 2: Chebyshev (position only)" | "The first two elements in the record, MID and RADIUS, are the midpoint and radius of the time interval ... transformations between the domain of the record ... and the domain of Chebyshev polynomials (from -1 to 1)." | OK |
| :407-453 | type-2 degree from RSIZE | SPK | "Type 2" degree formula | "the degree of each polynomial is ( RSIZE - 2 ) / 3 - 1" | OK |
| :407-453 | type-2 units | SPKE02 | Detailed_Input notes | "In the above record - Times are expressed as seconds past J2000 TDB. - Position components have units of km." | OK |
| :407-453 | velocity by differentiating the position series | SPK + SPKE02 | "Supported Data Types" item 2 + Particulars | "Chebyshev polynomials (position only) ... The velocity of the body is obtained by differentiation."; "SPKE02 calls the routine CHBINT for each set to evaluate the polynomial AND its first derivative (which it computes internally)" | OK |
| :407-453 | derivative chain-rule factor 1/radius | CHBINT | Detailed_Output (DPDX) | "DPDX is the value of the derivative of the polynomial at X. It is given by 1/X2S(2) [ CP(DEGP+1)*T'(DEGP,S) + ... + CP(2)*T'(1,S) ]" | OK |
| :454-507 `state_at_type3` | type-3 layout + degree | SPK | "Type 3: Chebyshev (position and velocity)" | "The only difference is that each logical record contains six sets of coefficients instead of three ... the degree of each polynomial is ( RSIZE - 2 ) / 6 - 1" | OK |
| :454-507 | type-3 units | SPKE03 | Detailed_Input notes | "Velocity components have units of km/s" | OK |
| :454-507 | **type-3 velocity: SPICE evaluates the velocity series directly (no radius scaling)** | SPKE03 + SPKW03 + CHBVAL | SPKE03 Particulars; SPKW03 CDATA list; CHBVAL source | "SPKE03 calls the routine CHBVAL to evaluate each polynomial"; SPKW03: "the coefficients for the derivative with respect to time of the first coordinate"; CHBVAL: "s = (\*x - x2s[0]) / x2s[1]; ... \*p = s\*w[0] - w[1] + cp[0];" (no division by the interval) | OK |
| chebyshev.rs:65-97 `chebyshev_eval` | Clenshaw backward recurrence | CHBVAL + OLI77 | CHBVAL source body; Oliver (1977) restating Clenshaw (1955) | "s = (\*x - x2s[0]) / x2s[1]; ... \*p = s\*w[0] - w[1] + cp[0];" | OK |
| chebyshev.rs:112-155 `chebyshev_derivative` | derivative-coefficient recurrence d_(k-1) = 2k c_k + d_(k+1) | Fox & Parker Ch. 4 (cited in code) | not fetchable | — | UNV |
| :508-637 `state_at_type13` | type-13 layout (6-double states + epochs, trailing directory) | SPK | "Type 13: Hermite Interpolation --- Unequal Time Steps" | "fitting a Hermite polynomial to the window's set of position and velocity values" (see Flags: our reader linearly interpolates instead) | OK |
| :285-330 `read_segment_directory` | directory words INIT/INTLEN/RSIZE/N at the segment end | SPKR02/SPKR03 | source (data array directory convention) | "the last four elements of the array ... contain the directory values" | OK |
| :249-284 `state_at` | implemented types 2 / 3 / 13, others rejected | SPK | "Supported Data Types" items 2, 3, 13 | "2. Chebyshev polynomials (position only) ... normally used for planet barycenters ... 3. Chebyshev polynomials (position and velocity)." | OK |
| service.rs:111-117 | time system: ET = TDB seconds past J2000 | SPK | Terminology + SPKE02 notes | "Times are expressed as seconds past J2000 TDB" | OK |
| :249-284 | km / km/s -> m / m/s | SPKE02/SPKE03 + TUT | unit notes; tutorial slide 40 | "[SPK] Position components have units of km ... Velocity components have units of km/s"; converted in `state_at` after evaluation (KM_TO_M) | OK |
| module docs | SPK kernel family uses Chebyshev data types | ACT96 | article text | "the SPK kernel family, which started with just three data types, currently consists of 16 data types, such as Chebyshev polynomials, traditional conic elements and NORAD's two-line elements." | OK |

## Flags

- **BUG (issue #205): type-3 velocity scaling.** `state_at_type3`
  (kernel.rs:454-507) multiplies the evaluated velocity series by
  `1/radius`; SPICE's SPKE03 evaluates all six coefficient sets with CHBVAL
  and applies **no** radius scaling, because the velocity sets already are
  derivatives with respect to time ([SPKW03]: "the coefficients for the
  derivative with respect to time"). Against a real type-3 segment our
  velocities are wrong by a factor of RADIUS (record half-interval in
  seconds). The round-trip tests hide this: the fixtures pre-multiply
  velocity samples by radius ("Convert physical velocity to normalized
  velocity before fitting"). Type 2 is correct (CHBINT's 1/X2S(2) chain-rule
  factor is exactly our 1/radius scaling of the *derivative of the position
  series*).
- `chebyshev_eval_3d` doc comment (chebyshev.rs:139-156) says "multiply by
  2.0 / radius" — inconsistent with the reader's own 1/radius convention
  and with the chain rule (d/dt = (1/radius) d/dx); the helper is unused by
  the reader path.
- LOCFMT: the parser accepts "LITTLE-IEEE" for little-endian, but the DAF
  spec (and real kernels, e.g. de421.bsp) store "LTL-IEEE" — real kernels
  currently fall into the byte-sniffing fallback. Harmless; should accept
  the spec string.
- Type 13: SPICE specifies sliding-window **Hermite** interpolation;
  apogee **linearly** interpolates between bracketing records (its own
  comment claims "sub-kilometre accuracy" for mission SPKs) — a documented
  simplification, but a divergence from the type-13 spec.
- Record-index edge case: SPICE clamps at segment end
  (`recno = min(recno,nrec)`); apogee errors when
  record_index >= n, so a request exactly at a segment's end epoch fails.
- apogee derives each record's MID/RADIUS from the directory
  (mid = INIT + record_index*INTLEN + INTLEN/2) instead of reading the
  stored MID/RADIUS words — numerically identical for conforming kernels
  (SPICE writers store exactly these values), but it trusts the directory
  over the record header.
- Quotes are reflowed from HTML/PDF-to-text conversion (whitespace
  normalized, words unchanged). The SPICELIB C sources quoted were fetched
  from the arturania/cspice GitHub mirror (an unofficial f2c translation);
  the official doc pages (spke02/spke03/chbint/chbval) were fetched directly
  from naif.jpl.nasa.gov and agree.