# IGRF-13 geomagnetic main field — primary sources

Main geomagnetic field as a spherical-harmonic potential expansion in
Schmidt quasi-normalized associated Legendre functions, with Gauss
coefficients from the 13th-generation IGRF (epoch 2020.0 + predictive
secular variation), plus a local Ap-driven disturbance proxy.

**Apogee code**

- `crates/apogee-core/src/magnetosphere/igrf.rs` — `field()` /
  `field_with_ap()` / `field_with_coeffs()` (the expansion), tests.
- `crates/apogee-core/src/magnetosphere/legendre.rs` —
  `schmidt_legendre()` recursion.
- `crates/apogee-core/src/magnetosphere/data.rs` — embedded coefficients.
- `crates/apogee-core/src/magnetosphere/disturbance.rs` — Ap perturbation
  (see Flags: internal placeholder).

## Primary sources

- **[A21]** Alken, P., E. Thebault, C. Beggan, et al. (2021),
  "International Geomagnetic Reference Field: the thirteenth generation",
  Earth, Planets and Space 73, 49, doi:10.1186/s40623-020-01288-x — open
  access (CC BY). Free PDF (White Rose mirror):
  https://eprints.whiterose.ac.uk/id/eprint/165378/8/s40623-020-01288-x.pdf
  Committed: `sources/pdfs/alken2021_igrf13.pdf`.
  NOTE: the sometimes-cited DOI 10.1186/s40623-021-01472-8 does not exist.
- **[W05]** Winch, D.E., D.J. Ivers, J.P.R. Turner, R.J. Stening (2005),
  "Geomagnetism and Schmidt quasi-normalization", Geophys. J. Int. 160(2),
  487-504, doi:10.1111/j.1365-246X.2004.02472.x — **not** Computers &
  Geosciences 31 (common mis-citation). Cloudflare-gated at academic.oup.com;
  equations verified via extraction backend, file not saved.
- **[NOAA]** NOAA NCEI / IAGA Working Group V-MOD: IGRF-13 coefficient file
  `igrf13coeffs.txt` and Fortran reference synthesis `igrf13.f` —
  https://www.ngdc.noaa.gov/IAGA/vmod/coeffs/igrf13coeffs.txt and
  https://www.ngdc.noaa.gov/IAGA/vmod/igrf13.f
  Committed: `sources/igrf13coeffs.txt` (verified: all 390 embedded values
  match the official file's 2020.0 and 2020-25 SV columns exactly — zero
  mismatches).
- **[S65]** Sugiura, M. (1965), "Hourly values of equatorial Dst for the
  IGY", NASA GSFC (NTRS 19650020355) —
  https://ntrs.nasa.gov/citations/19650020355
  Committed: `sources/pdfs/sugiura1965_dst.pdf`.
- **[SWPC]** NOAA SWPC, "Station K and A indices" (Kp/Ap definitions) —
  https://www.swpc.noaa.gov/products/station-k-and-indices (now redirects
  to spaceweather.gov).
- Context (not fetched): Chapman & Bartels (1940) "Geomagnetism" Vol. II
  (only Vol. I exists on archive.org); Mayaud (1980) AGU Monograph 22;
  Blakely (1995) "Potential Theory in Gravity and Magnetic Applications"
  CUP Section 4.2 (dipole closed form used by the tests).

## Equations and constants

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| igrf.rs:79-146 `field_with_coeffs` | geomagnetic potential expansion | A21 | Eq. (1), PDF p. 2 | "V(r,theta,phi,t) = a SUM_n SUM_m (a/r)^(n+1) [g_n^m(t) cos(m phi) + h_n^m(t) sin(m phi)] P_n^m(cos theta) (1)" | OK |
| :85-96 + doc | geocentric coordinates (geodetic input converted upstream) | A21 | Mathematical formulation p. 2; Table 2 caption p. 15 | "r, theta, phi refer to coordinates in a geocentric spherical coordinate system ... software to compute magnetic field components ... in both geocentric and geodetic coordinate systems" | OK |
| data.rs:6 + igrf.rs:93 | reference radius a = 6371.2 km | A21 + igrf13.f | Mathematical formulation p. 2; program header | "A reference radius a = 6371.2 km is chosen to approximate the mean Earth radius"; "Reference radius remains as 6371.2 km - it is NOT the mean radius (= 6371.0 km)" | OK |
| legendre.rs whole | Schmidt semi-normalized ALFs (vs geodesy's full normalization) | A21 -> W05 | A21 p. 2; W05 Eqs. (3.1)-(3.2), pp. 489-490 | "The P_n^m(cos theta) are Schmidt semi-normalized associated Legendre functions of degree n and order m (Winch et al. 2005)."; "P_m_n(mu) = sqrt[2^(1-delta_0m) (n-m)!/(n+m)!] P_n,m(mu) (3.2)" | OK |
| legendre.rs:24-96 `schmidt_legendre` | diagonal / sub-diagonal / vertical recurrences | igrf13.f | SYNTH subroutine (~ll. 1095-1121) | "one = sqrt(1.0 - 0.5/fm); p(k) = one*st*p(j) ... one = sqrt(fn*fn - gmm); two = sqrt(gn*gn - gmm)/one; three = (fn + gn)/one; p(k) = three*ct*p(i) - two*p(j)" — matches the Rust factors sqrt((2m-1)/(2m)) and (2n-1)/sqrt((n-m)(n+m)) | OK |
| data.rs:8-437 | coefficient table provenance (epoch 2020.0 main field deg 1-13; SV 2020-25 deg 1-8) | A21 + NOAA | A21 Table 1 (generations) p. 2, Table 2 p. 8; file header | "IGRF-13 provides a DGRF model for epoch 2015.0, an IGRF model for epoch 2020.0, and a predictive IGRF secular variation model for the 5-year time interval 2020.0 to 2025.0."; "# 13th Generation International Geomagnetic Reference Field Schmidt semi-normalised spherical harmonic coefficients, degree n=1,13" | OK |
| data.rs:440-458 `coefficients_at_epoch` | time dependence: linear from the 2020.0 epoch using SV rates | A21 | Eqs. (2)-(5), p. 2 + Table 2 caption p. 14 | "g_n^m(t) = g_n^m(T_t) + (t - T_t) gdot_n^m(T_t) (2) ... The model epochs ... starting in 1900 and ending in 2020 ... for the final 5 years ... the coefficients gdot_n^m(2020), hdot_n^m(2020) are explicitly provided ... in units of nT/year" | OK |
| data.rs / igrf.rs | IGRF_DEGREE = 13 | NOAA + A21 | host page + Table 2 | "N specifies the maximum spherical harmonic degree of expansion ... chosen to be 10 up to and including epoch 1995, after which it increases to 13" | OK |
| disturbance.rs:25-27 | Ap index input convention | SWPC | product page | "The A indices ... are daily indices obtained by converting the eight 3-hourly K indices" | OK |
| disturbance.rs:26-27 | Dst concept (ring-current storm depression) | S65 | title + abstract | "Hourly values of equatorial Dst for the IGY ... the hourly values of Dst are determined from the world-wide chain of observatories" | OK |
| disturbance.rs:40, 55-76 | AP_TO_G10_PERTURBATION = -2.0 nT per Ap unit on g_1^0 | none | — | internal first-order proxy for the ring-current/Dst depression of the low-latitude field; **no primary source exists** | UNV |

## Flags

- **Phantom citations in `legendre.rs:8-12`:** "Lowes, F. J. & Winch, D. E.
  (1991), Differentiation of associated Legendre functions" — no such
  publication could be found in any search; and "IAGA VMOD technical note 6"
  — the V-MOD page hosts no technical notes. The implemented recurrences
  actually match `igrf13.f` (verified line-by-line). These comments should
  be replaced by [A21]/[W05]/[igrf13.f] citations.
- **Broken URL in `igrf.rs:249`:** github.com/dawiggs/ppigrf returns 404.
  The real test-fixture generator is ppigrf (Karl M. Laundal, MIT) at
  https://github.com/IAGA-VMOD/ppigrf.
- **Time-interpolation simplification:** `coefficients_at_epoch` clamps to
  [2015, 2030] and linearly extrapolates from the 2020.0 epoch with the
  2020-25 SV rates ([A21] Eq. (2)/(3) with T_t = 2020). It does **not**
  implement the full 1900-2025 piecewise interpolation between historical
  5-year epochs (Eqs. (4)/(5)); historical dates are silently approximated
  and requests beyond 2025 extrapolate past official validity (Table 1).
  Deliberate simplification — documented here.
- **`disturbance.rs` is an internal placeholder**, not a published
  storm-time model: the -2.0 nT/Ap scaling of g_1^0 has no primary source.
  A realistic disturbance model (Dst-coupled, Tsyganenko-class) is deferred
  (per the code's own comments).
- The SWPC page moved: www.swpc.noaa.gov -> www.spaceweather.gov (the
  disturbance.rs comment URL still works via redirect).
- Winch (2005) quotes were extracted from the OUP PDF via the extraction
  backend (Cloudflare blocks direct download); the file could not be saved
  locally for this folder.