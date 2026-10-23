# Frames and time scales: IAU 2006/2000B, ERA, EOP — primary sources

Frame transformations: frame bias + IAU 2006 precession + IAU 2000B nutation
(77 luni-solar terms + fixed planetary offsets), CIO-based Earth Rotation
Angle, ITRF polar motion, TT/TDB/UTC time scales from leap tables + EOP.

**Apogee code**

- `crates/apogee-core/src/frames/nutation_precession.rs` — bias constants,
  mean obliquity, precession angles, Delaunay arguments, 77-term series.
- `crates/apogee-core/src/frames/frame_service.rs` — eci_to_ecef
  (W R_z(-ERA) N P B), earth_rotation_angle, polar motion.
- `crates/apogee-core/src/frames/eop.rs`, `clock.rs`, `leap_seconds.rs` —
  EOP C04 parsing, time scales, TAI-UTC tables.
- `crates/apogee-core/src/tests/frames/integration_erfa.rs` — cross-validation
  vs ERFA v2.0 (eraNut00b, eraObl06, eraEe00b, eraNum00b, eraBp06).
- `crates/apogee-core/src/aero/atmospheric_drag.rs:36`,
  `crates/apogee-core/src/components/drag_surfaces.rs:173` —
  omega_earth = 7.2921159e-5 rad/s.

## Primary sources

- **[TN36]** Petit, G., B. Luzum (eds.) (2010), "IERS Conventions (2010)",
  IERS Technical Note 36 (179 pp.) — free PDF:
  https://iers-conventions.obspm.fr/content/tn36.pdf (official mirror via
  https://www.iers.org; direct TLS from this host fails — fetched via a
  web.archive.org copy, https://web.archive.org/web/20190630104818/...).
  Committed: `sources/pdfs/tn36.pdf`.
- **[C00]** Capitaine, N., B. Guinot, D.D. McCarthy (2000), "Definition of
  the Celestial Ephemeris Origin and of UT1 in the International Celestial
  Reference Frame", Astron. Astrophys. 355, 398-405 —
  https://articles.adsabs.harvard.edu/pdf/2000A%26A...355..398C
  (publisher copyright — linked, not committed).
- **[ERFA]** ERFA - Essential Routines for Fundamental Astronomy, liberfa v2.0
  (open-source fork of IAU SOFA, based on SOFA release 20210512):
  https://github.com/liberfa/erfa — source files nut00b.c, p06e.c, obl06.c,
  bi00.c, num00b.c, ee00b.c, ee00.c, era00.c. IAU SOFA:
  http://www.iausofa.org/.
- **[IERS-data]** IERS Earth Orientation Center / Data Center products:
  Leap_Second.dat (https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat),
  EOP C04 (https://hpiers.obspm.fr/eoppc/eop/eopc04/readme),
  EOP 20 C04 one-file (https://datacenter.iers.org/data/latestVersion/EOP_20_C04_one_file_1962-now.txt).
- Cited by the code, **not fetched** (content cross-verified via [TN36] and
  [ERFA]): McCarthy & Petit (2004) IERS TN 32; McCarthy & Luzum (2003) IAU
  2000B paper; Capitaine, Wallace & Chapront (2003) A&A 412, 567-586 (P03
  precession).

## Equations and constants

All quotes `OK` (read from fetched [TN36] / [ERFA] text) unless marked.

| Apogee code | Item | Source | Where | Verbatim (key text) | St. |
| --- | --- | --- | --- | --- | --- |
| atmospheric_drag.rs:36; drag_surfaces.rs:173 | omega_earth = 7.2921159e-5 rad/s | TN36 | Table 1.2 (GRS80 parameters), p. 19 | "omega = 7.292115 x 10^-5 rad s^-1 — Nominal mean Earth's angular velocity" (see Flags: code value is the sidereal rate) | OK |
| frame_service.rs:195-210 `earth_rotation_angle` | ERA definition | TN36 | Eq. (5.14), Section 5.5.3, p. 52 | "ERA(Tu) = 2 pi (0.7790572732640 + 1.00273781191135448 Tu) (5.14) ... where Tu = (Julian UT1 date - 2451545.0)" | OK |
| :41, 199 (cited) | Eq. (5.15) form | TN36 | Eq. (5.15), p. 52 | "ERA(Tu) = 2 pi (UT1 Julian day fraction + 0.7790572732640 + 0.00273781191135448 Tu) (5.15)" (the code cites 5.15; what it implements is 5.14) | OK |
| :195-210 | ERA definition paper | C00 | Eqs. (22)-(23), Section 4.3, p. 403 | "theta(Tu) = 2 pi (0.7790572732640 + 1.00273781191135448 Tu x 36525) (22) ... Tu = (Julian UT1 date - 2451545.0)/36525 (23)" | OK |
| :195-210 | theta_0 + rate (Table 1.1 numerical standards) | TN36 | Table 1.1, p. 18 | "theta_0 = 0.7790572732640 rev ... dtheta/dt = 1.00273781191135448 rev/UT1day — Defining [3] Rate of advance of ERA" | OK |
| :206 | rate cross-check vs ERFA | ERFA | era00.c implementation | "theta = eraAnp(ERFA_D2PI \* (f + 0.7790572732640 + 0.00273781191135448 \* t));" | OK |
| :169-193 `eci_to_ecef` | overall transformation [GCRS] = Q R W [ITRS] | TN36 | Eq. (5.1), Section 5.2.1, p. 43 | "[GCRS] = Q(t) R(t) W(t) [ITRS] (5.1)" | OK |
| :241-258 `polar_motion_matrix` | W(t) = R3(-s0) R2(xp) R1(yp) | TN36 | Eq. (5.3), Section 5.4.1, p. 48 | "W(t) = R3 (-s0) . R2 (xp) . R1 (yp) (5.3) ... s0 ... which was neglected in the classical form prior to 1 January 2003" (code omits s0, see Flags) | OK |
| :180-185 | R(t) = R3(-ERA) | TN36 | Eq. (5.5), p. 48 | "R(t) = R3 (-ERA) (5.5) ... where ERA is the Earth Rotation Angle between the CIO and the TIO at date t on the equator of the CIP" | OK |
| :187 + eop.rs | EOP pole-coordinate corrections | TN36 | Eq. (5.11), Section 5.5.1, p. 48 | "(xp, yp) = (x, y)_IERS + (dx, dy)_ocean tides + (dx, dy)_libration (5.11)" (tide/libration terms not applied — see Flags) | OK |
| nutation_precession.rs:271-443 | nutation model identity: IAU 2000B, 77 luni-solar terms + planetary bias | TN36 | Section 5.6.1, p. 62 | "an abridged model, designated IAU 2000B, is available for those who need a model only at the 1 mas level. Such a model has been developed by McCarthy and Luzum (2003). It includes fewer than 80 lunisolar terms plus a bias to account for the effect of the planetary terms" | OK |
| :105-108 | DPPLAN = -0.135 mas, DEPLAN = +0.388 mas | ERFA | nut00b.c constants | "DPPLAN = -0.135 mas ... DEPLAN = +0.388 mas" | OK |
| :271-397 `NUTATION_SERIES` | the 77 luni-solar term coefficients | ERFA | nut00b.c series table | "{ 0, 0, 0, 0, 1, -172064161.0, -174666.0, 33386.0, 92052331.0, 9086.0, 15377.0}, ..." (verified against the source table) | OK |
| :238-268 `fundamental_arguments` | Delaunay arguments l, l', F, D, Omega | TN36 | Eq. (5.43), Section 5.7.2, pp. 66-67 | "F1 = l = Mean Longitude of the Moon = 134.96340251 deg + 1717915923.217800 t + ... t3 ... F5 = Omega = ... 125.04455501 deg - 6962890.543100 t + ..." | OK |
| :239-268 | SOFA rounding of l'/D fixed terms | TN36 | note after Eq. (5.43), p. 67 | "SOFA uses the original MHB2000 expressions for the Delaunay variables l' and D, that differ from Eq. (5.43) in that the fixed term is rounded to five digits (i.e. 1287104.79305 instead of ...)" | OK |
| :146-148 `mean_obliquity` | IAU 2006 mean obliquity polynomial (base 84381.406) | TN36 | Eq. (5.40), Section 5.7.1, pp. 64-65 | "eps_A = 84381.406 - 46.836769 t - 0.0001831 t^2 + 0.00200340 t^3 - ... mas" | OK |
| frame_service.rs:55 | fixed mean obliquity at J2000: 84381.406 arcsec (23.4392911 deg) | TN36 | Table 1.1, p. 18 | "eps_0 = 84381.406 mas — Defining [4] Mean obliquity of the ecliptic at J2000.0" | OK |
| nutation_precession.rs:185-191 `precession_angles` | IAU 2006 (P03) zeta_A / z_A / theta_A polynomials | ERFA | p06e.c (from Capitaine, Wallace & Chapront 2003 — paper not fetched) | "zeta_A, z_A, theta_A polynomial coefficients as in p06e.c" | 2H |
| :90, 94, 98 | frame bias constants XI0/ETA0/DA0 | TN36 | Eq. (5.33), Section 5.6.2, p. 62 | "xi_0 = -0.016617 arcsec ... eta_0 = -0.0068192 arcsec ... DA_0 = -0.0146 arcsec" | OK |
| :160-171 `frame_bias_matrix` | first-order small-angle B matrix (vs exact 3-rotation composition) | TN36 + ERFA | Section 5.6.2 + bi00.c | agrees at the sub-microarcsecond level (see Flags) | OK |
| :496-504 `equation_of_equinoxes` | EE = dpsi cos(eps_A) (classical form) | TN36 + ERFA | Section 5.5.7, p. 60; ee00.c | "EE = dpsi cos(eps_A) + [complementary terms]" — the complementary terms (eraEect00, up to ~0.1 mas) are not implemented | OK |
| frame_service.rs:9-19 (docs) | GMST (cited in docs, not computed) | TN36 | Eq. (5.32), pp. 59-61 | "GMST (discussion only — the implementation is ERA-based; see Flags)" | OK |
| clock.rs:122 | TT = TAI + 32.184 s | TN36 | Section 10.1, p. 151 | "TT = TAI + 32.184 s" | OK |
| clock.rs:137 | TDB / TCG / TCB relations | TN36 | Eqs. (10.1)-(10.3), Section 10.2 | "TCG - TT = L_G (TT - t_0) ... TCB - TDB" relations (delegated to hifitime) | OK |
| leap_seconds.rs | TAI-UTC leap-second table | IERS-data | Leap_Second.dat | "41317.0 1 1 1972 10.0 ... 41684.0 1 7 1972 11.0 ..." (rows embedded in unit tests) | OK |
| eop.rs | EOP C04 (ITRF2020-consistent) format | IERS-data | C04 readme + one-file header | "MJD ... x y UT1-UTC dX dY xrt yrt LOD" (13-field v4 format since 2023-02-14) | OK |
| integration_erfa.rs | ERFA v2.0 cross-validation | ERFA | repo header | "ERFA - Essential Routines for Fundamental Astronomy ... based on SOFA release 20210512" | OK |
| clock.rs:132 | GPS = TAI - 19 s | (standard constant; delegated to hifitime) | — | not covered by [TN36] | UNV |

## Flags

- **omega_earth mismatch:** code uses 7.2921159e-5 rad/s, which is the
  sidereal rate 2 pi/(86164.0905 s), not [TN36] Table 1.2's nominal
  7.292115e-5 (difference ~1.2e-7 relative). If the IERS nominal is
  intended, the literal should be 7.292115e-5 with Table 1.2 as citation.
- **ERA rate digit error:** frame_service.rs:206 uses
  1.002_737_811_911_354_6; [TN36] Table 1.1/Eq. (5.14) and era00.c both
  give 1.00273781191135448 (...48, not ...46). Negligible (~2e-17) but
  should be corrected for verbatim fidelity.
- **Wrong equation number in code:** :41/:199 cite Eq. (5.15); the
  definition actually implemented is Eq. (5.14) (5.15 is only the
  rounding-reduced equivalent).
- **Docs/implementation mismatch:** module docs say ECI<->ECEF rotates
  "by GMST", but the implementation is the CIO-based Earth Rotation Angle;
  GMST (Eq. 5.32) is never computed.
- **Convention mixing:** `eci_to_ecef` = W R3(-ERA) N P B combines the
  CIO-based Earth rotation (Eq. 5.5) with the equinox-based precession-
  nutation N P B. [TN36]-consistent pairings are either [GCRS] = Q R W
  [ITRS] (CIO-based Q) or the equinox-based W R3(GST) N P B with GAST
  (Section 5.4.5). Sub-arcsecond convention error at the CIP level; the
  ERFA integration tests constrain the practical impact.
- **Polar motion omits s0** (Eq. 5.3): [TN36] notes s0 was neglected prior
  to 2003; effect ~0.1 mas. Sign/axis conventions also differ from the
  TN36 frame rotations (active vector rotations R_y(-x) R_x(-y)).
- **EE complementary terms** (eraEect00, ~0.1 mas) not implemented — only
  dpsi cos(eps_A).
- **Bias matrix is first-order** small-angle, not the exact composition of
  the three bias rotations (bi00.c); sub-microarcsecond agreement.
- **EOP parsing:** targets the current 13-field C04 v4 format (since
  2023-02-14) and silently skips lines with < 13 fields (older 9-field
  eopc04_IAU2000 files would be dropped wholesale); parsed dX/dY celestial
  pole offsets are not applied to the precession-nutation model; Eq. (5.11)
  ocean-tide/libration contributions not added; `jd_ut1` approximates
  JD_UT1 = JD_TT + (UT1-UTC)/86400 (ignoring the TT-UTC leap offset in the
  day count — fine for the ERA fractional-day argument).
- Stale test comment in integration_erfa.rs ("our 20-term model") — the
  implementation is the full 77-term IAU 2000B series.
- Unfetched-but-cited: IERS TN 32, the IAU 2000B paper, and the P03
  precession paper — their numeric content is cross-verified through [TN36]
  (Eqs. 5.40, 5.43, Section 5.6.1) and the [ERFA] sources.