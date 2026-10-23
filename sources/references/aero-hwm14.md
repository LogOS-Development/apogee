# HWM14 — primary sources

Horizontal Wind Model 2014: empirical neutral wind fields (surface to
exobase) as height-modulated vector spherical harmonic expansions, plus a
3-hr-Ap-driven disturbance-wind component (DWM07). Vendored as Fortran and
wrapped via FFI behind the `hwm14` cargo feature.

**Apogee code**

- `crates/apogee-core/src/aero/hwm.rs` — FFI wrapper (`Hwm14::evaluate`,
  `WindInput`/`WindOutput`, unit conversions, `EVAL_LOCK`).
- `crates/apogee-core/external/hwm14/vendor/` — `hwm14.f90` (NRL),
  `hwm14_c.f90` (C-ABI wrapper), `hwm14_README.md`.
- `crates/apogee-core/external/hwm14/assets/` — `hwm123114.bin`,
  `dwm07b104i.dat`, `gd2qd.dat` (NRL coefficient files).
- `crates/apogee-core/build.rs` — gfortran compile behind the feature.

## Primary sources

- **[D15]** Drob, D.P., J.T. Emmert, J.W. Meriwether, J.J. Makela,
  E. Doornbos, M. Conde, G. Hernandez, J. Noto, K.A. Zawdie, S.E. McDonald,
  J.D. Huba, J.H. Klenzing (2015), "An update to the Horizontal Wind Model
  (HWM): The quiet time thermosphere", Earth and Space Science 2, 301-319,
  doi:10.1002/2014EA000089 — open access (CC BY-NC-ND); free PDF:
  https://map.nrl.navy.mil/map/pub/nrl/HWM/HWM14/HWM14_ess224.pdf
  (not committed: NC license).
- **[NRL]** NRL HWM14 distribution, version HWM14.123114 (release,
  31 Dec 2014), README.txt + `hwm14.f90` + the three coefficient files —
  https://map.nrl.navy.mil/map/pub/nrl/HWM/HWM14/ (README text fetched from
  the public mirror https://raw.githubusercontent.com/jacobwilliams/HWM14/master/README.txt,
  verified identical to the vendored `hwm14.f90` header).
- Context (not fetched): Drob et al. (2008) HWM07, doi:10.1029/2008JA013668;
  Emmert et al. (2008) DWM07, doi:10.1029/2008JA013541; Hedin et al. (1996)
  HWM93, J. Atmos. Terr. Phys. 58, 1421-1447.

## Equations and constants

All quotes `OK` except where marked.

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| hwm.rs:1-12 module doc | wind-field representation | D15 | Section 1, p. 301 | "The empirical model describes the atmosphere's vector wind fields from the surface to the exobase (~450 km) as a function of latitude, longitude, altitude, day of year, and time of day." | OK |
| hwm14.f90 `hwmqt` | quiet-time zonal wind U as height-modulated vector spherical harmonics | D15 | Eq. (1), Section 2.2, pp. 303-304 | "U(tau, delta, theta, phi, z) = SUM_j beta_j(z) u_j(tau, delta, theta, phi) (1), where beta_j(z) is the amplitude of the jth vertical cubic B-spline weighting kernel" | OK |
| hwm14.f90 `alfbasis` | horizontal expansion: zonal mean + migrating tides + stationary planetary waves | D15 | Eqs. (2)-(4), Section 2.2, pp. 304-305 | "annual and semiannual harmonics ... seasonal wave number s up to S = 2; ... tidal wave number l up to L = 3; ... longitudinal wave number m up to M = 2 ... The maximum order in latitude for all three sums is N = 8." | OK |
| hwm14.f90 `alfbasis` | V/W basis from associated Legendre polynomials; meridional via coefficient parity | D15 | Eqs. (5)-(6) + parity relation, Section 2.2, pp. 306-307 | "V_l_n(theta) = 1/sqrt(n(n+1)) d/dtheta P_l_n(theta); W_l_n(theta) = 1/sqrt(n(n+1)) m/cos(theta) P_l_n(theta)"; "u:{Cr,Ci,Br,Bi} <-> -v:{Br,Bi,-Cr,-Ci}" | OK |
| hwm123114.bin contents | harmonic-fit coefficient estimation (24,000 unknowns) | D15 | Section 2.3, Eqs. (7)-(8), pp. 308-309 | "there are a total of 24,000 unknown model coefficients in m ... estimated by solving an over-determined linear inverse problem d = Gm" | OK |
| hwm14.f90:74-79 | total wind = quiet time + disturbance (ap(2) >= 0) | NRL | README, MODEL FORMULATION | "the model consists of two parts: a quiet-time portion, and a geomagnetically disturbed portion. The quiet-time part represents average wind conditions when ap <= 12." | OK |
| hwm.rs:213-223; hwm14_c.f90 | FFI boundary units and argument conventions | NRL | README, TO GET TOTAL WINDS | "IYD - YEAR AND DAY AS YYDDD, SEC - UT(SEC), ALT - ALTITUDE(KM), GLAT - GEODETIC LATITUDE(DEG), GLON - GEODETIC LONGITUDE(DEG), STL - Not used ... AP(2) = CURRENT 3HR ap INDEX. W(1) = MERIDIONAL WIND (m/sec + Northward), W(2) = ZONAL WIND (m/sec + Eastward)" | OK |
| hwm.rs:220-221 | F107/F107A ignored by the model (hard-coded -1.0) | NRL + D15 | README, MODEL LIMITATIONS; D15 Section 4.2, p. 315 | "The model currently contains no solar activity dependence, and the F107 and F107A arguments are ignored."; "One missing component of HWM14 is a solar flux parameterization." | OK |
| hwm14.f90:21-37 | input/output units in the vendored source header | vendored file | header block (matches [NRL] README verbatim) | "iyd - year and day as yyddd / sec - ut(sec) / alt - altitude(km) ..." | OK |
| hwm.rs:104-132 | coefficient files + HWMPATH convention | NRL | README, RELEASE NOTES | "HWM14 requires three binary data files: hwm123114.bin, dwm07b104i.dat, and gd2qd.dat ... and/or 3) in a directory defined by the shell variable HWMPATH." | OK |
| hwm14_README.md:12-13 | vendoring provenance + version | NRL | README, VERSION HISTORY | "Horizontal Wind Model 2014 (HWM14), Version HWM14.123114, VERSION HISTORY: 31 Dec 2014 (Release version) ... update to the HWM07 empirical model ... which also supersedes HWM93 [Hedin et al., J. Atmos. Terr. Phys., vol. 58, 1421-1447, 1996]" | OK |
| hwm.rs:271-286 | reference test case (1993 DOY 323, 300 km, lat -11.95, lon -76.77, ap 35) | pyHWM14 (rilma) README examples | example values year=1993 / day=323 / ap=[-1, 35] confirmed in a fetched snippet; the single-point values 300 km / 12 UT / -11.95 / -76.77 not re-fetched | — | UNV |

## Flags

- `WindInput.f107` is dead: the model ignores F107/F107A entirely, and
  `Hwm14::evaluate` hard-codes -1.0 for both (hwm.rs:220-221). The API
  surface implies a solar dependence the model does not have.
- The oft-repeated description "quasi-orthogonal horizontal wind components"
  appears in no fetched HWM source. The verified formulation: both components
  are height-modulated vector spherical harmonic expansions (Eqs. 1-4) with
  V/W bases (Eqs. 5-6) linked by the coefficient parity relation.
- hwm.rs:212 passes local solar time x 3600 as the Fortran SEC (UT seconds)
  argument ("using LST as approximation"); the model computes local time
  from SEC (UT) and GLON, so at lon -76.77 deg the "12 UT" label in the test
  is approximate (the input is LST = 12).
- `two_digit_year_and_doy` hard-codes year 93 (1993) for all callers
  ("The model is not sensitive to the year for climatological winds").
- The HWM14 test lives inline in hwm.rs behind `#[cfg(feature = "hwm14")]` +
  `#[ignore]` (plus `examples/hwm14_samples.rs`); `src/tests/aero/` contains
  no HWM module.
- The old NRL page https://www.nrl.navy.mil/ssed/hwm/ is dead (404); the
  live distribution is https://map.nrl.navy.mil/map/pub/nrl/HWM/HWM14/.